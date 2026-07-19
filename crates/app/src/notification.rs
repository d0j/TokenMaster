use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokenmaster_desktop::{
    DesktopInAppNotification, DesktopInAppNotificationBatch, DesktopInAppNotificationBridge,
    DesktopNotificationErrorCode, DesktopNotificationKind, DesktopNotificationPresentationReceipt,
};
use tokenmaster_domain::{BenefitKind, NotificationChannel};
use tokenmaster_runtime::{BenefitReminderDelivery, BenefitReminderRuntime, RuntimeErrorCode};

const NOTIFICATION_ACK_RETRY: Duration = Duration::from_secs(60);
const NOTIFICATION_RECEIPT_WORKER_NAME: &str = "tokenmaster-notification-receipt";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PresentationFailure {
    InvalidData,
    Busy,
    StoreUnavailable,
    Closed,
    Internal,
}

impl PresentationFailure {
    const fn retryable(self) -> bool {
        matches!(self, Self::Busy | Self::StoreUnavailable)
    }
}

pub(crate) trait ReminderPresentationPort: Send + Sync + 'static {
    fn take(&self) -> Result<Option<DesktopInAppNotificationBatch>, PresentationFailure>;
    fn acknowledge(&self) -> Result<bool, PresentationFailure>;
    fn release(&self) -> Result<bool, PresentationFailure>;
}

pub(crate) trait NotificationPresenter: Send + Sync + 'static {
    fn present(
        &self,
        batch: DesktopInAppNotificationBatch,
        receipt: Arc<dyn DesktopNotificationPresentationReceipt>,
    ) -> Result<(), PresentationFailure>;
}

impl NotificationPresenter for DesktopInAppNotificationBridge {
    fn present(
        &self,
        batch: DesktopInAppNotificationBatch,
        receipt: Arc<dyn DesktopNotificationPresentationReceipt>,
    ) -> Result<(), PresentationFailure> {
        DesktopInAppNotificationBridge::present(self, batch, receipt)
            .map_err(|error| map_desktop_failure(error.code()))
    }
}

const fn map_desktop_failure(code: DesktopNotificationErrorCode) -> PresentationFailure {
    match code {
        DesktopNotificationErrorCode::InvalidValue
        | DesktopNotificationErrorCode::InvalidBatch
        | DesktopNotificationErrorCode::CapacityExceeded => PresentationFailure::InvalidData,
        DesktopNotificationErrorCode::Busy => PresentationFailure::Busy,
        DesktopNotificationErrorCode::Closed => PresentationFailure::Closed,
        DesktopNotificationErrorCode::StateUnavailable => PresentationFailure::Internal,
    }
}

pub(crate) struct RuntimeReminderPresentationPort {
    runtime: Arc<Mutex<BenefitReminderRuntime>>,
}

impl RuntimeReminderPresentationPort {
    pub(crate) fn new(runtime: Arc<Mutex<BenefitReminderRuntime>>) -> Self {
        Self { runtime }
    }
}

impl ReminderPresentationPort for RuntimeReminderPresentationPort {
    fn take(&self) -> Result<Option<DesktopInAppNotificationBatch>, PresentationFailure> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PresentationFailure::Internal)?;
        let Some(deliveries) = runtime
            .take_notifications()
            .map_err(|error| map_runtime_failure(error.code()))?
        else {
            return Ok(None);
        };
        let mapped = deliveries
            .iter()
            .map(map_delivery)
            .collect::<Result<Vec<_>, _>>()
            .and_then(|rows| {
                DesktopInAppNotificationBatch::new(rows)
                    .map_err(|error| map_desktop_failure(error.code()))
            });
        match mapped {
            Ok(batch) => Ok(Some(batch)),
            Err(error) => {
                let _ = runtime.release_notifications();
                Err(error)
            }
        }
    }

    fn acknowledge(&self) -> Result<bool, PresentationFailure> {
        self.runtime
            .lock()
            .map_err(|_| PresentationFailure::Internal)?
            .acknowledge_notifications()
            .map_err(|error| map_runtime_failure(error.code()))
    }

    fn release(&self) -> Result<bool, PresentationFailure> {
        self.runtime
            .lock()
            .map_err(|_| PresentationFailure::Internal)?
            .release_notifications()
            .map_err(|error| map_runtime_failure(error.code()))
    }
}

