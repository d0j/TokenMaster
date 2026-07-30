use crate::{
    AdapterBatch, AdapterCheckpoint, AdapterCounters, AdapterDiagnostics, CancellationToken,
    CompletionQuality, DiscoveredSource, EngineError, EngineErrorCode, MonotonicTime,
    RefreshDeadline, RefreshPermit, ScopeIdentity,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortErrorCode {
    Busy,
    Cancelled,
    DeadlineExceeded,
    InvalidData,
    CapacityExceeded,
    StaleState,
    RebuildRequired,
    Unavailable,
    Failed,
}

impl core::fmt::Display for PortErrorCode {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Busy => "busy",
            Self::Cancelled => "cancelled",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::InvalidData => "invalid_data",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::StaleState => "stale_state",
            Self::RebuildRequired => "rebuild_required",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{code}")]
pub struct PortError {
    code: PortErrorCode,
}

impl PortError {
    #[must_use]
    pub const fn new(code: PortErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> PortErrorCode {
        self.code
    }
}

impl From<EngineError> for PortError {
    fn from(error: EngineError) -> Self {
        let code = match error.code() {
            EngineErrorCode::InvalidValue => PortErrorCode::InvalidData,
            EngineErrorCode::CapacityExceeded => PortErrorCode::CapacityExceeded,
            EngineErrorCode::StaleRequest => PortErrorCode::StaleState,
        };
        Self::new(code)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterCompletion {
    quality: CompletionQuality,
    counters: AdapterCounters,
    diagnostics: AdapterDiagnostics,
}

impl AdapterCompletion {
    pub fn new(
        quality: CompletionQuality,
        counters: AdapterCounters,
        diagnostics: AdapterDiagnostics,
    ) -> Result<Self, EngineError> {
        if counters.diagnostics() != diagnostics.total()? {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        Ok(Self {
            quality,
            counters,
            diagnostics,
        })
    }

    #[must_use]
    pub const fn quality(self) -> CompletionQuality {
        self.quality
    }

    #[must_use]
    pub const fn counters(self) -> AdapterCounters {
        self.counters
    }

    #[must_use]
    pub const fn diagnostics(self) -> AdapterDiagnostics {
        self.diagnostics
    }
}

pub trait Clock: Send + Sync {
    fn now(&self) -> MonotonicTime;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationStop {
    Cancelled,
    DeadlineExceeded,
}

impl OperationStop {
    const fn error_code(self) -> PortErrorCode {
        match self {
            Self::Cancelled => PortErrorCode::Cancelled,
            Self::DeadlineExceeded => PortErrorCode::DeadlineExceeded,
        }
    }
}

pub struct OperationControl<'a> {
    cancellation: CancellationToken,
    deadline: Option<RefreshDeadline>,
    clock: &'a dyn Clock,
}

impl<'a> OperationControl<'a> {
    #[must_use]
    pub fn new(permit: &RefreshPermit, clock: &'a dyn Clock) -> Self {
        Self {
            cancellation: permit.cancellation_token(),
            deadline: permit.deadline(),
            clock,
        }
    }

    #[must_use]
    pub fn stop_reason(&self) -> Option<OperationStop> {
        if self.cancellation.is_cancelled() {
            return Some(OperationStop::Cancelled);
        }
        self.deadline
            .filter(|deadline| deadline.is_exceeded_at(self.clock.now()))
            .map(|_| OperationStop::DeadlineExceeded)
    }

    pub fn check(&self) -> Result<(), PortError> {
        self.stop_reason()
            .map_or(Ok(()), |stop| Err(PortError::new(stop.error_code())))
    }
}

impl core::fmt::Debug for OperationControl<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OperationControl")
            .field("cancelled", &self.cancellation.is_cancelled())
            .field("has_deadline", &self.deadline.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinkControl {
    Continue,
    Stop,
}

pub trait ScopeSink {
    fn on_scope(&mut self, scope: ScopeIdentity) -> Result<SinkControl, PortError>;
}

pub trait SourceSink {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_state: crate::AdapterSourceState,
    ) -> Result<SinkControl, PortError>;
}

pub trait SourceBatchReader {
    fn restore_checkpoint(
        &mut self,
        _progress: &crate::AdapterSourceProgress,
        control: &OperationControl<'_>,
    ) -> Result<AdapterCheckpoint, PortError> {
        control.check()?;
        Err(PortError::new(PortErrorCode::InvalidData))
    }

    fn validate_checkpoint(
        &mut self,
        _checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<(), PortError> {
        control.check()
    }

    fn read_batch(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError>;

    /// Takes the latest transient repository association produced by the preceding
    /// `read_batch` call. A later read may replace it; implementations must never
    /// encode the hint into an adapter batch or checkpoint.
    fn take_repository_activity_hint(
        &mut self,
    ) -> Option<tokenmaster_provider::RepositoryActivityHint> {
        None
    }
}

pub trait ReplaySourceSink {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_state: crate::AdapterSourceState,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError>;
}

pub trait Adapter: Send {
    fn visit_scopes(
        &mut self,
        control: &OperationControl<'_>,
        sink: &mut dyn ScopeSink,
    ) -> Result<AdapterCompletion, PortError>;

    fn visit_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn SourceSink,
    ) -> Result<AdapterCompletion, PortError>;

    fn visit_replay_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn ReplaySourceSink,
    ) -> Result<AdapterCompletion, PortError>;
}

pub trait WriterLeaseGuard: Send {}

pub trait WriterLease: Send {
    fn try_acquire(&mut self) -> Result<Box<dyn WriterLeaseGuard>, PortError>;
}
