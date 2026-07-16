mod execution;
mod health;
mod runtime;

use std::fmt;
use std::path::{Path, PathBuf};

use crate::{RuntimeError, RuntimeWriterLease};

pub use health::{
    BenefitReminderFailure, BenefitReminderRefreshSnapshot, BenefitReminderRetryMode,
    BenefitReminderRuntimePhase, BenefitReminderRuntimeSnapshot, BenefitReminderSchedulePhase,
    BenefitReminderScheduleSnapshot,
};
pub use runtime::BenefitReminderRuntime;
pub use tokenmaster_store::BenefitReminderDelivery;

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitReminderRuntimeConfig {
    archive_path: PathBuf,
}

impl BenefitReminderRuntimeConfig {
    pub fn new(archive_path: PathBuf) -> Result<Self, RuntimeError> {
        let _ = RuntimeWriterLease::new(&archive_path)?;
        Ok(Self { archive_path })
    }

    pub(super) fn archive_path(&self) -> &Path {
        &self.archive_path
    }
}

impl fmt::Debug for BenefitReminderRuntimeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitReminderRuntimeConfig")
            .field("archive_path", &"[redacted]")
            .finish()
    }
}