fn map_delivery(
    delivery: &BenefitReminderDelivery,
) -> Result<DesktopInAppNotification, PresentationFailure> {
    match delivery.channel() {
        NotificationChannel::InApp => {}
        NotificationChannel::OsScheduled => return Err(PresentationFailure::InvalidData),
    }
    let kind = match delivery.kind() {
        BenefitKind::BankedRateLimitReset => DesktopNotificationKind::BankedRateLimitReset,
        BenefitKind::UsageCredit => DesktopNotificationKind::UsageCredit,
        BenefitKind::TemporaryUsage => DesktopNotificationKind::TemporaryUsage,
        BenefitKind::Unknown => DesktopNotificationKind::Unknown,
    };
    DesktopInAppNotification::new(
        kind,
        delivery.quantity(),
        delivery.label_key(),
        delivery.lead_time().seconds(),
        delivery.due_at_ms(),
        delivery.expiry_at_ms(),
        delivery.delivered_at_ms(),
    )
    .map_err(|error| map_desktop_failure(error.code()))
}

const fn map_runtime_failure(code: RuntimeErrorCode) -> PresentationFailure {
    match code {
        RuntimeErrorCode::StoreUnavailable => PresentationFailure::StoreUnavailable,
        RuntimeErrorCode::Busy => PresentationFailure::Busy,
        RuntimeErrorCode::Closed => PresentationFailure::Closed,
        RuntimeErrorCode::InvalidConfiguration
        | RuntimeErrorCode::ProviderUnavailable
        | RuntimeErrorCode::Faulted
        | RuntimeErrorCode::Internal => PresentationFailure::Internal,
    }
}

#[derive(Clone, Copy)]
enum ReceiptAction {
    Presented,
    Failed,
}

struct ReceiptWorkerState {
    action: Option<ReceiptAction>,
    stopping: bool,
}

struct ReceiptWorkerSignal {
    state: Mutex<ReceiptWorkerState>,
    wake: Condvar,
    in_flight: AtomicBool,
}

impl ReceiptWorkerSignal {
    const fn new() -> Self {
        Self {
            state: Mutex::new(ReceiptWorkerState {
                action: None,
                stopping: false,
            }),
            wake: Condvar::new(),
            in_flight: AtomicBool::new(false),
        }
    }

    fn submit(&self, action: ReceiptAction) {
        if let Ok(mut state) = self.state.lock()
            && !state.stopping
            && state.action.is_none()
        {
            state.action = Some(action);
            self.wake.notify_one();
        }
    }

    fn stop(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.stopping = true;
            self.wake.notify_one();
        }
    }

    fn clear_in_flight(&self) {
        self.in_flight.store(false, Ordering::Release);
    }
}

struct CoordinatorReceipt {
    signal: Arc<ReceiptWorkerSignal>,
    completed: AtomicBool,
}

impl CoordinatorReceipt {
    fn new(signal: Arc<ReceiptWorkerSignal>) -> Self {
        Self {
            signal,
            completed: AtomicBool::new(false),
        }
    }

    fn submit_once(&self, action: ReceiptAction) {
        if self
            .completed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.signal.submit(action);
        }
    }
}

impl DesktopNotificationPresentationReceipt for CoordinatorReceipt {
    fn presented(&self) {
        self.submit_once(ReceiptAction::Presented);
    }

    fn failed(&self) {
        self.submit_once(ReceiptAction::Failed);
    }
}

pub(crate) struct ReminderPresentationCoordinator {
    port: Arc<dyn ReminderPresentationPort>,
    presenter: Option<Arc<dyn NotificationPresenter>>,
    signal: Arc<ReceiptWorkerSignal>,
    worker: Option<JoinHandle<()>>,
}

impl ReminderPresentationCoordinator {
    pub(crate) fn start(
        port: Arc<dyn ReminderPresentationPort>,
        presenter: Arc<dyn NotificationPresenter>,
    ) -> Result<Self, PresentationFailure> {
        Self::start_with_retry(port, presenter, NOTIFICATION_ACK_RETRY)
    }

    #[cfg(test)]
    pub(crate) fn start_for_test(
        port: Arc<dyn ReminderPresentationPort>,
        presenter: Arc<dyn NotificationPresenter>,
        retry: Duration,
    ) -> Result<Self, PresentationFailure> {
        Self::start_with_retry(port, presenter, retry)
    }

