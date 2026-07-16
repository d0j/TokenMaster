use rusqlite::{Transaction, TransactionBehavior, params};

use super::{
    UsageStore,
    types::{AggregateRebuildProgress, AggregateRebuildStatus, MAX_AGGREGATE_REBUILD_PAGE_SIZE},
};
use crate::{StoreError, StoreErrorCode};

#[derive(Clone, Copy, Eq, PartialEq)]
enum AggregateFault {
    None,
    #[cfg(test)]
    AfterStateInitialized,
    #[cfg(test)]
    AfterPageMaterialized,
    #[cfg(test)]
    BeforePublish,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum AggregateBoundary {
    StateInitialized,
    PageMaterialized,
    Publish,
}

const METRIC_COLUMNS: &str = "
  event_count, input_known_count, input_known_sum,
  cached_known_count, cached_known_sum, output_known_count, output_known_sum,
  reasoning_known_count, reasoning_known_sum, total_known_count, total_known_sum,
  fallback_model_count, long_context_yes_count, long_context_no_count,
  long_context_unavailable_count, activity_read, activity_edit_write,
  activity_search, activity_git, activity_build_test, activity_web,
  activity_subagents, activity_terminal";

const METRIC_SELECT: &str = "
  count(*), count(input_tokens), coalesce(sum(input_tokens), 0),
  count(cached_tokens), coalesce(sum(cached_tokens), 0),
  count(output_tokens), coalesce(sum(output_tokens), 0),
  count(reasoning_tokens), coalesce(sum(reasoning_tokens), 0),
  count(total_tokens), coalesce(sum(total_tokens), 0),
  sum(fallback_model),
  sum(CASE WHEN long_context = 'yes' THEN 1 ELSE 0 END),
  sum(CASE WHEN long_context = 'no' THEN 1 ELSE 0 END),
  sum(CASE WHEN long_context = 'unavailable' THEN 1 ELSE 0 END),
  sum(activity_read), sum(activity_edit_write), sum(activity_search),
  sum(activity_git), sum(activity_build_test), sum(activity_web),
  sum(activity_subagents), sum(activity_terminal)";

const METRIC_UPSERT: &str = "
  event_count = event_count + excluded.event_count,
  input_known_count = input_known_count + excluded.input_known_count,
  input_known_sum = input_known_sum + excluded.input_known_sum,
  cached_known_count = cached_known_count + excluded.cached_known_count,
  cached_known_sum = cached_known_sum + excluded.cached_known_sum,
  output_known_count = output_known_count + excluded.output_known_count,
  output_known_sum = output_known_sum + excluded.output_known_sum,
  reasoning_known_count = reasoning_known_count + excluded.reasoning_known_count,
  reasoning_known_sum = reasoning_known_sum + excluded.reasoning_known_sum,
  total_known_count = total_known_count + excluded.total_known_count,
  total_known_sum = total_known_sum + excluded.total_known_sum,
  fallback_model_count = fallback_model_count + excluded.fallback_model_count,
  long_context_yes_count = long_context_yes_count + excluded.long_context_yes_count,
  long_context_no_count = long_context_no_count + excluded.long_context_no_count,
  long_context_unavailable_count =
    long_context_unavailable_count + excluded.long_context_unavailable_count,
  activity_read = activity_read + excluded.activity_read,
  activity_edit_write = activity_edit_write + excluded.activity_edit_write,
  activity_search = activity_search + excluded.activity_search,
  activity_git = activity_git + excluded.activity_git,
  activity_build_test = activity_build_test + excluded.activity_build_test,
  activity_web = activity_web + excluded.activity_web,
  activity_subagents = activity_subagents + excluded.activity_subagents,
  activity_terminal = activity_terminal + excluded.activity_terminal";

const CURRENT_PAGE: &str = "
SELECT fingerprint, provider_id, profile_id, session_id, timestamp_seconds,
       timestamp_nanos, model, project_alias, input_tokens, cached_tokens,
       output_tokens, reasoning_tokens, total_tokens, fallback_model,
       long_context, service_tier, reported_cost_usd_micros,
       activity_read, activity_edit_write, activity_search,
       activity_git, activity_build_test, activity_web, activity_subagents,
       activity_terminal
FROM usage_event
WHERE (?2 IS NULL OR fingerprint > ?2)
ORDER BY fingerprint
LIMIT ?3";

const LEGACY_PAGE: &str = "
SELECT event.fingerprint,
       CASE WHEN source.file_key IS NOT NULL
                  AND source.profile_id = event.profile_id
            THEN source.provider_id ELSE 'unknown' END AS provider_id,
       event.profile_id, event.session_id, event.timestamp_seconds,
       event.timestamp_nanos, event.model, event.project_alias,
       event.input_tokens, event.cached_tokens, event.output_tokens,
       event.reasoning_tokens, event.total_tokens, event.fallback_model,
       event.long_context, event.service_tier, event.reported_cost_usd_micros,
       event.activity_read, event.activity_edit_write,
       event.activity_search, event.activity_git, event.activity_build_test,
       event.activity_web, event.activity_subagents, event.activity_terminal
FROM usage_legacy_event AS event
LEFT JOIN usage_source AS source ON source.file_key = event.selected_file_key
WHERE event.snapshot_id = 1 AND (?2 IS NULL OR event.fingerprint > ?2)
ORDER BY event.fingerprint
LIMIT ?3";

fn time_page_sql(dataset_kind: &str, page_sql: &str) -> String {
    format!(
        "WITH page AS ({page_sql}),
         expanded AS (
           SELECT page.*,
                  bucket.width AS bucket_width,
                  timestamp_seconds -
                    (((timestamp_seconds % bucket.seconds) + bucket.seconds)
                     % bucket.seconds) AS bucket_start_seconds,
                  dimension.kind AS dimension_kind,
                  CASE dimension.kind
                    WHEN 'all' THEN ''
                    WHEN 'model' THEN page.model
                    ELSE coalesce(page.project_alias, '')
                  END AS dimension_value
           FROM page
           CROSS JOIN (
             SELECT 'minute' AS width, 60 AS seconds
             UNION ALL SELECT 'hour', 3600
           ) AS bucket
           CROSS JOIN (
             SELECT 'all' AS kind
             UNION ALL SELECT 'model'
             UNION ALL SELECT 'project'
           ) AS dimension
         )
         INSERT INTO usage_time_rollup(
           aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
           provider_id, profile_id, dimension_kind, dimension_value,
           {METRIC_COLUMNS}
         )
         SELECT ?1, '{dataset_kind}', bucket_width, bucket_start_seconds,
                provider_id, profile_id, dimension_kind, dimension_value,
                {METRIC_SELECT}
         FROM expanded
         GROUP BY bucket_width, bucket_start_seconds, provider_id, profile_id,
                  dimension_kind, dimension_value
         ON CONFLICT(
           aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
           provider_id, profile_id, dimension_kind, dimension_value
         ) DO UPDATE SET {METRIC_UPSERT}"
    )
}

fn session_page_sql(dataset_kind: &str, page_sql: &str) -> String {
    format!(
        "WITH page AS ({page_sql}),
         expanded AS (
           SELECT page.*, dimension.kind AS dimension_kind,
                  CASE dimension.kind
                    WHEN 'all' THEN ''
                    WHEN 'model' THEN page.model
                    ELSE coalesce(page.project_alias, '')
                  END AS dimension_value
           FROM page
           CROSS JOIN (
             SELECT 'all' AS kind
             UNION ALL SELECT 'model'
             UNION ALL SELECT 'project'
           ) AS dimension
         ),
         ranked AS (
           SELECT expanded.*,
                  row_number() OVER (
                    PARTITION BY provider_id, profile_id, session_id,
                                 dimension_kind, dimension_value
                    ORDER BY timestamp_seconds, timestamp_nanos, fingerprint
                  ) AS first_rank,
                  row_number() OVER (
                    PARTITION BY provider_id, profile_id, session_id,
                                 dimension_kind, dimension_value
                    ORDER BY timestamp_seconds DESC, timestamp_nanos DESC,
                             fingerprint DESC
                  ) AS last_rank
           FROM expanded
         )
         INSERT INTO usage_session_rollup(
           aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
           dimension_kind, dimension_value, event_count,
           first_timestamp_seconds, first_timestamp_nanos,
           last_timestamp_seconds, last_timestamp_nanos,
           input_known_count, input_known_sum, cached_known_count, cached_known_sum,
           output_known_count, output_known_sum,
           reasoning_known_count, reasoning_known_sum,
           total_known_count, total_known_sum, fallback_model_count,
           long_context_yes_count, long_context_no_count,
           long_context_unavailable_count, activity_read, activity_edit_write,
           activity_search, activity_git, activity_build_test, activity_web,
           activity_subagents, activity_terminal
         )
         SELECT ?1, '{dataset_kind}', provider_id, profile_id, session_id,
                dimension_kind, dimension_value, count(*),
                CASE WHEN dimension_kind = 'all'
                     THEN max(CASE WHEN first_rank = 1 THEN timestamp_seconds END) END,
                CASE WHEN dimension_kind = 'all'
                     THEN max(CASE WHEN first_rank = 1 THEN timestamp_nanos END) END,
                CASE WHEN dimension_kind = 'all'
                     THEN max(CASE WHEN last_rank = 1 THEN timestamp_seconds END) END,
                CASE WHEN dimension_kind = 'all'
                     THEN max(CASE WHEN last_rank = 1 THEN timestamp_nanos END) END,
                count(input_tokens), coalesce(sum(input_tokens), 0),
                count(cached_tokens), coalesce(sum(cached_tokens), 0),
                count(output_tokens), coalesce(sum(output_tokens), 0),
                count(reasoning_tokens), coalesce(sum(reasoning_tokens), 0),
                count(total_tokens), coalesce(sum(total_tokens), 0),
                sum(fallback_model),
                sum(CASE WHEN long_context = 'yes' THEN 1 ELSE 0 END),
                sum(CASE WHEN long_context = 'no' THEN 1 ELSE 0 END),
                sum(CASE WHEN long_context = 'unavailable' THEN 1 ELSE 0 END),
                sum(activity_read), sum(activity_edit_write), sum(activity_search),
                sum(activity_git), sum(activity_build_test), sum(activity_web),
                sum(activity_subagents), sum(activity_terminal)
         FROM ranked
         GROUP BY provider_id, profile_id, session_id, dimension_kind, dimension_value
         ON CONFLICT(
           aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
           dimension_kind, dimension_value
         ) DO UPDATE SET
           first_timestamp_seconds = CASE
             WHEN usage_session_rollup.dimension_kind = 'all'
              AND (excluded.first_timestamp_seconds, excluded.first_timestamp_nanos)
                  < (usage_session_rollup.first_timestamp_seconds,
                     usage_session_rollup.first_timestamp_nanos)
             THEN excluded.first_timestamp_seconds
             ELSE usage_session_rollup.first_timestamp_seconds END,
           first_timestamp_nanos = CASE
             WHEN usage_session_rollup.dimension_kind = 'all'
              AND (excluded.first_timestamp_seconds, excluded.first_timestamp_nanos)
                  < (usage_session_rollup.first_timestamp_seconds,
                     usage_session_rollup.first_timestamp_nanos)
             THEN excluded.first_timestamp_nanos
             ELSE usage_session_rollup.first_timestamp_nanos END,
           last_timestamp_seconds = CASE
             WHEN usage_session_rollup.dimension_kind = 'all'
              AND (excluded.last_timestamp_seconds, excluded.last_timestamp_nanos)
                  > (usage_session_rollup.last_timestamp_seconds,
                     usage_session_rollup.last_timestamp_nanos)
             THEN excluded.last_timestamp_seconds
             ELSE usage_session_rollup.last_timestamp_seconds END,
           last_timestamp_nanos = CASE
             WHEN usage_session_rollup.dimension_kind = 'all'
              AND (excluded.last_timestamp_seconds, excluded.last_timestamp_nanos)
                  > (usage_session_rollup.last_timestamp_seconds,
                     usage_session_rollup.last_timestamp_nanos)
             THEN excluded.last_timestamp_nanos
             ELSE usage_session_rollup.last_timestamp_nanos END,
           {METRIC_UPSERT}"
    )
}

const PAGE_CALCULABLE: &str = "input_tokens IS NOT NULL AND cached_tokens IS NOT NULL AND cached_tokens <= input_tokens AND ((total_tokens IS NOT NULL AND total_tokens >= input_tokens AND (output_tokens IS NULL OR reasoning_tokens IS NULL OR (output_tokens <= total_tokens - input_tokens AND reasoning_tokens = total_tokens - input_tokens - output_tokens))) OR (total_tokens IS NULL AND output_tokens IS NOT NULL AND reasoning_tokens IS NOT NULL AND output_tokens <= 9223372036854775807 - reasoning_tokens))";

fn price_time_page_sql(dataset_kind: &str, page_sql: &str) -> String {
    format!(
        "WITH page AS ({page_sql}),
         normalized AS (
           SELECT page.*,
                  CASE WHEN service_tier IS NULL THEN 'standard_assumed'
                       WHEN lower(service_tier) IN ('standard','default')
                         THEN 'standard_reported'
                       WHEN lower(service_tier) IN ('priority','fast') THEN 'priority'
                       ELSE 'unknown' END AS price_tier,
                  CASE WHEN reported_cost_usd_micros IS NULL
                       THEN 'missing' ELSE 'present' END AS reported_state,
                  CASE WHEN {PAGE_CALCULABLE} THEN 1 ELSE 0 END AS calculable,
                  CASE WHEN {PAGE_CALCULABLE}
                       THEN input_tokens - cached_tokens ELSE 0 END AS uncached_input,
                  CASE WHEN {PAGE_CALCULABLE} THEN cached_tokens ELSE 0 END AS cached_input,
                  CASE WHEN {PAGE_CALCULABLE}
                       THEN CASE WHEN total_tokens IS NOT NULL
                                 THEN total_tokens - input_tokens
                                 ELSE output_tokens + reasoning_tokens END
                       ELSE 0 END AS billable_output
           FROM page
         ),
         expanded AS (
           SELECT normalized.*, bucket.width AS bucket_width,
                  timestamp_seconds -
                    (((timestamp_seconds % bucket.seconds) + bucket.seconds)
                     % bucket.seconds) AS bucket_start_seconds
           FROM normalized
           CROSS JOIN (
             SELECT 'minute' AS width, 60 AS seconds
             UNION ALL SELECT 'hour', 3600
           ) AS bucket
         )
         INSERT INTO usage_price_time_rollup(
           aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
           provider_id, profile_id, model, project_key, service_tier, long_context,
           reported_state,
           event_count, calculable_event_count, uncached_input_sum, cached_input_sum,
           billable_output_sum, reported_cost_count, reported_cost_sum
         )
         SELECT ?1, '{dataset_kind}', bucket_width, bucket_start_seconds,
                provider_id, profile_id, model, coalesce(project_alias, ''), price_tier,
                long_context, reported_state,
                count(*), sum(calculable), sum(uncached_input), sum(cached_input),
                sum(billable_output), count(reported_cost_usd_micros),
                coalesce(sum(reported_cost_usd_micros), 0)
         FROM expanded
         GROUP BY bucket_width, bucket_start_seconds, provider_id, profile_id,
                  model, project_alias, price_tier, long_context, reported_state
         ON CONFLICT(
           aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
           provider_id, profile_id, model, project_key, service_tier, long_context,
           reported_state
         ) DO UPDATE SET
           event_count = event_count + excluded.event_count,
           calculable_event_count =
             calculable_event_count + excluded.calculable_event_count,
           uncached_input_sum = uncached_input_sum + excluded.uncached_input_sum,
           cached_input_sum = cached_input_sum + excluded.cached_input_sum,
           billable_output_sum = billable_output_sum + excluded.billable_output_sum,
           reported_cost_count = reported_cost_count + excluded.reported_cost_count,
           reported_cost_sum = reported_cost_sum + excluded.reported_cost_sum"
    )
}

fn price_session_page_sql(dataset_kind: &str, page_sql: &str) -> String {
    format!(
        "WITH page AS ({page_sql}),
         normalized AS (
           SELECT page.*,
                  CASE WHEN service_tier IS NULL THEN 'standard_assumed'
                       WHEN lower(service_tier) IN ('standard','default')
                         THEN 'standard_reported'
                       WHEN lower(service_tier) IN ('priority','fast') THEN 'priority'
                       ELSE 'unknown' END AS price_tier,
                  CASE WHEN reported_cost_usd_micros IS NULL
                       THEN 'missing' ELSE 'present' END AS reported_state,
                  CASE WHEN {PAGE_CALCULABLE} THEN 1 ELSE 0 END AS calculable,
                  CASE WHEN {PAGE_CALCULABLE}
                       THEN input_tokens - cached_tokens ELSE 0 END AS uncached_input,
                  CASE WHEN {PAGE_CALCULABLE} THEN cached_tokens ELSE 0 END AS cached_input,
                  CASE WHEN {PAGE_CALCULABLE}
                       THEN CASE WHEN total_tokens IS NOT NULL
                                 THEN total_tokens - input_tokens
                                 ELSE output_tokens + reasoning_tokens END
                       ELSE 0 END AS billable_output
           FROM page
         )
         INSERT INTO usage_price_session_rollup(
           aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
           model, project_key, service_tier, long_context, reported_state,
           event_count, calculable_event_count, uncached_input_sum, cached_input_sum,
           billable_output_sum, reported_cost_count, reported_cost_sum
         )
         SELECT ?1, '{dataset_kind}', provider_id, profile_id, session_id,
                model, coalesce(project_alias, ''), price_tier, long_context, reported_state,
                count(*), sum(calculable), sum(uncached_input), sum(cached_input),
                sum(billable_output), count(reported_cost_usd_micros),
                coalesce(sum(reported_cost_usd_micros), 0)
         FROM normalized
         GROUP BY provider_id, profile_id, session_id, model, project_alias, price_tier,
                  long_context, reported_state
         ON CONFLICT(
           aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
           model, project_key, service_tier, long_context, reported_state
         ) DO UPDATE SET
           event_count = event_count + excluded.event_count,
           calculable_event_count =
             calculable_event_count + excluded.calculable_event_count,
           uncached_input_sum = uncached_input_sum + excluded.uncached_input_sum,
           cached_input_sum = cached_input_sum + excluded.cached_input_sum,
           billable_output_sum = billable_output_sum + excluded.billable_output_sum,
           reported_cost_count = reported_cost_count + excluded.reported_cost_count,
           reported_cost_sum = reported_cost_sum + excluded.reported_cost_sum"
    )
}

struct RebuildState {
    state: String,
    archive_generation: i64,
    expected_generation: i64,
    active_generation: i64,
    rebuild_generation: Option<i64>,
    dataset_kind: Option<String>,
    cursor: Option<Vec<u8>>,
    processed_events: i64,
    total_events: i64,
    current_events: i64,
    legacy_events: i64,
}

impl UsageStore {
    pub fn rebuild_aggregates_page(
        &mut self,
        max_events: usize,
    ) -> Result<AggregateRebuildProgress, StoreError> {
        self.rebuild_aggregates_page_inner(max_events, AggregateFault::None)
    }

