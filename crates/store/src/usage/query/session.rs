use std::{fmt, sync::OnceLock, time::Duration, time::Instant};

use rusqlite::{Connection, TransactionBehavior, params, params_from_iter, types::Value};

use super::analytics::{usage_breakdown, usage_breakdown_item};
use super::{
    MAX_QUERY_DURATION, MAX_USAGE_QUERY_SCOPES, PROGRESS_OP_INTERVAL, UsageAggregateMetrics,
    UsageBreakdown, UsageBreakdownKind, UsageQueryDatasetIdentity, UsageQueryPublication,
    UsageReadStore, load_query_publication, load_raw_publication, load_ready_aggregate_generation,
    map_sql, raw_metrics_at, valid_ascii_id,
};
use crate::usage::types::ScanScope;
use crate::{StoreError, StoreErrorCode};

pub const MAX_USAGE_SESSION_PAGE_SIZE: usize = 256;
pub const MAX_USAGE_SESSION_DETAIL_ITEMS: usize = 256;

const SESSION_METRICS_SQL: &str = "event_count,
       input_known_count, input_known_sum, cached_known_count, cached_known_sum,
       output_known_count, output_known_sum, reasoning_known_count, reasoning_known_sum,
       total_known_count, total_known_sum, fallback_model_count,
       long_context_yes_count, long_context_no_count, long_context_unavailable_count,
       activity_read, activity_edit_write, activity_search, activity_git,
       activity_build_test, activity_web, activity_subagents, activity_terminal";

const SESSION_SCOPE_SQL: &str = "(?3 = 0 OR (provider_id, profile_id) IN (VALUES
       (?4, ?5), (?6, ?7), (?8, ?9), (?10, ?11),
       (?12, ?13), (?14, ?15), (?16, ?17), (?18, ?19),
       (?20, ?21), (?22, ?23), (?24, ?25), (?26, ?27),
       (?28, ?29), (?30, ?31), (?32, ?33), (?34, ?35),
       (?36, ?37), (?38, ?39), (?40, ?41), (?42, ?43),
       (?44, ?45), (?46, ?47), (?48, ?49), (?50, ?51),
       (?52, ?53), (?54, ?55), (?56, ?57), (?58, ?59),
       (?60, ?61), (?62, ?63), (?64, ?65), (?66, ?67)
     ))";

const CURSOR_SESSION_SCOPE_SQL: &str = "(?8 = 0 OR (provider_id, profile_id) IN (VALUES
       (?9, ?10), (?11, ?12), (?13, ?14), (?15, ?16),
       (?17, ?18), (?19, ?20), (?21, ?22), (?23, ?24),
       (?25, ?26), (?27, ?28), (?29, ?30), (?31, ?32),
       (?33, ?34), (?35, ?36), (?37, ?38), (?39, ?40),
       (?41, ?42), (?43, ?44), (?45, ?46), (?47, ?48),
       (?49, ?50), (?51, ?52), (?53, ?54), (?55, ?56),
       (?57, ?58), (?59, ?60), (?61, ?62), (?63, ?64),
       (?65, ?66), (?67, ?68), (?69, ?70), (?71, ?72)
     ))";

#[derive(Clone, Eq, PartialEq)]
pub struct UsageSessionKey {
    dataset_identity: UsageQueryDatasetIdentity,
    provider_id: Box<str>,
    profile_id: Box<str>,
    session_id: Box<str>,
}

impl UsageSessionKey {
    pub(super) const fn dataset_identity_internal(&self) -> UsageQueryDatasetIdentity {
        self.dataset_identity
    }

    pub(super) fn provider_id_internal(&self) -> &str {
        &self.provider_id
    }

    pub(super) fn profile_id_internal(&self) -> &str {
        &self.profile_id
    }

    pub(super) fn session_id_internal(&self) -> &str {
        &self.session_id
    }
}