    fn start_with_retry(
        port: Arc<dyn ReminderPresentationPort>,
        presenter: Arc<dyn NotificationPresenter>,
        retry: Duration,
    ) -> Result<Self, PresentationFailure> {
        let signal = Arc::new(ReceiptWorkerSignal::new());
        let worker_signal = Arc::clone(&signal);
        let worker_port = Arc::clone(&port);
        let worker = thread::Builder::new()
            .name(String::from(NOTIFICATION_RECEIPT_WORKER_NAME))
            .spawn(move || run_receipt_worker(worker_signal, worker_port, retry))
            .map_err(|_| PresentationFailure::Internal)?;
        Ok(Self {
            port,
            presenter: Some(presenter),
            signal,
            worker: Some(worker),
        })
    }

    pub(crate) fn pump(&self) -> Result<bool, PresentationFailure> {
        let Some(presenter) = self.presenter.as_ref() else {
            return Err(PresentationFailure::Closed);
        };
        if self
            .signal
            .in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(false);
        }
        let batch = match self.port.take() {
            Ok(Some(batch)) => batch,
            Ok(None) => {
                self.signal.clear_in_flight();
                return Ok(false);
            }
            Err(error) => {
                self.signal.clear_in_flight();
                return Err(error);
            }
        };
        let receipt = Arc::new(CoordinatorReceipt::new(Arc::clone(&self.signal)));
        match presenter.present(batch, receipt.clone()) {
            Ok(()) => Ok(true),
            Err(error) => {
                receipt.failed();
                Err(error)
            }
        }
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), PresentationFailure> {
        self.presenter.take();
        self.signal.stop();
        let joined = self
            .worker
            .take()
            .map(|worker| worker.join().map_err(|_| PresentationFailure::Internal))
            .transpose()?;
        if joined.is_none() {
            return Ok(());
        }
        if self.signal.in_flight.swap(false, Ordering::AcqRel) {
            self.port.release().map(|_| ())?;
        }
        Ok(())
    }
}

impl Drop for ReminderPresentationCoordinator {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_receipt_worker(
    signal: Arc<ReceiptWorkerSignal>,
    port: Arc<dyn ReminderPresentationPort>,
    retry: Duration,
) {
    loop {
        let Some(action) = wait_for_action(&signal) else {
            release_in_flight(&signal, port.as_ref());
            return;
        };
        match action {
            ReceiptAction::Failed => release_in_flight(&signal, port.as_ref()),
            ReceiptAction::Presented => acknowledge_presented(&signal, port.as_ref(), retry),
        }
    }
}

fn wait_for_action(signal: &ReceiptWorkerSignal) -> Option<ReceiptAction> {
    let mut state = signal.state.lock().ok()?;
    while state.action.is_none() && !state.stopping {
        state = signal.wake.wait(state).ok()?;
    }
    if state.stopping {
        None
    } else {
        state.action.take()
    }
}

fn acknowledge_presented(
    signal: &ReceiptWorkerSignal,
    port: &dyn ReminderPresentationPort,
    retry: Duration,
) {
    loop {
        if is_stopping(signal) {
            release_in_flight(signal, port);
            return;
        }
        match port.acknowledge() {
            Ok(_) => {
                signal.clear_in_flight();
                return;
            }
            Err(error) if error.retryable() => {
                if wait_for_retry_or_stop(signal, retry) {
                    release_in_flight(signal, port);
                    return;
                }
            }
            Err(_) => {
                release_in_flight(signal, port);
                return;
            }
        }
    }
}

fn is_stopping(signal: &ReceiptWorkerSignal) -> bool {
    signal.state.lock().map_or(true, |state| state.stopping)
}

fn wait_for_retry_or_stop(signal: &ReceiptWorkerSignal, retry: Duration) -> bool {
    let Ok(state) = signal.state.lock() else {
        return true;
    };
    if state.stopping {
        return true;
    }
    signal
        .wake
        .wait_timeout_while(state, retry, |state| !state.stopping)
        .map_or(true, |(state, _timeout)| state.stopping)
}

fn release_in_flight(signal: &ReceiptWorkerSignal, port: &dyn ReminderPresentationPort) {
    if signal.in_flight.load(Ordering::Acquire) {
        let _ = port.release();
        signal.clear_in_flight();
    }
}