    fn rebuild_aggregates_page_inner(
        &mut self,
        max_events: usize,
        fault: AggregateFault,
    ) -> Result<AggregateRebuildProgress, StoreError> {
        if max_events == 0 || max_events > MAX_AGGREGATE_REBUILD_PAGE_SIZE {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_AGGREGATE_REBUILD_PAGE_SIZE as u64,
            ));
        }
        let limit = i64::try_from(max_events)
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let cleanup_limit = limit
            .checked_mul(12)
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut state = load_state(&transaction)?;
        if state.state == "ready" {
            let result = progress(
                AggregateRebuildStatus::Ready,
                state.total_events,
                state.total_events,
            )?;
            transaction.commit()?;
            return Ok(result);
        }

        let mut restarted = false;
        if state.state == "rebuilding" && state.archive_generation != state.expected_generation {
            reset_rebuild(
                &transaction,
                state.archive_generation,
                state.current_events,
                state.legacy_events,
            )?;
            state = load_state(&transaction)?;
            restarted = true;
        }
        if matches!(state.state.as_str(), "rebuild_required" | "failed") {
            let target_generation = if state.active_generation == i64::MAX {
                0
            } else {
                state.active_generation + 1
            };
            transaction.execute(
                "UPDATE usage_aggregate_state
                 SET state = 'rebuilding', failure_code = NULL,
                     expected_dataset_generation = ?1,
                     rebuild_aggregate_generation = ?2,
                     rebuild_dataset_kind = 'cleanup',
                     rebuild_cursor_fingerprint = NULL,
                     rebuild_processed_events = 0,
                     rebuild_total_events = current_event_count + legacy_event_count
                 WHERE singleton_id = 1",
                params![state.archive_generation, target_generation],
            )?;
            state = load_state(&transaction)?;
            aggregate_fault(fault, AggregateBoundary::StateInitialized)?;
        }
        if state.state != "rebuilding" {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        let rebuild_generation = state
            .rebuild_generation
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        match state.dataset_kind.as_deref() {
            Some("cleanup") => {
                let removed_time = transaction.execute(
                    "DELETE FROM usage_time_rollup WHERE rowid IN (
                       SELECT rowid FROM usage_time_rollup ORDER BY rowid LIMIT ?1
                     )",
                    [cleanup_limit],
                )?;
                let removed_session = transaction.execute(
                    "DELETE FROM usage_session_rollup WHERE rowid IN (
                       SELECT rowid FROM usage_session_rollup ORDER BY rowid LIMIT ?1
                     )",
                    [cleanup_limit],
                )?;
                let removed_price_time = transaction.execute(
                    "DELETE FROM usage_price_time_rollup WHERE rowid IN (
                       SELECT rowid FROM usage_price_time_rollup ORDER BY rowid LIMIT ?1
                     )",
                    [cleanup_limit],
                )?;
                let removed_price_session = transaction.execute(
                    "DELETE FROM usage_price_session_rollup WHERE rowid IN (
                       SELECT rowid FROM usage_price_session_rollup ORDER BY rowid LIMIT ?1
                     )",
                    [cleanup_limit],
                )?;
                if removed_time == 0
                    && removed_session == 0
                    && removed_price_time == 0
                    && removed_price_session == 0
                {
                    if state.total_events == 0 {
                        aggregate_fault(fault, AggregateBoundary::Publish)?;
                        finish_rebuild(&transaction, rebuild_generation, 0)?;
                        transaction.commit()?;
                        return progress(AggregateRebuildStatus::Ready, 0, 0);
                    }
                    let first_dataset = if state.current_events == 0 {
                        "legacy"
                    } else {
                        "current"
                    };
                    transaction.execute(
                        "UPDATE usage_aggregate_state
                         SET rebuild_dataset_kind = ?1,
                             rebuild_cursor_fingerprint = NULL
                         WHERE singleton_id = 1",
                        [first_dataset],
                    )?;
                }
                let result = progress(
                    if restarted {
                        AggregateRebuildStatus::Restarted
                    } else {
                        AggregateRebuildStatus::Rebuilding
                    },
                    0,
                    state.total_events,
                )?;
                transaction.commit()?;
                Ok(result)
            }
            Some("current" | "legacy") => {
                let dataset_kind = state
                    .dataset_kind
                    .as_deref()
                    .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
                let cursor = state.cursor.as_deref();
                let (page_count, last_fingerprint) =
                    page_bounds(&transaction, dataset_kind, cursor, limit)?;
                if page_count > 0 {
                    materialize_page(
                        &transaction,
                        dataset_kind,
                        rebuild_generation,
                        cursor,
                        limit,
                    )?;
                    aggregate_fault(fault, AggregateBoundary::PageMaterialized)?;
                }
                let processed = state
                    .processed_events
                    .checked_add(page_count)
                    .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
                if processed > state.total_events {
                    return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
                }
                let dataset_complete = page_count < limit;
                if dataset_complete && dataset_kind == "current" && state.legacy_events > 0 {
                    transaction.execute(
                        "UPDATE usage_aggregate_state
                         SET rebuild_dataset_kind = 'legacy',
                             rebuild_cursor_fingerprint = NULL,
                             rebuild_processed_events = ?1
                         WHERE singleton_id = 1",
                        [processed],
                    )?;
                } else if dataset_complete {
                    if processed != state.total_events {
                        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
                    }
                    aggregate_fault(fault, AggregateBoundary::Publish)?;
                    finish_rebuild(&transaction, rebuild_generation, processed)?;
                    transaction.commit()?;
                    return progress(AggregateRebuildStatus::Ready, processed, state.total_events);
                } else {
                    let last_fingerprint = last_fingerprint
                        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
                    transaction.execute(
                        "UPDATE usage_aggregate_state
                         SET rebuild_cursor_fingerprint = ?1,
                             rebuild_processed_events = ?2
                         WHERE singleton_id = 1",
                        params![last_fingerprint, processed],
                    )?;
                }
                let result = progress(
                    if restarted {
                        AggregateRebuildStatus::Restarted
                    } else {
                        AggregateRebuildStatus::Rebuilding
                    },
                    processed,
                    state.total_events,
                )?;
                transaction.commit()?;
                Ok(result)
            }
            _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        }
    }
}