impl fmt::Debug for UsageSessionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UsageSessionKey")
            .field("dataset_identity", &self.dataset_identity)
            .field("identity", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct UsageSessionCursor {
    key: UsageSessionKey,
    last_timestamp_seconds: i64,
    last_timestamp_nanos: u32,
}

impl fmt::Debug for UsageSessionCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UsageSessionCursor")
            .field("dataset_identity", &self.key.dataset_identity)
            .field("last_timestamp_seconds", &self.last_timestamp_seconds)
            .field("last_timestamp_nanos", &self.last_timestamp_nanos)
            .field("identity", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionPageQuery {
    expected_dataset: Option<UsageQueryDatasetIdentity>,
    before: Option<UsageSessionCursor>,
    scopes: Box<[ScanScope]>,
    page_size: usize,
    deadline: Duration,
}

impl UsageSessionPageQuery {
    pub fn new(
        expected_dataset: Option<UsageQueryDatasetIdentity>,
        before: Option<UsageSessionCursor>,
        scopes: Box<[ScanScope]>,
        page_size: usize,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        if page_size == 0
            || deadline.is_zero()
            || deadline > MAX_QUERY_DURATION
            || matches!(
                expected_dataset,
                Some(UsageQueryDatasetIdentity::ReplayRevision {
                    revision_id,
                    dataset_generation,
                }) if revision_id > i64::MAX as u64 || dataset_generation > i64::MAX as u64
            )
            || before.as_ref().is_some_and(|cursor| {
                expected_dataset != Some(cursor.key.dataset_identity)
                    || cursor.key.dataset_identity == UsageQueryDatasetIdentity::Empty
            })
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if page_size > MAX_USAGE_SESSION_PAGE_SIZE {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_USAGE_SESSION_PAGE_SIZE as u64,
            ));
        }
        if scopes.len() > MAX_USAGE_QUERY_SCOPES {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_USAGE_QUERY_SCOPES as u64,
            ));
        }
        let mut scopes = scopes.into_vec();
        scopes.sort_unstable();
        if scopes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            expected_dataset,
            before,
            scopes: scopes.into_boxed_slice(),
            page_size,
            deadline,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionSummary {
    key: UsageSessionKey,
    first_timestamp_seconds: i64,
    first_timestamp_nanos: u32,
    last_timestamp_seconds: i64,
    last_timestamp_nanos: u32,
    metrics: UsageAggregateMetrics,
}

