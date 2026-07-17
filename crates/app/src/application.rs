use std::fmt;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use slint::ComponentHandle;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_desktop::{
    DesktopController, DesktopQueryPlan, DesktopRefreshUrgency, DesktopRuntimeObservation,
    DesktopShell, DesktopSnapshotBridge, select_production_renderer,
};
use tokenmaster_engine::{WorkerCompletion, WorkerCompletionNotifier};
use tokenmaster_product::{
    ProductGitRuntimeHealth, ProductQuotaRuntimeHealth, ProductReducer,
    ProductReminderRuntimeHealth, ProductRuntimeGeneration, ProductRuntimeObservationError,
    ProductUsageRuntimeHealth,
};
use tokenmaster_runtime::{
    BenefitReminderRuntime, BenefitReminderRuntimeConfig, CodexQuotaRuntime,
    CodexQuotaRuntimeConfig, LiveRuntime, RuntimeErrorCode,
};

use crate::{ApplicationEnvironment, DataRoot};

type SharedBundle = Arc<Mutex<Option<ApplicationBundle>>>;

pub fn run() -> Result<(), ApplicationError> {
    select_production_renderer().map_err(|_| ApplicationError::ui_unavailable())?;
    let environment = ApplicationEnvironment::capture().map_err(|_| ApplicationError::data())?;
    let mut application = Application::start(&environment)?;
    let event_result = application.run_event_loop();
    let shutdown_result = application.shutdown();
    event_result.and(shutdown_result)
}

struct Application {
    shell: DesktopShell,
    _bridge: DesktopSnapshotBridge,
    bundle: SharedBundle,
    shutdown: bool,
}