fn aggregate_fault(fault: AggregateFault, boundary: AggregateBoundary) -> Result<(), StoreError> {
    #[cfg(test)]
    let triggered = matches!(
        (fault, boundary),
        (
            AggregateFault::AfterStateInitialized,
            AggregateBoundary::StateInitialized
        ) | (
            AggregateFault::AfterPageMaterialized,
            AggregateBoundary::PageMaterialized
        ) | (AggregateFault::BeforePublish, AggregateBoundary::Publish)
    );
    #[cfg(not(test))]
    let triggered = {
        let _ = (fault, boundary);
        false
    };
    if triggered {
        Err(StoreError::new(StoreErrorCode::Database))
    } else {
        Ok(())
    }
}

fn load_state(transaction: &Transaction<'_>) -> Result<RebuildState, StoreError> {
    Ok(transaction.query_row(
        "SELECT aggregate.state, archive.dataset_generation,
                aggregate.expected_dataset_generation,
                aggregate.active_aggregate_generation,
                aggregate.rebuild_aggregate_generation,
                aggregate.rebuild_dataset_kind,
                aggregate.rebuild_cursor_fingerprint,
                aggregate.rebuild_processed_events,
                aggregate.rebuild_total_events,
                aggregate.current_event_count, aggregate.legacy_event_count
         FROM usage_aggregate_state AS aggregate
         JOIN usage_archive_state AS archive ON archive.singleton_id = 1
         WHERE aggregate.singleton_id = 1",
        [],
        |row| {
            Ok(RebuildState {
                state: row.get(0)?,
                archive_generation: row.get(1)?,
                expected_generation: row.get(2)?,
                active_generation: row.get(3)?,
                rebuild_generation: row.get(4)?,
                dataset_kind: row.get(5)?,
                cursor: row.get(6)?,
                processed_events: row.get(7)?,
                total_events: row.get(8)?,
                current_events: row.get(9)?,
                legacy_events: row.get(10)?,
            })
        },
    )?)
}

