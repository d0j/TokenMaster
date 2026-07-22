use std::fmt;
use std::rc::Rc;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use slint::ComponentHandle;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_desktop::{
    DesktopBridgeFactory, DesktopController, DesktopCurrentUserStartupPresenter,
    DesktopCurrentUserStartupStatus, DesktopHistoryRangeIntent, DesktopHistoryRangeIntentAdmission,
    DesktopHistoryRangeIntentRouter, DesktopHistoryRangeIntentSink, DesktopIntent,
    DesktopIntentAdmission, DesktopIntentRouter, DesktopIntentSink, DesktopLifecycleIntent,
    DesktopLifecycleIntentAdmission, DesktopLifecycleIntentRouter, DesktopLifecycleIntentSink,
    DesktopOperationKind, DesktopOperationPhase, DesktopOperationSnapshot, DesktopQueryPlan,
    DesktopRefreshAdmission, DesktopRefreshIngress, DesktopRefreshUrgency,
    DesktopReliableStateNotifier, DesktopReliableStateProjection, DesktopReminderPolicy,
    DesktopReminderSyncState, DesktopRestoreSelection, DesktopRuntimeObservation,
    DesktopSessionDetailIntent, DesktopSessionDetailIntentAdmission,
    DesktopSessionDetailIntentRouter, DesktopSessionDetailIntentSink, DesktopSessionPageIntent,
    DesktopSessionPageIntentAdmission, DesktopSessionPageIntentRouter,
    DesktopSessionPageIntentSink, DesktopShell, DesktopSnapshotBridge, MainWindow,
    select_production_renderer,
};
use tokenmaster_engine::{
    RefreshOutcome, RefreshUrgency, WorkerCompletion, WorkerCompletionNotifier,
};
use tokenmaster_platform::{
    CurrentSessionActivationAdmission, CurrentSessionActivationSink, CurrentSessionClaim,
    CurrentSessionIntegration, CurrentSessionPrimary, CurrentUserStartup, CurrentUserStartupAction,
    CurrentUserStartupError, CurrentUserStartupStatus, ExclusiveFileLeaseGuard, FileDialogFileType,
    FileDialogResult, FileDialogSelector, NativeFileDialog,
};
use tokenmaster_product::{
    ProductGitRuntimeHealth, ProductQuotaRuntimeHealth, ProductReducer,
    ProductReminderRuntimeHealth, ProductRuntimeGeneration, ProductRuntimeObservationError,
    ProductUsageRuntimeHealth,
};
use tokenmaster_runtime::{
    BenefitReminderRuntime, BenefitReminderRuntimeConfig, CodexQuotaRuntimeConfig,
    CodexUsageProviderFactory, LiveRuntime, ProviderQuotaRuntime, RuntimeErrorCode,
};
use tokenmaster_state::{
    BackupMaintenanceRuntime, BackupPassphrase, BootstrapOutcome, MAX_CONFIG_PACKAGE_BYTES,
    MaintenanceCompletion, MaintenanceOutcome, MaintenancePurpose, MaintenanceSourceState,
    ReminderPolicy, RestoreMode, RestoreSafety, StateErrorCode,
};
use tokenmaster_store::BackupControl;

use crate::command::{
    ApplicationBackupPolicyUpdate, ApplicationBackupSelection, ApplicationCommand,
    ApplicationCommandAdmission, ApplicationCommandExecution, ApplicationCommandFailure,
    ApplicationCommandPermit, ApplicationOperationPayload, ApplicationOperationRequest,
    ApplicationReminderPolicyUpdate,
};
use crate::notification::{ReminderPresentationCoordinator, RuntimeReminderPresentationPort};
use crate::operation::{
    ApplicationOperationSubmitter, ApplicationOperationWorker, ApplicationOperationWorkerPhase,
};
use crate::state::{ApplicationPreflight, ApplicationStateOwner};
use crate::{ApplicationEnvironment, DataRoot};

type SharedBundle = Arc<Mutex<ApplicationBundleSlot>>;
type SharedBridge = Arc<Mutex<Option<DesktopSnapshotBridge>>>;
const MANDATORY_BACKUP_TIMEOUT: Duration = Duration::from_secs(5 * 60);

struct ApplicationBundleSlot {
    generation: u64,
    bundle: Option<ApplicationBundle>,
}

impl ApplicationBundleSlot {
    const fn new() -> Self {
        Self {
            generation: 0,
            bundle: None,
        }
    }

    const fn as_ref(&self) -> Option<&ApplicationBundle> {
        self.bundle.as_ref()
    }

    fn as_mut(&mut self) -> Option<&mut ApplicationBundle> {
        self.bundle.as_mut()
    }

    const fn is_none(&self) -> bool {
        self.bundle.is_none()
    }

    #[cfg(test)]
    const fn is_some(&self) -> bool {
        self.bundle.is_some()
    }

    fn take(&mut self) -> Option<ApplicationBundle> {
        self.bundle.take()
    }
}

pub fn run() -> Result<(), ApplicationError> {
    let claim = CurrentSessionIntegration::claim()
        .map_err(|_| ApplicationError::current_session_unavailable())?;
    match claim {
        CurrentSessionClaim::Secondary(_) => Ok(()),
        CurrentSessionClaim::Primary(current_session) => {
            select_production_renderer().map_err(|_| ApplicationError::ui_unavailable())?;
            let environment =
                ApplicationEnvironment::capture().map_err(|_| ApplicationError::data())?;
            let mut application =
                Application::start_with_current_session(&environment, current_session)?;
            let event_result = application.run_event_loop();
            let shutdown_result = application.shutdown();
            event_result.and(shutdown_result)
        }
    }
}

struct Application {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 12B command worker binds controlled restart")
    )]
    environment: ApplicationEnvironment,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 12B command worker binds controlled restart")
    )]
    data_root: DataRoot,
    shell: DesktopShell,
    bridge_factory: DesktopBridgeFactory,
    bridge: SharedBridge,
    bundle: SharedBundle,
    commands: ApplicationOperationWorker,
    reliable_notifier: DesktopReliableStateNotifier,
    #[cfg(test)]
    reliable_publish_count: Arc<AtomicU64>,
    state: Arc<ApplicationStateOwner>,
    preflight: Arc<Mutex<ApplicationPreflight>>,
    live_started: Arc<AtomicBool>,
    current_session: Option<CurrentSessionPrimary>,
    current_session_activation: Option<Arc<ApplicationSessionActivationBridge>>,
    shutdown: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplicationStartBoundary {
    BeforeReconstructionReconciliation,
    PreMigrationBackupPublished,
    BeforePostMigrationBackup,
}

impl Application {
    #[cfg(test)]
    fn start(environment: &ApplicationEnvironment) -> Result<Self, ApplicationError> {
        Self::start_with_observer(environment, |_| Ok(()))
    }

    fn start_with_current_session(
        environment: &ApplicationEnvironment,
        current_session: CurrentSessionPrimary,
    ) -> Result<Self, ApplicationError> {
        Self::start_with_observer_and_current_session(
            environment,
            |_| Ok(()),
            Some(current_session),
        )
    }

    #[cfg(test)]
    fn start_with_observer<F>(
        environment: &ApplicationEnvironment,
        observer: F,
    ) -> Result<Self, ApplicationError>
    where
        F: FnMut(ApplicationStartBoundary) -> Result<(), ApplicationError>,
    {
        Self::start_with_observer_and_current_session(environment, observer, None)
    }

    fn start_with_observer_and_current_session<F>(
        environment: &ApplicationEnvironment,
        mut observer: F,
        mut session_owner: Option<CurrentSessionPrimary>,
    ) -> Result<Self, ApplicationError>
    where
        F: FnMut(ApplicationStartBoundary) -> Result<(), ApplicationError>,
    {
        let data_root = DataRoot::resolve(environment).map_err(|_| ApplicationError::data())?;
        let state = Arc::new(ApplicationStateOwner::open(&data_root)?);
        let mut preflight = state.prepare(&data_root)?;
        let initial = ProductReducer::new().snapshot();
        let reliable_state = state
            .reliable_state_projection_for_outcome(preflight.effective_outcome(), None)
            .unwrap_or_else(|_| DesktopReliableStateProjection::unavailable());
        let bundle = Arc::new(Mutex::new(ApplicationBundleSlot::new()));
        let intent_router = Rc::new(DesktopIntentRouter::new());
        let history_range_router = Rc::new(DesktopHistoryRangeIntentRouter::new());
        let session_detail_router = Rc::new(DesktopSessionDetailIntentRouter::new());
        let session_page_router = Rc::new(DesktopSessionPageIntentRouter::new());
        let lifecycle_router = Rc::new(DesktopLifecycleIntentRouter::new());
        #[cfg(not(test))]
        let shell = DesktopShell::new_with_reliable_state_and_all_history_and_session_sinks(
            &initial,
            reliable_state,
            intent_router.clone(),
            history_range_router.clone(),
            session_detail_router.clone(),
            session_page_router.clone(),
            lifecycle_router.clone(),
        )
        .map_err(|_| ApplicationError::ui_unavailable())?;
        #[cfg(test)]
        let shell = DesktopShell::new_with_reliable_state_and_history_and_session_sinks(
            &initial,
            reliable_state,
            intent_router.clone(),
            history_range_router.clone(),
            session_detail_router.clone(),
            session_page_router.clone(),
        )
        .map_err(|_| ApplicationError::ui_unavailable())?;
        let bridge_factory = shell.bridge_factory();
        let outcome = preflight.report().outcome();
        let may_start_live = matches!(
            outcome,
            BootstrapOutcome::Healthy
                | BootstrapOutcome::FirstInstall
                | BootstrapOutcome::MigrationRequired
        );
        let (live_started, bridge) = if may_start_live {
            match start_live_bundle(
                environment,
                &data_root,
                &state,
                &mut preflight,
                &bridge_factory,
                &bundle,
                outcome,
                &mut observer,
            ) {
                Ok(bridge) => {
                    preflight.session_mut().authorize_healthy_launch();
                    (true, Some(bridge))
                }
                Err(_) => {
                    preflight.release_startup_guard();
                    discard_bundle(&bundle)?;
                    (false, None)
                }
            }
        } else {
            preflight.release_startup_guard();
            (false, None)
        };

        let bridge = Arc::new(Mutex::new(bridge));
        let preflight = Arc::new(Mutex::new(preflight));
        let live_started = Arc::new(AtomicBool::new(live_started));
        let reliable_notifier = shell.reliable_state_notifier();
        #[cfg(test)]
        let reliable_publish_count = Arc::new(AtomicU64::new(0));
        let command_environment = environment.clone();
        let command_data_root = data_root.clone();
        let command_state = Arc::clone(&state);
        let command_preflight = Arc::clone(&preflight);
        let command_bundle = Arc::clone(&bundle);
        let command_factory = bridge_factory.clone();
        let command_bridge = Arc::clone(&bridge);
        let command_live_started = Arc::clone(&live_started);
        let command_notifier = reliable_notifier.clone();
        let commands = ApplicationOperationWorker::spawn_with_payload(move |permit, payload| {
            let _ = command_notifier
                .publish_operation(Some(application_operation_running(permit.command())));
            let execution = execute_application_operation(
                &command_environment,
                &command_data_root,
                &command_state,
                &command_preflight,
                &command_bundle,
                &command_factory,
                &command_bridge,
                &command_live_started,
                &command_notifier,
                permit,
                payload,
            );
            let operation = application_operation_completion(permit.command(), execution);
            let projection = command_preflight
                .lock()
                .map_err(|_| ApplicationError::internal())
                .and_then(|preflight| {
                    command_state.reliable_state_projection_for_outcome(
                        preflight.effective_outcome(),
                        Some(operation),
                    )
                });
            match projection {
                Ok(projection) => {
                    let _ = command_notifier.publish(projection);
                }
                Err(_) => {
                    let _ = command_notifier.publish_operation(Some(operation));
                }
            }
            execution
        })
        .map_err(|_| ApplicationError::internal())?;
        let current_user_startup_port: Rc<dyn ApplicationCurrentUserStartupPort> =
            Rc::new(NativeApplicationCurrentUserStartupPort);
        let current_user_startup_presenter = shell.current_user_startup_presenter();
        let _ = current_user_startup_presenter.present(map_current_user_startup_status(
            current_user_startup_port.inspect(),
        ));
        intent_router
            .install(Rc::new(ApplicationDesktopIntentSink::new_with_startup(
                commands.submitter(),
                current_user_startup_port,
                current_user_startup_presenter,
            )))
            .map_err(|_| ApplicationError::internal())?;
        history_range_router
            .install(Rc::new(ApplicationHistoryRangeIntentSink::new(
                Arc::downgrade(&bundle),
            )))
            .map_err(|_| ApplicationError::internal())?;
        session_detail_router
            .install(Rc::new(ApplicationSessionDetailIntentSink::new(
                Arc::downgrade(&bundle),
            )))
            .map_err(|_| ApplicationError::internal())?;
        session_page_router
            .install(Rc::new(ApplicationSessionPageIntentSink::new(
                Arc::downgrade(&bundle),
            )))
            .map_err(|_| ApplicationError::internal())?;
        lifecycle_router
            .install(Rc::new(ApplicationDesktopLifecycleSink::new(
                shell.window().as_weak(),
            )))
            .map_err(|_| ApplicationError::internal())?;

        let current_session_activation = if let Some(owner) = session_owner.as_mut() {
            let activation = ApplicationSessionActivationBridge::new(
                Arc::new(SlintApplicationEventScheduler),
                Arc::new(SlintApplicationSessionActivationDelivery::new(
                    shell.window().as_weak(),
                )),
            );
            owner
                .start(activation.clone())
                .map_err(|_| ApplicationError::internal())?;
            Some(activation)
        } else {
            None
        };

        let application = Self {
            environment: environment.clone(),
            data_root,
            shell,
            bridge_factory,
            bridge,
            bundle,
            commands,
            reliable_notifier,
            #[cfg(test)]
            reliable_publish_count,
            state,
            preflight,
            live_started,
            current_session: session_owner,
            current_session_activation,
            shutdown: false,
        };
        application.publish_live_reliable_projection();
        Ok(application)
    }