impl UsageSessionSummary {
    #[must_use]
    pub const fn key(&self) -> &UsageSessionKey {
        &self.key
    }

    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.key.provider_id
    }

    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.key.profile_id
    }

    #[must_use]
    pub const fn first_timestamp_seconds(&self) -> i64 {
        self.first_timestamp_seconds
    }

    #[must_use]
    pub const fn first_timestamp_nanos(&self) -> u32 {
        self.first_timestamp_nanos
    }

    #[must_use]
    pub const fn last_timestamp_seconds(&self) -> i64 {
        self.last_timestamp_seconds
    }

    #[must_use]
    pub const fn last_timestamp_nanos(&self) -> u32 {
        self.last_timestamp_nanos
    }

    #[must_use]
    pub const fn metrics(&self) -> &UsageAggregateMetrics {
        &self.metrics
    }

    #[must_use]
    pub fn cursor(&self) -> UsageSessionCursor {
        UsageSessionCursor {
            key: self.key.clone(),
            last_timestamp_seconds: self.last_timestamp_seconds,
            last_timestamp_nanos: self.last_timestamp_nanos,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionPageCapture {
    publication: UsageQueryPublication,
    sessions: Box<[UsageSessionSummary]>,
    next_cursor: Option<UsageSessionCursor>,
    has_more: bool,
}

impl UsageSessionPageCapture {
    #[must_use]
    pub const fn publication(&self) -> &UsageQueryPublication {
        &self.publication
    }

    #[must_use]
    pub const fn sessions(&self) -> &[UsageSessionSummary] {
        &self.sessions
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&UsageSessionCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionDetailQuery {
    expected_dataset: UsageQueryDatasetIdentity,
    key: UsageSessionKey,
    deadline: Duration,
}

impl UsageSessionDetailQuery {
    pub fn new(
        expected_dataset: UsageQueryDatasetIdentity,
        key: UsageSessionKey,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        if expected_dataset == UsageQueryDatasetIdentity::Empty
            || expected_dataset != key.dataset_identity
            || deadline.is_zero()
            || deadline > MAX_QUERY_DURATION
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            expected_dataset,
            key,
            deadline,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionDetail {
    summary: UsageSessionSummary,
    breakdowns: Box<[UsageBreakdown]>,
}

impl UsageSessionDetail {
    #[must_use]
    pub const fn summary(&self) -> &UsageSessionSummary {
        &self.summary
    }

    #[must_use]
    pub const fn breakdowns(&self) -> &[UsageBreakdown] {
        &self.breakdowns
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionDetailCapture {
    publication: UsageQueryPublication,
    detail: Option<UsageSessionDetail>,
}

impl UsageSessionDetailCapture {
    #[must_use]
    pub const fn publication(&self) -> &UsageQueryPublication {
        &self.publication
    }

    #[must_use]
    pub const fn detail(&self) -> Option<&UsageSessionDetail> {
        self.detail.as_ref()
    }
}

impl UsageReadStore {
    pub fn capture_usage_session_page(
        &mut self,
        query: UsageSessionPageQuery,
    ) -> Result<UsageSessionPageCapture, StoreError> {
        self.capture_usage_session_page_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    fn capture_usage_session_page_with_options<F>(
        &mut self,
        query: UsageSessionPageQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_publication: F,
    ) -> Result<UsageSessionPageCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || started.elapsed() >= deadline),
        ))?;
        let result = capture_usage_session_page(&mut self.connection, query, after_publication);
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    pub fn capture_usage_session_detail(
        &mut self,
        query: UsageSessionDetailQuery,
    ) -> Result<UsageSessionDetailCapture, StoreError> {
        self.capture_usage_session_detail_with_options(
            query,
            PROGRESS_OP_INTERVAL,
            false,
            || Ok(()),
        )
    }

    fn capture_usage_session_detail_with_options<F>(
        &mut self,
        query: UsageSessionDetailQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_publication: F,
    ) -> Result<UsageSessionDetailCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || started.elapsed() >= deadline),
        ))?;
        let result = capture_usage_session_detail(&mut self.connection, query, after_publication);
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

fn capture_usage_session_page<F>(
    connection: &mut Connection,
    query: UsageSessionPageQuery,
    after_publication: F,
) -> Result<UsageSessionPageCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let raw_publication = load_raw_publication(&transaction)?;
    after_publication()?;
    let dataset_identity = raw_publication.dataset_identity()?;
    if query
        .expected_dataset
        .is_some_and(|expected| expected != dataset_identity)
    {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let publication = load_query_publication(&transaction, &raw_publication, dataset_identity)?;
    let active_generation =
        load_ready_aggregate_generation(&transaction, raw_publication.dataset_generation)?;
    let dataset_kind = dataset_kind(dataset_identity);
    let mut sessions = match dataset_kind {
        Some(kind) => load_session_page(
            &transaction,
            active_generation,
            kind,
            dataset_identity,
            &query,
        )?,
        None => Vec::new(),
    };
    let has_more = sessions.len() > query.page_size;
    if has_more {
        sessions.truncate(query.page_size);
    }
    let next_cursor = if has_more {
        sessions.last().map(UsageSessionSummary::cursor)
    } else {
        None
    };
    map_sql(transaction.commit())?;
    Ok(UsageSessionPageCapture {
        publication,
        sessions: sessions.into_boxed_slice(),
        next_cursor,
        has_more,
    })
}

fn capture_usage_session_detail<F>(
    connection: &mut Connection,
    query: UsageSessionDetailQuery,
    after_publication: F,
) -> Result<UsageSessionDetailCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let raw_publication = load_raw_publication(&transaction)?;
    after_publication()?;
    let dataset_identity = raw_publication.dataset_identity()?;
    if query.expected_dataset != dataset_identity {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let publication = load_query_publication(&transaction, &raw_publication, dataset_identity)?;
    let active_generation =
        load_ready_aggregate_generation(&transaction, raw_publication.dataset_generation)?;
    let dataset_kind = dataset_kind(dataset_identity)
        .ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
    let detail = load_session_detail(
        &transaction,
        active_generation,
        dataset_kind,
        dataset_identity,
        &query.key,
    )?;
    map_sql(transaction.commit())?;
    Ok(UsageSessionDetailCapture {
        publication,
        detail,
    })
}

const fn dataset_kind(identity: UsageQueryDatasetIdentity) -> Option<&'static str> {
    match identity {
        UsageQueryDatasetIdentity::Empty => None,
        UsageQueryDatasetIdentity::LegacySnapshotV1 => Some("legacy"),
        UsageQueryDatasetIdentity::ReplayRevision { .. } => Some("current"),
    }
}

fn load_session_page(
    connection: &Connection,
    active_generation: i64,
    dataset_kind: &'static str,
    dataset_identity: UsageQueryDatasetIdentity,
    query: &UsageSessionPageQuery,
) -> Result<Vec<UsageSessionSummary>, StoreError> {
    let (sql, parameters) = if let Some(cursor) = &query.before {
        (
            cursor_session_page_sql(),
            cursor_session_page_parameters(
                active_generation,
                dataset_kind,
                cursor,
                &query.scopes,
                query.page_size,
            )?,
        )
    } else {
        (
            first_session_page_sql(),
            first_session_page_parameters(
                active_generation,
                dataset_kind,
                &query.scopes,
                query.page_size,
            )?,
        )
    };
    let mut statement = map_sql(connection.prepare_cached(sql))?;
    let rows = map_sql(statement.query_map(params_from_iter(parameters.iter()), raw_session_row))?;
    let mut sessions = Vec::with_capacity(query.page_size + 1);
    for row in rows {
        sessions.push(map_sql(row)?.validate(dataset_identity)?);
    }
    Ok(sessions)
}

fn load_session_detail(
    connection: &Connection,
    active_generation: i64,
    dataset_kind: &'static str,
    dataset_identity: UsageQueryDatasetIdentity,
    key: &UsageSessionKey,
) -> Result<Option<UsageSessionDetail>, StoreError> {
    let mut statement = map_sql(connection.prepare_cached(session_detail_summary_sql()))?;
    let mut rows = map_sql(statement.query(params![
        active_generation,
        dataset_kind,
        key.provider_id.as_ref(),
        key.profile_id.as_ref(),
        key.session_id.as_ref(),
    ]))?;
    let Some(row) = map_sql(rows.next())? else {
        return Ok(None);
    };
    let raw = RawSessionSummary {
        provider_id: key.provider_id.to_string(),
        profile_id: key.profile_id.to_string(),
        session_id: key.session_id.to_string(),
        first_timestamp_seconds: map_sql(row.get(0))?,
        first_timestamp_nanos: map_sql(row.get(1))?,
        last_timestamp_seconds: map_sql(row.get(2))?,
        last_timestamp_nanos: map_sql(row.get(3))?,
        metrics: map_sql(raw_metrics_at(row, 4))?,
    };
    let summary = raw.validate(dataset_identity)?;
    drop(rows);
    drop(statement);
    let model = load_session_breakdown(
        connection,
        active_generation,
        dataset_kind,
        key,
        UsageBreakdownKind::Model,
    )?;
    let project = load_session_breakdown(
        connection,
        active_generation,
        dataset_kind,
        key,
        UsageBreakdownKind::Project,
    )?;
    Ok(Some(UsageSessionDetail {
        summary,
        breakdowns: vec![model, project].into_boxed_slice(),
    }))
}

fn load_session_breakdown(
    connection: &Connection,
    active_generation: i64,
    dataset_kind: &'static str,
    key: &UsageSessionKey,
    kind: UsageBreakdownKind,
) -> Result<UsageBreakdown, StoreError> {
    let dimension = match kind {
        UsageBreakdownKind::Model => "model",
        UsageBreakdownKind::Project => "project",
        UsageBreakdownKind::Provider | UsageBreakdownKind::Profile => {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
    };
    let mut statement = map_sql(connection.prepare_cached(session_detail_breakdown_sql()))?;
    let rows = map_sql(statement.query_map(
        params![
            active_generation,
            dataset_kind,
            key.provider_id.as_ref(),
            key.profile_id.as_ref(),
            key.session_id.as_ref(),
            dimension,
        ],
        |row| Ok((row.get::<_, String>(0)?, raw_metrics_at(row, 1)?)),
    ))?;
    let mut items = Vec::with_capacity(MAX_USAGE_SESSION_DETAIL_ITEMS + 1);
    for row in rows {
        let (identity, raw) = map_sql(row)?;
        items.push(usage_breakdown_item(kind, identity, raw.validate()?)?);
    }
    let truncated = items.len() > MAX_USAGE_SESSION_DETAIL_ITEMS;
    if truncated {
        items.truncate(MAX_USAGE_SESSION_DETAIL_ITEMS);
    }
    Ok(usage_breakdown(kind, items, truncated))
}

fn first_session_page_parameters(
    active_generation: i64,
    dataset_kind: &'static str,
    scopes: &[ScanScope],
    page_size: usize,
) -> Result<Vec<Value>, StoreError> {
    let mut values = Vec::with_capacity(68);
    values.push(Value::Integer(active_generation));
    values.push(Value::Text(dataset_kind.to_owned()));
    push_scope_parameters(&mut values, scopes)?;
    values.push(Value::Integer(page_limit(page_size)?));
    Ok(values)
}

fn cursor_session_page_parameters(
    active_generation: i64,
    dataset_kind: &'static str,
    cursor: &UsageSessionCursor,
    scopes: &[ScanScope],
    page_size: usize,
) -> Result<Vec<Value>, StoreError> {
    let mut values = Vec::with_capacity(73);
    values.push(Value::Integer(active_generation));
    values.push(Value::Text(dataset_kind.to_owned()));
    values.push(Value::Integer(cursor.last_timestamp_seconds));
    values.push(Value::Integer(i64::from(cursor.last_timestamp_nanos)));
    values.push(Value::Text(cursor.key.provider_id.to_string()));
    values.push(Value::Text(cursor.key.profile_id.to_string()));
    values.push(Value::Text(cursor.key.session_id.to_string()));
    push_scope_parameters(&mut values, scopes)?;
    values.push(Value::Integer(page_limit(page_size)?));
    Ok(values)
}

fn push_scope_parameters(values: &mut Vec<Value>, scopes: &[ScanScope]) -> Result<(), StoreError> {
    values
        .push(Value::Integer(i64::try_from(scopes.len()).map_err(
            |_| StoreError::new(StoreErrorCode::CapacityExceeded),
        )?));
    for index in 0..MAX_USAGE_QUERY_SCOPES {
        if let Some(scope) = scopes.get(index) {
            values.push(Value::Text(scope.provider_id().to_owned()));
            values.push(Value::Text(scope.profile_id().to_owned()));
        } else {
            values.push(Value::Null);
            values.push(Value::Null);
        }
    }
    Ok(())
}

fn page_limit(page_size: usize) -> Result<i64, StoreError> {
    i64::try_from(page_size + 1).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn raw_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawSessionSummary> {
    Ok(RawSessionSummary {
        provider_id: row.get(0)?,
        profile_id: row.get(1)?,
        session_id: row.get(2)?,
        first_timestamp_seconds: row.get(3)?,
        first_timestamp_nanos: row.get(4)?,
        last_timestamp_seconds: row.get(5)?,
        last_timestamp_nanos: row.get(6)?,
        metrics: raw_metrics_at(row, 7)?,
    })
}

struct RawSessionSummary {
    provider_id: String,
    profile_id: String,
    session_id: String,
    first_timestamp_seconds: i64,
    first_timestamp_nanos: i64,
    last_timestamp_seconds: i64,
    last_timestamp_nanos: i64,
    metrics: super::RawAggregateMetrics,
}

impl RawSessionSummary {
    fn validate(
        self,
        dataset_identity: UsageQueryDatasetIdentity,
    ) -> Result<UsageSessionSummary, StoreError> {
        if !valid_ascii_id(&self.provider_id, 64)
            || !valid_ascii_id(&self.profile_id, 128)
            || !valid_private_session_id(&self.session_id)
        {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        let first_timestamp_nanos = valid_nanos(self.first_timestamp_nanos)?;
        let last_timestamp_nanos = valid_nanos(self.last_timestamp_nanos)?;
        if (self.first_timestamp_seconds, first_timestamp_nanos)
            > (self.last_timestamp_seconds, last_timestamp_nanos)
        {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        Ok(UsageSessionSummary {
            key: UsageSessionKey {
                dataset_identity,
                provider_id: self.provider_id.into_boxed_str(),
                profile_id: self.profile_id.into_boxed_str(),
                session_id: self.session_id.into_boxed_str(),
            },
            first_timestamp_seconds: self.first_timestamp_seconds,
            first_timestamp_nanos,
            last_timestamp_seconds: self.last_timestamp_seconds,
            last_timestamp_nanos,
            metrics: self.metrics.validate()?,
        })
    }
}

fn valid_nanos(value: i64) -> Result<u32, StoreError> {
    let value =
        u32::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    if value > 999_999_999 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(value)
}

fn valid_private_session_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn first_session_page_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        format!(
            "SELECT provider_id, profile_id, session_id,
                    first_timestamp_seconds, first_timestamp_nanos,
                    last_timestamp_seconds, last_timestamp_nanos, {SESSION_METRICS_SQL}
             FROM usage_session_rollup
             WHERE aggregate_generation = ?1 AND dataset_kind = ?2
               AND dimension_kind = 'all' AND dimension_value = ''
               AND {SESSION_SCOPE_SQL}
             ORDER BY last_timestamp_seconds DESC, last_timestamp_nanos DESC,
                      provider_id ASC, profile_id ASC, session_id ASC
             LIMIT ?68"
        )
    })
}

fn cursor_session_page_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        format!(
            "SELECT provider_id, profile_id, session_id,
                    first_timestamp_seconds, first_timestamp_nanos,
                    last_timestamp_seconds, last_timestamp_nanos, {SESSION_METRICS_SQL}
             FROM usage_session_rollup
             WHERE aggregate_generation = ?1 AND dataset_kind = ?2
               AND dimension_kind = 'all' AND dimension_value = ''
               AND (
                 last_timestamp_seconds < ?3
                 OR (last_timestamp_seconds = ?3 AND last_timestamp_nanos < ?4)
                 OR (last_timestamp_seconds = ?3 AND last_timestamp_nanos = ?4
                     AND (provider_id, profile_id, session_id) > (?5, ?6, ?7))
               )
               AND {CURSOR_SESSION_SCOPE_SQL}
             ORDER BY last_timestamp_seconds DESC, last_timestamp_nanos DESC,
                      provider_id ASC, profile_id ASC, session_id ASC
             LIMIT ?73"
        )
    })
}

fn session_detail_summary_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        format!(
            "SELECT first_timestamp_seconds, first_timestamp_nanos,
                    last_timestamp_seconds, last_timestamp_nanos, {SESSION_METRICS_SQL}
             FROM usage_session_rollup
             WHERE aggregate_generation = ?1 AND dataset_kind = ?2
               AND provider_id = ?3 AND profile_id = ?4 AND session_id = ?5
               AND dimension_kind = 'all' AND dimension_value = ''"
        )
    })
}