fn reset_rebuild(
    transaction: &Transaction<'_>,
    dataset_generation: i64,
    current_events: i64,
    legacy_events: i64,
) -> Result<(), StoreError> {
    let total = current_events
        .checked_add(legacy_events)
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    transaction.execute(
        "UPDATE usage_aggregate_state
         SET state = 'rebuild_required', failure_code = NULL,
             expected_dataset_generation = ?1,
             rebuild_aggregate_generation = NULL,
             rebuild_dataset_kind = NULL, rebuild_cursor_fingerprint = NULL,
             rebuild_processed_events = 0, rebuild_total_events = ?2
         WHERE singleton_id = 1",
        params![dataset_generation, total],
    )?;
    Ok(())
}

fn page_bounds(
    transaction: &Transaction<'_>,
    dataset_kind: &str,
    cursor: Option<&[u8]>,
    limit: i64,
) -> Result<(i64, Option<Vec<u8>>), StoreError> {
    let table = match dataset_kind {
        "current" => "usage_event",
        "legacy" => "usage_legacy_event",
        _ => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    };
    let legacy_filter = if dataset_kind == "legacy" {
        "snapshot_id = 1 AND"
    } else {
        ""
    };
    let sql = format!(
        "SELECT count(*), max(fingerprint) FROM (
           SELECT fingerprint FROM {table}
           WHERE {legacy_filter} (?1 IS NULL OR fingerprint > ?1)
           ORDER BY fingerprint LIMIT ?2
         )"
    );
    Ok(transaction.query_row(&sql, params![cursor, limit], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?)
}

