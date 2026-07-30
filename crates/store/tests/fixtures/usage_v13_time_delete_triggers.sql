CREATE TRIGGER usage_event_aggregate_time_after_delete
AFTER DELETE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  SELECT CASE WHEN (
    SELECT count(*) FROM usage_time_rollup
    WHERE aggregate_generation =
          (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
      AND dataset_kind = 'current'
      AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
      AND (
        (bucket_width = 'minute' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60))
        OR
        (bucket_width = 'hour' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600))
      )
      AND (
        (dimension_kind = 'all' AND dimension_value = '')
        OR (dimension_kind = 'model' AND dimension_value = OLD.model)
        OR (dimension_kind = 'project'
            AND dimension_value = coalesce(OLD.project_alias, ''))
      )
  ) <> 6 THEN RAISE(ABORT, 'aggregate time rows unavailable') END;
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND (
      (bucket_width = 'minute' AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60))
      OR
      (bucket_width = 'hour' AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600))
    )
    AND (
      (dimension_kind = 'all' AND dimension_value = '')
      OR (dimension_kind = 'model' AND dimension_value = OLD.model)
      OR (dimension_kind = 'project'
          AND dimension_value = coalesce(OLD.project_alias, ''))
    );
  UPDATE usage_time_rollup
  SET event_count = event_count - 1,
      input_known_count = input_known_count -
        CASE WHEN OLD.input_tokens IS NULL THEN 0 ELSE 1 END,
      input_known_sum = input_known_sum - coalesce(OLD.input_tokens, 0),
      cached_known_count = cached_known_count -
        CASE WHEN OLD.cached_tokens IS NULL THEN 0 ELSE 1 END,
      cached_known_sum = cached_known_sum - coalesce(OLD.cached_tokens, 0),
      output_known_count = output_known_count -
        CASE WHEN OLD.output_tokens IS NULL THEN 0 ELSE 1 END,
      output_known_sum = output_known_sum - coalesce(OLD.output_tokens, 0),
      reasoning_known_count = reasoning_known_count -
        CASE WHEN OLD.reasoning_tokens IS NULL THEN 0 ELSE 1 END,
      reasoning_known_sum = reasoning_known_sum - coalesce(OLD.reasoning_tokens, 0),
      total_known_count = total_known_count -
        CASE WHEN OLD.total_tokens IS NULL THEN 0 ELSE 1 END,
      total_known_sum = total_known_sum - coalesce(OLD.total_tokens, 0),
      fallback_model_count = fallback_model_count - OLD.fallback_model,
      long_context_yes_count = long_context_yes_count -
        CASE WHEN OLD.long_context = 'yes' THEN 1 ELSE 0 END,
      long_context_no_count = long_context_no_count -
        CASE WHEN OLD.long_context = 'no' THEN 1 ELSE 0 END,
      long_context_unavailable_count = long_context_unavailable_count -
        CASE WHEN OLD.long_context = 'unavailable' THEN 1 ELSE 0 END,
      activity_read = activity_read - OLD.activity_read,
      activity_edit_write = activity_edit_write - OLD.activity_edit_write,
      activity_search = activity_search - OLD.activity_search,
      activity_git = activity_git - OLD.activity_git,
      activity_build_test = activity_build_test - OLD.activity_build_test,
      activity_web = activity_web - OLD.activity_web,
      activity_subagents = activity_subagents - OLD.activity_subagents,
      activity_terminal = activity_terminal - OLD.activity_terminal
  WHERE dataset_kind = 'current'
    AND aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND (
      (bucket_width = 'minute' AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60))
      OR
      (bucket_width = 'hour' AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600))
    )
    AND (
      (dimension_kind = 'all' AND dimension_value = '')
      OR (dimension_kind = 'model' AND dimension_value = OLD.model)
      OR (dimension_kind = 'project'
          AND dimension_value = coalesce(OLD.project_alias, ''))
    );
