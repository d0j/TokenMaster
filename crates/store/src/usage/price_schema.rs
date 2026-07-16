use std::sync::OnceLock;

pub(super) const V9_PRICE_ROLLUP_SCHEMA: &str = r#"
CREATE TABLE usage_price_time_rollup (
  aggregate_generation INTEGER NOT NULL CHECK(aggregate_generation >= 0),
  dataset_kind TEXT NOT NULL CHECK(dataset_kind IN ('current','legacy')),
  bucket_width TEXT NOT NULL CHECK(bucket_width IN ('minute','hour')),
  bucket_start_seconds INTEGER NOT NULL,
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  model TEXT NOT NULL CHECK(length(CAST(model AS BLOB)) BETWEEN 1 AND 64),
  project_key TEXT NOT NULL CHECK(length(CAST(project_key AS BLOB)) <= 512),
  service_tier TEXT NOT NULL CHECK(service_tier IN ('standard_reported','standard_assumed','priority','unknown')),
  long_context TEXT NOT NULL CHECK(long_context IN ('yes','no','unavailable')),
  reported_state TEXT NOT NULL CHECK(reported_state IN ('present','missing')),
  event_count INTEGER NOT NULL CHECK(event_count > 0),
  calculable_event_count INTEGER NOT NULL CHECK(calculable_event_count BETWEEN 0 AND event_count),
  uncached_input_sum INTEGER NOT NULL CHECK(uncached_input_sum >= 0),
  cached_input_sum INTEGER NOT NULL CHECK(cached_input_sum >= 0),
  billable_output_sum INTEGER NOT NULL CHECK(billable_output_sum >= 0),
  reported_cost_count INTEGER NOT NULL CHECK(reported_cost_count BETWEEN 0 AND event_count),
  reported_cost_sum INTEGER NOT NULL CHECK(reported_cost_sum >= 0),
  PRIMARY KEY(aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
              provider_id, profile_id, model, project_key, service_tier, long_context,
              reported_state),
  CHECK((bucket_width = 'minute' AND bucket_start_seconds % 60 = 0)
     OR (bucket_width = 'hour' AND bucket_start_seconds % 3600 = 0)),
  CHECK(calculable_event_count > 0
     OR (uncached_input_sum = 0 AND cached_input_sum = 0 AND billable_output_sum = 0)),
  CHECK((reported_state = 'present' AND reported_cost_count = event_count)
     OR (reported_state = 'missing' AND reported_cost_count = 0 AND reported_cost_sum = 0))
) STRICT;

CREATE TABLE usage_price_session_rollup (
  aggregate_generation INTEGER NOT NULL CHECK(aggregate_generation >= 0),
  dataset_kind TEXT NOT NULL CHECK(dataset_kind IN ('current','legacy')),
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  session_id TEXT NOT NULL CHECK(length(CAST(session_id AS BLOB)) BETWEEN 1 AND 512),
  model TEXT NOT NULL CHECK(length(CAST(model AS BLOB)) BETWEEN 1 AND 64),
  project_key TEXT NOT NULL CHECK(length(CAST(project_key AS BLOB)) <= 512),
  service_tier TEXT NOT NULL CHECK(service_tier IN ('standard_reported','standard_assumed','priority','unknown')),
  long_context TEXT NOT NULL CHECK(long_context IN ('yes','no','unavailable')),
  reported_state TEXT NOT NULL CHECK(reported_state IN ('present','missing')),
  event_count INTEGER NOT NULL CHECK(event_count > 0),
  calculable_event_count INTEGER NOT NULL CHECK(calculable_event_count BETWEEN 0 AND event_count),
  uncached_input_sum INTEGER NOT NULL CHECK(uncached_input_sum >= 0),
  cached_input_sum INTEGER NOT NULL CHECK(cached_input_sum >= 0),
  billable_output_sum INTEGER NOT NULL CHECK(billable_output_sum >= 0),
  reported_cost_count INTEGER NOT NULL CHECK(reported_cost_count BETWEEN 0 AND event_count),
  reported_cost_sum INTEGER NOT NULL CHECK(reported_cost_sum >= 0),
  PRIMARY KEY(aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
              model, project_key, service_tier, long_context, reported_state),
  CHECK(calculable_event_count > 0
     OR (uncached_input_sum = 0 AND cached_input_sum = 0 AND billable_output_sum = 0)),
  CHECK((reported_state = 'present' AND reported_cost_count = event_count)
     OR (reported_state = 'missing' AND reported_cost_count = 0 AND reported_cost_sum = 0))
) STRICT;

