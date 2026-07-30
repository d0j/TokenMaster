use core::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::migration::decode_portable_candidate;
use super::value::{PortableSettings, SETTINGS_SCHEMA_VERSION};
use crate::StateError;
use crate::record::MAX_RECORD_PAYLOAD_BYTES;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PortableSettingsDigest([u8; 32]);

impl PortableSettingsDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for PortableSettingsDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PortableSettingsDigest([redacted])")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PortableSettingsTarget {
    generation: u64,
    digest: PortableSettingsDigest,
}

impl PortableSettingsTarget {
    pub fn from_persisted(generation: u64, digest: [u8; 32]) -> Result<Self, StateError> {
        if generation == 0 {
            return Err(StateError::invalid_input());
        }
        Ok(Self {
            generation,
            digest: PortableSettingsDigest(digest),
        })
    }

    pub(crate) const fn new(generation: u64, digest: PortableSettingsDigest) -> Self {
        Self { generation, digest }
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn digest(self) -> PortableSettingsDigest {
        self.digest
    }
}

impl fmt::Debug for PortableSettingsTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PortableSettingsTarget([redacted])")
    }
}

#[derive(Serialize)]
struct PortableCandidateWire<'a> {
    schema_version: u16,
    portable: &'a PortableSettings,
}

#[derive(Clone, Eq, PartialEq)]
pub struct PortableSettingsCandidate {
    portable: PortableSettings,
    digest: PortableSettingsDigest,
    source_schema_version: u16,
}

impl PortableSettingsCandidate {
    pub fn new(portable: PortableSettings) -> Result<Self, StateError> {
        let encoded = encode_portable(&portable)?;
        Ok(Self {
            portable,
            digest: PortableSettingsDigest(Sha256::digest(encoded).into()),
            source_schema_version: SETTINGS_SCHEMA_VERSION,
        })
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, StateError> {
        let decoded = decode_portable_candidate(bytes)?;
        let mut candidate = Self::new(decoded.portable)?;
        candidate.source_schema_version = decoded.source_schema_version;
        Ok(candidate)
    }

    pub fn encode_json(&self) -> Result<Vec<u8>, StateError> {
        encode_portable(&self.portable)
    }

    #[must_use]
    pub const fn digest(&self) -> PortableSettingsDigest {
        self.digest
    }

    pub(crate) const fn portable(&self) -> &PortableSettings {
        &self.portable
    }

    pub(crate) const fn source_schema_version(&self) -> u16 {
        self.source_schema_version
    }
}

impl fmt::Debug for PortableSettingsCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PortableSettingsCandidate([redacted])")
    }
}

fn encode_portable(portable: &PortableSettings) -> Result<Vec<u8>, StateError> {
    let bytes = serde_json::to_vec(&PortableCandidateWire {
        schema_version: SETTINGS_SCHEMA_VERSION,
        portable,
    })
    .map_err(|_| StateError::invalid_input())?;
    let len = u64::try_from(bytes.len()).map_err(|_| StateError::capacity_exceeded())?;
    if len > MAX_RECORD_PAYLOAD_BYTES {
        return Err(StateError::capacity_exceeded());
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsChangeCategory {
    ReminderProfile,
    BackupSchedule,
    BackupRetention,
    Presentation,
}

pub struct SettingsImportPreview {
    pub(crate) base_generation: Option<u64>,
    pub(crate) base_record_digest: Option<[u8; 32]>,
    pub(crate) candidate: PortableSettingsCandidate,
    categories: Box<[SettingsChangeCategory]>,
    changed_field_count: usize,
}

impl SettingsImportPreview {
    pub(crate) fn new(
        base_generation: Option<u64>,
        base_record_digest: Option<[u8; 32]>,
        current: &PortableSettings,
        candidate: PortableSettingsCandidate,
    ) -> Self {
        let mut categories = Vec::with_capacity(4);
        let mut changed_field_count = 0_usize;
        if current.reminders() != candidate.portable().reminders() {
            categories.push(SettingsChangeCategory::ReminderProfile);
            changed_field_count += usize::from(
                current.reminders().enabled() != candidate.portable().reminders().enabled(),
            );
            changed_field_count += usize::from(
                current.reminders().lead_seconds()
                    != candidate.portable().reminders().lead_seconds(),
            );
        }
        if current.backup().periodic_enabled() != candidate.portable().backup().periodic_enabled()
            || current.backup().quiet_seconds() != candidate.portable().backup().quiet_seconds()
            || current.backup().interval_seconds()
                != candidate.portable().backup().interval_seconds()
        {
            categories.push(SettingsChangeCategory::BackupSchedule);
            changed_field_count += usize::from(
                current.backup().periodic_enabled()
                    != candidate.portable().backup().periodic_enabled(),
            );
            changed_field_count += usize::from(
                current.backup().quiet_seconds() != candidate.portable().backup().quiet_seconds(),
            );
            changed_field_count += usize::from(
                current.backup().interval_seconds()
                    != candidate.portable().backup().interval_seconds(),
            );
        }
        if current.backup().retention_budget_bytes()
            != candidate.portable().backup().retention_budget_bytes()
        {
            categories.push(SettingsChangeCategory::BackupRetention);
            changed_field_count += 1;
        }
        if current.presentation() != candidate.portable().presentation() {
            categories.push(SettingsChangeCategory::Presentation);
            changed_field_count += 1;
        }
        Self {
            base_generation,
            base_record_digest,
            candidate,
            categories: categories.into_boxed_slice(),
            changed_field_count,
        }
    }

    #[must_use]
    pub fn changed_category_count(&self) -> usize {
        self.categories.len()
    }

    #[must_use]
    pub const fn changed_field_count(&self) -> usize {
        self.changed_field_count
    }

    #[must_use]
    pub fn categories(&self) -> &[SettingsChangeCategory] {
        &self.categories
    }
}

impl fmt::Debug for SettingsImportPreview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SettingsImportPreview")
            .field("changed_category_count", &self.changed_category_count())
            .field("changed_field_count", &self.changed_field_count)
            .finish()
    }
}