fn session_detail_breakdown_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        format!(
            "SELECT dimension_value, {SESSION_METRICS_SQL}
             FROM usage_session_rollup
             WHERE aggregate_generation = ?1 AND dataset_kind = ?2
               AND provider_id = ?3 AND profile_id = ?4 AND session_id = ?5
               AND dimension_kind = ?6
             ORDER BY total_known_sum DESC, event_count DESC, dimension_value ASC
             LIMIT 257"
        )
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;
    use crate::UsageStore;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn empty_archive() -> TestResult<(TempDir, std::path::PathBuf)> {
        let directory = TempDir::new()?;
        let path = directory.path().join("session-query.sqlite3");
        drop(UsageStore::open(&path)?);
        Ok((directory, path))
    }

    fn empty_current_archive() -> TestResult<(TempDir, std::path::PathBuf)> {
        let (directory, path) = empty_archive()?;
        let connection = Connection::open(&path)?;
        connection.execute(
            "INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted, scan_set_id
             ) VALUES (0, 'current', 1, 2, 1, 0, 0, 1, 1, NULL)",
            [],
        )?;
        connection.execute(
            "UPDATE usage_archive_state
             SET current_revision_id = 0, incremental_state = 'partial'
             WHERE singleton_id = 1",
            [],
        )?;
        Ok((directory, path))
    }

    fn page_query(deadline: Duration) -> Result<UsageSessionPageQuery, StoreError> {
        UsageSessionPageQuery::new(None, None, Box::default(), 16, deadline)
    }

    fn current_key() -> UsageSessionKey {
        UsageSessionKey {
            dataset_identity: UsageQueryDatasetIdentity::ReplayRevision {
                revision_id: 0,
                dataset_generation: 0,
            },
            provider_id: "codex".into(),
            profile_id: "default".into(),
            session_id: "private-session".into(),
        }
    }

    fn explain(
        connection: &Connection,
        sql: &str,
        parameters: &[Value],
    ) -> TestResult<Vec<String>> {
        let mut statement = connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let rows = statement.query_map(params_from_iter(parameters.iter()), |row| row.get(3))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    #[test]
    fn session_plans_use_only_fixed_rollups_and_keyset_indexes() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let store = UsageReadStore::open(&path)?;
        let first_parameters =
            first_session_page_parameters(0, "current", &[], MAX_USAGE_SESSION_PAGE_SIZE)?;
        let first = explain(
            &store.connection,
            first_session_page_sql(),
            &first_parameters,
        )?;
        assert!(first.iter().any(|detail| {
            detail.contains("usage_session_rollup_page")
                || detail.contains("sqlite_autoindex_usage_session_rollup")
        }));
        assert!(!first.iter().any(|detail| detail.contains("TEMP B-TREE")));

        let cursor = UsageSessionCursor {
            key: current_key(),
            last_timestamp_seconds: 100,
            last_timestamp_nanos: 1,
        };
        let cursor_parameters = cursor_session_page_parameters(
            0,
            "current",
            &cursor,
            &[],
            MAX_USAGE_SESSION_PAGE_SIZE,
        )?;
        let cursor_plan = explain(
            &store.connection,
            cursor_session_page_sql(),
            &cursor_parameters,
        )?;
        assert!(
            cursor_plan
                .iter()
                .any(|detail| detail.contains("usage_session_rollup_page"))
        );
        assert!(
            !cursor_plan
                .iter()
                .any(|detail| detail.contains("TEMP B-TREE"))
        );

        let fixed_sql = [
            first_session_page_sql(),
            cursor_session_page_sql(),
            session_detail_summary_sql(),
            session_detail_breakdown_sql(),
        ];
        for sql in fixed_sql {
            let normalized = sql.to_ascii_lowercase();
            assert!(normalized.contains("usage_session_rollup"));
            assert!(!normalized.contains("usage_event"));
            assert!(!normalized.contains("offset"));
        }
        Ok(())
    }

    #[test]
    fn session_page_cancellation_is_cleared_for_the_next_query() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let interrupted = match store.capture_usage_session_page_with_options(
            page_query(Duration::from_secs(2))?,
            1,
            true,
            || Ok(()),
        ) {
            Err(error) => error,
            Ok(_) => return Err("forced session cancellation unexpectedly completed".into()),
        };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next = store.capture_usage_session_page(page_query(Duration::from_secs(2))?)?;
        assert!(next.sessions().is_empty());
        Ok(())
    }

    #[test]
    fn session_detail_cancellation_is_cleared_for_the_next_query() -> TestResult {
        let (_directory, path) = empty_current_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let query = || {
            UsageSessionDetailQuery::new(
                UsageQueryDatasetIdentity::ReplayRevision {
                    revision_id: 0,
                    dataset_generation: 0,
                },
                current_key(),
                Duration::from_secs(2),
            )
        };
        let interrupted =
            match store.capture_usage_session_detail_with_options(query()?, 1, true, || Ok(())) {
                Err(error) => error,
                Ok(_) => return Err("forced detail cancellation unexpectedly completed".into()),
            };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next = store.capture_usage_session_detail(query()?)?;
        assert!(next.detail().is_none());
        Ok(())
    }

    #[test]
    fn session_page_snapshot_keeps_ready_state_exact_during_concurrent_change() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let writer_path = path.clone();
        let capture = store.capture_usage_session_page_with_options(
            page_query(Duration::from_secs(2))?,
            PROGRESS_OP_INTERVAL,
            false,
            move || {
                let writer = map_sql(Connection::open(&writer_path))?;
                map_sql(writer.execute(
                    "UPDATE usage_aggregate_state SET state = 'rebuild_required'
                     WHERE singleton_id = 1",
                    [],
                ))?;
                Ok(())
            },
        )?;
        assert!(capture.sessions().is_empty());
        let error = match store.capture_usage_session_page(page_query(Duration::from_secs(2))?) {
            Err(error) => error,
            Ok(_) => return Err("new transaction ignored unavailable session rollups".into()),
        };
        assert_eq!(error.code(), StoreErrorCode::RebuildRequired);
        Ok(())
    }
}