CREATE INDEX usage_price_time_scope_range
  ON usage_price_time_rollup(aggregate_generation, dataset_kind, provider_id, profile_id,
                             bucket_width, bucket_start_seconds, project_key, model,
                             service_tier, long_context, reported_state);
CREATE INDEX usage_price_session_scope
  ON usage_price_session_rollup(aggregate_generation, dataset_kind, provider_id, profile_id,
                                session_id, project_key, model, service_tier, long_context,
                                reported_state);
"#;

const NEW_TIER: &str = "CASE WHEN NEW.service_tier IS NULL THEN 'standard_assumed' WHEN lower(NEW.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(NEW.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END";
const OLD_TIER: &str = "CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END";
const NEW_CALCULABLE: &str = "NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens))";
const OLD_CALCULABLE: &str = "OLD.input_tokens IS NOT NULL AND OLD.cached_tokens IS NOT NULL AND OLD.cached_tokens <= OLD.input_tokens AND ((OLD.total_tokens IS NOT NULL AND OLD.total_tokens >= OLD.input_tokens AND (OLD.output_tokens IS NULL OR OLD.reasoning_tokens IS NULL OR (OLD.output_tokens <= OLD.total_tokens - OLD.input_tokens AND OLD.reasoning_tokens = OLD.total_tokens - OLD.input_tokens - OLD.output_tokens))) OR (OLD.total_tokens IS NULL AND OLD.output_tokens IS NOT NULL AND OLD.reasoning_tokens IS NOT NULL AND OLD.output_tokens <= 9223372036854775807 - OLD.reasoning_tokens))";

pub(super) fn price_time_insert_trigger() -> Option<&'static str> {
    static TRIGGER: OnceLock<String> = OnceLock::new();
    Some(
        TRIGGER
            .get_or_init(|| {
                format!(
                    r#"CREATE TRIGGER usage_event_price_time_after_insert
AFTER INSERT ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  INSERT INTO usage_price_time_rollup(
    aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
    provider_id, profile_id, model, project_key, service_tier, long_context, reported_state,
    event_count, calculable_event_count, uncached_input_sum, cached_input_sum,
    billable_output_sum, reported_cost_count, reported_cost_sum
  )
  SELECT
    (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1),
    'current', bucket.width,
    NEW.timestamp_seconds - (((NEW.timestamp_seconds % bucket.seconds) + bucket.seconds) % bucket.seconds),
    NEW.provider_id, NEW.profile_id, NEW.model, coalesce(NEW.project_alias, ''),
    {NEW_TIER}, NEW.long_context,
    CASE WHEN NEW.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END,
    1, CASE WHEN {NEW_CALCULABLE} THEN 1 ELSE 0 END,
    CASE WHEN {NEW_CALCULABLE} THEN NEW.input_tokens - NEW.cached_tokens ELSE 0 END,
    CASE WHEN {NEW_CALCULABLE} THEN NEW.cached_tokens ELSE 0 END,
    CASE WHEN {NEW_CALCULABLE} THEN
      CASE WHEN NEW.total_tokens IS NOT NULL THEN NEW.total_tokens - NEW.input_tokens
           ELSE NEW.output_tokens + NEW.reasoning_tokens END
      ELSE 0 END,
    CASE WHEN NEW.reported_cost_usd_micros IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.reported_cost_usd_micros, 0)
  FROM (SELECT 'minute' AS width, 60 AS seconds UNION ALL SELECT 'hour', 3600) AS bucket
  WHERE true
  ON CONFLICT(aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
              provider_id, profile_id, model, project_key, service_tier, long_context,
              reported_state)
  DO UPDATE SET
    event_count = event_count + 1,
    calculable_event_count = calculable_event_count + excluded.calculable_event_count,
    uncached_input_sum = uncached_input_sum + excluded.uncached_input_sum,
    cached_input_sum = cached_input_sum + excluded.cached_input_sum,
    billable_output_sum = billable_output_sum + excluded.billable_output_sum,
    reported_cost_count = reported_cost_count + excluded.reported_cost_count,
    reported_cost_sum = reported_cost_sum + excluded.reported_cost_sum;
END;
"#
                )
            })
            .as_str(),
    )
}

