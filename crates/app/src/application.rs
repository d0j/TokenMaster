use std::fmt;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use slint::ComponentHandle;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_desktop::{
    DesktopController, DesktopQueryPlan, DesktopRefreshUrgency, DesktopRuntimeObservation,
    DesktopShell, DesktopSnapshotBridge, select_production_renderer,
};
use tokenmaster_engine::{WorkerCompletion, WorkerCompletionNotifier};
use tokenmaster_platform::ExclusiveFileLeaseGuard;
use tokenmaster_product::{
    ProductGitRuntimeHealth, ProductQuotaRuntimeHealth, ProductReducer,
    ProductReminderRuntimeHealth, ProductRuntimeGeneration, ProductRuntimeObservationError,
    ProductUsageRuntimeHealth,
};
use tokenmaster_runtime::{
    BenefitReminderRuntime, BenefitReminderRuntimeConfig, CodexQuotaRuntime,
    CodexQuotaRuntimeConfig, LiveRuntime, RuntimeErrorCode,
};
use tokenmaster_state::{
    BackupMaintenanceRuntime, BootstrapOutcome, MaintenanceCompletion, MaintenanceOutcome,
    MaintenancePurpose, MaintenanceSourceState, RestoreMode, RestoreSafety, StateErrorCode,
};
use tokenmaster_store::BackupControl;

use crate::command::{
    ApplicationBackupSelection, ApplicationCommand, ApplicationCommandExecution,
    ApplicationCommandFailure, ApplicationCommandPermit,
};
use crate::operation::{ApplicationOperationWorker, ApplicationOperationWorkerPhase};
use crate::state::{ApplicationPreflight, ApplicationStateOwner};
use crate::{ApplicationEnvironment, DataRoot};

type SharedBundle = Arc<Mutex<ApplicationBundleSlot>>;
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

    #[cfg(test)]
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
    select_production_renderer().map_err(|_| ApplicationError::ui_unavailable())?;
    let environment = ApplicationEnvironment::capture().map_err(|_| ApplicationError::data())?;
    let mut application = Application::start(&environment)?;
    let event_result = application.run_event_loop();
    let shutdown_result = application.shutdown();
    event_result.and(shutdown_result)
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
    _bridge: Option<DesktopSnapshotBridge>,
    bundle: SharedBundle,
    commands: ApplicationOperationWorker,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 12B command worker binds controlled restart")
    )]
    state: ApplicationStateOwner,
    preflight: ApplicationPreflight,
    live_started: bool,
    shutdown: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplicationStartBoundary {
    PreMigrationBackupPublished,
    BeforePostMigrationBackup,
}

impl Application {
    fn start(environment: &ApplicationEnvironment) -> Result<Self, ApplicationError> {
        Self::start_with_observer(environment, |_| Ok(()))
    }

    fn start_with_observer<F>(
        environment: &ApplicationEnvironment,
        mut observer: F,
    ) -> Result<Self, ApplicationError>
    where
        F: FnMut(ApplicationStartBoundary) -> Result<(), ApplicationError>,
    {
        let data_root = DataRoot::resolve(environment).map_err(|_| ApplicationError::data())?;
        let state = ApplicationStateOwner::open(&data_root)?;
        let mut preflight = state.prepare(&data_root)?;
        let initial = ProductReducer::new().snapshot();
        let shell = DesktopShell::new(&initial).map_err(|_| ApplicationError::ui_unavailable())?;
        let bundle = Arc::new(Mutex::new(ApplicationBundleSlot::new()));
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
                &shell,
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

        let command_bundle = Arc::clone(&bundle);
        let commands = ApplicationOperationWorker::spawn(move |permit| {
            execute_application_operation(&command_bundle, permit)
        })
        .map_err(|_| ApplicationError::internal())?;
        Ok(Self {
            environment: environment.clone(),
            data_root,
            shell,
            _bridge: bridge,
            bundle,
            commands,
            state,
            preflight,
            live_started,
            shutdown: false,
        })
    }