    fn run_event_loop(&self) -> Result<(), ApplicationError> {
        self.shell
            .window()
            .show()
            .map_err(|_| ApplicationError::ui_unavailable())?;
        let _ = self.shell.show_lifecycle_surface();
        if let Some(activation) = self.current_session_activation.as_ref() {
            activation.flush();
        }
        slint::run_event_loop_until_quit().map_err(|_| ApplicationError::event_loop())
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 12B command worker binds controlled restart")
    )]
    fn restart_services(&mut self) -> Result<(), ApplicationError> {
        if self.shutdown || !self.live_started.load(Ordering::Acquire) {
            return Err(ApplicationError::invalid_lifecycle());
        }
        if self
            .bundle
            .lock()
            .map_err(|_| ApplicationError::internal())?
            .is_none()
        {
            return Err(ApplicationError::invalid_lifecycle());
        }
        self.commands
            .pause_admission()
            .map_err(|_| ApplicationError::internal())?;
        self.live_started.store(false, Ordering::Release);
        drop(
            self.bridge
                .lock()
                .map_err(|_| ApplicationError::internal())?
                .take(),
        );
        let result = (|| {
            let owned = self
                .bundle
                .lock()
                .map_err(|_| ApplicationError::internal())?
                .take();
            let mut owned = owned.ok_or_else(ApplicationError::invalid_lifecycle)?;
            owned.shutdown()?;
            let guard = self.state.acquire_runtime_guard(&self.data_root)?;
            start_current_bundle(
                &self.environment,
                &self.data_root,
                &self.state,
                &self.bridge_factory,
                &self.bundle,
                guard,
            )
        })();
        self.commands
            .resume_admission()
            .map_err(|_| ApplicationError::internal())?;
        match result {
            Ok(bridge) => {
                *self
                    .bridge
                    .lock()
                    .map_err(|_| ApplicationError::internal())? = Some(bridge);
                self.live_started.store(true, Ordering::Release);
                self.publish_live_reliable_projection();
                Ok(())
            }
            Err(error) => {
                discard_bundle(&self.bundle)?;
                Err(error)
            }
        }
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 12B command worker binds selected restore")
    )]
    fn restore_selected(
        &mut self,
        selection: ApplicationBackupSelection,
        mode: RestoreMode,
    ) -> Result<(), ApplicationError> {
        if self.shutdown
            || !self.live_started.load(Ordering::Acquire)
            || mode == RestoreMode::AutomaticDataOnly
            || self
                .bundle
                .lock()
                .map_err(|_| ApplicationError::internal())?
                .is_none()
        {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let selection_pin = self.state.bind_backup_selection(selection)?;
        let binding = selection_pin.binding();
        self.commands
            .pause_admission()
            .map_err(|_| ApplicationError::internal())?;
        self.live_started.store(false, Ordering::Release);
        drop(
            self.bridge
                .lock()
                .map_err(|_| ApplicationError::internal())?
                .take(),
        );
        let result = (|| {
            let owned = self
                .bundle
                .lock()
                .map_err(|_| ApplicationError::internal())?
                .take();
            let mut owned = owned.ok_or_else(ApplicationError::invalid_lifecycle)?;
            owned.shutdown()?;

            let guard = self.state.acquire_runtime_guard(&self.data_root)?;
            let mut maintenance = self.state.start_protected_maintenance(
                &self.data_root,
                MaintenanceSourceState::Healthy,
                binding,
            )?;
            let safety = wait_for_mandatory_backup(&maintenance, MaintenancePurpose::PreRestore);
            let maintenance_shutdown = maintenance.shutdown();
            let safety = safety?;
            maintenance_shutdown.map_err(|_| ApplicationError::state())?;
            drop(selection_pin);

            let control =
                BackupControl::new(Arc::new(AtomicBool::new(false)), MANDATORY_BACKUP_TIMEOUT)
                    .map_err(|_| ApplicationError::state())?;
            let receipt = self.state.restore_selected(
                binding,
                mode,
                RestoreSafety::PreRestoreBackupPublished(safety),
                &guard,
                &control,
            )?;
            let mut preflight = self
                .preflight
                .lock()
                .map_err(|_| ApplicationError::internal())?;
            preflight.bind_recovery_launch(receipt)?;
            start_restored_bundle(
                &self.environment,
                &self.data_root,
                &self.state,
                &mut preflight,
                &self.bridge_factory,
                &self.bundle,
                guard,
            )
        })();
        self.commands
            .resume_admission()
            .map_err(|_| ApplicationError::internal())?;
        match result {
            Ok(bridge) => {
                *self
                    .bridge
                    .lock()
                    .map_err(|_| ApplicationError::internal())? = Some(bridge);
                self.live_started.store(true, Ordering::Release);
                self.publish_live_reliable_projection();
                Ok(())
            }
            Err(error) => {
                discard_bundle(&self.bundle)?;
                Err(error)
            }
        }
    }

    fn publish_live_reliable_projection(&self) {
        if !self.live_started.load(Ordering::Acquire) {
            return;
        }
        let projection = self
            .preflight
            .lock()
            .map_err(|_| ApplicationError::internal())
            .and_then(|preflight| {
                self.state
                    .reliable_state_projection_for_outcome(preflight.effective_outcome(), None)
            })
            .unwrap_or_else(|_| DesktopReliableStateProjection::unavailable());
        let _ = self.reliable_notifier.publish(projection);
        #[cfg(test)]
        self.reliable_publish_count.fetch_add(1, Ordering::AcqRel);
    }

    fn shutdown(&mut self) -> Result<(), ApplicationError> {
        if self.shutdown {
            return Ok(());
        }
        self.shutdown = true;
        if let Some(activation) = self.current_session_activation.as_ref() {
            activation.close();
        }
        let current_session_result = match self.current_session.as_mut() {
            Some(current_session) => current_session
                .shutdown()
                .map_err(|_| ApplicationError::shutdown()),
            None => Ok(()),
        };
        let command_result = match self.commands.shutdown() {
            Ok(ApplicationOperationWorkerPhase::Stopped) => Ok(()),
            Ok(
                ApplicationOperationWorkerPhase::Running
                | ApplicationOperationWorkerPhase::Stopping
                | ApplicationOperationWorkerPhase::Faulted,
            )
            | Err(_) => Err(ApplicationError::shutdown()),
        };
        drop(
            self.bridge
                .lock()
                .map_err(|_| ApplicationError::internal())?
                .take(),
        );
        let bundle = self
            .bundle
            .lock()
            .map_err(|_| ApplicationError::internal())?
            .take();
        let bundle_result = match bundle {
            Some(mut bundle) => bundle.shutdown(),
            None => Ok(()),
        };
        let result = current_session_result
            .and(command_result)
            .and(bundle_result);
        if result.is_ok() && self.live_started.load(Ordering::Acquire) {
            self.preflight
                .lock()
                .map_err(|_| ApplicationError::internal())?
                .session_mut()
                .mark_clean()
                .map_err(|_| ApplicationError::state())?;
        }
        self.live_started.store(false, Ordering::Release);
        if result.is_ok() {
            self.current_session.take();
            self.current_session_activation.take();
        }
        result
    }
}

struct ApplicationDesktopIntentSink {
    dialog: NativeFileDialog,
    submitter: ApplicationOperationSubmitter,
    current_user_startup: Option<ApplicationCurrentUserStartupBinding>,
}

struct ApplicationCurrentUserStartupBinding {
    port: Rc<dyn ApplicationCurrentUserStartupPort>,
    presenter: DesktopCurrentUserStartupPresenter,
}

trait ApplicationCurrentUserStartupPort {
    fn inspect(&self) -> CurrentUserStartupStatus;
    fn apply(
        &self,
        action: CurrentUserStartupAction,
    ) -> Result<CurrentUserStartupStatus, CurrentUserStartupError>;
}

struct NativeApplicationCurrentUserStartupPort;

impl ApplicationCurrentUserStartupPort for NativeApplicationCurrentUserStartupPort {
    fn inspect(&self) -> CurrentUserStartupStatus {
        CurrentUserStartup::inspect().status()
    }

    fn apply(
        &self,
        action: CurrentUserStartupAction,
    ) -> Result<CurrentUserStartupStatus, CurrentUserStartupError> {
        CurrentUserStartup::apply(action).map(|snapshot| snapshot.status())
    }
}

struct ApplicationDesktopLifecycleSink {
    window: slint::Weak<MainWindow>,
}

type ApplicationEventTask = Box<dyn FnOnce() + Send + 'static>;

trait ApplicationEventScheduler: Send + Sync + 'static {
    fn schedule(&self, task: ApplicationEventTask) -> Result<(), ApplicationEventScheduleError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplicationEventScheduleError {
    Unavailable,
    Terminated,
    Internal,
}

struct SlintApplicationEventScheduler;

impl ApplicationEventScheduler for SlintApplicationEventScheduler {
    fn schedule(&self, task: ApplicationEventTask) -> Result<(), ApplicationEventScheduleError> {
        slint::invoke_from_event_loop(task).map_err(|error| match error {
            slint::EventLoopError::NoEventLoopProvider => {
                ApplicationEventScheduleError::Unavailable
            }
            slint::EventLoopError::EventLoopTerminated => ApplicationEventScheduleError::Terminated,
            _ => ApplicationEventScheduleError::Internal,
        })
    }
}