pub(super) fn price_session_insert_trigger() -> Option<&'static str> {
    static TRIGGER: OnceLock<String> = OnceLock::new();
    Some(
        TRIGGER
            .get_or_init(|| {
                format!(
                    r#"CREATE TRIGGER usage_event_price_session_after_insert
AFTER INSERT ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  INSERT INTO usage_price_session_rollup(
    aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
    model, project_key, service_tier, long_context, reported_state,
    event_count, calculable_event_count, uncached_input_sum, cached_input_sum,
    billable_output_sum, reported_cost_count, reported_cost_sum
  ) VALUES (
    (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1),
    'current', NEW.provider_id, NEW.profile_id, NEW.session_id,
    NEW.model, coalesce(NEW.project_alias, ''), {NEW_TIER}, NEW.long_context,
    CASE WHEN NEW.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END,
    1, CASE WHEN {NEW_CALCULABLE} THEN 1 ELSE 0 END,
    CASE WHEN {NEW_CALCULABLE} THEN NEW.input_tokens - NEW.cached_tokens ELSE 0 END,
    CASE WHEN {NEW_CALCULABLE} THEN NEW.cached_tokens ELSE 0 END,
    CASE WHEN {NEW_CALCULABLE} THEN
      CASE WHEN NEW.total_tokens IS NOT NULL THEN NEW.total_tokens - NEW.input_tokens
           ELSE NEW.output_tokens + NEW.reasoning_tokens END
      ELSE 0 END,
    CASE WHEN NEW.reported_cost_usd_micros IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.reported_cost_usd_micros, 0)
  )
  ON CONFLICT(aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
              model, project_key, service_tier, long_context, reported_state)
  DO UPDATE SET
    event_count = event_count + 1,
    calculable_event_count = calculable_event_count + excluded.calculable_event_count,
    uncached_input_sum = uncached_input_sum + excluded.uncached_input_sum,
    cached_input_sum = cached_input_sum + excluded.cached_input_sum,
    billable_output_sum = billable_output_sum + excluded.billable_output_sum,
    reported_cost_count = reported_cost_count + excluded.reported_cost_count,
    reported_cost_sum = reported_cost_sum + excluded.reported_cost_sum;
END;
"#
                )
            })
            .as_str(),
    )
}

pub(super) fn price_time_delete_trigger() -> Option<&'static str> {
    static TRIGGER: OnceLock<String> = OnceLock::new();
    Some(
        TRIGGER
            .get_or_init(|| {
                format!(
                    r#"CREATE TRIGGER usage_event_price_time_after_delete
AFTER DELETE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  SELECT CASE WHEN (
    SELECT count(*) FROM usage_price_time_rollup
    WHERE aggregate_generation =
          (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
      AND dataset_kind = 'current' AND provider_id = OLD.provider_id
      AND profile_id = OLD.profile_id AND model = OLD.model
      AND project_key = coalesce(OLD.project_alias, '')
      AND service_tier = {OLD_TIER} AND long_context = OLD.long_context
      AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
      AND ((bucket_width = 'minute' AND bucket_start_seconds =
            OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60))
        OR (bucket_width = 'hour' AND bucket_start_seconds =
            OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)))
  ) <> 2 THEN RAISE(ABORT, 'price time rows unavailable') END;
  DELETE FROM usage_price_time_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND provider_id = OLD.provider_id
    AND profile_id = OLD.profile_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = {OLD_TIER} AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1
    AND ((bucket_width = 'minute' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60))
      OR (bucket_width = 'hour' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)));
  UPDATE usage_price_time_rollup
  SET event_count = event_count - 1,
      calculable_event_count = calculable_event_count - CASE WHEN {OLD_CALCULABLE} THEN 1 ELSE 0 END,
      uncached_input_sum = uncached_input_sum - CASE WHEN {OLD_CALCULABLE} THEN OLD.input_tokens - OLD.cached_tokens ELSE 0 END,
      cached_input_sum = cached_input_sum - CASE WHEN {OLD_CALCULABLE} THEN OLD.cached_tokens ELSE 0 END,
      billable_output_sum = billable_output_sum - CASE WHEN {OLD_CALCULABLE} THEN CASE WHEN OLD.total_tokens IS NOT NULL THEN OLD.total_tokens - OLD.input_tokens ELSE OLD.output_tokens + OLD.reasoning_tokens END ELSE 0 END,
      reported_cost_count = reported_cost_count - CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 0 ELSE 1 END,
      reported_cost_sum = reported_cost_sum - coalesce(OLD.reported_cost_usd_micros, 0)
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND provider_id = OLD.provider_id
    AND profile_id = OLD.profile_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = {OLD_TIER} AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1
    AND ((bucket_width = 'minute' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60))
      OR (bucket_width = 'hour' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)));