impl Application {
    fn start(environment: &ApplicationEnvironment) -> Result<Self, ApplicationError> {
        let data_root = DataRoot::resolve(environment).map_err(|_| ApplicationError::data())?;
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

        let bundle = Arc::new(Mutex::new(None));
        let notifier = Arc::new(ApplicationRuntimeNotifier::new(Arc::downgrade(&bundle)));
        let notifier_port: Arc<dyn WorkerCompletionNotifier> = notifier.clone();
        let live =
            LiveRuntime::start_notified(data_root.archive_path(), discovery, notifier_port.clone())
                .map_err(|_| ApplicationError::live_runtime())?;
        let archive_path = data_root.archive_path().to_path_buf();
        let quota =
            OptionalRuntime::start(CodexQuotaRuntimeConfig::new(archive_path.clone()).and_then(
                |config| CodexQuotaRuntime::start_notified(config, notifier_port.clone()),
            ));
        let reminder = OptionalRuntime::start(
            BenefitReminderRuntimeConfig::new(archive_path.clone()).and_then(|config| {
                BenefitReminderRuntime::start_notified(config, notifier_port.clone())
            }),
        );
        let mut controller = DesktopController::open(
            &archive_path,
            DesktopQueryPlan::overview().map_err(|_| ApplicationError::controller())?,
        )
        .map_err(|_| ApplicationError::controller())?;

        let initial = ProductReducer::new().snapshot();
        let shell = DesktopShell::new(&initial).map_err(|_| ApplicationError::ui_unavailable())?;
        let bridge = shell.snapshot_bridge(controller.snapshot_receiver());
        controller
            .attach_snapshot_notifier(bridge.notifier())
            .map_err(|_| ApplicationError::controller())?;

        {
            let mut slot = bundle.lock().map_err(|_| ApplicationError::internal())?;
            *slot = Some(ApplicationBundle {
                live,
                quota,
                reminder,
                controller,
            });
        }
        notifier
            .publish()
            .map_err(|_| ApplicationError::controller())?;

        Ok(Self {
            shell,
            _bridge: bridge,
            bundle,
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

    fn shutdown(&mut self) -> Result<(), ApplicationError> {
        if self.shutdown {
            return Ok(());
        }
        self.shutdown = true;
        let bundle = self
            .bundle
            .lock()
            .map_err(|_| ApplicationError::internal())?
            .take();
        match bundle {
            Some(mut bundle) => bundle.shutdown(),
            None => Ok(()),
        }
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
    bundle: Weak<Mutex<Option<ApplicationBundle>>>,
    pending: AtomicBool,
    next_generation: AtomicU64,
}

impl ApplicationRuntimeNotifier {
    fn new(bundle: Weak<Mutex<Option<ApplicationBundle>>>) -> Self {
        Self {
            bundle,
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
    ControllerUnavailable,
    UiUnavailable,
    EventLoopUnavailable,
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
            Self::ControllerUnavailable => "controller_unavailable",
            Self::UiUnavailable => "ui_unavailable",
            Self::EventLoopUnavailable => "event_loop_unavailable",
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::time::{Duration, Instant};

    use tempfile::TempDir;
    use tokenmaster_product::ProductSectionKind;

    use super::*;

    #[test]
    fn early_notification_sets_one_pending_bit_without_allocating_generation() {
        let bundle: SharedBundle = Arc::new(Mutex::new(None));
        let notifier = ApplicationRuntimeNotifier::new(Arc::downgrade(&bundle));

        notifier.publish().expect("lossy early notification");

        assert!(notifier.pending.load(Ordering::Acquire));
        assert_eq!(notifier.next_generation.load(Ordering::Acquire), 1);
    }

    #[test]
    fn runtime_generation_overflow_is_checked_and_path_free() {
        let bundle: SharedBundle = Arc::new(Mutex::new(None));
        let notifier = ApplicationRuntimeNotifier::new(Arc::downgrade(&bundle));
        notifier.next_generation.store(u64::MAX, Ordering::Release);

        let error = notifier
            .next_generation()
            .expect_err("generation must not wrap");
        assert_eq!(error.code(), ApplicationErrorCode::GenerationOverflow);
        assert_eq!(error.to_string(), "generation_overflow");
    }

    #[test]
    fn real_bundle_joins_live_health_and_independent_optional_failures_then_shuts_down() {
        let temporary = TempDir::new().expect("temporary directory");
        let codex_root = temporary.path().join("codex");
        std::fs::create_dir(&codex_root).expect("Codex root");
        let configured = [ConfiguredCodexRoot::new(&codex_root, None, true)];
        let discovery = build_discovery_request(CodexRootInput {
            user_profile: None,
            codex_home: None,
            configured: &configured,
        })
        .expect("discovery request");
        let archive = temporary.path().join("application.sqlite3");
        let live = LiveRuntime::start(&archive, discovery).expect("live runtime");
        let controller =
            DesktopController::open(&archive, DesktopQueryPlan::overview().expect("query plan"))
                .expect("desktop controller");
        let mut bundle = ApplicationBundle {
            live,
            quota: OptionalRuntime {
                owner: None,
                failure: Some(RuntimeErrorCode::ProviderUnavailable),
            },
            reminder: OptionalRuntime {
                owner: None,
                failure: Some(RuntimeErrorCode::StoreUnavailable),
            },
            controller,
        };

        bundle
            .publish_runtime(ProductRuntimeGeneration::new(1).expect("generation"))
            .expect("publish runtime health");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if bundle
                .controller
                .try_completion()
                .expect("controller completion")
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline, "controller completion timed out");
            std::thread::yield_now();
        }
        let snapshot = bundle
            .controller
            .take_snapshot()
            .expect("snapshot mailbox")
            .expect("product snapshot");
        assert_eq!(snapshot.runtime().usage().kind(), ProductSectionKind::Ready);
        assert_eq!(snapshot.runtime().git().kind(), ProductSectionKind::Ready);
        assert_eq!(
            snapshot.runtime().quota().observation_error(),
            Some(ProductRuntimeObservationError::ProviderUnavailable)
        );
        assert_eq!(
            snapshot.runtime().reminder().observation_error(),
            Some(ProductRuntimeObservationError::StoreUnavailable)
        );

        bundle.shutdown().expect("bundle shutdown");
    }
}