    fn run_event_loop(&self) -> Result<(), ApplicationError> {
        self.shell
            .window()
            .show()
            .map_err(|_| ApplicationError::ui_unavailable())?;
        slint::run_event_loop().map_err(|_| ApplicationError::event_loop())
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 12B command worker binds controlled restart")
    )]
    fn restart_services(&mut self) -> Result<(), ApplicationError> {
        if self.shutdown || !self.live_started {
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
        self.live_started = false;
        drop(self._bridge.take());
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
                &self.shell,
                &self.bundle,
                guard,
            )
        })();
        self.commands
            .resume_admission()
            .map_err(|_| ApplicationError::internal())?;
        match result {
            Ok(bridge) => {
                self._bridge = Some(bridge);
                self.live_started = true;
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
            || !self.live_started
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
        self.live_started = false;
        drop(self._bridge.take());
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
            self.preflight.bind_recovery_launch(receipt)?;
            start_restored_bundle(
                &self.environment,
                &self.data_root,
                &self.state,
                &mut self.preflight,
                &self.shell,
                &self.bundle,
                guard,
            )
        })();
        self.commands
            .resume_admission()
            .map_err(|_| ApplicationError::internal())?;
        match result {
            Ok(bridge) => {
                self._bridge = Some(bridge);
                self.live_started = true;
                Ok(())
            }
            Err(error) => {
                discard_bundle(&self.bundle)?;
                Err(error)
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), ApplicationError> {
        if self.shutdown {
            return Ok(());
        }
        self.shutdown = true;
        let command_result = match self.commands.shutdown() {
            Ok(ApplicationOperationWorkerPhase::Stopped) => Ok(()),
            Ok(
                ApplicationOperationWorkerPhase::Running
                | ApplicationOperationWorkerPhase::Stopping
                | ApplicationOperationWorkerPhase::Faulted,
            )
            | Err(_) => Err(ApplicationError::shutdown()),
        };
        let bundle = self
            .bundle
            .lock()
            .map_err(|_| ApplicationError::internal())?
            .take();
        let bundle_result = match bundle {
            Some(mut bundle) => bundle.shutdown(),
            None => Ok(()),
        };
        let result = command_result.and(bundle_result);
        if result.is_ok() && self.live_started {
            self.preflight
                .session_mut()
                .mark_clean()
                .map_err(|_| ApplicationError::state())?;
        }
        result
    }
}