END;
CREATE TRIGGER usage_event_price_time_after_delete
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
      AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
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
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1
    AND ((bucket_width = 'minute' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60))
      OR (bucket_width = 'hour' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)));
  UPDATE usage_price_time_rollup
  SET event_count = event_count - 1,
      calculable_event_count = calculable_event_count - CASE WHEN OLD.input_tokens IS NOT NULL AND OLD.cached_tokens IS NOT NULL AND OLD.cached_tokens <= OLD.input_tokens AND ((OLD.total_tokens IS NOT NULL AND OLD.total_tokens >= OLD.input_tokens AND (OLD.output_tokens IS NULL OR OLD.reasoning_tokens IS NULL OR (OLD.output_tokens <= OLD.total_tokens - OLD.input_tokens AND OLD.reasoning_tokens = OLD.total_tokens - OLD.input_tokens - OLD.output_tokens))) OR (OLD.total_tokens IS NULL AND OLD.output_tokens IS NOT NULL AND OLD.reasoning_tokens IS NOT NULL AND OLD.output_tokens <= 9223372036854775807 - OLD.reasoning_tokens)) THEN 1 ELSE 0 END,
      uncached_input_sum = uncached_input_sum - CASE WHEN OLD.input_tokens IS NOT NULL AND OLD.cached_tokens IS NOT NULL AND OLD.cached_tokens <= OLD.input_tokens AND ((OLD.total_tokens IS NOT NULL AND OLD.total_tokens >= OLD.input_tokens AND (OLD.output_tokens IS NULL OR OLD.reasoning_tokens IS NULL OR (OLD.output_tokens <= OLD.total_tokens - OLD.input_tokens AND OLD.reasoning_tokens = OLD.total_tokens - OLD.input_tokens - OLD.output_tokens))) OR (OLD.total_tokens IS NULL AND OLD.output_tokens IS NOT NULL AND OLD.reasoning_tokens IS NOT NULL AND OLD.output_tokens <= 9223372036854775807 - OLD.reasoning_tokens)) THEN OLD.input_tokens - OLD.cached_tokens ELSE 0 END,
      cached_input_sum = cached_input_sum - CASE WHEN OLD.input_tokens IS NOT NULL AND OLD.cached_tokens IS NOT NULL AND OLD.cached_tokens <= OLD.input_tokens AND ((OLD.total_tokens IS NOT NULL AND OLD.total_tokens >= OLD.input_tokens AND (OLD.output_tokens IS NULL OR OLD.reasoning_tokens IS NULL OR (OLD.output_tokens <= OLD.total_tokens - OLD.input_tokens AND OLD.reasoning_tokens = OLD.total_tokens - OLD.input_tokens - OLD.output_tokens))) OR (OLD.total_tokens IS NULL AND OLD.output_tokens IS NOT NULL AND OLD.reasoning_tokens IS NOT NULL AND OLD.output_tokens <= 9223372036854775807 - OLD.reasoning_tokens)) THEN OLD.cached_tokens ELSE 0 END,
      billable_output_sum = billable_output_sum - CASE WHEN OLD.input_tokens IS NOT NULL AND OLD.cached_tokens IS NOT NULL AND OLD.cached_tokens <= OLD.input_tokens AND ((OLD.total_tokens IS NOT NULL AND OLD.total_tokens >= OLD.input_tokens AND (OLD.output_tokens IS NULL OR OLD.reasoning_tokens IS NULL OR (OLD.output_tokens <= OLD.total_tokens - OLD.input_tokens AND OLD.reasoning_tokens = OLD.total_tokens - OLD.input_tokens - OLD.output_tokens))) OR (OLD.total_tokens IS NULL AND OLD.output_tokens IS NOT NULL AND OLD.reasoning_tokens IS NOT NULL AND OLD.output_tokens <= 9223372036854775807 - OLD.reasoning_tokens)) THEN CASE WHEN OLD.total_tokens IS NOT NULL THEN OLD.total_tokens - OLD.input_tokens ELSE OLD.output_tokens + OLD.reasoning_tokens END ELSE 0 END,
      reported_cost_count = reported_cost_count - CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 0 ELSE 1 END,
      reported_cost_sum = reported_cost_sum - coalesce(OLD.reported_cost_usd_micros, 0)
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND provider_id = OLD.provider_id
    AND profile_id = OLD.profile_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1
    AND ((bucket_width = 'minute' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60))
      OR (bucket_width = 'hour' AND bucket_start_seconds =
          OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)));
END;