trait ApplicationSessionActivationDelivery: Send + Sync + 'static {
    fn deliver(&self) -> CurrentSessionActivationAdmission;
}

struct SlintApplicationSessionActivationDelivery {
    window: slint::Weak<MainWindow>,
}

impl SlintApplicationSessionActivationDelivery {
    const fn new(window: slint::Weak<MainWindow>) -> Self {
        Self { window }
    }
}

impl ApplicationSessionActivationDelivery for SlintApplicationSessionActivationDelivery {
    fn deliver(&self) -> CurrentSessionActivationAdmission {
        let Some(window) = self.window.upgrade() else {
            return CurrentSessionActivationAdmission::Rejected;
        };
        match ApplicationDesktopLifecycleSink::show_and_activate(&window) {
            DesktopLifecycleIntentAdmission::Accepted => {
                CurrentSessionActivationAdmission::Accepted
            }
            DesktopLifecycleIntentAdmission::Rejected => {
                CurrentSessionActivationAdmission::Rejected
            }
        }
    }
}

struct ApplicationSessionActivationBridge {
    self_weak: Weak<Self>,
    scheduler: Arc<dyn ApplicationEventScheduler>,
    delivery: Arc<dyn ApplicationSessionActivationDelivery>,
    pending: AtomicBool,
    scheduled: AtomicBool,
    closed: AtomicBool,
    received_count: AtomicU64,
    coalesced_count: AtomicU64,
    delivered_count: AtomicU64,
    rejected_count: AtomicU64,
    scheduling_failure_count: AtomicU64,
    panicked_count: AtomicU64,
    overflowed: AtomicBool,
}