fn materialize_page(
    transaction: &Transaction<'_>,
    dataset_kind: &str,
    aggregate_generation: i64,
    cursor: Option<&[u8]>,
    limit: i64,
) -> Result<(), StoreError> {
    let page_sql = match dataset_kind {
        "current" => CURRENT_PAGE,
        "legacy" => LEGACY_PAGE,
        _ => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    };
    transaction.execute(
        &time_page_sql(dataset_kind, page_sql),
        params![aggregate_generation, cursor, limit],
    )?;
    transaction.execute(
        &session_page_sql(dataset_kind, page_sql),
        params![aggregate_generation, cursor, limit],
    )?;
    transaction.execute(
        &price_time_page_sql(dataset_kind, page_sql),
        params![aggregate_generation, cursor, limit],
    )?;
    transaction.execute(
        &price_session_page_sql(dataset_kind, page_sql),
        params![aggregate_generation, cursor, limit],
    )?;
    Ok(())
}

fn finish_rebuild(
    transaction: &Transaction<'_>,
    aggregate_generation: i64,
    processed_events: i64,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE usage_aggregate_state
         SET state = 'ready', failure_code = NULL,
             active_aggregate_generation = ?1,
             rebuild_aggregate_generation = NULL,
             rebuild_dataset_kind = NULL, rebuild_cursor_fingerprint = NULL,
             rebuild_processed_events = 0
         WHERE singleton_id = 1 AND state = 'rebuilding'
           AND rebuild_aggregate_generation = ?1
           AND expected_dataset_generation = (
             SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1
           ) AND rebuild_total_events = ?2",
        params![aggregate_generation, processed_events],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    Ok(())
}

