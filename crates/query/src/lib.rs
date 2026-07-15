//! Bounded immutable read values for TokenMaster frontends.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod activity;
mod clock;
mod error;
mod identity;
mod publication;
mod service;

pub use activity::{
    ActivityCursor, ActivityItem, ActivityItemParts, LatestActivityPage, MAX_QUERY_PAGE_SIZE,
    PageSize,
};
pub use clock::{QueryClock, QueryTimeSample, SystemQueryClock};
pub use error::{QueryError, QueryErrorCode};
pub use identity::{
    DatasetIdentity, MAX_QUERY_SCOPES, MAX_QUERY_WARNINGS, PublicationGeneration,
    QUERY_SCHEMA_VERSION, QueryEnvelope, QueryFreshness, QueryHeader, QueryHeaderParts,
    QueryQuality, QueryScope, QueryWarningCode, ReplayRevision, SnapshotGeneration,
};
pub use publication::{PublishOutcome, QuerySnapshotSlot};
pub use service::{
    LatestActivityRequest, QUERY_FRESH_MAX_AGE_MS, QUERY_STALE_MIN_AGE_MS, QueryService,
};
