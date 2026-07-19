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
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
    presenter: Arc<Mutex<Option<Arc<dyn NotificationPresenter>>>>,
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
        let presenter = Arc::new(Mutex::new(Some(presenter)));
        let worker_signal = Arc::clone(&signal);
        let worker_port = Arc::clone(&port);
        let worker_presenter = Arc::clone(&presenter);
        let worker = thread::Builder::new()
            .name(String::from(NOTIFICATION_RECEIPT_WORKER_NAME))
            .spawn(move || {
                run_receipt_worker(worker_signal, worker_port, worker_presenter, retry);
            })
            .map_err(|_| PresentationFailure::Internal)?;
        Ok(Self {
            port,
            presenter,
            signal,
            worker: Some(worker),
        })
    }

    pub(crate) fn pump(&self) -> Result<bool, PresentationFailure> {
        let presenter = current_presenter(&self.presenter)?;
        pump_presentation(&self.signal, self.port.as_ref(), presenter.as_ref())
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), PresentationFailure> {
        let presenter_closed = match self.presenter.lock() {
            Ok(mut presenter) => {
                presenter.take();
                Ok(())
            }
            Err(poisoned) => {
                poisoned.into_inner().take();
                Err(PresentationFailure::Internal)
            }
        };
        self.signal.stop();
        let joined = self
            .worker
            .take()
            .map(thread::JoinHandle::join)
            .transpose()
            .map_err(|_| PresentationFailure::Internal);
        let released = release_in_flight(&self.signal, self.port.as_ref()).map(|_| ());
        presenter_closed.and(joined).and(released)
    }
}

impl Drop for ReminderPresentationCoordinator {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn current_presenter(
    presenter: &Mutex<Option<Arc<dyn NotificationPresenter>>>,
) -> Result<Arc<dyn NotificationPresenter>, PresentationFailure> {
    presenter
        .lock()
        .map_err(|_| PresentationFailure::Internal)?
        .as_ref()
        .cloned()
        .ok_or(PresentationFailure::Closed)
}

fn pump_presentation(
    signal: &Arc<ReceiptWorkerSignal>,
    port: &dyn ReminderPresentationPort,
    presenter: &dyn NotificationPresenter,
) -> Result<bool, PresentationFailure> {
    if is_stopping(signal) {
        return Err(PresentationFailure::Closed);
    }
    if signal
        .in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(false);
    }
    let batch = match port.take() {
        Ok(Some(batch)) => batch,
        Ok(None) => {
            signal.clear_in_flight();
            return Ok(false);
        }
        Err(error) => {
            signal.clear_in_flight();
            return Err(error);
        }
    };
    if is_stopping(signal) {
        release_in_flight(signal, port)?;
        return Err(PresentationFailure::Closed);
    }
    let receipt = Arc::new(CoordinatorReceipt::new(Arc::clone(signal)));
    match presenter.present(batch, receipt.clone()) {
        Ok(()) => Ok(true),
        Err(error) => {
            receipt.failed();
            Err(error)
        }
    }
}

fn run_receipt_worker(
    signal: Arc<ReceiptWorkerSignal>,
    port: Arc<dyn ReminderPresentationPort>,
    presenter: Arc<Mutex<Option<Arc<dyn NotificationPresenter>>>>,
    retry: Duration,
) {
    loop {
        let Some(action) = wait_for_action(&signal) else {
            let _ = release_in_flight(&signal, port.as_ref());
            return;
        };
        match action {
            ReceiptAction::Failed => {
                release_then_retry_presentation(&signal, port.as_ref(), &presenter, retry)
            }
            ReceiptAction::Presented => {
                acknowledge_presented(&signal, port.as_ref(), retry);
            }
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
    signal: &Arc<ReceiptWorkerSignal>,
    port: &dyn ReminderPresentationPort,
    retry: Duration,
) {
    loop {
        if is_stopping(signal) {
            let _ = release_in_flight(signal, port);
            return;
        }
        match port.acknowledge() {
            Ok(_) => {
                signal.clear_in_flight();
                return;
            }
            Err(error) if error.retryable() => {
                if wait_for_retry_or_stop(signal, retry) {
                    let _ = release_in_flight(signal, port);
                    return;
                }
            }
            Err(_) => {
                let _ = release_with_retry(signal, port, retry);
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

fn wait_for_presentation_retry_or_action(signal: &ReceiptWorkerSignal, retry: Duration) -> bool {
    let Ok(state) = signal.state.lock() else {
        return true;
    };
    if state.stopping || state.action.is_some() {
        return true;
    }
    signal
        .wake
        .wait_timeout_while(state, retry, |state| {
            !state.stopping && state.action.is_none()
        })
        .map_or(true, |(state, timeout)| {
            state.stopping || state.action.is_some() || !timeout.timed_out()
        })
}

fn release_then_retry_presentation(
    signal: &Arc<ReceiptWorkerSignal>,
    port: &dyn ReminderPresentationPort,
    presenter: &Mutex<Option<Arc<dyn NotificationPresenter>>>,
    retry: Duration,
) {
    let released = release_with_retry(signal, port, retry);
    if !released || wait_for_presentation_retry_or_action(signal, retry) {
        return;
    }
    let Ok(presenter) = current_presenter(presenter) else {
        return;
    };
    let _ = pump_presentation(signal, port, presenter.as_ref());
}

fn release_with_retry(
    signal: &ReceiptWorkerSignal,
    port: &dyn ReminderPresentationPort,
    retry: Duration,
) -> bool {
    loop {
        match release_in_flight(signal, port) {
            Ok(released) => return released,
            Err(error) if error.retryable() => {
                if wait_for_retry_or_stop(signal, retry) {
                    return false;
                }
            }
            Err(_) => return false,
        }
    }
}

fn release_in_flight(
    signal: &ReceiptWorkerSignal,
    port: &dyn ReminderPresentationPort,
) -> Result<bool, PresentationFailure> {
    if !signal.in_flight.load(Ordering::Acquire) {
        return Ok(false);
    }
    if !port.release()? {
        return Err(PresentationFailure::Internal);
    }
    signal.clear_in_flight();
    Ok(true)
}