fn progress(
    status: AggregateRebuildStatus,
    processed_events: i64,
    total_events: i64,
) -> Result<AggregateRebuildProgress, StoreError> {
    Ok(AggregateRebuildProgress::new(
        status,
        u64::try_from(processed_events)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        u64::try_from(total_events)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn rebuild_store() -> TestResult<UsageStore> {
        let store = UsageStore::in_memory()?;
        store.connection.execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               fallback_model, long_context, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             ) VALUES (
               zeroblob(32), 'event', zeroblob(32), 0, 0,
               'codex', 'default', 'session', 'source', 1, 0, 'model', 1,
               0, 'no', 0, 0, 0, 0, 0, 0, 0, 0
             )",
            [],
        )?;
        store.connection.execute_batch(
            "UPDATE usage_aggregate_state
             SET state = 'rebuild_required', rebuild_aggregate_generation = NULL,
                 rebuild_dataset_kind = NULL, rebuild_cursor_fingerprint = NULL,
                 rebuild_processed_events = 0,
                 rebuild_total_events = current_event_count + legacy_event_count
             WHERE singleton_id = 1;
             DELETE FROM usage_time_rollup;
             DELETE FROM usage_session_rollup;
             DELETE FROM usage_price_time_rollup;
             DELETE FROM usage_price_session_rollup;",
        )?;
        Ok(store)
    }

    fn rebuild_state(store: &UsageStore) -> TestResult<(String, i64, i64, i64, i64, i64)> {
        Ok(store.connection.query_row(
            "SELECT state, rebuild_processed_events,
                    (SELECT count(*) FROM usage_time_rollup),
                    (SELECT count(*) FROM usage_session_rollup),
                    (SELECT count(*) FROM usage_price_time_rollup),
                    (SELECT count(*) FROM usage_price_session_rollup)
             FROM usage_aggregate_state WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?)
    }

    #[test]
    fn initialization_fault_rolls_back_state_and_rows() -> TestResult {
        let mut store = rebuild_store()?;
        let before = rebuild_state(&store)?;
        let error =
            match store.rebuild_aggregates_page_inner(2, AggregateFault::AfterStateInitialized) {
                Ok(_) => return Err("faulted initialization unexpectedly committed".into()),
                Err(error) => error,
            };
        assert_eq!(error.code(), StoreErrorCode::Database);
        assert_eq!(rebuild_state(&store)?, before);
        Ok(())
    }

    #[test]
    fn page_and_publish_faults_leave_exact_resumable_state() -> TestResult {
        for fault in [
            AggregateFault::AfterPageMaterialized,
            AggregateFault::BeforePublish,
        ] {
            let mut store = rebuild_store()?;
            let cleanup = store.rebuild_aggregates_page(2)?;
            assert_eq!(cleanup.status(), AggregateRebuildStatus::Rebuilding);
            let before = rebuild_state(&store)?;
            let error = match store.rebuild_aggregates_page_inner(2, fault) {
                Ok(_) => return Err("faulted rebuild page unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(rebuild_state(&store)?, before);
            let complete = store.rebuild_aggregates_page(2)?;
            assert_eq!(complete.status(), AggregateRebuildStatus::Ready);
            assert_eq!(complete.processed_events(), 1);
        }
        Ok(())
    }
}
