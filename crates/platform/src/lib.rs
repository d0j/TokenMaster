#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fmt;
use std::fs::File;

#[cfg(unix)]
mod unix;
#[cfg(not(any(unix, windows)))]
mod unsupported;
#[cfg(windows)]
mod windows;

/// Stable, path-private identity for the physical file referenced by an open handle.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct PhysicalFileIdentity([u8; 32]);

impl PhysicalFileIdentity {
    /// Queries the operating system identity of `file` and hashes its opaque fields.
    pub fn from_file(file: &File) -> Result<Self, PhysicalIdentityError> {
        platform_identity(file)
    }

    /// Reconstructs an opaque identity from its controlled persistent representation.
    #[must_use]
    pub const fn from_persisted_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the opaque identity bytes for equality checks and persistence.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for PhysicalFileIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PhysicalFileIdentity([redacted])")
    }
}

/// Stable failure categories for physical identity queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PhysicalIdentityError {
    #[error("physical file identity is unavailable on this platform")]
    Unavailable,
    #[error("physical file identity query failed")]
    QueryFailed,
}

#[cfg(unix)]
use unix::platform_identity;
#[cfg(not(any(unix, windows)))]
use unsupported::platform_identity;
#[cfg(windows)]
use windows::platform_identity;

fn from_digest(digest: impl Into<[u8; 32]>) -> PhysicalFileIdentity {
    PhysicalFileIdentity(digest.into())
}