END;
"#
                )
            })
            .as_str(),
    )
}

pub(super) fn price_session_delete_trigger() -> Option<&'static str> {
    static TRIGGER: OnceLock<String> = OnceLock::new();
    Some(
        TRIGGER
            .get_or_init(|| {
                format!(
                    r#"CREATE TRIGGER usage_event_price_session_after_delete
AFTER DELETE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  SELECT CASE WHEN (
    SELECT count(*) FROM usage_price_session_rollup
    WHERE aggregate_generation =
          (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
      AND dataset_kind = 'current' AND provider_id = OLD.provider_id
      AND profile_id = OLD.profile_id AND session_id = OLD.session_id AND model = OLD.model
      AND project_key = coalesce(OLD.project_alias, '')
      AND service_tier = {OLD_TIER} AND long_context = OLD.long_context
      AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
  ) <> 1 THEN RAISE(ABORT, 'price session row unavailable') END;
  DELETE FROM usage_price_session_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND provider_id = OLD.provider_id
    AND profile_id = OLD.profile_id AND session_id = OLD.session_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = {OLD_TIER} AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1;
  UPDATE usage_price_session_rollup
  SET event_count = event_count - 1,
      calculable_event_count = calculable_event_count - CASE WHEN {OLD_CALCULABLE} THEN 1 ELSE 0 END,
      uncached_input_sum = uncached_input_sum - CASE WHEN {OLD_CALCULABLE} THEN OLD.input_tokens - OLD.cached_tokens ELSE 0 END,
      cached_input_sum = cached_input_sum - CASE WHEN {OLD_CALCULABLE} THEN OLD.cached_tokens ELSE 0 END,
      billable_output_sum = billable_output_sum - CASE WHEN {OLD_CALCULABLE} THEN CASE WHEN OLD.total_tokens IS NOT NULL THEN OLD.total_tokens - OLD.input_tokens ELSE OLD.output_tokens + OLD.reasoning_tokens END ELSE 0 END,
      reported_cost_count = reported_cost_count - CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 0 ELSE 1 END,
      reported_cost_sum = reported_cost_sum - coalesce(OLD.reported_cost_usd_micros, 0)
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND provider_id = OLD.provider_id
    AND profile_id = OLD.profile_id AND session_id = OLD.session_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = {OLD_TIER} AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1;
END;
"#
                )
            })
            .as_str(),
    )
}

fn combine_update(name: &str, delete_trigger: &str, insert_trigger: &str) -> Option<String> {
    let delete_body = delete_trigger
        .split_once("\nBEGIN\n")?
        .1
        .strip_suffix("END;\n")?;
    let insert_body = insert_trigger
        .split_once("\nBEGIN\n")?
        .1
        .strip_suffix("END;\n")?;
    Some(format!(
        "CREATE TRIGGER {name}\nAFTER UPDATE ON usage_event\nWHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'\nBEGIN\n{delete_body}{insert_body}END;\n"
    ))
}

pub(super) fn price_time_update_trigger() -> Option<&'static str> {
    static TRIGGER: OnceLock<Option<String>> = OnceLock::new();
    TRIGGER
        .get_or_init(|| {
            combine_update(
                "usage_event_price_time_after_update",
                price_time_delete_trigger()?,
                price_time_insert_trigger()?,
            )
        })
        .as_deref()
}

pub(super) fn price_session_update_trigger() -> Option<&'static str> {
    static TRIGGER: OnceLock<Option<String>> = OnceLock::new();
    TRIGGER
        .get_or_init(|| {
            combine_update(
                "usage_event_price_session_after_update",
                price_session_delete_trigger()?,
                price_session_insert_trigger()?,
            )
        })
        .as_deref()
}