#[allow(clippy::too_many_arguments)]
fn start_live_bundle(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    preflight: &mut ApplicationPreflight,
    shell: &DesktopShell,
    bundle: &SharedBundle,
    outcome: BootstrapOutcome,
    observer: &mut dyn FnMut(ApplicationStartBoundary) -> Result<(), ApplicationError>,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
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
        shell,
        bundle,
        started,
        maintenance,
        maintenance_source,
    )
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Task 12B command worker binds controlled restart")
)]
fn start_current_bundle(
    environment: &ApplicationEnvironment,
    data_root: &DataRoot,
    state: &ApplicationStateOwner,
    shell: &DesktopShell,
    bundle: &SharedBundle,
    guard: ExclusiveFileLeaseGuard,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
    let started = start_guarded_live(environment, data_root, bundle, guard)?;
    finish_live_bundle(
        data_root,
        state,
        shell,
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
    shell: &DesktopShell,
    bundle: &SharedBundle,
    guard: ExclusiveFileLeaseGuard,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
    if !state.restored_archive_requires_migration(data_root)? {
        return start_current_bundle(environment, data_root, state, shell, bundle, guard);
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
        shell,
        bundle,
        started,
        Some(maintenance),
        MaintenanceSourceState::Healthy,
    )
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
    let live = LiveRuntime::start_notified_guarded(
        data_root.archive_path(),
        discovery,
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
    shell: &DesktopShell,
    bundle: &SharedBundle,
    started: GuardedLiveStart,
    maintenance: Option<BackupMaintenanceRuntime>,
    maintenance_source: MaintenanceSourceState,
) -> Result<DesktopSnapshotBridge, ApplicationError> {
    let archive_path = data_root.archive_path().to_path_buf();
    let quota =
        OptionalRuntime::start(CodexQuotaRuntimeConfig::new(archive_path.clone()).and_then(
            |config| CodexQuotaRuntime::start_notified(config, started.notifier_port.clone()),
        ));
    let reminder = OptionalRuntime::start(
        BenefitReminderRuntimeConfig::new(archive_path.clone()).and_then(|config| {
            BenefitReminderRuntime::start_notified(config, started.notifier_port.clone())
        }),
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
    let live_bridge = shell.snapshot_bridge(controller.snapshot_receiver());
    controller
        .attach_snapshot_notifier(live_bridge.notifier())
        .map_err(|_| ApplicationError::controller())?;

    {
        let mut slot = bundle.lock().map_err(|_| ApplicationError::internal())?;
        if slot.generation != started.bundle_generation || slot.bundle.is_some() {
            return Err(ApplicationError::internal());
        }
        slot.bundle = Some(ApplicationBundle {
            live: started.live,
            quota,
            reminder,
            controller,
            maintenance,
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

fn execute_application_operation(
    bundle: &SharedBundle,
    permit: &ApplicationCommandPermit,
) -> ApplicationCommandExecution {
    if permit.is_cancelled() {
        return ApplicationCommandExecution::Cancelled;
    }
    match permit.command() {
        ApplicationCommand::Backup => execute_manual_backup_command(bundle, permit),
        ApplicationCommand::ExportConfig
        | ApplicationCommand::ImportConfig
        | ApplicationCommand::Verify
        | ApplicationCommand::RestoreData(_)
        | ApplicationCommand::RestoreDataAndPortableSettings(_)
        | ApplicationCommand::Rebuild => {
            ApplicationCommandExecution::Failed(ApplicationCommandFailure::Unavailable)
        }
    }
}

fn execute_manual_backup_command(
    bundle: &SharedBundle,
    permit: &ApplicationCommandPermit,
) -> ApplicationCommandExecution {
    if permit.begin_irreversible().is_err() {
        return if permit.is_cancelled() {
            ApplicationCommandExecution::Cancelled
        } else {
            ApplicationCommandExecution::Failed(ApplicationCommandFailure::Internal)
        };
    }
    let slot = match bundle.lock() {
        Ok(slot) => slot,
        Err(_) => {
            return ApplicationCommandExecution::Failed(ApplicationCommandFailure::Internal);
        }
    };
    let Some(bundle) = slot.bundle.as_ref() else {
        return ApplicationCommandExecution::Failed(ApplicationCommandFailure::Unavailable);
    };
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
    quota: OptionalRuntime<CodexQuotaRuntime>,
    reminder: OptionalRuntime<BenefitReminderRuntime>,
    controller: DesktopController,
    maintenance: BackupMaintenanceRuntime,
    #[cfg(test)]
    notifier: Arc<ApplicationRuntimeNotifier>,
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
        if self.maintenance.pause().is_err() {
            first = Some(ApplicationError::shutdown());
        }
        remember_failure(&mut first, self.live.pause().map(|_| ()));
        if let Some(quota) = self.quota.owner_mut() {
            remember_failure(&mut first, quota.pause().map(|_| ()));
        }
        if let Some(reminder) = self.reminder.owner_mut() {
            remember_failure(&mut first, reminder.pause().map(|_| ()));
        }
        if self.controller.shutdown().is_err() && first.is_none() {
            first = Some(ApplicationError::shutdown());
        }
        if self.maintenance.shutdown().is_err() && first.is_none() {
            first = Some(ApplicationError::shutdown());
        }
        if let Some(reminder) = self.reminder.owner_mut() {
            remember_failure(&mut first, reminder.shutdown().map(|_| ()));
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
