//! Path-bearing discovery descriptors intentionally do not implement serialization.
//!
//! ```compile_fail
//! fn assert_serialize<T: serde::Serialize>() {}
//! assert_serialize::<tokenmaster_provider::ProfileDescriptor>();
//! ```
//!
//! Transient repository hints are also deliberately non-serializable:
//!
//! ```compile_fail
//! fn assert_serialize<T: serde::Serialize>() {}
//! assert_serialize::<tokenmaster_provider::RepositoryActivityHint>();
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod capability;
mod discovery;
mod error;
mod identity;
mod repository;

pub use capability::{ProviderCapability, ProviderDescriptor};
pub use discovery::{
    DiagnosticCode, DiscoveryDiagnostics, DiscoveryProvider, DiscoveryRequest, DiscoveryRoot,
    DiscoverySnapshot, MAX_LABEL_BYTES, MAX_PATH_BYTES, MAX_PROFILES, MAX_SOURCES,
    ProfileAvailability, ProfileDescriptor, RootOrigin, SourceDescriptor, SourceKind,
};
pub use error::{ProviderError, ProviderErrorCode};
pub use identity::{ProfileId, ProviderId, SourceId};
pub use repository::{
    RepositoryActivityHint, RepositoryActivityHintParts, RepositoryCandidatePath,
};