impl ApplicationSessionActivationBridge {
    fn new(
        scheduler: Arc<dyn ApplicationEventScheduler>,
        delivery: Arc<dyn ApplicationSessionActivationDelivery>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|self_weak| Self {
            self_weak: self_weak.clone(),
            scheduler,
            delivery,
            pending: AtomicBool::new(false),
            scheduled: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            received_count: AtomicU64::new(0),
            coalesced_count: AtomicU64::new(0),
            delivered_count: AtomicU64::new(0),
            rejected_count: AtomicU64::new(0),
            scheduling_failure_count: AtomicU64::new(0),
            panicked_count: AtomicU64::new(0),
            overflowed: AtomicBool::new(false),
        })
    }

    fn request(self: &Arc<Self>) -> CurrentSessionActivationAdmission {
        if self.closed.load(Ordering::Acquire) {
            return CurrentSessionActivationAdmission::Rejected;
        }
        self.increment(&self.received_count);
        if self.pending.swap(true, Ordering::AcqRel) {
            self.increment(&self.coalesced_count);
        }
        self.schedule_pending()
    }

    fn schedule_pending(self: &Arc<Self>) -> CurrentSessionActivationAdmission {
        if self.closed.load(Ordering::Acquire) {
            self.pending.store(false, Ordering::Release);
            return CurrentSessionActivationAdmission::Rejected;
        }
        if !self.pending.load(Ordering::Acquire) || self.scheduled.swap(true, Ordering::AcqRel) {
            return CurrentSessionActivationAdmission::Accepted;
        }

        let bridge = Arc::clone(self);
        match self
            .scheduler
            .schedule(Box::new(move || bridge.run_scheduled()))
        {
            Ok(()) => CurrentSessionActivationAdmission::Accepted,
            Err(error) => {
                self.increment(&self.scheduling_failure_count);
                self.scheduled.store(false, Ordering::Release);
                if error == ApplicationEventScheduleError::Terminated {
                    self.closed.store(true, Ordering::Release);
                    self.pending.store(false, Ordering::Release);
                }
                CurrentSessionActivationAdmission::Rejected
            }
        }
    }

    fn run_scheduled(self: &Arc<Self>) {
        if self.closed.load(Ordering::Acquire) {
            self.pending.store(false, Ordering::Release);
            self.scheduled.store(false, Ordering::Release);
            return;
        }

        if self.pending.swap(false, Ordering::AcqRel) {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.delivery.deliver()))
            {
                Ok(CurrentSessionActivationAdmission::Accepted) => {
                    self.increment(&self.delivered_count);
                }
                Ok(CurrentSessionActivationAdmission::Rejected) => {
                    self.increment(&self.rejected_count);
                }
                Err(_) => {
                    self.increment(&self.panicked_count);
                }
            }
        }

        self.scheduled.store(false, Ordering::Release);
        if self.pending.load(Ordering::Acquire) && !self.closed.load(Ordering::Acquire) {
            let _ = self.schedule_pending();
        }
    }

    fn flush(self: &Arc<Self>) {
        let _ = self.schedule_pending();
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.pending.store(false, Ordering::Release);
    }

    fn increment(&self, counter: &AtomicU64) {
        if counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .is_err()
        {
            self.overflowed.store(true, Ordering::Release);
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> ApplicationSessionActivationSnapshot {
        ApplicationSessionActivationSnapshot {
            pending: self.pending.load(Ordering::Acquire),
            scheduled: self.scheduled.load(Ordering::Acquire),
            closed: self.closed.load(Ordering::Acquire),
            received_count: self.received_count.load(Ordering::Acquire),
            coalesced_count: self.coalesced_count.load(Ordering::Acquire),
            delivered_count: self.delivered_count.load(Ordering::Acquire),
            rejected_count: self.rejected_count.load(Ordering::Acquire),
            scheduling_failure_count: self.scheduling_failure_count.load(Ordering::Acquire),
            panicked_count: self.panicked_count.load(Ordering::Acquire),
            overflowed: self.overflowed.load(Ordering::Acquire),
        }
    }
}

impl CurrentSessionActivationSink for ApplicationSessionActivationBridge {
    fn request_activation(&self) -> CurrentSessionActivationAdmission {
        let Some(bridge) = self.self_weak.upgrade() else {
            return CurrentSessionActivationAdmission::Rejected;
        };
        bridge.request()
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ApplicationSessionActivationSnapshot {
    pending: bool,
    scheduled: bool,
    closed: bool,
    received_count: u64,
    coalesced_count: u64,
    delivered_count: u64,
    rejected_count: u64,
    scheduling_failure_count: u64,
    panicked_count: u64,
    overflowed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplicationDesktopLifecycleEffect {
    Show,
    Hide,
    OpenRoute(&'static str),
    Quit,
}

impl ApplicationDesktopLifecycleEffect {
    const fn from_intent(intent: DesktopLifecycleIntent) -> Self {
        match intent {
            DesktopLifecycleIntent::Show => Self::Show,
            DesktopLifecycleIntent::Hide => Self::Hide,
            DesktopLifecycleIntent::OpenCompact => Self::OpenRoute("compact_widget"),
            DesktopLifecycleIntent::OpenDashboard => Self::OpenRoute("dashboard"),
            DesktopLifecycleIntent::Quit => Self::Quit,
        }
    }
}

struct ApplicationSessionDetailIntentSink {
    bundle: Weak<Mutex<ApplicationBundleSlot>>,
}

struct ApplicationHistoryRangeIntentSink {
    bundle: Weak<Mutex<ApplicationBundleSlot>>,
}

struct ApplicationSessionPageIntentSink {
    bundle: Weak<Mutex<ApplicationBundleSlot>>,
}

impl ApplicationDesktopLifecycleSink {
    const fn new(window: slint::Weak<MainWindow>) -> Self {
        Self { window }
    }

    fn show_route(&self, route: &str) -> DesktopLifecycleIntentAdmission {
        let Some(window) = self.window.upgrade() else {
            return DesktopLifecycleIntentAdmission::Rejected;
        };
        window.invoke_select_route(route.into());
        Self::show_and_activate(&window)
    }

    fn show_and_activate(window: &MainWindow) -> DesktopLifecycleIntentAdmission {
        window.window().set_minimized(false);
        if window.show().is_err() {
            return DesktopLifecycleIntentAdmission::Rejected;
        }
        tokenmaster_desktop::activate_window(window.window())
            .map_or(DesktopLifecycleIntentAdmission::Rejected, |()| {
                DesktopLifecycleIntentAdmission::Accepted
            })
    }
}

impl DesktopLifecycleIntentSink for ApplicationDesktopLifecycleSink {
    fn submit(&self, intent: DesktopLifecycleIntent) -> DesktopLifecycleIntentAdmission {
        match ApplicationDesktopLifecycleEffect::from_intent(intent) {
            ApplicationDesktopLifecycleEffect::Show => {
                let Some(window) = self.window.upgrade() else {
                    return DesktopLifecycleIntentAdmission::Rejected;
                };
                Self::show_and_activate(&window)
            }
            ApplicationDesktopLifecycleEffect::Hide => {
                let Some(window) = self.window.upgrade() else {
                    return DesktopLifecycleIntentAdmission::Rejected;
                };
                match window.hide() {
                    Ok(()) => DesktopLifecycleIntentAdmission::Accepted,
                    Err(_) => DesktopLifecycleIntentAdmission::Rejected,
                }
            }
            ApplicationDesktopLifecycleEffect::OpenRoute(route) => self.show_route(route),
            ApplicationDesktopLifecycleEffect::Quit => {
                if self.window.upgrade().is_none() {
                    return DesktopLifecycleIntentAdmission::Rejected;
                }
                let _ = slint::quit_event_loop();
                DesktopLifecycleIntentAdmission::Accepted
            }
        }
    }
}

impl ApplicationSessionDetailIntentSink {
    fn new(bundle: Weak<Mutex<ApplicationBundleSlot>>) -> Self {
        Self { bundle }
    }
}

impl ApplicationHistoryRangeIntentSink {
    fn new(bundle: Weak<Mutex<ApplicationBundleSlot>>) -> Self {
        Self { bundle }
    }

    fn request(&self, intent: DesktopHistoryRangeIntent) -> Result<DesktopRefreshAdmission, ()> {
        let bundle = self.bundle.upgrade().ok_or(())?;
        let slot = bundle.try_lock().map_err(|_| ())?;
        let bundle = slot.as_ref().ok_or(())?;
        bundle
            .controller
            .request_history_range(intent)
            .map_err(|_| ())
    }
}

impl ApplicationSessionPageIntentSink {
    fn new(bundle: Weak<Mutex<ApplicationBundleSlot>>) -> Self {
        Self { bundle }
    }

    fn request(&self, intent: DesktopSessionPageIntent) -> Result<DesktopRefreshAdmission, ()> {
        let bundle = self.bundle.upgrade().ok_or(())?;
        let slot = bundle.try_lock().map_err(|_| ())?;
        let bundle = slot.as_ref().ok_or(())?;
        bundle
            .controller
            .request_session_page(intent)
            .map_err(|_| ())
    }
}

impl DesktopSessionDetailIntentSink for ApplicationSessionDetailIntentSink {
    fn submit(&self, intent: DesktopSessionDetailIntent) -> DesktopSessionDetailIntentAdmission {
        let Some(bundle) = self.bundle.upgrade() else {
            return DesktopSessionDetailIntentAdmission::Rejected;
        };
        let Ok(slot) = bundle.try_lock() else {
            return DesktopSessionDetailIntentAdmission::Rejected;
        };
        let Some(bundle) = slot.as_ref() else {
            return DesktopSessionDetailIntentAdmission::Rejected;
        };
        match bundle.controller.request_session_detail(intent) {
            Ok(
                DesktopRefreshAdmission::Started { .. } | DesktopRefreshAdmission::Coalesced { .. },
            ) => DesktopSessionDetailIntentAdmission::Accepted,
            Ok(DesktopRefreshAdmission::DeadlineExceeded { .. }) | Err(_) => {
                DesktopSessionDetailIntentAdmission::Rejected
            }
        }
    }
}

impl DesktopHistoryRangeIntentSink for ApplicationHistoryRangeIntentSink {
    fn submit(&self, intent: DesktopHistoryRangeIntent) -> DesktopHistoryRangeIntentAdmission {
        match self.request(intent) {
            Ok(
                DesktopRefreshAdmission::Started { .. } | DesktopRefreshAdmission::Coalesced { .. },
            ) => DesktopHistoryRangeIntentAdmission::Accepted,
            Ok(DesktopRefreshAdmission::DeadlineExceeded { .. }) | Err(_) => {
                DesktopHistoryRangeIntentAdmission::Rejected
            }
        }
    }
}

impl DesktopSessionPageIntentSink for ApplicationSessionPageIntentSink {
    fn submit(&self, intent: DesktopSessionPageIntent) -> DesktopSessionPageIntentAdmission {
        match self.request(intent) {
            Ok(
                DesktopRefreshAdmission::Started { .. } | DesktopRefreshAdmission::Coalesced { .. },
            ) => DesktopSessionPageIntentAdmission::Accepted,
            Ok(DesktopRefreshAdmission::DeadlineExceeded { .. }) | Err(_) => {
                DesktopSessionPageIntentAdmission::Rejected
            }
        }
    }
}

impl ApplicationDesktopIntentSink {
    #[cfg(test)]
    fn new(submitter: ApplicationOperationSubmitter) -> Self {
        Self {
            dialog: NativeFileDialog::default(),
            submitter,
            current_user_startup: None,
        }
    }

    fn new_with_startup(
        submitter: ApplicationOperationSubmitter,
        port: Rc<dyn ApplicationCurrentUserStartupPort>,
        presenter: DesktopCurrentUserStartupPresenter,
    ) -> Self {
        Self {
            dialog: NativeFileDialog::default(),
            submitter,
            current_user_startup: Some(ApplicationCurrentUserStartupBinding { port, presenter }),
        }
    }

    fn submit_plain(&self, command: ApplicationCommand) -> DesktopIntentAdmission {
        self.map_admission(self.submitter.submit(command))
    }

    fn submit_request(&self, request: ApplicationOperationRequest) -> DesktopIntentAdmission {
        self.map_admission(self.submitter.submit_request(request))
    }

    fn map_admission(&self, admission: ApplicationCommandAdmission) -> DesktopIntentAdmission {
        map_command_admission(admission)
    }

    fn selection(selection: DesktopRestoreSelection) -> Option<ApplicationBackupSelection> {
        ApplicationBackupSelection::new(selection.catalog_generation(), selection.ordinal())
    }

    fn submit_current_user_startup(
        &self,
        action: CurrentUserStartupAction,
    ) -> DesktopIntentAdmission {
        let Some(binding) = self.current_user_startup.as_ref() else {
            return DesktopIntentAdmission::Rejected;
        };
        match binding.port.apply(action) {
            Ok(status) => binding
                .presenter
                .present(map_current_user_startup_status(status))
                .map_or(DesktopIntentAdmission::Rejected, |()| {
                    DesktopIntentAdmission::Started
                }),
            Err(error) => {
                let _ = binding
                    .presenter
                    .present(map_current_user_startup_error(error));
                DesktopIntentAdmission::Rejected
            }
        }
    }
}

impl DesktopIntentSink for ApplicationDesktopIntentSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        match intent {
            DesktopIntent::ExportConfig => {
                match self.dialog.select_output(FileDialogFileType::Config) {
                    FileDialogResult::Selected(output) => {
                        self.submit_request(ApplicationOperationRequest::export_config(output))
                    }
                    FileDialogResult::Cancelled | FileDialogResult::Failed(_) => {
                        DesktopIntentAdmission::Rejected
                    }
                }
            }
            DesktopIntent::ImportConfig => match self
                .dialog
                .select_input(FileDialogFileType::Config, MAX_CONFIG_PACKAGE_BYTES)
            {
                FileDialogResult::Selected(input) => {
                    self.submit_request(ApplicationOperationRequest::import_config(input))
                }
                FileDialogResult::Cancelled | FileDialogResult::Failed(_) => {
                    DesktopIntentAdmission::Rejected
                }
            },
            DesktopIntent::ConfirmConfigImport => {
                self.submit_plain(ApplicationCommand::ConfirmConfigImport)
            }
            DesktopIntent::CancelConfigImport => {
                self.submit_plain(ApplicationCommand::CancelConfigImport)
            }
            DesktopIntent::BackupNormal => self.submit_plain(ApplicationCommand::Backup),
            DesktopIntent::BackupCompact => {
                match self.dialog.select_output(FileDialogFileType::Backup) {
                    FileDialogResult::Selected(output) => {
                        self.submit_request(ApplicationOperationRequest::compact_backup(output))
                    }
                    FileDialogResult::Cancelled | FileDialogResult::Failed(_) => {
                        DesktopIntentAdmission::Rejected
                    }
                }
            }
            DesktopIntent::BackupEncrypted { passphrase } => {
                let mut secret = passphrase.into_string();
                let Ok(passphrase) = BackupPassphrase::existing(&mut secret) else {
                    return DesktopIntentAdmission::Rejected;
                };
                match self
                    .dialog
                    .select_output(FileDialogFileType::EncryptedBackup)
                {
                    FileDialogResult::Selected(output) => self.submit_request(
                        ApplicationOperationRequest::encrypted_backup(output, passphrase),
                    ),
                    FileDialogResult::Cancelled | FileDialogResult::Failed(_) => {
                        DesktopIntentAdmission::Rejected
                    }
                }
            }
            DesktopIntent::VerifyBackups => self.submit_plain(ApplicationCommand::Verify),
            DesktopIntent::PreviewRestore(selection) => {
                if Self::selection(selection).is_some() {
                    DesktopIntentAdmission::Started
                } else {
                    DesktopIntentAdmission::Rejected
                }
            }
            DesktopIntent::ConfirmRestore {
                selection,
                portable_settings,
            } => Self::selection(selection).map_or(DesktopIntentAdmission::Rejected, |selection| {
                self.submit_plain(if portable_settings {
                    ApplicationCommand::RestoreDataAndPortableSettings(selection)
                } else {
                    ApplicationCommand::RestoreData(selection)
                })
            }),
            DesktopIntent::RetryOperation => self.map_admission(self.submitter.retry_last()),
            DesktopIntent::CancelOperation => {
                if self.submitter.cancel_active() {
                    DesktopIntentAdmission::Started
                } else {
                    DesktopIntentAdmission::Rejected
                }
            }
            DesktopIntent::UpdateBackupPolicy {
                periodic_enabled,
                quiet_seconds,
                interval_seconds,
                retention_budget_mib,
            } => self.submit_request(ApplicationOperationRequest::update_backup_policy(
                ApplicationBackupPolicyUpdate::new(
                    periodic_enabled,
                    quiet_seconds,
                    interval_seconds,
                    retention_budget_mib,
                ),
            )),
            DesktopIntent::UpdateReminderPolicy(update) => {
                ApplicationReminderPolicyUpdate::from_desktop(update).map_or(
                    DesktopIntentAdmission::Rejected,
                    |update| {
                        self.submit_request(ApplicationOperationRequest::update_reminder_policy(
                            update,
                        ))
                    },
                )
            }
            DesktopIntent::UpdatePresentation(selection) => {
                self.submit_request(ApplicationOperationRequest::update_presentation(selection))
            }
            DesktopIntent::EnableCurrentUserStartup => {
                self.submit_current_user_startup(CurrentUserStartupAction::Enable)
            }
            DesktopIntent::RepairCurrentUserStartup => {
                self.submit_current_user_startup(CurrentUserStartupAction::RepairStale)
            }
            DesktopIntent::DisableCurrentUserStartup => {
                self.submit_current_user_startup(CurrentUserStartupAction::Disable)
            }
            DesktopIntent::RebuildData => self.submit_plain(ApplicationCommand::Rebuild),
        }
    }
}

const fn map_current_user_startup_status(
    status: CurrentUserStartupStatus,
) -> DesktopCurrentUserStartupStatus {
    match status {
        CurrentUserStartupStatus::Disabled => DesktopCurrentUserStartupStatus::Disabled,
        CurrentUserStartupStatus::EnabledVerified => {
            DesktopCurrentUserStartupStatus::EnabledVerified
        }
        CurrentUserStartupStatus::StaleRelocation => {
            DesktopCurrentUserStartupStatus::StaleRelocation
        }
        CurrentUserStartupStatus::Conflict => DesktopCurrentUserStartupStatus::Conflict,
        CurrentUserStartupStatus::AccessDenied => DesktopCurrentUserStartupStatus::AccessDenied,
        CurrentUserStartupStatus::Unavailable => DesktopCurrentUserStartupStatus::Unavailable,
    }
}

const fn map_current_user_startup_error(
    error: CurrentUserStartupError,
) -> DesktopCurrentUserStartupStatus {
    match error {
        CurrentUserStartupError::AccessDenied => DesktopCurrentUserStartupStatus::AccessDenied,
        CurrentUserStartupError::StaleRequiresRepair => {
            DesktopCurrentUserStartupStatus::StaleRelocation
        }
        CurrentUserStartupError::Conflict => DesktopCurrentUserStartupStatus::Conflict,
        CurrentUserStartupError::Unavailable
        | CurrentUserStartupError::InvalidState
        | CurrentUserStartupError::ReadbackFailed => DesktopCurrentUserStartupStatus::Unavailable,
    }
}

fn map_command_admission(admission: ApplicationCommandAdmission) -> DesktopIntentAdmission {
    match admission {
        ApplicationCommandAdmission::Started(_) => DesktopIntentAdmission::Started,
        ApplicationCommandAdmission::Queued { .. } => DesktopIntentAdmission::Queued,
        ApplicationCommandAdmission::Coalesced { .. } => DesktopIntentAdmission::Coalesced,
        ApplicationCommandAdmission::Rejected(_) => DesktopIntentAdmission::Rejected,
    }
}

const fn application_operation_kind(command: ApplicationCommand) -> DesktopOperationKind {
    match command {
        ApplicationCommand::ExportConfig => DesktopOperationKind::ExportConfig,
        ApplicationCommand::ImportConfig | ApplicationCommand::CancelConfigImport => {
            DesktopOperationKind::ImportConfig
        }
        ApplicationCommand::ConfirmConfigImport => DesktopOperationKind::ApplyConfig,
        ApplicationCommand::Backup
        | ApplicationCommand::BackupCompact
        | ApplicationCommand::BackupEncrypted => DesktopOperationKind::Backup,
        ApplicationCommand::Verify => DesktopOperationKind::Verify,
        ApplicationCommand::RestoreData(_) => DesktopOperationKind::Restore,
        ApplicationCommand::RestoreDataAndPortableSettings(_) => {
            DesktopOperationKind::RestoreWithPortableSettings
        }
        ApplicationCommand::Rebuild => DesktopOperationKind::Rebuild,
        ApplicationCommand::UpdateBackupPolicy | ApplicationCommand::UpdateReminderPolicy => {
            DesktopOperationKind::UpdatePolicy
        }
        ApplicationCommand::UpdatePresentation => DesktopOperationKind::UpdatePresentation,
    }
}

const fn application_operation_cancellable(command: ApplicationCommand) -> bool {
    !matches!(command, ApplicationCommand::CancelConfigImport)
}

fn application_operation_running(command: ApplicationCommand) -> DesktopOperationSnapshot {
    DesktopOperationSnapshot::new(
        application_operation_kind(command),
        DesktopOperationPhase::Running,
        application_operation_cancellable(command),
        None,
    )
}

fn publish_atomic_operation(
    reliable_state: &DesktopReliableStateNotifier,
    command: ApplicationCommand,
) {
    let _ = reliable_state.publish_operation(Some(DesktopOperationSnapshot::new(
        application_operation_kind(command),
        DesktopOperationPhase::AtomicPromotion,
        false,
        None,
    )));
}

fn publish_pending_reminder_policy(
    reliable_state: &DesktopReliableStateNotifier,
    command: ApplicationCommand,
    policy: &ReminderPolicy,
) -> Result<(), ApplicationError> {
    let policy = DesktopReminderPolicy::new(
        policy.enabled(),
        policy.lead_seconds(),
        DesktopReminderSyncState::Pending,
    )
    .ok_or_else(ApplicationError::state)?;
    reliable_state
        .publish_pending_reminder_policy(
            policy,
            DesktopOperationSnapshot::new(
                application_operation_kind(command),
                DesktopOperationPhase::AtomicPromotion,
                false,
                None,
            ),
        )
        .map_err(|_| ApplicationError::state())
}

fn publish_pending_reminder_operation(
    reliable_state: &DesktopReliableStateNotifier,
    command: ApplicationCommand,
) -> Result<(), ApplicationError> {
    reliable_state
        .publish_pending_reminder_operation(DesktopOperationSnapshot::new(
            application_operation_kind(command),
            DesktopOperationPhase::AtomicPromotion,
            false,
            None,
        ))
        .map_err(|_| ApplicationError::state())
}

fn application_operation_completion(
    command: ApplicationCommand,
    execution: ApplicationCommandExecution,
) -> DesktopOperationSnapshot {
    let (phase, failure_code) = match execution {
        ApplicationCommandExecution::Succeeded => (DesktopOperationPhase::Succeeded, None),
        ApplicationCommandExecution::Cancelled => (DesktopOperationPhase::Cancelled, None),
        ApplicationCommandExecution::Failed(failure) => (
            DesktopOperationPhase::Failed,
            Some(match failure {
                ApplicationCommandFailure::Unavailable => "unavailable",
                ApplicationCommandFailure::InvalidSelection => "invalid_selection",
                ApplicationCommandFailure::Integrity => "integrity",
                ApplicationCommandFailure::CapacityExceeded => "capacity_exceeded",
                ApplicationCommandFailure::Internal => "internal",
            }),
        ),
    };
    DesktopOperationSnapshot::new(
        application_operation_kind(command),
        phase,
        false,
        failure_code,
    )
}

#[allow(clippy::too_many_arguments)]
fn start_live_bundle(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    preflight: &mut ApplicationPreflight,
    bridge_factory: &DesktopBridgeFactory,
    bundle: &SharedBundle,
    outcome: BootstrapOutcome,
    observer: &mut dyn FnMut(ApplicationStartBoundary) -> Result<(), ApplicationError>,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
    if preflight.requires_source_reconciliation() {
        if outcome != BootstrapOutcome::Healthy {
            return Err(ApplicationError::state());
        }
        observer(ApplicationStartBoundary::BeforeReconstructionReconciliation)?;
        let startup_guard = preflight.take_startup_guard()?;
        let bridge = start_reconstructed_bundle(
            environment,
            data_root,
            state,
            bridge_factory,
            bundle,
            startup_guard,
        )?;
        preflight.mark_live_healthy();
        return Ok(bridge);
    }
    let mut pending_migration = preflight.report().prior_run().pending_migration();
    if let Some(pending) = pending_migration {
        state.validate_pending_migration(pending)?;
        if !matches!(
            outcome,
            BootstrapOutcome::Healthy | BootstrapOutcome::MigrationRequired
        ) {
            return Err(ApplicationError::state());
        }
    }
    let maintenance =
        if outcome == BootstrapOutcome::MigrationRequired || pending_migration.is_some() {
            let runtime =
                state.start_maintenance(data_root, MaintenanceSourceState::HealthyUnpublished)?;
            Some(runtime)
        } else {
            None
        };
    if outcome == BootstrapOutcome::MigrationRequired {
        let runtime = maintenance
            .as_ref()
            .ok_or_else(ApplicationError::internal)?;
        let _ = wait_for_mandatory_backup(runtime, MaintenancePurpose::PreMigration)?;
        let (from_schema_version, to_schema_version) = state.migration_versions(data_root)?;
        let pending = preflight
            .session_mut()
            .require_post_migration(from_schema_version, to_schema_version)
            .map_err(|_| ApplicationError::state())?;
        pending_migration = Some(pending);
        observer(ApplicationStartBoundary::PreMigrationBackupPublished)?;
    }
    let startup_guard = preflight.take_startup_guard()?;
    let started = start_guarded_live(environment, data_root, bundle, startup_guard)?;
    if let Some(pending) = pending_migration {
        observer(ApplicationStartBoundary::BeforePostMigrationBackup)?;
        let runtime = maintenance
            .as_ref()
            .ok_or_else(ApplicationError::internal)?;
        let _ = wait_for_mandatory_backup(runtime, MaintenancePurpose::PostMigration)?;
        preflight
            .session_mut()
            .complete_post_migration(pending)
            .map_err(|_| ApplicationError::state())?;
    }
    let maintenance_source = match outcome {
        BootstrapOutcome::Healthy => MaintenanceSourceState::Healthy,
        BootstrapOutcome::FirstInstall => MaintenanceSourceState::HealthyUnpublished,
        BootstrapOutcome::MigrationRequired => MaintenanceSourceState::Healthy,
        _ => return Err(ApplicationError::internal()),
    };
    finish_live_bundle(
        data_root,
        state,
        bridge_factory,
        bundle,
        started,
        maintenance,
        maintenance_source,
    )
}

fn start_current_bundle(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    bridge_factory: &DesktopBridgeFactory,
    bundle: &SharedBundle,
    guard: ExclusiveFileLeaseGuard,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
    let started = start_guarded_live(environment, data_root, bundle, guard)?;
    finish_live_bundle(
        data_root,
        state,
        bridge_factory,
        bundle,
        started,
        None,
        MaintenanceSourceState::Healthy,
    )
}

#[allow(clippy::too_many_arguments)]
fn start_restored_bundle(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    preflight: &mut ApplicationPreflight,
    bridge_factory: &DesktopBridgeFactory,
    bundle: &SharedBundle,
    guard: ExclusiveFileLeaseGuard,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
    if !state.restored_archive_requires_migration(data_root)? {
        return start_current_bundle(environment, data_root, state, bridge_factory, bundle, guard);
    }

    let maintenance =
        state.start_maintenance(data_root, MaintenanceSourceState::HealthyUnpublished)?;
    let _ = wait_for_mandatory_backup(&maintenance, MaintenancePurpose::PreMigration)?;
    let (from_schema_version, to_schema_version) = state.migration_versions(data_root)?;
    let pending = preflight
        .session_mut()
        .require_post_migration(from_schema_version, to_schema_version)
        .map_err(|_| ApplicationError::state())?;
    let started = start_guarded_live(environment, data_root, bundle, guard)?;
    let _ = wait_for_mandatory_backup(&maintenance, MaintenancePurpose::PostMigration)?;
    preflight
        .session_mut()
        .complete_post_migration(pending)
        .map_err(|_| ApplicationError::state())?;
    finish_live_bundle(
        data_root,
        state,
        bridge_factory,
        bundle,
        started,
        Some(maintenance),
        MaintenanceSourceState::Healthy,
    )
}

fn start_reconstructed_bundle(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    bridge_factory: &DesktopBridgeFactory,
    bundle: &SharedBundle,
    guard: ExclusiveFileLeaseGuard,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
    let started = start_guarded_live(environment, data_root, bundle, guard)?;
    started
        .live
        .refresh_now(RefreshUrgency::Recovery)
        .map_err(|_| ApplicationError::live_runtime())?;
    wait_for_reconstructed_reconciliation(&started.live)?;
    finish_live_bundle(
        data_root,
        state,
        bridge_factory,
        bundle,
        started,
        None,
        MaintenanceSourceState::Healthy,
    )
}

fn wait_for_reconstructed_reconciliation(live: &LiveRuntime) -> Result<(), ApplicationError> {
    let deadline = std::time::Instant::now() + MANDATORY_BACKUP_TIMEOUT;
    let mut observed_completion = false;
    loop {
        let snapshot = live
            .snapshot()
            .map_err(|_| ApplicationError::live_runtime())?;
        let refresh = snapshot.refresh();
        if observed_completion
            && refresh.outcome() == Some(RefreshOutcome::Completed)
            && refresh.error().is_none()
            && snapshot.engine().diagnostics().completed_refreshes() > 0
            && !snapshot.scheduler().dirty()
            && !snapshot.scheduler().force_reconcile()
            && snapshot.worker().active_request_id().is_none()
            && snapshot.worker().pending_count() == 0
        {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(ApplicationError::live_runtime());
        }
        let Some(completion) = live
            .wait_for_completion(remaining)
            .map_err(|_| ApplicationError::live_runtime())?
        else {
            return Err(ApplicationError::live_runtime());
        };
        if completion.outcome() != RefreshOutcome::Completed {
            return Err(ApplicationError::live_runtime());
        }
        observed_completion = true;
    }
}

struct GuardedLiveStart {
    live: LiveRuntime,
    notifier: Arc<ApplicationRuntimeNotifier>,
    notifier_port: Arc<dyn WorkerCompletionNotifier>,
    bundle_generation: u64,
}

fn start_guarded_live(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    bundle: &SharedBundle,
    guard: ExclusiveFileLeaseGuard,
) -> Result<GuardedLiveStart, ApplicationError> {
    let codex_home = environment
        .codex_home()
        .map(|value| value.to_str().ok_or_else(ApplicationError::discovery))
        .transpose()?;
    let configured: [ConfiguredCodexRoot; 0] = [];
    let discovery = build_discovery_request(CodexRootInput {
        user_profile: environment.user_profile(),
        codex_home,
        configured: &configured,
    })
    .map_err(|_| ApplicationError::discovery())?;
    let bundle_generation = begin_bundle_generation(bundle)?;
    let notifier = Arc::new(ApplicationRuntimeNotifier::new(
        Arc::downgrade(bundle),
        bundle_generation,
    ));
    let notifier_port: Arc<dyn WorkerCompletionNotifier> = notifier.clone();
    let usage_provider =
        CodexUsageProviderFactory::new(discovery).map_err(|_| ApplicationError::live_runtime())?;
    let live = LiveRuntime::start_notified_guarded_with_provider(
        data_root.archive_path(),
        Box::new(usage_provider),
        guard,
        notifier_port.clone(),
    )
    .map_err(|_| ApplicationError::live_runtime())?;
    Ok(GuardedLiveStart {
        live,
        notifier,
        notifier_port,
        bundle_generation,
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_live_bundle(
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    bridge_factory: &DesktopBridgeFactory,
    bundle: &SharedBundle,
    started: GuardedLiveStart,
    maintenance: Option<BackupMaintenanceRuntime>,
    maintenance_source: MaintenanceSourceState,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
    let archive_path = data_root.archive_path().to_path_buf();
    let quota =
        OptionalRuntime::start(CodexQuotaRuntimeConfig::new(archive_path.clone()).and_then(
            |config| ProviderQuotaRuntime::start_notified(config, started.notifier_port.clone()),
        ));
    let reminder = start_optional_reminder_runtime(
        data_root,
        state,
        archive_path.clone(),
        started.notifier_port.clone(),
    );
    let maintenance = match maintenance {
        Some(maintenance) => maintenance,
        None => state.start_maintenance(data_root, maintenance_source)?,
    };
    let mut controller = DesktopController::open(
        &archive_path,
        DesktopQueryPlan::overview().map_err(|_| ApplicationError::controller())?,
    )
    .map_err(|_| ApplicationError::controller())?;
    let live_bridge = bridge_factory
        .snapshot_bridge(controller.snapshot_receiver())
        .map_err(|_| ApplicationError::controller())?;
    controller
        .bind_snapshot_epoch(live_bridge.epoch())
        .map_err(|_| ApplicationError::controller())?;
    controller
        .attach_snapshot_notifier(live_bridge.notifier())
        .map_err(|_| ApplicationError::controller())?;
    controller
        .attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())
        .map_err(|_| ApplicationError::controller())?;
    controller
        .attach_terminal_history_range_notifier(live_bridge.terminal_history_range_notifier())
        .map_err(|_| ApplicationError::controller())?;
    let refresh_ingress = controller.refresh_ingress();
    let notification_presentation = match reminder.owner() {
        Some(runtime) => {
            let presenter = bridge_factory
                .in_app_notification_bridge()
                .map_err(|_| ApplicationError::controller())?;
            let port = Arc::new(RuntimeReminderPresentationPort::new(Arc::clone(runtime)));
            Some(
                ReminderPresentationCoordinator::start(port, Arc::new(presenter))
                    .map_err(|_| ApplicationError::controller())?,
            )
        }
        None => None,
    };

    {
        let mut slot = bundle.lock().map_err(|_| ApplicationError::internal())?;
        if slot.generation != started.bundle_generation || slot.bundle.is_some() {
            return Err(ApplicationError::internal());
        }
        slot.bundle = Some(ApplicationBundle {
            live: started.live,
            quota,
            reminder,
            notification_presentation,
            controller,
            refresh_ingress,
            maintenance,
            #[cfg(test)]
            reminder_sync_counters: Arc::new(ReminderSyncCounters::default()),
            #[cfg(test)]
            notifier: Arc::clone(&started.notifier),
        });
    }
    started
        .notifier
        .publish()
        .map_err(|_| ApplicationError::controller())?;
    Ok(live_bridge)
}

fn start_optional_reminder_runtime(
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    archive_path: std::path::PathBuf,
    notifier_port: Arc<dyn WorkerCompletionNotifier>,
) -> OptionalReminderRuntime {
    match state.synchronize_reminder_profile(data_root) {
        Ok(_) => OptionalReminderRuntime::start(
            BenefitReminderRuntimeConfig::new(archive_path)
                .and_then(|config| BenefitReminderRuntime::start_notified(config, notifier_port)),
        ),
        Err(_) => OptionalReminderRuntime::failed(RuntimeErrorCode::StoreUnavailable),
    }
}

fn begin_bundle_generation(bundle: &SharedBundle) -> Result<u64, ApplicationError> {
    let mut slot = bundle.lock().map_err(|_| ApplicationError::internal())?;
    if slot.bundle.is_some() {
        return Err(ApplicationError::internal());
    }
    slot.generation = slot
        .generation
        .checked_add(1)
        .ok_or_else(ApplicationError::generation_overflow)?;
    Ok(slot.generation)
}

fn discard_bundle(bundle: &SharedBundle) -> Result<(), ApplicationError> {
    let owned = bundle
        .lock()
        .map_err(|_| ApplicationError::internal())?
        .take();
    match owned {
        Some(mut owned) => owned.shutdown(),
        None => Ok(()),
    }
}

fn wait_for_mandatory_backup(
    maintenance: &BackupMaintenanceRuntime,
    purpose: MaintenancePurpose,
) -> Result<MaintenanceCompletion, ApplicationError> {
    let completion = maintenance
        .submit_and_wait(purpose, MANDATORY_BACKUP_TIMEOUT)
        .map_err(|_| ApplicationError::state())?;
    match completion.outcome() {
        MaintenanceOutcome::Published => Ok(completion),
        MaintenanceOutcome::RetryScheduled
        | MaintenanceOutcome::SourceSuspect
        | MaintenanceOutcome::Cancelled
        | MaintenanceOutcome::Failed => Err(ApplicationError::state()),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_application_operation(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    preflight: &Arc<Mutex<ApplicationPreflight>>,
    bundle: &SharedBundle,
    bridge_factory: &DesktopBridgeFactory,
    bridge: &SharedBridge,
    live_started: &Arc<AtomicBool>,
    reliable_state: &DesktopReliableStateNotifier,
    permit: &ApplicationCommandPermit,
    payload: ApplicationOperationPayload,
) -> ApplicationCommandExecution {
    if permit.is_cancelled() {
        return ApplicationCommandExecution::Cancelled;
    }
    match (permit.command(), payload) {
        (ApplicationCommand::Backup, ApplicationOperationPayload::Empty) => {
            execute_manual_backup_command(bundle, reliable_state, permit)
        }
        (ApplicationCommand::BackupCompact, ApplicationOperationPayload::BackupOutput(output)) => {
            execute_state_command(state.export_compact_backup(data_root, permit, output, || {
                publish_atomic_operation(reliable_state, permit.command());
            }))
        }
        (
            ApplicationCommand::BackupEncrypted,
            ApplicationOperationPayload::EncryptedBackupOutput { output, passphrase },
        ) => execute_state_command(state.export_encrypted_backup(
            data_root,
            permit,
            output,
            passphrase,
            || {
                publish_atomic_operation(reliable_state, permit.command());
            },
        )),
        (ApplicationCommand::ExportConfig, ApplicationOperationPayload::ConfigOutput(output)) => {
            execute_state_command(current_utc_millis().and_then(|now| {
                state.export_config(permit, output, now, || {
                    publish_atomic_operation(reliable_state, permit.command());
                })
            }))
        }
        (ApplicationCommand::ImportConfig, ApplicationOperationPayload::ConfigInput(input)) => {
            execute_state_command(state.stage_config_import_preview(permit, input))
        }
        (ApplicationCommand::ConfirmConfigImport, ApplicationOperationPayload::Empty) => {
            match state.commit_pending_config_import(permit, || {
                publish_pending_reminder_operation(reliable_state, permit.command())
            }) {
                Ok(_) => execute_state_command(synchronize_reminder_policy_after_settings(
                    state, data_root, bundle,
                )),
                Err(error) => execute_state_command::<()>(Err(error)),
            }
        }
        (ApplicationCommand::CancelConfigImport, ApplicationOperationPayload::Empty) => {
            execute_state_command(state.cancel_pending_config_import(permit))
        }
        (ApplicationCommand::Verify, ApplicationOperationPayload::Empty) => {
            execute_state_command(state.verify_backups(permit))
        }
        (
            ApplicationCommand::UpdateBackupPolicy,
            ApplicationOperationPayload::BackupPolicy(update),
        ) => execute_state_command(
            state
                .update_backup_policy(permit, update, || {
                    publish_atomic_operation(reliable_state, permit.command());
                })
                .and_then(|policy| update_live_backup_policy(bundle, &policy)),
        ),
        (
            ApplicationCommand::UpdateReminderPolicy,
            ApplicationOperationPayload::ReminderPolicy(update),
        ) => {
            let policy = update.into_policy();
            let pending_policy = policy.clone();
            match state.update_reminder_policy(permit, policy, || {
                publish_pending_reminder_policy(reliable_state, permit.command(), &pending_policy)
            }) {
                Ok(()) => execute_state_command(synchronize_reminder_policy_after_settings(
                    state, data_root, bundle,
                )),
                Err(error) => execute_state_command::<()>(Err(error)),
            }
        }
        (
            ApplicationCommand::UpdatePresentation,
            ApplicationOperationPayload::Presentation(update),
        ) => execute_state_command(state.update_presentation(
            permit,
            update.into_state_presentation(),
            || publish_atomic_operation(reliable_state, permit.command()),
        )),
        (ApplicationCommand::RestoreData(selection), ApplicationOperationPayload::Empty) => {
            execute_restore_operation(
                environment,
                data_root,
                state,
                preflight,
                bundle,
                bridge_factory,
                bridge,
                live_started,
                reliable_state,
                selection,
                RestoreMode::DataOnly,
                permit,
            )
        }
        (
            ApplicationCommand::RestoreDataAndPortableSettings(selection),
            ApplicationOperationPayload::Empty,
        ) => execute_restore_operation(
            environment,
            data_root,
            state,
            preflight,
            bundle,
            bridge_factory,
            bridge,
            live_started,
            reliable_state,
            selection,
            RestoreMode::DataAndPortableSettings,
            permit,
        ),
        (ApplicationCommand::Rebuild, ApplicationOperationPayload::Empty) => {
            execute_rebuild_operation(
                environment,
                data_root,
                state,
                preflight,
                bundle,
                bridge_factory,
                bridge,
                live_started,
                reliable_state,
                permit,
            )
        }
        _ => ApplicationCommandExecution::Failed(ApplicationCommandFailure::InvalidSelection),
    }
}

fn update_live_backup_policy(
    bundle: &SharedBundle,
    policy: &tokenmaster_state::BackupPolicy,
) -> Result<(), ApplicationError> {
    let slot = bundle.lock().map_err(|_| ApplicationError::internal())?;
    match slot.as_ref() {
        Some(bundle) => bundle
            .maintenance
            .update_policy(policy)
            .map_err(|_| ApplicationError::state()),
        None => Ok(()),
    }
}

fn synchronize_reminder_policy_after_settings(
    state: &ApplicationStateOwner,
    data_root: &DataRoot,
    bundle: &SharedBundle,
) -> Result<(), ApplicationError> {
    if state.synchronize_reminder_profile(data_root).is_err() {
        return Ok(());
    }
    let (reminder, ingress) = {
        let slot = bundle.lock().map_err(|_| ApplicationError::internal())?;
        let Some(bundle) = slot.as_ref() else {
            return Ok(());
        };
        (
            bundle.reminder.owner().cloned(),
            bundle.refresh_ingress.clone(),
        )
    };
    #[cfg(test)]
    let counters = {
        let slot = bundle.lock().map_err(|_| ApplicationError::internal())?;
        let Some(bundle) = slot.as_ref() else {
            return Ok(());
        };
        Arc::clone(&bundle.reminder_sync_counters)
    };
    if let Some(reminder) = reminder {
        reminder
            .lock()
            .map_err(|_| ApplicationError::internal())?
            .notify_profile_changed()
            .map_err(|_| ApplicationError::state())?;
        #[cfg(test)]
        counters.profile_hints.fetch_add(1, Ordering::AcqRel);
    }
    ingress
        .refresh(DesktopRefreshUrgency::Hint)
        .map_err(|_| ApplicationError::controller())?;
    #[cfg(test)]
    counters.controller_refreshes.fetch_add(1, Ordering::AcqRel);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_rebuild_operation(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    preflight: &Arc<Mutex<ApplicationPreflight>>,
    bundle: &SharedBundle,
    bridge_factory: &DesktopBridgeFactory,
    bridge: &SharedBridge,
    live_started: &Arc<AtomicBool>,
    reliable_state: &DesktopReliableStateNotifier,
    permit: &ApplicationCommandPermit,
) -> ApplicationCommandExecution {
    match try_rebuild_operation(
        environment,
        data_root,
        state,
        preflight,
        bundle,
        bridge_factory,
        bridge,
        live_started,
        reliable_state,
        permit,
    ) {
        Ok(true) => ApplicationCommandExecution::Succeeded,
        Ok(false) => ApplicationCommandExecution::Cancelled,
        Err(error) => execute_state_command::<()>(Err(error)),
    }
}

#[allow(clippy::too_many_arguments)]
fn try_rebuild_operation(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    preflight: &Arc<Mutex<ApplicationPreflight>>,
    bundle: &SharedBundle,
    bridge_factory: &DesktopBridgeFactory,
    bridge: &SharedBridge,
    live_started: &Arc<AtomicBool>,
    reliable_state: &DesktopReliableStateNotifier,
    permit: &ApplicationCommandPermit,
) -> Result<bool, ApplicationError> {
    if live_started.load(Ordering::Acquire)
        || !bundle
            .lock()
            .map_err(|_| ApplicationError::internal())?
            .is_none()
        || bridge
            .lock()
            .map_err(|_| ApplicationError::internal())?
            .is_some()
    {
        return Err(ApplicationError::invalid_lifecycle());
    }
    let (outcome, source_reconciliation_required) = {
        let preflight = preflight.lock().map_err(|_| ApplicationError::internal())?;
        (
            preflight.effective_outcome(),
            preflight.requires_source_reconciliation(),
        )
    };
    if !matches!(
        outcome,
        BootstrapOutcome::RecoveryRequired | BootstrapOutcome::SafeMode
    ) {
        return Err(ApplicationError::invalid_lifecycle());
    }
    let guard = state.acquire_runtime_guard(data_root)?;
    if source_reconciliation_required {
        if permit.begin_irreversible().is_err() {
            return Ok(false);
        }
        publish_atomic_operation(reliable_state, permit.command());
        let start_result = (|| {
            let mut preflight = preflight.lock().map_err(|_| ApplicationError::internal())?;
            let rebuilt_bridge = start_reconstructed_bundle(
                environment,
                data_root,
                state,
                bridge_factory,
                bundle,
                guard,
            )?;
            preflight.session_mut().authorize_healthy_launch();
            preflight.mark_live_healthy();
            Ok(rebuilt_bridge)
        })();
        return match start_result {
            Ok(rebuilt_bridge) => {
                *bridge.lock().map_err(|_| ApplicationError::internal())? = Some(rebuilt_bridge);
                live_started.store(true, Ordering::Release);
                Ok(true)
            }
            Err(error) => {
                let _ = discard_bundle(bundle);
                Err(error)
            }
        };
    }
    let receipt = match state.reconstruct_definitively_corrupt(permit, &guard, || {
        publish_atomic_operation(reliable_state, permit.command());
    }) {
        Ok(receipt) => receipt,
        Err(_) if permit.is_cancelled() => return Ok(false),
        Err(error) => return Err(error),
    };
    let start_result = (|| {
        let mut preflight = preflight.lock().map_err(|_| ApplicationError::internal())?;
        preflight.bind_recovery_launch(receipt)?;
        let rebuilt_bridge = start_reconstructed_bundle(
            environment,
            data_root,
            state,
            bridge_factory,
            bundle,
            guard,
        )?;
        preflight.mark_live_healthy();
        Ok(rebuilt_bridge)
    })();
    match start_result {
        Ok(rebuilt_bridge) => {
            *bridge.lock().map_err(|_| ApplicationError::internal())? = Some(rebuilt_bridge);
            live_started.store(true, Ordering::Release);
            Ok(true)
        }
        Err(error) => {
            let _ = discard_bundle(bundle);
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_restore_operation(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    preflight: &Arc<Mutex<ApplicationPreflight>>,
    bundle: &SharedBundle,
    bridge_factory: &DesktopBridgeFactory,
    bridge: &SharedBridge,
    live_started: &Arc<AtomicBool>,
    reliable_state: &DesktopReliableStateNotifier,
    selection: ApplicationBackupSelection,
    mode: RestoreMode,
    permit: &ApplicationCommandPermit,
) -> ApplicationCommandExecution {
    match try_restore_operation(
        environment,
        data_root,
        state,
        preflight,
        bundle,
        bridge_factory,
        bridge,
        live_started,
        reliable_state,
        selection,
        mode,
        permit,
    ) {
        Ok(true) => ApplicationCommandExecution::Succeeded,
        Ok(false) => ApplicationCommandExecution::Cancelled,
        Err(error) => execute_state_command::<()>(Err(error)),
    }
}

#[allow(clippy::too_many_arguments)]
fn try_restore_operation(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    preflight: &Arc<Mutex<ApplicationPreflight>>,
    bundle: &SharedBundle,
    bridge_factory: &DesktopBridgeFactory,
    bridge: &SharedBridge,
    live_started: &Arc<AtomicBool>,
    reliable_state: &DesktopReliableStateNotifier,
    selection: ApplicationBackupSelection,
    mode: RestoreMode,
    permit: &ApplicationCommandPermit,
) -> Result<bool, ApplicationError> {
    if !live_started.load(Ordering::Acquire) || mode == RestoreMode::AutomaticDataOnly {
        return Err(ApplicationError::invalid_lifecycle());
    }
    let selection_pin = state.bind_backup_selection(selection)?;
    let binding = selection_pin.binding();
    live_started.store(false, Ordering::Release);
    drop(
        bridge
            .lock()
            .map_err(|_| ApplicationError::internal())?
            .take(),
    );
    let owned = bundle
        .lock()
        .map_err(|_| ApplicationError::internal())?
        .take()
        .ok_or_else(ApplicationError::invalid_lifecycle)?;
    let mut owned = owned;
    owned.shutdown()?;

    let guard = state.acquire_runtime_guard(data_root)?;
    let mut maintenance = match state.start_protected_maintenance(
        data_root,
        MaintenanceSourceState::Healthy,
        binding,
    ) {
        Ok(maintenance) => maintenance,
        Err(error) => {
            drop(selection_pin);
            restart_current_after_pre_mutation_exit(
                environment,
                data_root,
                state,
                bridge_factory,
                bundle,
                bridge,
                live_started,
                guard,
            )?;
            return Err(error);
        }
    };
    let safety_result = wait_for_mandatory_backup(&maintenance, MaintenancePurpose::PreRestore);
    let shutdown_result = maintenance
        .shutdown()
        .map_err(|_| ApplicationError::state());
    drop(maintenance);
    let safety = match (safety_result, shutdown_result) {
        (Ok(safety), Ok(())) => safety,
        (Err(error), _) | (Ok(_), Err(error)) => {
            drop(selection_pin);
            restart_current_after_pre_mutation_exit(
                environment,
                data_root,
                state,
                bridge_factory,
                bundle,
                bridge,
                live_started,
                guard,
            )?;
            return Err(error);
        }
    };

    if permit.begin_irreversible().is_err() {
        let cancelled = permit.is_cancelled();
        drop(selection_pin);
        restart_current_after_pre_mutation_exit(
            environment,
            data_root,
            state,
            bridge_factory,
            bundle,
            bridge,
            live_started,
            guard,
        )?;
        return if cancelled {
            Ok(false)
        } else {
            Err(ApplicationError::internal())
        };
    }
    publish_atomic_operation(reliable_state, permit.command());

    drop(selection_pin);
    let control = BackupControl::new(Arc::new(AtomicBool::new(false)), MANDATORY_BACKUP_TIMEOUT)
        .map_err(|_| ApplicationError::state())?;
    let receipt = match state.restore_selected(
        binding,
        mode,
        RestoreSafety::PreRestoreBackupPublished(safety),
        &guard,
        &control,
    ) {
        Ok(receipt) => receipt,
        Err(error) => {
            let _ = discard_bundle(bundle);
            return Err(error);
        }
    };
    let start_result = (|| {
        let mut preflight = preflight.lock().map_err(|_| ApplicationError::internal())?;
        preflight.bind_recovery_launch(receipt)?;
        start_restored_bundle(
            environment,
            data_root,
            state,
            &mut preflight,
            bridge_factory,
            bundle,
            guard,
        )
    })();
    match start_result {
        Ok(restored_bridge) => {
            let mut bridge_slot = match bridge.lock() {
                Ok(bridge_slot) => bridge_slot,
                Err(_) => {
                    drop(restored_bridge);
                    let _ = discard_bundle(bundle);
                    return Err(ApplicationError::internal());
                }
            };
            *bridge_slot = Some(restored_bridge);
            live_started.store(true, Ordering::Release);
            Ok(true)
        }
        Err(error) => {
            let _ = discard_bundle(bundle);
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn restart_current_after_pre_mutation_exit(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    bridge_factory: &DesktopBridgeFactory,
    bundle: &SharedBundle,
    bridge: &SharedBridge,
    live_started: &Arc<AtomicBool>,
    guard: ExclusiveFileLeaseGuard,
) -> Result<(), ApplicationError> {
    match start_current_bundle(environment, data_root, state, bridge_factory, bundle, guard) {
        Ok(current_bridge) => {
            let mut bridge_slot = match bridge.lock() {
                Ok(bridge_slot) => bridge_slot,
                Err(_) => {
                    drop(current_bridge);
                    let _ = discard_bundle(bundle);
                    return Err(ApplicationError::internal());
                }
            };
            *bridge_slot = Some(current_bridge);
            live_started.store(true, Ordering::Release);
            Ok(())
        }
        Err(error) => {
            let _ = discard_bundle(bundle);
            Err(error)
        }
    }
}

fn execute_state_command<T>(result: Result<T, ApplicationError>) -> ApplicationCommandExecution {
    match result {
        Ok(_) => ApplicationCommandExecution::Succeeded,
        Err(error) => ApplicationCommandExecution::Failed(match error.code() {
            ApplicationErrorCode::InvalidLifecycle => ApplicationCommandFailure::InvalidSelection,
            ApplicationErrorCode::GenerationOverflow => ApplicationCommandFailure::CapacityExceeded,
            ApplicationErrorCode::Internal => ApplicationCommandFailure::Internal,
            ApplicationErrorCode::CurrentSessionUnavailable
            | ApplicationErrorCode::DataUnavailable
            | ApplicationErrorCode::DiscoveryUnavailable
            | ApplicationErrorCode::LiveRuntimeUnavailable
            | ApplicationErrorCode::StateUnavailable
            | ApplicationErrorCode::ControllerUnavailable
            | ApplicationErrorCode::UiUnavailable
            | ApplicationErrorCode::EventLoopUnavailable
            | ApplicationErrorCode::ShutdownFailed => ApplicationCommandFailure::Unavailable,
        }),
    }
}

fn current_utc_millis() -> Result<i64, ApplicationError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApplicationError::state())
        .and_then(|duration| {
            i64::try_from(duration.as_millis()).map_err(|_| ApplicationError::generation_overflow())
        })
}

fn execute_manual_backup_command(
    bundle: &SharedBundle,
    reliable_state: &DesktopReliableStateNotifier,
    permit: &ApplicationCommandPermit,
) -> ApplicationCommandExecution {
    let slot = match bundle.lock() {
        Ok(slot) => slot,
        Err(_) => {
            return ApplicationCommandExecution::Failed(ApplicationCommandFailure::Internal);
        }
    };
    let Some(bundle) = slot.bundle.as_ref() else {
        return ApplicationCommandExecution::Failed(ApplicationCommandFailure::Unavailable);
    };
    if permit.begin_irreversible().is_err() {
        return if permit.is_cancelled() {
            ApplicationCommandExecution::Cancelled
        } else {
            ApplicationCommandExecution::Failed(ApplicationCommandFailure::Internal)
        };
    }
    publish_atomic_operation(reliable_state, permit.command());
    let completion = match bundle
        .maintenance
        .submit_and_wait(MaintenancePurpose::Manual, MANDATORY_BACKUP_TIMEOUT)
    {
        Ok(completion) => completion,
        Err(error) => {
            return ApplicationCommandExecution::Failed(map_state_command_failure(error.code()));
        }
    };
    match completion.outcome() {
        MaintenanceOutcome::Published => ApplicationCommandExecution::Succeeded,
        MaintenanceOutcome::RetryScheduled
        | MaintenanceOutcome::SourceSuspect
        | MaintenanceOutcome::Cancelled
        | MaintenanceOutcome::Failed => {
            ApplicationCommandExecution::Failed(completion.failure_code().map_or(
                ApplicationCommandFailure::Unavailable,
                map_state_command_failure,
            ))
        }
    }
}

const fn map_state_command_failure(code: StateErrorCode) -> ApplicationCommandFailure {
    match code {
        StateErrorCode::CapacityExceeded | StateErrorCode::DiskCapacity => {
            ApplicationCommandFailure::CapacityExceeded
        }
        StateErrorCode::Integrity | StateErrorCode::RecoveryRequired => {
            ApplicationCommandFailure::Integrity
        }
        StateErrorCode::InternalInvariant => ApplicationCommandFailure::Internal,
        StateErrorCode::InvalidInput
        | StateErrorCode::UnsupportedVersion
        | StateErrorCode::Unavailable
        | StateErrorCode::Busy => ApplicationCommandFailure::Unavailable,
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct ApplicationBundle {
    live: LiveRuntime,
    quota: OptionalRuntime<ProviderQuotaRuntime>,
    reminder: OptionalReminderRuntime,
    notification_presentation: Option<ReminderPresentationCoordinator>,
    controller: DesktopController,
    refresh_ingress: DesktopRefreshIngress,
    maintenance: BackupMaintenanceRuntime,
    #[cfg(test)]
    reminder_sync_counters: Arc<ReminderSyncCounters>,
    #[cfg(test)]
    notifier: Arc<ApplicationRuntimeNotifier>,
}

#[cfg(test)]
#[derive(Default)]
struct ReminderSyncCounters {
    profile_hints: AtomicU64,
    controller_refreshes: AtomicU64,
}

impl ApplicationBundle {
    fn publish_runtime(
        &self,
        generation: ProductRuntimeGeneration,
    ) -> Result<(), ApplicationError> {
        let (usage, git) = match self.live.snapshot() {
            Ok(snapshot) => (
                Ok(ProductUsageRuntimeHealth::from(snapshot)),
                Ok(ProductGitRuntimeHealth::from(snapshot.git())),
            ),
            Err(error) => {
                let error = ProductRuntimeObservationError::from(error.code());
                (Err(error), Err(error))
            }
        };
        let quota = self.quota.snapshot(|runtime| {
            runtime
                .snapshot()
                .map(ProductQuotaRuntimeHealth::from)
                .map_err(|error| error.code())
        });
        let reminder = self.reminder.snapshot(|runtime| {
            runtime
                .snapshot()
                .map(ProductReminderRuntimeHealth::from)
                .map_err(|error| error.code())
        });
        let observation = DesktopRuntimeObservation::new(generation, usage, quota, reminder, git);
        if let Some(presentation) = self.notification_presentation.as_ref() {
            let _ = presentation.pump();
        }
        self.controller
            .observe_runtime(observation)
            .map_err(|_| ApplicationError::controller())?;
        self.controller
            .refresh(DesktopRefreshUrgency::Hint)
            .map_err(|_| ApplicationError::controller())?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), ApplicationError> {
        let mut first = None;
        if let Some(mut presentation) = self.notification_presentation.take()
            && presentation.shutdown().is_err()
        {
            first = Some(ApplicationError::shutdown());
        }
        if self.maintenance.pause().is_err() && first.is_none() {
            first = Some(ApplicationError::shutdown());
        }
        remember_failure(&mut first, self.live.pause().map(|_| ()));
        if let Some(quota) = self.quota.owner_mut() {
            remember_failure(&mut first, quota.pause().map(|_| ()));
        }
        if let Some(reminder) = self.reminder.owner() {
            match reminder.lock() {
                Ok(mut reminder) => remember_failure(&mut first, reminder.pause().map(|_| ())),
                Err(_) if first.is_none() => first = Some(ApplicationError::shutdown()),
                Err(_) => {}
            }
        }
        if self.controller.shutdown().is_err() && first.is_none() {
            first = Some(ApplicationError::shutdown());
        }
        if self.maintenance.shutdown().is_err() && first.is_none() {
            first = Some(ApplicationError::shutdown());
        }
        if let Some(reminder) = self.reminder.owner() {
            match reminder.lock() {
                Ok(mut reminder) => remember_failure(&mut first, reminder.shutdown().map(|_| ())),
                Err(_) if first.is_none() => first = Some(ApplicationError::shutdown()),
                Err(_) => {}
            }
        }
        if let Some(quota) = self.quota.owner_mut() {
            remember_failure(&mut first, quota.shutdown().map(|_| ()));
        }
        remember_failure(&mut first, self.live.shutdown().map(|_| ()));
        first.map_or(Ok(()), Err)
    }
}

fn remember_failure<T>(
    first: &mut Option<ApplicationError>,
    result: Result<T, tokenmaster_runtime::RuntimeError>,
) {
    if result.is_err() && first.is_none() {
        *first = Some(ApplicationError::shutdown());
    }
}

struct OptionalRuntime<T> {
    owner: Option<T>,
    failure: Option<RuntimeErrorCode>,
}

struct OptionalReminderRuntime {
    owner: Option<Arc<Mutex<BenefitReminderRuntime>>>,
    failure: Option<RuntimeErrorCode>,
}

impl OptionalReminderRuntime {
    fn start(result: Result<BenefitReminderRuntime, tokenmaster_runtime::RuntimeError>) -> Self {
        match result {
            Ok(owner) => Self {
                owner: Some(Arc::new(Mutex::new(owner))),
                failure: None,
            },
            Err(error) => Self {
                owner: None,
                failure: Some(error.code()),
            },
        }
    }

    const fn failed(error: RuntimeErrorCode) -> Self {
        Self {
            owner: None,
            failure: Some(error),
        }
    }

    fn snapshot<H>(
        &self,
        capture: impl FnOnce(&BenefitReminderRuntime) -> Result<H, RuntimeErrorCode>,
    ) -> Result<H, ProductRuntimeObservationError> {
        match (&self.owner, self.failure) {
            (Some(owner), _) => owner
                .lock()
                .map_err(|_| ProductRuntimeObservationError::Internal)
                .and_then(|owner| capture(&owner).map_err(ProductRuntimeObservationError::from)),
            (None, Some(error)) => Err(ProductRuntimeObservationError::from(error)),
            (None, None) => Err(ProductRuntimeObservationError::Internal),
        }
    }

    const fn owner(&self) -> Option<&Arc<Mutex<BenefitReminderRuntime>>> {
        self.owner.as_ref()
    }
}

impl<T> OptionalRuntime<T> {
    fn start(result: Result<T, tokenmaster_runtime::RuntimeError>) -> Self {
        match result {
            Ok(owner) => Self {
                owner: Some(owner),
                failure: None,
            },
            Err(error) => Self {
                owner: None,
                failure: Some(error.code()),
            },
        }
    }

    fn snapshot<H>(
        &self,
        capture: impl FnOnce(&T) -> Result<H, RuntimeErrorCode>,
    ) -> Result<H, ProductRuntimeObservationError> {
        match (&self.owner, self.failure) {
            (Some(owner), _) => capture(owner).map_err(ProductRuntimeObservationError::from),
            (None, Some(error)) => Err(ProductRuntimeObservationError::from(error)),
            (None, None) => Err(ProductRuntimeObservationError::Internal),
        }
    }

    fn owner_mut(&mut self) -> Option<&mut T> {
        self.owner.as_mut()
    }
}

struct ApplicationRuntimeNotifier {
    bundle: Weak<Mutex<ApplicationBundleSlot>>,
    bundle_generation: u64,
    pending: AtomicBool,
    next_generation: AtomicU64,
}

impl ApplicationRuntimeNotifier {
    fn new(bundle: Weak<Mutex<ApplicationBundleSlot>>, bundle_generation: u64) -> Self {
        Self {
            bundle,
            bundle_generation,
            pending: AtomicBool::new(false),
            next_generation: AtomicU64::new(1),
        }
    }

    fn publish(&self) -> Result<(), ApplicationError> {
        let Some(bundle) = self.bundle.upgrade() else {
            self.pending.store(false, Ordering::Release);
            return Ok(());
        };
        let mut slot = match bundle.lock() {
            Ok(slot) => slot,
            Err(_) => {
                self.pending.store(true, Ordering::Release);
                return Err(ApplicationError::internal());
            }
        };
        if slot.generation != self.bundle_generation {
            self.pending.store(false, Ordering::Release);
            return Ok(());
        }
        let Some(bundle) = slot.as_mut() else {
            self.pending.store(true, Ordering::Release);
            return Ok(());
        };
        let generation = self.next_generation()?;
        let result = bundle.publish_runtime(generation);
        self.pending.store(result.is_err(), Ordering::Release);
        result
    }

    fn next_generation(&self) -> Result<ProductRuntimeGeneration, ApplicationError> {
        let value = self
            .next_generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ApplicationError::generation_overflow())?;
        ProductRuntimeGeneration::new(value).ok_or_else(ApplicationError::generation_overflow)
    }
}

impl WorkerCompletionNotifier for ApplicationRuntimeNotifier {
    fn completion_ready(&self, _completion: WorkerCompletion) {
        let _ = self.publish();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationErrorCode {
    CurrentSessionUnavailable,
    DataUnavailable,
    DiscoveryUnavailable,
    LiveRuntimeUnavailable,
    StateUnavailable,
    ControllerUnavailable,
    UiUnavailable,
    EventLoopUnavailable,
    InvalidLifecycle,
    GenerationOverflow,
    ShutdownFailed,
    Internal,
}

impl ApplicationErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::CurrentSessionUnavailable => "current_session_unavailable",
            Self::DataUnavailable => "data_unavailable",
            Self::DiscoveryUnavailable => "discovery_unavailable",
            Self::LiveRuntimeUnavailable => "live_runtime_unavailable",
            Self::StateUnavailable => "state_unavailable",
            Self::ControllerUnavailable => "controller_unavailable",
            Self::UiUnavailable => "ui_unavailable",
            Self::EventLoopUnavailable => "event_loop_unavailable",
            Self::InvalidLifecycle => "invalid_lifecycle",
            Self::GenerationOverflow => "generation_overflow",
            Self::ShutdownFailed => "shutdown_failed",
            Self::Internal => "internal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationError {
    code: ApplicationErrorCode,
}

impl ApplicationError {
    const fn current_session_unavailable() -> Self {
        Self {
            code: ApplicationErrorCode::CurrentSessionUnavailable,
        }
    }

    const fn data() -> Self {
        Self {
            code: ApplicationErrorCode::DataUnavailable,
        }
    }

    const fn discovery() -> Self {
        Self {
            code: ApplicationErrorCode::DiscoveryUnavailable,
        }
    }

    const fn live_runtime() -> Self {
        Self {
            code: ApplicationErrorCode::LiveRuntimeUnavailable,
        }
    }

    pub(crate) const fn state() -> Self {
        Self {
            code: ApplicationErrorCode::StateUnavailable,
        }
    }

    const fn controller() -> Self {
        Self {
            code: ApplicationErrorCode::ControllerUnavailable,
        }
    }

    const fn ui_unavailable() -> Self {
        Self {
            code: ApplicationErrorCode::UiUnavailable,
        }
    }

    const fn event_loop() -> Self {
        Self {
            code: ApplicationErrorCode::EventLoopUnavailable,
        }
    }

    pub(crate) const fn invalid_lifecycle() -> Self {
        Self {
            code: ApplicationErrorCode::InvalidLifecycle,
        }
    }

    const fn generation_overflow() -> Self {
        Self {
            code: ApplicationErrorCode::GenerationOverflow,
        }
    }

    const fn shutdown() -> Self {
        Self {
            code: ApplicationErrorCode::ShutdownFailed,
        }
    }

    const fn internal() -> Self {
        Self {
            code: ApplicationErrorCode::Internal,
        }
    }

    #[must_use]
    pub const fn code(self) -> ApplicationErrorCode {
        self.code
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.stable_code())
    }
}

impl std::error::Error for ApplicationError {}

#[cfg(test)]
#[path = "application_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
