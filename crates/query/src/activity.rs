use std::{fmt, sync::Arc};

use tokenmaster_domain::{ModelKey, TokenUsage};
use tokenmaster_store::EventCursor;

use crate::{QueryError, QueryErrorCode, QueryScope};

pub const MAX_QUERY_PAGE_SIZE: usize = 256;
const MAX_EVENT_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageSize(usize);

impl PageSize {
    pub fn new(value: usize) -> Result<Self, QueryError> {
        if !(1..=MAX_QUERY_PAGE_SIZE).contains(&value) {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ActivityCursor {
    inner: EventCursor,
}

impl ActivityCursor {
    pub fn new(
        timestamp_seconds: i64,
        timestamp_nanos: u32,
        fingerprint: [u8; 32],
    ) -> Result<Self, QueryError> {
        EventCursor::new(timestamp_seconds, timestamp_nanos, fingerprint)
            .map(|inner| Self { inner })
            .map_err(|_| QueryError::new(QueryErrorCode::InvalidValue))
    }

    pub(crate) const fn from_store(inner: EventCursor) -> Self {
        Self { inner }
    }

    pub(crate) const fn store_cursor(self) -> EventCursor {
        self.inner
    }
}

impl fmt::Debug for ActivityCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActivityCursor")
            .field("timestamp_seconds", &self.inner.timestamp_seconds())
            .field("timestamp_nanos", &self.inner.timestamp_nanos())
            .field("fingerprint", &Redacted)
            .finish()
    }
}

pub struct ActivityItemParts {
    pub scope: QueryScope,
    pub event_id: String,
    pub timestamp_seconds: i64,
    pub timestamp_nanos: u32,
    pub model: ModelKey,
    pub usage: TokenUsage,
    pub fingerprint: [u8; 32],
}

#[derive(Clone, Eq, PartialEq)]
pub struct ActivityItem {
    scope: QueryScope,
    event_id: Box<str>,
    model: ModelKey,
    usage: TokenUsage,
    cursor: ActivityCursor,
}

impl ActivityItem {
    pub fn new(parts: ActivityItemParts) -> Result<Self, QueryError> {
        let cursor = ActivityCursor::new(
            parts.timestamp_seconds,
            parts.timestamp_nanos,
            parts.fingerprint,
        )?;
        Self::new_with_cursor(
            parts.scope,
            parts.event_id,
            parts.model,
            parts.usage,
            cursor,
        )
    }

    pub(crate) fn new_with_cursor(
        scope: QueryScope,
        event_id: String,
        model: ModelKey,
        usage: TokenUsage,
        cursor: ActivityCursor,
    ) -> Result<Self, QueryError> {
        if !valid_event_id(&event_id) {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self {
            scope,
            event_id: event_id.into_boxed_str(),
            model,
            usage,
            cursor,
        })
    }

    #[must_use]
    pub const fn scope(&self) -> &QueryScope {
        &self.scope
    }

    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    #[must_use]
    pub const fn timestamp_seconds(&self) -> i64 {
        self.cursor.inner.timestamp_seconds()
    }

    #[must_use]
    pub const fn timestamp_nanos(&self) -> u32 {
        self.cursor.inner.timestamp_nanos()
    }

    #[must_use]
    pub const fn model(&self) -> &ModelKey {
        &self.model
    }

    #[must_use]
    pub const fn usage(&self) -> &TokenUsage {
        &self.usage
    }

    #[must_use]
    pub const fn cursor(&self) -> ActivityCursor {
        self.cursor
    }
}

impl fmt::Debug for ActivityItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActivityItem")
            .field("scope", &self.scope)
            .field("event_id", &self.event_id)
            .field("timestamp_seconds", &self.timestamp_seconds())
            .field("timestamp_nanos", &self.timestamp_nanos())
            .field("model", &self.model)
            .field("usage", &self.usage)
            .field("fingerprint", &Redacted)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatestActivityPage {
    items: Arc<[ActivityItem]>,
    next_cursor: Option<ActivityCursor>,
    has_more: bool,
}

impl LatestActivityPage {
    pub fn new(
        items: Vec<ActivityItem>,
        next_cursor: Option<ActivityCursor>,
        has_more: bool,
    ) -> Result<Self, QueryError> {
        if items.len() > MAX_QUERY_PAGE_SIZE {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        if has_more != next_cursor.is_some()
            || next_cursor
                .is_some_and(|cursor| items.last().map(ActivityItem::cursor) != Some(cursor))
        {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self {
            items: Arc::from(items),
            next_cursor,
            has_more,
        })
    }

    #[must_use]
    pub const fn items(&self) -> &Arc<[ActivityItem]> {
        &self.items
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<ActivityCursor> {
        self.next_cursor
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

fn valid_event_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_EVENT_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}
