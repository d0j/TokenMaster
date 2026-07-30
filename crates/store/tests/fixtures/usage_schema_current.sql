-- index benefit_change_scope_sequence
CREATE UNIQUE INDEX benefit_change_scope_sequence
  ON benefit_change(scope_id, sequence)

-- index benefit_delivery_scope_time
CREATE INDEX benefit_delivery_scope_time
  ON benefit_reminder_delivery(scope_id, delivered_at_ms DESC, delivery_id DESC)

-- index benefit_due_next
CREATE INDEX benefit_due_next
  ON benefit_reminder_due(due_at_ms, expiry_at_ms, scope_id, lot_id)

-- index benefit_lot_current_expiry
CREATE INDEX benefit_lot_current_expiry
  ON benefit_lot_current(scope_id, state, conservative_expiry_at_ms, lot_id)

-- index benefit_lot_revision_retention
CREATE INDEX benefit_lot_revision_retention
  ON benefit_lot_revision(scope_id, lot_id, lot_revision DESC)

-- index benefit_profile_scope
CREATE INDEX benefit_profile_scope
  ON benefit_reminder_profile(profile_scope_id, profile_kind)

-- index git_association_repository_activity
CREATE INDEX git_association_repository_activity
  ON git_activity_association(repository_id, last_activity_at_ms DESC, association_id)

-- index git_day_category_repository_range
CREATE INDEX git_day_category_repository_range
  ON git_day_category_aggregate(repository_id, aggregate_generation, day_index, category)

-- index git_day_repository_range
CREATE INDEX git_day_repository_range
  ON git_day_aggregate(repository_id, aggregate_generation, day_index)

-- index git_repository_observed
CREATE INDEX git_repository_observed
  ON git_repository(observed_at_ms DESC, repository_id)

-- index quota_definition_scope_revision
CREATE INDEX quota_definition_scope_revision
  ON quota_window_definition(scope_id, window_id, revision DESC)

-- index quota_epoch_history_retention
CREATE INDEX quota_epoch_history_retention
  ON quota_epoch_history(scope_id, window_id, last_observed_at_ms DESC, epoch_id DESC)

-- index quota_sample_retention
CREATE INDEX quota_sample_retention
  ON quota_sample(scope_id, window_id, observed_at_ms DESC, observation_id DESC)

-- index quota_transition_window_sequence
CREATE UNIQUE INDEX quota_transition_window_sequence
  ON quota_transition(scope_id, window_id, sequence)

-- index quota_window_current_scope
CREATE INDEX quota_window_current_scope
  ON quota_window_current(scope_id, window_id)

-- index sqlite_autoindex_benefit_change_1


-- index sqlite_autoindex_benefit_lot_current_1


-- index sqlite_autoindex_benefit_lot_revision_1


-- index sqlite_autoindex_benefit_reminder_ack_1


-- index sqlite_autoindex_benefit_reminder_delivery_1


-- index sqlite_autoindex_benefit_reminder_due_1


-- index sqlite_autoindex_benefit_reminder_profile_1


-- index sqlite_autoindex_benefit_reminder_threshold_1


-- index sqlite_autoindex_benefit_scope_1


-- index sqlite_autoindex_git_activity_association_1


-- index sqlite_autoindex_git_category_aggregate_1


-- index sqlite_autoindex_git_day_aggregate_1


-- index sqlite_autoindex_git_day_category_aggregate_1


-- index sqlite_autoindex_git_repository_1


-- index sqlite_autoindex_git_warning_1


-- index sqlite_autoindex_quota_epoch_current_1


-- index sqlite_autoindex_quota_epoch_current_2


-- index sqlite_autoindex_quota_epoch_current_3


-- index sqlite_autoindex_quota_epoch_current_4


-- index sqlite_autoindex_quota_epoch_history_1


-- index sqlite_autoindex_quota_epoch_history_2


-- index sqlite_autoindex_quota_sample_1


-- index sqlite_autoindex_quota_sample_2


-- index sqlite_autoindex_quota_sample_3


-- index sqlite_autoindex_quota_transition_1


-- index sqlite_autoindex_quota_window_current_1


-- index sqlite_autoindex_quota_window_definition_1


-- index sqlite_autoindex_usage_event_1


-- index sqlite_autoindex_usage_generation_1


-- index sqlite_autoindex_usage_legacy_event_1


-- index sqlite_autoindex_usage_observation_1


-- index sqlite_autoindex_usage_price_session_rollup_1


-- index sqlite_autoindex_usage_price_time_rollup_1


-- index sqlite_autoindex_usage_replay_observation_1


-- index sqlite_autoindex_usage_replay_selection_1


-- index sqlite_autoindex_usage_replay_session_1


-- index sqlite_autoindex_usage_replay_source_1


-- index sqlite_autoindex_usage_replay_work_1


-- index sqlite_autoindex_usage_scan_1


-- index sqlite_autoindex_usage_session_rollup_1


-- index sqlite_autoindex_usage_source_1


-- index sqlite_autoindex_usage_source_chunk_1


-- index sqlite_autoindex_usage_time_rollup_1


-- index usage_event_model_time
CREATE INDEX usage_event_model_time
           ON usage_event(model, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC)

-- index usage_event_session_time
CREATE INDEX usage_event_session_time
  ON usage_event(provider_id, profile_id, session_id, timestamp_seconds,
                 timestamp_nanos, fingerprint)

-- index usage_event_time_desc
CREATE INDEX usage_event_time_desc
           ON usage_event(timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC)

-- index usage_generation_one_current
CREATE UNIQUE INDEX usage_generation_one_current
  ON usage_generation(file_key) WHERE status = 'current'

-- index usage_generation_one_staging
CREATE UNIQUE INDEX usage_generation_one_staging
  ON usage_generation(file_key) WHERE status = 'staging'

-- index usage_legacy_event_model_time
CREATE INDEX usage_legacy_event_model_time
  ON usage_legacy_event(snapshot_id, model, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC)

-- index usage_legacy_event_time_desc
CREATE INDEX usage_legacy_event_time_desc
  ON usage_legacy_event(snapshot_id, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC)

-- index usage_observation_fingerprint
CREATE INDEX usage_observation_fingerprint
  ON usage_observation(fingerprint, profile_id, file_key, generation, source_offset)

-- index usage_price_session_scope
CREATE INDEX usage_price_session_scope
  ON usage_price_session_rollup(aggregate_generation, dataset_kind, provider_id, profile_id,
                                session_id, project_key, model, service_tier, long_context,
                                reported_state)

-- index usage_price_time_scope_range
CREATE INDEX usage_price_time_scope_range
  ON usage_price_time_rollup(aggregate_generation, dataset_kind, provider_id, profile_id,
                             bucket_width, bucket_start_seconds, project_key, model,
                             service_tier, long_context, reported_state)

-- index usage_replay_observation_disposition
CREATE INDEX usage_replay_observation_disposition
  ON usage_replay_observation(revision_id, disposition)

-- index usage_replay_observation_fingerprint
CREATE INDEX usage_replay_observation_fingerprint
  ON usage_replay_observation(revision_id, fingerprint, disposition, file_key, generation, source_offset)

-- index usage_replay_observation_parent
CREATE INDEX usage_replay_observation_parent
  ON usage_replay_observation(revision_id, provider_id, profile_id, session_id, session_ordinal)

-- index usage_replay_revision_one_current
CREATE UNIQUE INDEX usage_replay_revision_one_current
           ON usage_replay_revision(status) WHERE status = 'current'

-- index usage_replay_revision_one_staging
CREATE UNIQUE INDEX usage_replay_revision_one_staging
           ON usage_replay_revision(status) WHERE status = 'staging'

-- index usage_scan_one_running_scope
CREATE UNIQUE INDEX usage_scan_one_running_scope
           ON usage_scan(provider_id, profile_id) WHERE completion_state = 'running'

-- index usage_scan_set_one_running
CREATE UNIQUE INDEX usage_scan_set_one_running
           ON usage_scan_set(completion_state) WHERE completion_state = 'running'

-- index usage_session_rollup_page
CREATE INDEX usage_session_rollup_page
  ON usage_session_rollup(aggregate_generation, dataset_kind, last_timestamp_seconds DESC,
                          last_timestamp_nanos DESC, provider_id, profile_id, session_id)
  WHERE dimension_kind = 'all'

-- index usage_source_scope_missing
CREATE INDEX usage_source_scope_missing
           ON usage_source(provider_id, profile_id, missing, file_key)

-- index usage_time_rollup_scope_range
CREATE INDEX usage_time_rollup_scope_range
  ON usage_time_rollup(aggregate_generation, dataset_kind, provider_id, profile_id, dimension_kind,
                       dimension_value, bucket_width, bucket_start_seconds)

-- table benefit_change
CREATE TABLE benefit_change (
  change_id BLOB PRIMARY KEY NOT NULL CHECK(length(change_id) = 32),
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  sequence INTEGER NOT NULL CHECK(sequence > 0),
  lot_id BLOB NOT NULL CHECK(length(lot_id) = 32),
  lot_revision INTEGER NOT NULL CHECK(lot_revision > 0),
  kind TEXT NOT NULL
    CHECK(kind IN ('awarded','quantity_changed','state_changed','expiry_changed','corrected',
                   'disappeared_ambiguous','reappeared','retired_terminal')),
  before_revision INTEGER CHECK(before_revision IS NULL OR before_revision > 0),
  after_revision INTEGER CHECK(after_revision IS NULL OR after_revision > 0),
  observed_at_ms INTEGER NOT NULL CHECK(observed_at_ms > 0),
  FOREIGN KEY(scope_id) REFERENCES benefit_scope(scope_id),
  FOREIGN KEY(scope_id, lot_id, before_revision)
    REFERENCES benefit_lot_revision(scope_id, lot_id, lot_revision),
  FOREIGN KEY(scope_id, lot_id, after_revision)
    REFERENCES benefit_lot_revision(scope_id, lot_id, lot_revision),
  CHECK(before_revision IS NOT NULL OR after_revision IS NOT NULL),
  CHECK((after_revision IS NOT NULL AND lot_revision = after_revision)
     OR (after_revision IS NULL AND before_revision IS NOT NULL
         AND lot_revision = before_revision + 1))
) STRICT

-- table benefit_lot_current
CREATE TABLE benefit_lot_current (
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  lot_id BLOB NOT NULL CHECK(length(lot_id) = 32),
  lot_revision INTEGER NOT NULL CHECK(lot_revision > 0),
  kind TEXT NOT NULL
    CHECK(kind IN ('banked_rate_limit_reset','usage_credit','temporary_usage','unknown')),
  quantity INTEGER NOT NULL CHECK(quantity > 0),
  state TEXT NOT NULL
    CHECK(state IN ('available','activation_pending','activated','expired','revoked','ambiguous')),
  detail_kind TEXT NOT NULL CHECK(detail_kind IN ('provider_detail','provider_aggregate','manual')),
  conservative_expiry_at_ms INTEGER
    CHECK(conservative_expiry_at_ms IS NULL OR conservative_expiry_at_ms > 0),
  PRIMARY KEY(scope_id, lot_id),
  FOREIGN KEY(scope_id, lot_id, lot_revision)
    REFERENCES benefit_lot_revision(scope_id, lot_id, lot_revision)
) STRICT

-- table benefit_lot_revision
CREATE TABLE benefit_lot_revision (
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  lot_id BLOB NOT NULL CHECK(length(lot_id) = 32),
  lot_revision INTEGER NOT NULL CHECK(lot_revision > 0),
  kind TEXT NOT NULL
    CHECK(kind IN ('banked_rate_limit_reset','usage_credit','temporary_usage','unknown')),
  quantity INTEGER NOT NULL CHECK(quantity > 0),
  state TEXT NOT NULL
    CHECK(state IN ('available','activation_pending','activated','expired','revoked','ambiguous')),
  target_kind TEXT NOT NULL CHECK(target_kind IN ('provider','quota_window')),
  target_window_id TEXT
    CHECK(target_window_id IS NULL OR (
      length(CAST(target_window_id AS BLOB)) BETWEEN 1 AND 128
      AND target_window_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  granted_at_ms INTEGER CHECK(granted_at_ms IS NULL OR granted_at_ms > 0),
  expiry_kind TEXT NOT NULL
    CHECK(expiry_kind IN ('exact_utc','provider_local','provider_date','bounded_utc','unknown')),
  expiry_exact_at_ms INTEGER CHECK(expiry_exact_at_ms IS NULL OR expiry_exact_at_ms > 0),
  expiry_local_year INTEGER CHECK(expiry_local_year IS NULL OR expiry_local_year BETWEEN 1 AND 9999),
  expiry_local_month INTEGER CHECK(expiry_local_month IS NULL OR expiry_local_month BETWEEN 1 AND 12),
  expiry_local_day INTEGER CHECK(expiry_local_day IS NULL OR expiry_local_day BETWEEN 1 AND 31),
  expiry_local_hour INTEGER CHECK(expiry_local_hour IS NULL OR expiry_local_hour BETWEEN 0 AND 23),
  expiry_local_minute INTEGER CHECK(expiry_local_minute IS NULL OR expiry_local_minute BETWEEN 0 AND 59),
  expiry_local_second INTEGER CHECK(expiry_local_second IS NULL OR expiry_local_second BETWEEN 0 AND 59),
  expiry_local_millisecond INTEGER
    CHECK(expiry_local_millisecond IS NULL OR expiry_local_millisecond BETWEEN 0 AND 999),
  expiry_time_zone TEXT
    CHECK(expiry_time_zone IS NULL OR (
      length(CAST(expiry_time_zone AS BLOB)) BETWEEN 1 AND 128
      AND expiry_time_zone NOT GLOB '*[^A-Za-z0-9/._+-]*'
    )),
  expiry_bounded_earliest_at_ms INTEGER
    CHECK(expiry_bounded_earliest_at_ms IS NULL OR expiry_bounded_earliest_at_ms > 0),
  expiry_bounded_latest_at_ms INTEGER
    CHECK(expiry_bounded_latest_at_ms IS NULL OR expiry_bounded_latest_at_ms > 0),
  source TEXT NOT NULL CHECK(source IN ('provider_official','provider_local','manual','unknown')),
  confidence TEXT NOT NULL CHECK(confidence IN ('high','medium','low','unknown')),
  detail_kind TEXT NOT NULL CHECK(detail_kind IN ('provider_detail','provider_aggregate','manual')),
  label_key TEXT NOT NULL
    CHECK(length(CAST(label_key AS BLOB)) BETWEEN 1 AND 128
      AND label_key NOT GLOB '*[^A-Za-z0-9._-]*'),
  PRIMARY KEY(scope_id, lot_id, lot_revision),
  FOREIGN KEY(scope_id) REFERENCES benefit_scope(scope_id),
  CHECK((target_kind = 'provider' AND target_window_id IS NULL)
     OR (target_kind = 'quota_window' AND target_window_id IS NOT NULL)),
  CHECK(
    (expiry_kind = 'exact_utc'
      AND expiry_exact_at_ms IS NOT NULL
      AND expiry_local_year IS NULL AND expiry_local_month IS NULL
      AND expiry_local_day IS NULL AND expiry_local_hour IS NULL
      AND expiry_local_minute IS NULL AND expiry_local_second IS NULL
      AND expiry_local_millisecond IS NULL AND expiry_time_zone IS NULL
      AND expiry_bounded_earliest_at_ms IS NULL AND expiry_bounded_latest_at_ms IS NULL)
    OR
    (expiry_kind = 'provider_local'
      AND expiry_exact_at_ms IS NULL
      AND expiry_local_year IS NOT NULL AND expiry_local_month IS NOT NULL
      AND expiry_local_day IS NOT NULL AND expiry_local_hour IS NOT NULL
      AND expiry_local_minute IS NOT NULL AND expiry_local_second IS NOT NULL
      AND expiry_local_millisecond IS NOT NULL AND expiry_time_zone IS NOT NULL
      AND expiry_bounded_earliest_at_ms IS NULL AND expiry_bounded_latest_at_ms IS NULL)
    OR
    (expiry_kind = 'provider_date'
      AND expiry_exact_at_ms IS NULL
      AND expiry_local_year IS NOT NULL AND expiry_local_month IS NOT NULL
      AND expiry_local_day IS NOT NULL AND expiry_local_hour IS NULL
      AND expiry_local_minute IS NULL AND expiry_local_second IS NULL
      AND expiry_local_millisecond IS NULL
      AND expiry_bounded_earliest_at_ms IS NULL AND expiry_bounded_latest_at_ms IS NULL)
    OR
    (expiry_kind = 'bounded_utc'
      AND expiry_exact_at_ms IS NULL
      AND expiry_local_year IS NULL AND expiry_local_month IS NULL
      AND expiry_local_day IS NULL AND expiry_local_hour IS NULL
      AND expiry_local_minute IS NULL AND expiry_local_second IS NULL
      AND expiry_local_millisecond IS NULL AND expiry_time_zone IS NULL
      AND expiry_bounded_earliest_at_ms IS NOT NULL
      AND expiry_bounded_latest_at_ms >= expiry_bounded_earliest_at_ms)
    OR
    (expiry_kind = 'unknown'
      AND expiry_exact_at_ms IS NULL
      AND expiry_local_year IS NULL AND expiry_local_month IS NULL
      AND expiry_local_day IS NULL AND expiry_local_hour IS NULL
      AND expiry_local_minute IS NULL AND expiry_local_second IS NULL
      AND expiry_local_millisecond IS NULL AND expiry_time_zone IS NULL
      AND expiry_bounded_earliest_at_ms IS NULL AND expiry_bounded_latest_at_ms IS NULL)
  )
) STRICT

-- table benefit_reminder_ack
CREATE TABLE benefit_reminder_ack (
  delivery_id BLOB PRIMARY KEY NOT NULL CHECK(length(delivery_id) = 32),
  acknowledged_at_ms INTEGER NOT NULL CHECK(acknowledged_at_ms > 0),
  FOREIGN KEY(delivery_id)
    REFERENCES benefit_reminder_delivery(delivery_id)
    ON DELETE CASCADE
) STRICT

-- table benefit_reminder_delivery
CREATE TABLE benefit_reminder_delivery (
  delivery_id BLOB PRIMARY KEY NOT NULL CHECK(length(delivery_id) = 32),
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  lot_id BLOB NOT NULL CHECK(length(lot_id) = 32),
  lot_revision INTEGER NOT NULL CHECK(lot_revision > 0),
  threshold_seconds INTEGER NOT NULL CHECK(threshold_seconds BETWEEN 60 AND 31536000),
  channel TEXT NOT NULL CHECK(channel IN ('in_app','os_scheduled')),
  due_at_ms INTEGER NOT NULL,
  expiry_at_ms INTEGER NOT NULL CHECK(expiry_at_ms > 0),
  delivered_at_ms INTEGER NOT NULL CHECK(delivered_at_ms > 0),
  FOREIGN KEY(scope_id, lot_id, lot_revision)
    REFERENCES benefit_lot_revision(scope_id, lot_id, lot_revision),
  CHECK(due_at_ms < expiry_at_ms)
) STRICT

-- table benefit_reminder_due
CREATE TABLE benefit_reminder_due (
  delivery_id BLOB PRIMARY KEY NOT NULL CHECK(length(delivery_id) = 32),
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  lot_id BLOB NOT NULL CHECK(length(lot_id) = 32),
  lot_revision INTEGER NOT NULL CHECK(lot_revision > 0),
  threshold_seconds INTEGER NOT NULL CHECK(threshold_seconds BETWEEN 60 AND 31536000),
  channel TEXT NOT NULL CHECK(channel IN ('in_app','os_scheduled')),
  due_at_ms INTEGER NOT NULL,
  expiry_at_ms INTEGER NOT NULL CHECK(expiry_at_ms > 0),
  profile_revision INTEGER NOT NULL CHECK(profile_revision > 0),
  FOREIGN KEY(scope_id, lot_id, lot_revision)
    REFERENCES benefit_lot_revision(scope_id, lot_id, lot_revision),
  CHECK(due_at_ms < expiry_at_ms)
) STRICT

-- table benefit_reminder_profile
CREATE TABLE benefit_reminder_profile (
  profile_kind TEXT NOT NULL CHECK(profile_kind IN ('global','scope')),
  profile_scope_id BLOB NOT NULL
    CHECK((profile_kind = 'global' AND length(profile_scope_id) = 0)
       OR (profile_kind = 'scope' AND length(profile_scope_id) = 32)),
  revision INTEGER NOT NULL CHECK(revision > 0),
  channel_in_app INTEGER NOT NULL CHECK(channel_in_app IN (0,1)),
  channel_os_scheduled INTEGER NOT NULL CHECK(channel_os_scheduled IN (0,1)),
  PRIMARY KEY(profile_kind, profile_scope_id)
) STRICT

-- table benefit_reminder_threshold
CREATE TABLE benefit_reminder_threshold (
  profile_kind TEXT NOT NULL CHECK(profile_kind IN ('global','scope')),
  profile_scope_id BLOB NOT NULL,
  threshold_seconds INTEGER NOT NULL CHECK(threshold_seconds BETWEEN 60 AND 31536000),
  PRIMARY KEY(profile_kind, profile_scope_id, threshold_seconds),
  FOREIGN KEY(profile_kind, profile_scope_id)
    REFERENCES benefit_reminder_profile(profile_kind, profile_scope_id) ON DELETE CASCADE
) STRICT

-- table benefit_scope
CREATE TABLE benefit_scope (
  scope_id BLOB PRIMARY KEY NOT NULL CHECK(length(scope_id) = 32),
  provider_id TEXT NOT NULL
    CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64
      AND provider_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  account_id TEXT NOT NULL
    CHECK(length(CAST(account_id AS BLOB)) BETWEEN 1 AND 128
      AND account_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  workspace_id TEXT
    CHECK(workspace_id IS NULL OR (
      length(CAST(workspace_id AS BLOB)) BETWEEN 1 AND 128
      AND workspace_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  inventory_revision INTEGER NOT NULL CHECK(inventory_revision >= 0),
  last_change_sequence INTEGER NOT NULL CHECK(last_change_sequence >= 0),
  observation_id BLOB CHECK(observation_id IS NULL OR length(observation_id) = 32),
  observed_at_ms INTEGER,
  fresh_until_ms INTEGER,
  stale_after_ms INTEGER,
  completeness TEXT
    CHECK(completeness IS NULL
      OR completeness IN ('complete','complete_quantity_partial_details','partial')),
  current_lot_count INTEGER NOT NULL CHECK(current_lot_count BETWEEN 0 AND 64),
  CHECK(
    (observation_id IS NULL AND observed_at_ms IS NULL AND fresh_until_ms IS NULL
      AND stale_after_ms IS NULL AND completeness IS NULL)
    OR
    (observation_id IS NOT NULL
      AND observed_at_ms > 0
      AND observed_at_ms <= fresh_until_ms
      AND fresh_until_ms <= stale_after_ms
      AND completeness IS NOT NULL)
  )
) STRICT

-- table benefit_state
CREATE TABLE benefit_state (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  revision INTEGER NOT NULL CHECK(revision >= 0),
  current_lot_count INTEGER NOT NULL CHECK(current_lot_count >= 0),
  retained_change_count INTEGER NOT NULL CHECK(retained_change_count >= 0),
  pending_due_count INTEGER NOT NULL CHECK(pending_due_count >= 0),
  retained_delivery_count INTEGER NOT NULL CHECK(retained_delivery_count >= 0),
  last_published_at_ms INTEGER CHECK(last_published_at_ms IS NULL OR last_published_at_ms > 0),
  CHECK((revision = 0 AND last_published_at_ms IS NULL)
     OR (revision > 0 AND last_published_at_ms IS NOT NULL))
) STRICT

-- table git_activity_association
CREATE TABLE git_activity_association (
  association_id BLOB PRIMARY KEY CHECK(length(association_id) = 32),
  repository_id BLOB NOT NULL CHECK(length(repository_id) = 32),
  project_key BLOB CHECK(project_key IS NULL OR length(project_key) = 32),
  first_activity_at_ms INTEGER NOT NULL CHECK(first_activity_at_ms > 0),
  last_activity_at_ms INTEGER NOT NULL CHECK(last_activity_at_ms >= first_activity_at_ms),
  FOREIGN KEY(repository_id) REFERENCES git_repository(repository_id) ON DELETE CASCADE
) STRICT

-- table git_category_aggregate
CREATE TABLE git_category_aggregate (
  repository_id BLOB NOT NULL CHECK(length(repository_id) = 32),
  aggregate_generation INTEGER NOT NULL CHECK(aggregate_generation >= 1),
  category TEXT NOT NULL CHECK(category IN (
    'product_code','test','docs_spec','config_build','schema_migration',
    'vendor_generated','asset','other'
  )),
  added_lines INTEGER NOT NULL CHECK(added_lines >= 0),
  removed_lines INTEGER NOT NULL CHECK(removed_lines >= 0),
  PRIMARY KEY(repository_id, aggregate_generation, category),
  FOREIGN KEY(repository_id) REFERENCES git_repository(repository_id) ON DELETE CASCADE
) STRICT

-- table git_day_aggregate
CREATE TABLE git_day_aggregate (
  repository_id BLOB NOT NULL CHECK(length(repository_id) = 32),
  aggregate_generation INTEGER NOT NULL CHECK(aggregate_generation >= 1),
  day_index INTEGER NOT NULL CHECK(day_index BETWEEN -719162 AND 2932896),
  commits INTEGER NOT NULL CHECK(commits >= 0),
  merge_commits INTEGER NOT NULL CHECK(merge_commits BETWEEN 0 AND commits),
  added_lines INTEGER NOT NULL CHECK(added_lines >= 0),
  removed_lines INTEGER NOT NULL CHECK(removed_lines >= 0),
  PRIMARY KEY(repository_id, aggregate_generation, day_index),
  FOREIGN KEY(repository_id) REFERENCES git_repository(repository_id) ON DELETE CASCADE
) STRICT

-- table git_day_category_aggregate
CREATE TABLE git_day_category_aggregate (
  repository_id BLOB NOT NULL CHECK(length(repository_id) = 32),
  aggregate_generation INTEGER NOT NULL CHECK(aggregate_generation >= 1),
  day_index INTEGER NOT NULL CHECK(day_index BETWEEN -719162 AND 2932896),
  category TEXT NOT NULL CHECK(category IN (
    'product_code','test','docs_spec','config_build','schema_migration',
    'vendor_generated','asset','other'
  )),
  added_lines INTEGER NOT NULL CHECK(added_lines >= 0),
  removed_lines INTEGER NOT NULL CHECK(removed_lines >= 0),
  PRIMARY KEY(repository_id, aggregate_generation, day_index, category),
  FOREIGN KEY(repository_id) REFERENCES git_repository(repository_id) ON DELETE CASCADE
) STRICT

-- table git_installation_state
CREATE TABLE git_installation_state (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  installation_salt BLOB NOT NULL CHECK(length(installation_salt) = 32),
  publication_revision INTEGER NOT NULL CHECK(publication_revision >= 0),
  repository_count INTEGER NOT NULL CHECK(repository_count BETWEEN 0 AND 32),
  association_count INTEGER NOT NULL CHECK(association_count BETWEEN 0 AND 4096),
  last_published_at_ms INTEGER CHECK(last_published_at_ms IS NULL OR last_published_at_ms > 0),
  CHECK((publication_revision = 0 AND last_published_at_ms IS NULL)
     OR (publication_revision > 0 AND last_published_at_ms IS NOT NULL))
) STRICT

-- table git_repository
CREATE TABLE git_repository (
  repository_id BLOB PRIMARY KEY CHECK(length(repository_id) = 32),
  active_generation INTEGER NOT NULL CHECK(active_generation >= 1),
  scan_revision INTEGER NOT NULL CHECK(scan_revision >= 1),
  object_format TEXT CHECK(object_format IS NULL OR object_format IN ('sha1','sha256')),
  heads_fingerprint BLOB CHECK(heads_fingerprint IS NULL OR length(heads_fingerprint) = 32),
  mailmap_fingerprint BLOB CHECK(mailmap_fingerprint IS NULL OR length(mailmap_fingerprint) = 32),
  author_fingerprint BLOB CHECK(author_fingerprint IS NULL OR length(author_fingerprint) = 32),
  category_version INTEGER CHECK(category_version IS NULL OR category_version BETWEEN 1 AND 65535),
  shallow INTEGER CHECK(shallow IS NULL OR shallow IN (0,1)),
  observed_at_ms INTEGER NOT NULL CHECK(observed_at_ms > 0),
  data_through_ms INTEGER CHECK(data_through_ms IS NULL OR data_through_ms > 0),
  quality TEXT NOT NULL CHECK(quality IN ('complete','partial','unavailable')),
  unavailable_reason TEXT CHECK(unavailable_reason IS NULL OR unavailable_reason IN (
    'git_not_found','git_not_native','repository_not_found','repository_path_rejected',
    'author_identity_missing','unsupported_git_version','unsupported_object_format',
    'too_many_repositories','too_many_refs','history_limit_exceeded',
    'output_limit_exceeded','deadline_exceeded','process_failed',
    'history_changed_during_scan','cache_incompatible','store_unavailable'
  )),
  publication_state TEXT NOT NULL CHECK(publication_state IN ('ready','rebuild_required')),
  commits INTEGER NOT NULL CHECK(commits >= 0),
  merge_commits INTEGER NOT NULL CHECK(merge_commits BETWEEN 0 AND commits),
  added_lines INTEGER NOT NULL CHECK(added_lines >= 0),
  removed_lines INTEGER NOT NULL CHECK(removed_lines >= 0),
  binary_files INTEGER NOT NULL CHECK(binary_files >= 0),
  submodule_changes INTEGER NOT NULL CHECK(submodule_changes >= 0),
  omitted_commits INTEGER NOT NULL CHECK(omitted_commits >= 0),
  omitted_paths INTEGER NOT NULL CHECK(omitted_paths >= 0),
  CHECK(data_through_ms IS NULL OR data_through_ms <= observed_at_ms),
  CHECK((quality = 'unavailable' AND unavailable_reason IS NOT NULL
         AND data_through_ms IS NULL AND commits = 0 AND merge_commits = 0
         AND added_lines = 0 AND removed_lines = 0 AND binary_files = 0
         AND submodule_changes = 0 AND omitted_commits = 0 AND omitted_paths = 0
         AND object_format IS NULL AND heads_fingerprint IS NULL
         AND mailmap_fingerprint IS NULL AND author_fingerprint IS NULL
         AND category_version IS NULL AND shallow IS NULL)
     OR (quality IN ('complete','partial') AND unavailable_reason IS NULL
         AND data_through_ms IS NOT NULL AND object_format IS NOT NULL
         AND heads_fingerprint IS NOT NULL AND mailmap_fingerprint IS NOT NULL
         AND author_fingerprint IS NOT NULL AND category_version IS NOT NULL
         AND shallow IS NOT NULL))
) STRICT

-- table git_warning
CREATE TABLE git_warning (
  repository_id BLOB NOT NULL CHECK(length(repository_id) = 32),
  aggregate_generation INTEGER NOT NULL CHECK(aggregate_generation >= 1),
  warning TEXT NOT NULL CHECK(warning IN (
    'shallow_history','binary_files_omitted','submodule_lines_omitted',
    'oversized_fields_omitted','invalid_commit_omitted',
    'daily_history_truncated',
    'incremental_rebuild_pending','association_incomplete'
  )),
  PRIMARY KEY(repository_id, aggregate_generation, warning),
  FOREIGN KEY(repository_id) REFERENCES git_repository(repository_id) ON DELETE CASCADE
) STRICT

-- table quota_epoch_current
CREATE TABLE quota_epoch_current (
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  window_id TEXT NOT NULL
    CHECK(length(CAST(window_id AS BLOB)) BETWEEN 1 AND 128
      AND window_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  epoch_id BLOB NOT NULL UNIQUE CHECK(length(epoch_id) = 32),
  epoch_definition_revision INTEGER NOT NULL,
  definition_revision INTEGER NOT NULL,
  first_observation_id BLOB NOT NULL CHECK(length(first_observation_id) = 32),
  last_observation_id BLOB NOT NULL CHECK(length(last_observation_id) = 32),
  first_observed_at_ms INTEGER NOT NULL CHECK(first_observed_at_ms > 0),
  last_observed_at_ms INTEGER NOT NULL CHECK(last_observed_at_ms > 0),
  maximum_used_ratio_ppm INTEGER
    CHECK(maximum_used_ratio_ppm IS NULL OR maximum_used_ratio_ppm BETWEEN 0 AND 1000000),
  maximum_used_ratio_observation_id BLOB
    CHECK(maximum_used_ratio_observation_id IS NULL
      OR length(maximum_used_ratio_observation_id) = 32),
  maximum_unit_id TEXT
    CHECK(maximum_unit_id IS NULL OR (
      length(CAST(maximum_unit_id AS BLOB)) BETWEEN 1 AND 128
      AND maximum_unit_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  maximum_used_units INTEGER CHECK(maximum_used_units IS NULL OR maximum_used_units >= 0),
  maximum_remaining_units INTEGER
    CHECK(maximum_remaining_units IS NULL OR maximum_remaining_units >= 0),
  maximum_capacity_units INTEGER
    CHECK(maximum_capacity_units IS NULL OR maximum_capacity_units >= 0),
  maximum_used_units_observation_id BLOB
    CHECK(maximum_used_units_observation_id IS NULL
      OR length(maximum_used_units_observation_id) = 32),
  provider_epoch_id TEXT
    CHECK(provider_epoch_id IS NULL OR (
      length(CAST(provider_epoch_id AS BLOB)) BETWEEN 1 AND 128
      AND provider_epoch_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  advertised_resets_at_ms INTEGER
    CHECK(advertised_resets_at_ms IS NULL OR advertised_resets_at_ms > 0),
  last_transition_sequence INTEGER NOT NULL CHECK(last_transition_sequence >= 0),
  PRIMARY KEY(scope_id, window_id),
  UNIQUE(scope_id, window_id, epoch_id),
  UNIQUE(scope_id, window_id, definition_revision, epoch_id),
  FOREIGN KEY(scope_id, window_id, epoch_definition_revision)
    REFERENCES quota_window_definition(scope_id, window_id, revision),
  FOREIGN KEY(scope_id, window_id, definition_revision)
    REFERENCES quota_window_definition(scope_id, window_id, revision),
  FOREIGN KEY(scope_id, window_id, epoch_definition_revision, first_observation_id)
    REFERENCES quota_sample(scope_id, window_id, definition_revision, observation_id),
  FOREIGN KEY(scope_id, window_id, definition_revision, last_observation_id)
    REFERENCES quota_sample(scope_id, window_id, definition_revision, observation_id),
  FOREIGN KEY(scope_id, window_id, maximum_used_ratio_observation_id)
    REFERENCES quota_sample(scope_id, window_id, observation_id),
  FOREIGN KEY(scope_id, window_id, maximum_used_units_observation_id)
    REFERENCES quota_sample(scope_id, window_id, observation_id),
  CHECK(epoch_definition_revision > 0
    AND definition_revision >= epoch_definition_revision),
  CHECK(first_observed_at_ms <= last_observed_at_ms),
  CHECK((maximum_used_ratio_ppm IS NULL)
     = (maximum_used_ratio_observation_id IS NULL)),
  CHECK(
    (maximum_unit_id IS NULL
      AND maximum_used_units IS NULL
      AND maximum_remaining_units IS NULL
      AND maximum_capacity_units IS NULL
      AND maximum_used_units_observation_id IS NULL)
    OR
    (maximum_unit_id IS NOT NULL
      AND maximum_used_units IS NOT NULL
      AND maximum_used_units_observation_id IS NOT NULL
      AND (maximum_capacity_units IS NULL OR maximum_used_units <= maximum_capacity_units)
      AND (maximum_capacity_units IS NULL
        OR maximum_remaining_units IS NULL
        OR maximum_remaining_units <= maximum_capacity_units))
  )
) STRICT

-- table quota_epoch_history
CREATE TABLE quota_epoch_history (
  epoch_id BLOB PRIMARY KEY NOT NULL CHECK(length(epoch_id) = 32),
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  window_id TEXT NOT NULL
    CHECK(length(CAST(window_id AS BLOB)) BETWEEN 1 AND 128
      AND window_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  epoch_definition_revision INTEGER NOT NULL,
  definition_revision INTEGER NOT NULL,
  first_observation_id BLOB NOT NULL CHECK(length(first_observation_id) = 32),
  last_observation_id BLOB NOT NULL CHECK(length(last_observation_id) = 32),
  first_observed_at_ms INTEGER NOT NULL CHECK(first_observed_at_ms > 0),
  last_observed_at_ms INTEGER NOT NULL CHECK(last_observed_at_ms > 0),
  maximum_used_ratio_ppm INTEGER
    CHECK(maximum_used_ratio_ppm IS NULL OR maximum_used_ratio_ppm BETWEEN 0 AND 1000000),
  maximum_used_ratio_observation_id BLOB
    CHECK(maximum_used_ratio_observation_id IS NULL
      OR length(maximum_used_ratio_observation_id) = 32),
  maximum_unit_id TEXT
    CHECK(maximum_unit_id IS NULL OR (
      length(CAST(maximum_unit_id AS BLOB)) BETWEEN 1 AND 128
      AND maximum_unit_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  maximum_used_units INTEGER CHECK(maximum_used_units IS NULL OR maximum_used_units >= 0),
  maximum_remaining_units INTEGER
    CHECK(maximum_remaining_units IS NULL OR maximum_remaining_units >= 0),
  maximum_capacity_units INTEGER
    CHECK(maximum_capacity_units IS NULL OR maximum_capacity_units >= 0),
  maximum_used_units_observation_id BLOB
    CHECK(maximum_used_units_observation_id IS NULL
      OR length(maximum_used_units_observation_id) = 32),
  final_provider_epoch_id TEXT
    CHECK(final_provider_epoch_id IS NULL OR (
      length(CAST(final_provider_epoch_id AS BLOB)) BETWEEN 1 AND 128
      AND final_provider_epoch_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  final_advertised_resets_at_ms INTEGER
    CHECK(final_advertised_resets_at_ms IS NULL OR final_advertised_resets_at_ms > 0),
  closing_transition_sequence INTEGER NOT NULL CHECK(closing_transition_sequence > 0),
  UNIQUE(scope_id, window_id, closing_transition_sequence),
  FOREIGN KEY(scope_id, window_id, epoch_definition_revision)
    REFERENCES quota_window_definition(scope_id, window_id, revision),
  FOREIGN KEY(scope_id, window_id, definition_revision)
    REFERENCES quota_window_definition(scope_id, window_id, revision),
  FOREIGN KEY(scope_id, window_id, epoch_definition_revision, first_observation_id)
    REFERENCES quota_sample(scope_id, window_id, definition_revision, observation_id),
  FOREIGN KEY(scope_id, window_id, definition_revision, last_observation_id)
    REFERENCES quota_sample(scope_id, window_id, definition_revision, observation_id),
  FOREIGN KEY(scope_id, window_id, maximum_used_ratio_observation_id)
    REFERENCES quota_sample(scope_id, window_id, observation_id),
  FOREIGN KEY(scope_id, window_id, maximum_used_units_observation_id)
    REFERENCES quota_sample(scope_id, window_id, observation_id),
  CHECK(epoch_definition_revision > 0
    AND definition_revision >= epoch_definition_revision),
  CHECK(first_observed_at_ms <= last_observed_at_ms),
  CHECK((maximum_used_ratio_ppm IS NULL)
     = (maximum_used_ratio_observation_id IS NULL)),
  CHECK(
    (maximum_unit_id IS NULL
      AND maximum_used_units IS NULL
      AND maximum_remaining_units IS NULL
      AND maximum_capacity_units IS NULL
      AND maximum_used_units_observation_id IS NULL)
    OR
    (maximum_unit_id IS NOT NULL
      AND maximum_used_units IS NOT NULL
      AND maximum_used_units_observation_id IS NOT NULL
      AND (maximum_capacity_units IS NULL OR maximum_used_units <= maximum_capacity_units)
      AND (maximum_capacity_units IS NULL
        OR maximum_remaining_units IS NULL
        OR maximum_remaining_units <= maximum_capacity_units))
  )
) STRICT

-- table quota_sample
CREATE TABLE quota_sample (
  observation_id BLOB PRIMARY KEY NOT NULL CHECK(length(observation_id) = 32),
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  window_id TEXT NOT NULL
    CHECK(length(CAST(window_id AS BLOB)) BETWEEN 1 AND 128
      AND window_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  definition_revision INTEGER NOT NULL CHECK(definition_revision > 0),
  observed_at_ms INTEGER NOT NULL,
  fresh_until_ms INTEGER NOT NULL,
  stale_after_ms INTEGER NOT NULL,
  provider_epoch_id TEXT
    CHECK(provider_epoch_id IS NULL OR (
      length(CAST(provider_epoch_id AS BLOB)) BETWEEN 1 AND 128
      AND provider_epoch_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  used_ratio_ppm INTEGER CHECK(used_ratio_ppm IS NULL OR used_ratio_ppm BETWEEN 0 AND 1000000),
  remaining_ratio_ppm INTEGER
    CHECK(remaining_ratio_ppm IS NULL OR remaining_ratio_ppm BETWEEN 0 AND 1000000),
  unit_id TEXT
    CHECK(unit_id IS NULL OR (
      length(CAST(unit_id AS BLOB)) BETWEEN 1 AND 128
      AND unit_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  used_units INTEGER CHECK(used_units IS NULL OR used_units >= 0),
  remaining_units INTEGER CHECK(remaining_units IS NULL OR remaining_units >= 0),
  capacity_units INTEGER CHECK(capacity_units IS NULL OR capacity_units >= 0),
  advertised_resets_at_ms INTEGER
    CHECK(advertised_resets_at_ms IS NULL OR advertised_resets_at_ms > 0),
  quality TEXT NOT NULL CHECK(quality IN ('authoritative','partial','conflict','unknown')),
  source TEXT NOT NULL
    CHECK(source IN ('provider_local','provider_official','local_reset_event','manual','unknown')),
  confidence TEXT NOT NULL CHECK(confidence IN ('high','medium','low','unknown')),
  reset_evidence TEXT NOT NULL
    CHECK(reset_evidence IN ('none','explicit_provider','explicit_local','manual_or_banked')),
  reset_occurred_at_ms INTEGER,
  UNIQUE(scope_id, window_id, observation_id),
  UNIQUE(scope_id, window_id, definition_revision, observation_id),
  FOREIGN KEY(scope_id, window_id, definition_revision)
    REFERENCES quota_window_definition(scope_id, window_id, revision),
  CHECK(observed_at_ms > 0
    AND observed_at_ms <= fresh_until_ms
    AND fresh_until_ms <= stale_after_ms),
  CHECK(
    (unit_id IS NULL AND used_units IS NULL
      AND remaining_units IS NULL AND capacity_units IS NULL)
    OR
    (unit_id IS NOT NULL
      AND (used_units IS NOT NULL OR remaining_units IS NOT NULL OR capacity_units IS NOT NULL)
      AND (capacity_units IS NULL OR used_units IS NULL OR used_units <= capacity_units)
      AND (capacity_units IS NULL OR remaining_units IS NULL OR remaining_units <= capacity_units))
  ),
  CHECK(provider_epoch_id IS NOT NULL
     OR used_ratio_ppm IS NOT NULL
     OR remaining_ratio_ppm IS NOT NULL
     OR unit_id IS NOT NULL
     OR advertised_resets_at_ms IS NOT NULL
     OR reset_evidence <> 'none'),
  CHECK(
    (reset_occurred_at_ms IS NULL)
    OR
    (reset_evidence <> 'none'
      AND reset_occurred_at_ms > 0
      AND reset_occurred_at_ms <= observed_at_ms)
  )
) STRICT

-- table quota_state
CREATE TABLE quota_state (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  revision INTEGER NOT NULL CHECK(revision >= 0),
  retained_sample_count INTEGER NOT NULL CHECK(retained_sample_count >= 0),
  retained_epoch_count INTEGER NOT NULL CHECK(retained_epoch_count >= 0),
  retained_transition_count INTEGER NOT NULL CHECK(retained_transition_count >= 0),
  last_published_at_ms INTEGER CHECK(last_published_at_ms IS NULL OR last_published_at_ms > 0),
  CHECK((revision = 0 AND last_published_at_ms IS NULL)
     OR (revision > 0 AND last_published_at_ms IS NOT NULL))
) STRICT

-- table quota_transition
CREATE TABLE quota_transition (
  transition_id BLOB PRIMARY KEY NOT NULL CHECK(length(transition_id) = 32),
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  window_id TEXT NOT NULL
    CHECK(length(CAST(window_id AS BLOB)) BETWEEN 1 AND 128
      AND window_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  definition_revision INTEGER NOT NULL CHECK(definition_revision > 0),
  sequence INTEGER NOT NULL CHECK(sequence > 0),
  kind TEXT NOT NULL
    CHECK(kind IN ('scheduled_reset','early_reset','manual_or_banked_reset','unknown_reset','allowance_changed')),
  previous_epoch_id BLOB NOT NULL CHECK(length(previous_epoch_id) = 32),
  current_epoch_id BLOB NOT NULL CHECK(length(current_epoch_id) = 32),
  pre_observation_id BLOB NOT NULL CHECK(length(pre_observation_id) = 32),
  post_observation_id BLOB NOT NULL CHECK(length(post_observation_id) = 32),
  maximum_used_ratio_ppm INTEGER
    CHECK(maximum_used_ratio_ppm IS NULL OR maximum_used_ratio_ppm BETWEEN 0 AND 1000000),
  maximum_used_ratio_observation_id BLOB
    CHECK(maximum_used_ratio_observation_id IS NULL
      OR length(maximum_used_ratio_observation_id) = 32),
  maximum_unit_id TEXT
    CHECK(maximum_unit_id IS NULL OR (
      length(CAST(maximum_unit_id AS BLOB)) BETWEEN 1 AND 128
      AND maximum_unit_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  maximum_used_units INTEGER CHECK(maximum_used_units IS NULL OR maximum_used_units >= 0),
  maximum_remaining_units INTEGER
    CHECK(maximum_remaining_units IS NULL OR maximum_remaining_units >= 0),
  maximum_capacity_units INTEGER
    CHECK(maximum_capacity_units IS NULL OR maximum_capacity_units >= 0),
  maximum_used_units_observation_id BLOB
    CHECK(maximum_used_units_observation_id IS NULL
      OR length(maximum_used_units_observation_id) = 32),
  old_resets_at_ms INTEGER CHECK(old_resets_at_ms IS NULL OR old_resets_at_ms > 0),
  new_resets_at_ms INTEGER CHECK(new_resets_at_ms IS NULL OR new_resets_at_ms > 0),
  allowance_change_kind TEXT
    CHECK(allowance_change_kind IS NULL
      OR allowance_change_kind IN ('increased','decreased','unit_changed')),
  allowance_old_unit_id TEXT,
  allowance_old_used_units INTEGER CHECK(allowance_old_used_units IS NULL OR allowance_old_used_units >= 0),
  allowance_old_remaining_units INTEGER
    CHECK(allowance_old_remaining_units IS NULL OR allowance_old_remaining_units >= 0),
  allowance_old_capacity_units INTEGER
    CHECK(allowance_old_capacity_units IS NULL OR allowance_old_capacity_units >= 0),
  allowance_new_unit_id TEXT,
  allowance_new_used_units INTEGER CHECK(allowance_new_used_units IS NULL OR allowance_new_used_units >= 0),
  allowance_new_remaining_units INTEGER
    CHECK(allowance_new_remaining_units IS NULL OR allowance_new_remaining_units >= 0),
  allowance_new_capacity_units INTEGER
    CHECK(allowance_new_capacity_units IS NULL OR allowance_new_capacity_units >= 0),
  source TEXT NOT NULL
    CHECK(source IN ('provider_local','provider_official','local_reset_event','manual','unknown')),
  confidence TEXT NOT NULL CHECK(confidence IN ('high','medium','low','unknown')),
  detection_time_kind TEXT NOT NULL CHECK(detection_time_kind IN ('exact','interval')),
  exact_at_ms INTEGER CHECK(exact_at_ms IS NULL OR exact_at_ms > 0),
  after_ms INTEGER CHECK(after_ms IS NULL OR after_ms > 0),
  at_or_before_ms INTEGER CHECK(at_or_before_ms IS NULL OR at_or_before_ms > 0),
  FOREIGN KEY(scope_id, window_id, definition_revision)
    REFERENCES quota_window_definition(scope_id, window_id, revision),
  FOREIGN KEY(scope_id, window_id, pre_observation_id)
    REFERENCES quota_sample(scope_id, window_id, observation_id),
  FOREIGN KEY(scope_id, window_id, definition_revision, post_observation_id)
    REFERENCES quota_sample(scope_id, window_id, definition_revision, observation_id),
  FOREIGN KEY(scope_id, window_id, maximum_used_ratio_observation_id)
    REFERENCES quota_sample(scope_id, window_id, observation_id),
  FOREIGN KEY(scope_id, window_id, maximum_used_units_observation_id)
    REFERENCES quota_sample(scope_id, window_id, observation_id),
  CHECK((maximum_used_ratio_ppm IS NULL)
     = (maximum_used_ratio_observation_id IS NULL)),
  CHECK(
    (maximum_unit_id IS NULL
      AND maximum_used_units IS NULL
      AND maximum_remaining_units IS NULL
      AND maximum_capacity_units IS NULL
      AND maximum_used_units_observation_id IS NULL)
    OR
    (maximum_unit_id IS NOT NULL
      AND maximum_used_units IS NOT NULL
      AND maximum_used_units_observation_id IS NOT NULL
      AND (maximum_capacity_units IS NULL OR maximum_used_units <= maximum_capacity_units)
      AND (maximum_capacity_units IS NULL
        OR maximum_remaining_units IS NULL
        OR maximum_remaining_units <= maximum_capacity_units))
  ),
  CHECK(
    (allowance_change_kind IS NULL
      AND allowance_old_unit_id IS NULL
      AND allowance_old_used_units IS NULL
      AND allowance_old_remaining_units IS NULL
      AND allowance_old_capacity_units IS NULL
      AND allowance_new_unit_id IS NULL
      AND allowance_new_used_units IS NULL
      AND allowance_new_remaining_units IS NULL
      AND allowance_new_capacity_units IS NULL)
    OR
    (allowance_change_kind IS NOT NULL
      AND allowance_old_unit_id IS NOT NULL
      AND length(CAST(allowance_old_unit_id AS BLOB)) BETWEEN 1 AND 128
      AND allowance_old_unit_id NOT GLOB '*[^A-Za-z0-9._-]*'
      AND allowance_old_capacity_units IS NOT NULL
      AND allowance_new_unit_id IS NOT NULL
      AND length(CAST(allowance_new_unit_id AS BLOB)) BETWEEN 1 AND 128
      AND allowance_new_unit_id NOT GLOB '*[^A-Za-z0-9._-]*'
      AND allowance_new_capacity_units IS NOT NULL
      AND (allowance_old_used_units IS NULL
        OR allowance_old_used_units <= allowance_old_capacity_units)
      AND (allowance_old_remaining_units IS NULL
        OR allowance_old_remaining_units <= allowance_old_capacity_units)
      AND (allowance_new_used_units IS NULL
        OR allowance_new_used_units <= allowance_new_capacity_units)
      AND (allowance_new_remaining_units IS NULL
        OR allowance_new_remaining_units <= allowance_new_capacity_units)
      AND ((allowance_change_kind = 'unit_changed'
            AND allowance_old_unit_id <> allowance_new_unit_id)
        OR (allowance_change_kind = 'increased'
            AND allowance_old_unit_id = allowance_new_unit_id
            AND allowance_new_capacity_units > allowance_old_capacity_units)
        OR (allowance_change_kind = 'decreased'
            AND allowance_old_unit_id = allowance_new_unit_id
            AND allowance_new_capacity_units < allowance_old_capacity_units)))
  ),
  CHECK((kind = 'allowance_changed' AND previous_epoch_id = current_epoch_id
         AND allowance_change_kind IS NOT NULL)
     OR (kind <> 'allowance_changed' AND previous_epoch_id <> current_epoch_id)),
  CHECK((detection_time_kind = 'exact'
         AND exact_at_ms IS NOT NULL
         AND after_ms IS NULL
         AND at_or_before_ms IS NULL)
     OR (detection_time_kind = 'interval'
         AND exact_at_ms IS NULL
         AND after_ms IS NOT NULL
         AND at_or_before_ms IS NOT NULL
         AND after_ms < at_or_before_ms))
) STRICT

-- table quota_window_current
CREATE TABLE quota_window_current (
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  window_id TEXT NOT NULL
    CHECK(length(CAST(window_id AS BLOB)) BETWEEN 1 AND 128
      AND window_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  definition_revision INTEGER NOT NULL CHECK(definition_revision > 0),
  sample_observation_id BLOB NOT NULL CHECK(length(sample_observation_id) = 32),
  epoch_id BLOB NOT NULL CHECK(length(epoch_id) = 32),
  observed_at_ms INTEGER NOT NULL CHECK(observed_at_ms > 0),
  fresh_until_ms INTEGER NOT NULL CHECK(fresh_until_ms >= observed_at_ms),
  stale_after_ms INTEGER NOT NULL CHECK(stale_after_ms >= fresh_until_ms),
  quality TEXT NOT NULL CHECK(quality IN ('authoritative','partial','conflict','unknown')),
  source TEXT NOT NULL
    CHECK(source IN ('provider_local','provider_official','local_reset_event','manual','unknown')),
  confidence TEXT NOT NULL CHECK(confidence IN ('high','medium','low','unknown')),
  last_transition_sequence INTEGER NOT NULL CHECK(last_transition_sequence >= 0),
  PRIMARY KEY(scope_id, window_id),
  FOREIGN KEY(scope_id, window_id, definition_revision)
    REFERENCES quota_window_definition(scope_id, window_id, revision),
  FOREIGN KEY(scope_id, window_id, definition_revision, sample_observation_id)
    REFERENCES quota_sample(scope_id, window_id, definition_revision, observation_id),
  FOREIGN KEY(scope_id, window_id, definition_revision, epoch_id)
    REFERENCES quota_epoch_current(scope_id, window_id, definition_revision, epoch_id)
) STRICT

-- table quota_window_definition
CREATE TABLE quota_window_definition (
  scope_id BLOB NOT NULL CHECK(length(scope_id) = 32),
  window_id TEXT NOT NULL
    CHECK(length(CAST(window_id AS BLOB)) BETWEEN 1 AND 128
      AND window_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  revision INTEGER NOT NULL CHECK(revision > 0),
  provider_id TEXT NOT NULL
    CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64
      AND provider_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  account_id TEXT NOT NULL
    CHECK(length(CAST(account_id AS BLOB)) BETWEEN 1 AND 128
      AND account_id NOT GLOB '*[^A-Za-z0-9._-]*'),
  workspace_id TEXT
    CHECK(workspace_id IS NULL OR (
      length(CAST(workspace_id AS BLOB)) BETWEEN 1 AND 128
      AND workspace_id NOT GLOB '*[^A-Za-z0-9._-]*'
    )),
  label_key TEXT NOT NULL
    CHECK(length(CAST(label_key AS BLOB)) BETWEEN 1 AND 128
      AND label_key NOT GLOB '*[^A-Za-z0-9._-]*'),
  presentation TEXT NOT NULL CHECK(presentation IN ('used','remaining','pace')),
  semantics TEXT NOT NULL CHECK(semantics IN ('fixed','rolling','credit','unknown')),
  nominal_duration_seconds INTEGER
    CHECK(nominal_duration_seconds IS NULL OR nominal_duration_seconds > 0),
  maximum_post_reset_used_ppm INTEGER
    CHECK(maximum_post_reset_used_ppm IS NULL
      OR maximum_post_reset_used_ppm BETWEEN 0 AND 1000000),
  minimum_post_reset_remaining_ppm INTEGER
    CHECK(minimum_post_reset_remaining_ppm IS NULL
      OR minimum_post_reset_remaining_ppm BETWEEN 0 AND 1000000),
  minimum_used_ratio_drop_ppm INTEGER
    CHECK(minimum_used_ratio_drop_ppm IS NULL
      OR minimum_used_ratio_drop_ppm BETWEEN 1 AND 1000000),
  PRIMARY KEY(scope_id, window_id, revision),
  CHECK((maximum_post_reset_used_ppm IS NULL
         AND minimum_post_reset_remaining_ppm IS NULL
         AND minimum_used_ratio_drop_ppm IS NULL)
     OR (semantics = 'fixed'
         AND (maximum_post_reset_used_ppm IS NOT NULL
              OR minimum_post_reset_remaining_ppm IS NOT NULL)))
) STRICT

-- table usage_aggregate_state
CREATE TABLE usage_aggregate_state (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  aggregate_schema_version INTEGER NOT NULL CHECK(aggregate_schema_version = 2),
  state TEXT NOT NULL CHECK(state IN ('ready','rebuild_required','rebuilding','failed')),
  expected_dataset_generation INTEGER NOT NULL CHECK(expected_dataset_generation >= 0),
  active_aggregate_generation INTEGER NOT NULL CHECK(active_aggregate_generation >= 0),
  rebuild_aggregate_generation INTEGER CHECK(rebuild_aggregate_generation IS NULL OR rebuild_aggregate_generation >= 0),
  current_event_count INTEGER NOT NULL CHECK(current_event_count >= 0),
  legacy_event_count INTEGER NOT NULL CHECK(legacy_event_count >= 0),
  failure_code TEXT CHECK(failure_code IS NULL OR length(CAST(failure_code AS BLOB)) BETWEEN 1 AND 64),
  rebuild_dataset_kind TEXT CHECK(rebuild_dataset_kind IN ('cleanup','current','legacy')),
  rebuild_cursor_fingerprint BLOB CHECK(rebuild_cursor_fingerprint IS NULL OR length(rebuild_cursor_fingerprint) = 32),
  rebuild_processed_events INTEGER NOT NULL DEFAULT 0 CHECK(rebuild_processed_events >= 0),
  rebuild_total_events INTEGER NOT NULL CHECK(rebuild_total_events >= 0),
  CHECK(rebuild_processed_events <= rebuild_total_events),
  CHECK(
    (state = 'ready' AND failure_code IS NULL AND rebuild_aggregate_generation IS NULL
      AND rebuild_dataset_kind IS NULL
      AND rebuild_cursor_fingerprint IS NULL AND rebuild_processed_events = 0)
    OR
    (state = 'rebuild_required' AND failure_code IS NULL
      AND rebuild_aggregate_generation IS NULL AND rebuild_dataset_kind IS NULL
      AND rebuild_cursor_fingerprint IS NULL AND rebuild_processed_events = 0)
    OR
    (state = 'rebuilding' AND failure_code IS NULL
      AND rebuild_aggregate_generation IS NOT NULL
      AND rebuild_aggregate_generation <> active_aggregate_generation
      AND rebuild_dataset_kind IS NOT NULL)
    OR
    (state = 'failed' AND failure_code IS NOT NULL)
  )
) STRICT

-- table usage_archive_state
CREATE TABLE usage_archive_state (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  archive_generation INTEGER NOT NULL CHECK(archive_generation >= 0),
  dataset_generation INTEGER NOT NULL DEFAULT 0 CHECK(dataset_generation >= 0),
  current_revision_id INTEGER CHECK(current_revision_id IS NULL OR current_revision_id >= 0),
  latest_complete_scan_set_id INTEGER CHECK(latest_complete_scan_set_id IS NULL OR latest_complete_scan_set_id >= 0),
  incremental_state TEXT NOT NULL CHECK(incremental_state IN ('empty','complete','partial','recovery_pending')),
  CHECK(incremental_state <> 'empty' OR
        (current_revision_id IS NULL AND latest_complete_scan_set_id IS NULL)),
  CHECK(incremental_state <> 'complete' OR
        (current_revision_id IS NOT NULL AND latest_complete_scan_set_id IS NOT NULL)),
  CHECK(incremental_state NOT IN ('partial','recovery_pending') OR
        current_revision_id IS NOT NULL),
  FOREIGN KEY(current_revision_id) REFERENCES usage_replay_revision(revision_id),
  FOREIGN KEY(latest_complete_scan_set_id) REFERENCES usage_scan_set(scan_set_id)
) STRICT

-- table usage_event
CREATE TABLE "usage_event" (
  fingerprint BLOB PRIMARY KEY CHECK(length(fingerprint) = 32),
  event_id TEXT NOT NULL CHECK(length(CAST(event_id AS BLOB)) BETWEEN 1 AND 128),
  selected_file_key BLOB NOT NULL CHECK(length(selected_file_key) = 32),
  selected_generation INTEGER NOT NULL CHECK(selected_generation >= 0),
  selected_source_offset INTEGER NOT NULL CHECK(selected_source_offset >= 0),
  projection_revision_id INTEGER CHECK(projection_revision_id IS NULL OR projection_revision_id >= 0),
  origin_revision_id INTEGER CHECK(origin_revision_id IS NULL OR origin_revision_id >= 0),
  retained INTEGER NOT NULL CHECK(retained IN (0,1)) DEFAULT 0,
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  session_id TEXT NOT NULL CHECK(length(CAST(session_id AS BLOB)) BETWEEN 1 AND 512),
  source_id TEXT NOT NULL CHECK(length(CAST(source_id AS BLOB)) BETWEEN 1 AND 128),
  timestamp_seconds INTEGER NOT NULL,
  timestamp_nanos INTEGER NOT NULL CHECK(timestamp_nanos BETWEEN 0 AND 999999999),
  model TEXT NOT NULL CHECK(length(CAST(model AS BLOB)) BETWEEN 1 AND 64),
  raw_model TEXT CHECK(raw_model IS NULL OR length(CAST(raw_model AS BLOB)) BETWEEN 1 AND 512),
  input_tokens INTEGER CHECK(input_tokens IS NULL OR input_tokens >= 0),
  cached_tokens INTEGER CHECK(cached_tokens IS NULL OR cached_tokens >= 0),
  output_tokens INTEGER CHECK(output_tokens IS NULL OR output_tokens >= 0),
  reasoning_tokens INTEGER CHECK(reasoning_tokens IS NULL OR reasoning_tokens >= 0),
  total_tokens INTEGER CHECK(total_tokens IS NULL OR total_tokens >= 0),
  fallback_model INTEGER NOT NULL CHECK(fallback_model IN (0,1)),
  long_context TEXT NOT NULL CHECK(long_context IN ('yes','no','unavailable')),
  service_tier TEXT CHECK(service_tier IS NULL OR length(CAST(service_tier AS BLOB)) BETWEEN 1 AND 512),
  project_alias TEXT CHECK(project_alias IS NULL OR length(CAST(project_alias AS BLOB)) BETWEEN 1 AND 512),
  originator TEXT CHECK(originator IS NULL OR length(CAST(originator AS BLOB)) BETWEEN 1 AND 512),
  activity_read INTEGER NOT NULL CHECK(activity_read >= 0),
  activity_edit_write INTEGER NOT NULL CHECK(activity_edit_write >= 0),
  activity_search INTEGER NOT NULL CHECK(activity_search >= 0),
  activity_git INTEGER NOT NULL CHECK(activity_git >= 0),
  activity_build_test INTEGER NOT NULL CHECK(activity_build_test >= 0),
  activity_web INTEGER NOT NULL CHECK(activity_web >= 0),
  activity_subagents INTEGER NOT NULL CHECK(activity_subagents >= 0),
  activity_terminal INTEGER NOT NULL CHECK(activity_terminal >= 0), reported_cost_usd_micros INTEGER
           CHECK(reported_cost_usd_micros IS NULL OR reported_cost_usd_micros >= 0),
  CHECK(
    (projection_revision_id IS NULL AND origin_revision_id IS NULL AND retained = 0)
    OR
    (projection_revision_id IS NOT NULL AND (
      (retained = 0 AND origin_revision_id = projection_revision_id)
      OR
      (retained = 1 AND origin_revision_id < projection_revision_id)
    ))
  ),
  FOREIGN KEY(projection_revision_id) REFERENCES usage_replay_revision(revision_id)
    DEFERRABLE INITIALLY DEFERRED
) STRICT

-- table usage_generation
CREATE TABLE usage_generation (
  file_key BLOB NOT NULL CHECK(length(file_key) = 32),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  status TEXT NOT NULL CHECK(status IN ('staging','current')),
  parser_schema_version INTEGER NOT NULL CHECK(parser_schema_version BETWEEN 1 AND 65535),
  physical_identity BLOB CHECK(physical_identity IS NULL OR length(physical_identity) = 32),
  logical_identity BLOB NOT NULL CHECK(length(logical_identity) = 32),
  committed_offset INTEGER NOT NULL CHECK(committed_offset >= 0),
  scan_offset INTEGER NOT NULL CHECK(scan_offset >= committed_offset),
  observed_file_length INTEGER NOT NULL CHECK(observed_file_length >= scan_offset),
  modified_time_ns INTEGER,
  anchor_start INTEGER NOT NULL CHECK(anchor_start >= 0),
  anchor_len INTEGER NOT NULL CHECK(anchor_len BETWEEN 0 AND 4096),
  anchor_sha256 BLOB NOT NULL CHECK(length(anchor_sha256) = 32),
  resume_payload BLOB NOT NULL CHECK(length(resume_payload) <= 32768),
  discarding_oversized_line INTEGER NOT NULL CHECK(discarding_oversized_line IN (0,1)),
  incomplete_tail INTEGER NOT NULL CHECK(incomplete_tail IN (0,1)),
  verification_level TEXT NOT NULL CHECK(verification_level IN ('incremental','full_prefix')),
  PRIMARY KEY(file_key, generation),
  CHECK(anchor_start <= committed_offset),
  CHECK(anchor_len <= committed_offset - anchor_start),
  CHECK(discarding_oversized_line = 1 OR scan_offset = committed_offset),
  CHECK(discarding_oversized_line = 0 OR (incomplete_tail = 1 AND scan_offset > committed_offset)),
  FOREIGN KEY(file_key) REFERENCES usage_source(file_key) ON DELETE CASCADE
) STRICT

-- table usage_legacy_event
CREATE TABLE usage_legacy_event (
  snapshot_id INTEGER NOT NULL CHECK(snapshot_id = 1),
  fingerprint BLOB NOT NULL CHECK(length(fingerprint) = 32),
  event_id TEXT NOT NULL CHECK(length(CAST(event_id AS BLOB)) BETWEEN 1 AND 128),
  selected_file_key BLOB NOT NULL CHECK(length(selected_file_key) = 32),
  selected_generation INTEGER NOT NULL CHECK(selected_generation >= 0),
  selected_source_offset INTEGER NOT NULL CHECK(selected_source_offset >= 0),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  session_id TEXT NOT NULL CHECK(length(CAST(session_id AS BLOB)) BETWEEN 1 AND 512),
  source_id TEXT NOT NULL CHECK(length(CAST(source_id AS BLOB)) BETWEEN 1 AND 128),
  timestamp_seconds INTEGER NOT NULL,
  timestamp_nanos INTEGER NOT NULL CHECK(timestamp_nanos BETWEEN 0 AND 999999999),
  model TEXT NOT NULL CHECK(length(CAST(model AS BLOB)) BETWEEN 1 AND 64),
  raw_model TEXT CHECK(raw_model IS NULL OR length(CAST(raw_model AS BLOB)) BETWEEN 1 AND 512),
  input_tokens INTEGER CHECK(input_tokens IS NULL OR input_tokens >= 0),
  cached_tokens INTEGER CHECK(cached_tokens IS NULL OR cached_tokens >= 0),
  output_tokens INTEGER CHECK(output_tokens IS NULL OR output_tokens >= 0),
  reasoning_tokens INTEGER CHECK(reasoning_tokens IS NULL OR reasoning_tokens >= 0),
  total_tokens INTEGER CHECK(total_tokens IS NULL OR total_tokens >= 0),
  fallback_model INTEGER NOT NULL CHECK(fallback_model IN (0,1)),
  long_context TEXT NOT NULL CHECK(long_context IN ('yes','no','unavailable')),
  service_tier TEXT CHECK(service_tier IS NULL OR length(CAST(service_tier AS BLOB)) BETWEEN 1 AND 512),
  project_alias TEXT CHECK(project_alias IS NULL OR length(CAST(project_alias AS BLOB)) BETWEEN 1 AND 512),
  originator TEXT CHECK(originator IS NULL OR length(CAST(originator AS BLOB)) BETWEEN 1 AND 512),
  activity_read INTEGER NOT NULL CHECK(activity_read >= 0),
  activity_edit_write INTEGER NOT NULL CHECK(activity_edit_write >= 0),
  activity_search INTEGER NOT NULL CHECK(activity_search >= 0),
  activity_git INTEGER NOT NULL CHECK(activity_git >= 0),
  activity_build_test INTEGER NOT NULL CHECK(activity_build_test >= 0),
  activity_web INTEGER NOT NULL CHECK(activity_web >= 0),
  activity_subagents INTEGER NOT NULL CHECK(activity_subagents >= 0),
  activity_terminal INTEGER NOT NULL CHECK(activity_terminal >= 0), reported_cost_usd_micros INTEGER
           CHECK(reported_cost_usd_micros IS NULL OR reported_cost_usd_micros >= 0),
  PRIMARY KEY(snapshot_id, fingerprint),
  FOREIGN KEY(snapshot_id) REFERENCES usage_legacy_snapshot(snapshot_id)
) STRICT

-- table usage_legacy_snapshot
CREATE TABLE usage_legacy_snapshot (
  snapshot_id INTEGER PRIMARY KEY CHECK(snapshot_id = 1),
  source_schema_version INTEGER NOT NULL CHECK(source_schema_version = 1),
  quality_state TEXT NOT NULL CHECK(quality_state = 'legacy_unverified'),
  event_count INTEGER NOT NULL CHECK(event_count >= 0)
) STRICT

-- table usage_observation
CREATE TABLE usage_observation (
  file_key BLOB NOT NULL CHECK(length(file_key) = 32),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  source_offset INTEGER NOT NULL CHECK(source_offset >= 0),
  fingerprint BLOB NOT NULL CHECK(length(fingerprint) = 32),
  event_id TEXT NOT NULL CHECK(length(CAST(event_id AS BLOB)) BETWEEN 1 AND 128),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  session_id TEXT NOT NULL CHECK(length(CAST(session_id AS BLOB)) BETWEEN 1 AND 512),
  source_id TEXT NOT NULL CHECK(length(CAST(source_id AS BLOB)) BETWEEN 1 AND 128),
  timestamp_seconds INTEGER NOT NULL,
  timestamp_nanos INTEGER NOT NULL CHECK(timestamp_nanos BETWEEN 0 AND 999999999),
  model TEXT NOT NULL CHECK(length(CAST(model AS BLOB)) BETWEEN 1 AND 64),
  raw_model TEXT CHECK(raw_model IS NULL OR length(CAST(raw_model AS BLOB)) BETWEEN 1 AND 512),
  input_tokens INTEGER CHECK(input_tokens IS NULL OR input_tokens >= 0),
  cached_tokens INTEGER CHECK(cached_tokens IS NULL OR cached_tokens >= 0),
  output_tokens INTEGER CHECK(output_tokens IS NULL OR output_tokens >= 0),
  reasoning_tokens INTEGER CHECK(reasoning_tokens IS NULL OR reasoning_tokens >= 0),
  total_tokens INTEGER CHECK(total_tokens IS NULL OR total_tokens >= 0),
  fallback_model INTEGER NOT NULL CHECK(fallback_model IN (0,1)),
  long_context TEXT NOT NULL CHECK(long_context IN ('yes','no','unavailable')),
  service_tier TEXT CHECK(service_tier IS NULL OR length(CAST(service_tier AS BLOB)) BETWEEN 1 AND 512),
  project_alias TEXT CHECK(project_alias IS NULL OR length(CAST(project_alias AS BLOB)) BETWEEN 1 AND 512),
  originator TEXT CHECK(originator IS NULL OR length(CAST(originator AS BLOB)) BETWEEN 1 AND 512),
  activity_read INTEGER NOT NULL CHECK(activity_read >= 0),
  activity_edit_write INTEGER NOT NULL CHECK(activity_edit_write >= 0),
  activity_search INTEGER NOT NULL CHECK(activity_search >= 0),
  activity_git INTEGER NOT NULL CHECK(activity_git >= 0),
  activity_build_test INTEGER NOT NULL CHECK(activity_build_test >= 0),
  activity_web INTEGER NOT NULL CHECK(activity_web >= 0),
  activity_subagents INTEGER NOT NULL CHECK(activity_subagents >= 0),
  activity_terminal INTEGER NOT NULL CHECK(activity_terminal >= 0), reported_cost_usd_micros INTEGER
           CHECK(reported_cost_usd_micros IS NULL OR reported_cost_usd_micros >= 0),
  PRIMARY KEY(file_key, generation, source_offset, fingerprint),
  FOREIGN KEY(file_key, generation)
    REFERENCES usage_generation(file_key, generation) ON DELETE CASCADE
) STRICT

-- table usage_price_session_rollup
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
) STRICT

-- table usage_price_time_rollup
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
) STRICT

-- table usage_replay_observation
CREATE TABLE usage_replay_observation (
  revision_id INTEGER NOT NULL CHECK(revision_id >= 0),
  file_key BLOB NOT NULL CHECK(length(file_key) = 32),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  source_offset INTEGER NOT NULL CHECK(source_offset >= 0),
  fingerprint BLOB NOT NULL CHECK(length(fingerprint) = 32),
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  session_id TEXT NOT NULL CHECK(length(CAST(session_id AS BLOB)) BETWEEN 1 AND 512),
  parent_session_id TEXT CHECK(parent_session_id IS NULL OR length(CAST(parent_session_id AS BLOB)) BETWEEN 1 AND 512),
  session_ordinal INTEGER NOT NULL CHECK(session_ordinal >= 0),
  canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
  fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
  replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
  replay_signature BLOB NOT NULL CHECK(length(replay_signature) = 32),
  evidence TEXT NOT NULL CHECK(evidence IN ('strong_cumulative','weak_usage_only')),
  disposition TEXT NOT NULL CHECK(disposition IN ('eligible','replay','pending','conflict')),
  declared_conflict INTEGER NOT NULL CHECK(declared_conflict IN (0,1)),
  evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
  PRIMARY KEY(revision_id, file_key, generation, source_offset, fingerprint),
  FOREIGN KEY(revision_id) REFERENCES usage_replay_revision(revision_id) ON DELETE CASCADE,
  FOREIGN KEY(file_key, generation, source_offset, fingerprint)
    REFERENCES usage_observation(file_key, generation, source_offset, fingerprint)
    ON DELETE CASCADE
) STRICT

-- table usage_replay_revision
CREATE TABLE "usage_replay_revision" (
  revision_id INTEGER PRIMARY KEY CHECK(revision_id >= 0),
  status TEXT NOT NULL CHECK(status IN ('staging','current')),
  canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
  fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
  replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
  expected_source_count INTEGER NOT NULL CHECK(expected_source_count >= 0),
  evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
  sealed INTEGER NOT NULL CHECK(sealed IN (0,1)),
  promoted INTEGER NOT NULL CHECK(promoted IN (0,1)),
  scan_set_id INTEGER CHECK(scan_set_id IS NULL OR scan_set_id >= 0),
  CHECK((status = 'staging' AND promoted = 0) OR
        (status = 'current' AND sealed = 1 AND promoted = 1)),
  FOREIGN KEY(scan_set_id) REFERENCES usage_scan_set(scan_set_id)
) STRICT

-- table usage_replay_selection
CREATE TABLE usage_replay_selection (
  revision_id INTEGER NOT NULL CHECK(revision_id >= 0),
  fingerprint BLOB NOT NULL CHECK(length(fingerprint) = 32),
  file_key BLOB NOT NULL CHECK(length(file_key) = 32),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  source_offset INTEGER NOT NULL CHECK(source_offset >= 0),
  canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
  fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
  replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
  PRIMARY KEY(revision_id, fingerprint),
  FOREIGN KEY(revision_id, file_key, generation, source_offset, fingerprint)
    REFERENCES usage_replay_observation(revision_id, file_key, generation, source_offset, fingerprint)
    ON DELETE CASCADE
) STRICT

-- table usage_replay_session
CREATE TABLE usage_replay_session (
  revision_id INTEGER NOT NULL CHECK(revision_id >= 0),
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  session_id TEXT NOT NULL CHECK(length(CAST(session_id AS BLOB)) BETWEEN 1 AND 512),
  parent_session_id TEXT CHECK(parent_session_id IS NULL OR length(CAST(parent_session_id AS BLOB)) BETWEEN 1 AND 512),
  relation_conflict INTEGER NOT NULL CHECK(relation_conflict IN (0,1)),
  state TEXT NOT NULL CHECK(state IN ('root','matching','diverged','pending','conflict')),
  completion_state TEXT NOT NULL CHECK(completion_state IN ('open','sealed_complete')),
  first_relation_file_key BLOB CHECK(first_relation_file_key IS NULL OR length(first_relation_file_key) = 32),
  first_relation_source_offset INTEGER CHECK(first_relation_source_offset IS NULL OR first_relation_source_offset >= 0),
  last_classified_ordinal INTEGER CHECK(last_classified_ordinal IS NULL OR last_classified_ordinal >= 0),
  evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
  PRIMARY KEY(revision_id, provider_id, profile_id, session_id),
  CHECK((first_relation_file_key IS NULL) = (first_relation_source_offset IS NULL)),
  CHECK(parent_session_id IS NULL OR parent_session_id <> session_id OR relation_conflict = 1),
  FOREIGN KEY(revision_id) REFERENCES usage_replay_revision(revision_id) ON DELETE CASCADE
) STRICT

-- table usage_replay_source
CREATE TABLE usage_replay_source (
  revision_id INTEGER NOT NULL CHECK(revision_id >= 0),
  file_key BLOB NOT NULL CHECK(length(file_key) = 32),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  state TEXT NOT NULL CHECK(state IN ('pending','complete')),
  PRIMARY KEY(revision_id, file_key),
  FOREIGN KEY(revision_id) REFERENCES usage_replay_revision(revision_id) ON DELETE CASCADE,
  FOREIGN KEY(file_key) REFERENCES usage_source(file_key),
  FOREIGN KEY(file_key, generation)
    REFERENCES usage_generation(file_key, generation)
    DEFERRABLE INITIALLY DEFERRED
) STRICT

-- table usage_replay_work
CREATE TABLE usage_replay_work (
  revision_id INTEGER NOT NULL CHECK(revision_id >= 0),
  work_kind TEXT NOT NULL CHECK(work_kind IN ('classify_session','scan_children')),
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  session_id TEXT NOT NULL CHECK(length(CAST(session_id AS BLOB)) BETWEEN 1 AND 512),
  reason TEXT NOT NULL CHECK(reason IN ('late_relation','missing_parent','parent_changed','depth_bound','fanout_bound')),
  next_ordinal INTEGER NOT NULL CHECK(next_ordinal >= 0),
  child_session_cursor TEXT CHECK(child_session_cursor IS NULL OR length(CAST(child_session_cursor AS BLOB)) BETWEEN 1 AND 512),
  expected_evidence_epoch INTEGER NOT NULL CHECK(expected_evidence_epoch >= 0),
  PRIMARY KEY(revision_id, work_kind, provider_id, profile_id, session_id),
  FOREIGN KEY(revision_id) REFERENCES usage_replay_revision(revision_id) ON DELETE CASCADE
) STRICT

-- table usage_scan
CREATE TABLE "usage_scan" (
  scan_id INTEGER PRIMARY KEY CHECK(scan_id >= 0),
  scan_set_id INTEGER NOT NULL CHECK(scan_set_id >= 0),
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  started_at_ms INTEGER NOT NULL,
  completed_at_ms INTEGER,
  completion_state TEXT NOT NULL CHECK(completion_state IN ('running','complete','partial','cancelled','failed','timed_out')),
  sources_seen INTEGER NOT NULL DEFAULT 0 CHECK(sources_seen >= 0),
  files_read INTEGER NOT NULL DEFAULT 0 CHECK(files_read >= 0),
  bytes_read INTEGER NOT NULL DEFAULT 0 CHECK(bytes_read >= 0),
  events_observed INTEGER NOT NULL DEFAULT 0 CHECK(events_observed >= 0),
  diagnostics INTEGER NOT NULL DEFAULT 0 CHECK(diagnostics >= 0),
  UNIQUE(scan_set_id, provider_id, profile_id),
  CHECK((completion_state = 'running' AND completed_at_ms IS NULL) OR
        (completion_state <> 'running' AND completed_at_ms IS NOT NULL)),
  CHECK(completed_at_ms IS NULL OR completed_at_ms >= started_at_ms),
  FOREIGN KEY(scan_set_id) REFERENCES usage_scan_set(scan_set_id)
) STRICT

-- table usage_scan_set
CREATE TABLE usage_scan_set (
  scan_set_id INTEGER PRIMARY KEY CHECK(scan_set_id >= 0),
  started_at_ms INTEGER NOT NULL,
  completed_at_ms INTEGER,
  completion_state TEXT NOT NULL CHECK(completion_state IN ('running','complete','partial','cancelled','failed','timed_out')),
  expected_scope_count INTEGER NOT NULL CHECK(expected_scope_count BETWEEN 1 AND 256),
  CHECK((completion_state = 'running' AND completed_at_ms IS NULL) OR
        (completion_state <> 'running' AND completed_at_ms IS NOT NULL)),
  CHECK(completed_at_ms IS NULL OR completed_at_ms >= started_at_ms)
) STRICT

-- table usage_session_rollup
CREATE TABLE usage_session_rollup (
  aggregate_generation INTEGER NOT NULL CHECK(aggregate_generation >= 0),
  dataset_kind TEXT NOT NULL CHECK(dataset_kind IN ('current','legacy')),
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  session_id TEXT NOT NULL CHECK(length(CAST(session_id AS BLOB)) BETWEEN 1 AND 512),
  dimension_kind TEXT NOT NULL CHECK(dimension_kind IN ('all','model','project')),
  dimension_value TEXT NOT NULL CHECK(length(CAST(dimension_value AS BLOB)) <= 512),
  event_count INTEGER NOT NULL CHECK(event_count > 0),
  first_timestamp_seconds INTEGER,
  first_timestamp_nanos INTEGER CHECK(first_timestamp_nanos IS NULL OR first_timestamp_nanos BETWEEN 0 AND 999999999),
  last_timestamp_seconds INTEGER,
  last_timestamp_nanos INTEGER CHECK(last_timestamp_nanos IS NULL OR last_timestamp_nanos BETWEEN 0 AND 999999999),
  input_known_count INTEGER NOT NULL CHECK(input_known_count BETWEEN 0 AND event_count),
  input_known_sum INTEGER NOT NULL CHECK(input_known_sum >= 0),
  cached_known_count INTEGER NOT NULL CHECK(cached_known_count BETWEEN 0 AND event_count),
  cached_known_sum INTEGER NOT NULL CHECK(cached_known_sum >= 0),
  output_known_count INTEGER NOT NULL CHECK(output_known_count BETWEEN 0 AND event_count),
  output_known_sum INTEGER NOT NULL CHECK(output_known_sum >= 0),
  reasoning_known_count INTEGER NOT NULL CHECK(reasoning_known_count BETWEEN 0 AND event_count),
  reasoning_known_sum INTEGER NOT NULL CHECK(reasoning_known_sum >= 0),
  total_known_count INTEGER NOT NULL CHECK(total_known_count BETWEEN 0 AND event_count),
  total_known_sum INTEGER NOT NULL CHECK(total_known_sum >= 0),
  fallback_model_count INTEGER NOT NULL CHECK(fallback_model_count BETWEEN 0 AND event_count),
  long_context_yes_count INTEGER NOT NULL CHECK(long_context_yes_count BETWEEN 0 AND event_count),
  long_context_no_count INTEGER NOT NULL CHECK(long_context_no_count BETWEEN 0 AND event_count),
  long_context_unavailable_count INTEGER NOT NULL CHECK(long_context_unavailable_count BETWEEN 0 AND event_count),
  activity_read INTEGER NOT NULL CHECK(activity_read >= 0),
  activity_edit_write INTEGER NOT NULL CHECK(activity_edit_write >= 0),
  activity_search INTEGER NOT NULL CHECK(activity_search >= 0),
  activity_git INTEGER NOT NULL CHECK(activity_git >= 0),
  activity_build_test INTEGER NOT NULL CHECK(activity_build_test >= 0),
  activity_web INTEGER NOT NULL CHECK(activity_web >= 0),
  activity_subagents INTEGER NOT NULL CHECK(activity_subagents >= 0),
  activity_terminal INTEGER NOT NULL CHECK(activity_terminal >= 0),
  PRIMARY KEY(aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
              dimension_kind, dimension_value),
  CHECK((dimension_kind = 'all' AND dimension_value = ''
         AND first_timestamp_seconds IS NOT NULL AND first_timestamp_nanos IS NOT NULL
         AND last_timestamp_seconds IS NOT NULL AND last_timestamp_nanos IS NOT NULL
         AND (first_timestamp_seconds, first_timestamp_nanos)
             <= (last_timestamp_seconds, last_timestamp_nanos))
     OR (dimension_kind <> 'all' AND dimension_value <> ''
         AND first_timestamp_seconds IS NULL AND first_timestamp_nanos IS NULL
         AND last_timestamp_seconds IS NULL AND last_timestamp_nanos IS NULL)
     OR (dimension_kind = 'project' AND dimension_value = ''
         AND first_timestamp_seconds IS NULL AND first_timestamp_nanos IS NULL
         AND last_timestamp_seconds IS NULL AND last_timestamp_nanos IS NULL))
) STRICT

-- table usage_source
CREATE TABLE usage_source (
  file_key BLOB PRIMARY KEY CHECK(length(file_key) = 32),
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  source_id TEXT NOT NULL CHECK(length(CAST(source_id AS BLOB)) BETWEEN 1 AND 128),
  source_kind TEXT NOT NULL CHECK(source_kind IN ('active','direct','archived')),
  logical_identity BLOB NOT NULL CHECK(length(logical_identity) = 32),
  physical_identity BLOB CHECK(physical_identity IS NULL OR length(physical_identity) = 32),
  current_generation INTEGER CHECK(current_generation IS NULL OR current_generation >= 0),
  last_seen_scan_id INTEGER CHECK(last_seen_scan_id IS NULL OR last_seen_scan_id >= 0),
  missing INTEGER NOT NULL DEFAULT 0 CHECK(missing IN (0,1)),
  last_error_code TEXT CHECK(last_error_code IS NULL OR length(CAST(last_error_code AS BLOB)) BETWEEN 1 AND 64),
  verification_level TEXT CHECK(verification_level IS NULL OR verification_level IN ('incremental','full_prefix')),
  diagnostic_count INTEGER NOT NULL DEFAULT 0 CHECK(diagnostic_count >= 0),
  FOREIGN KEY(last_seen_scan_id) REFERENCES usage_scan(scan_id),
  FOREIGN KEY(file_key, current_generation)
    REFERENCES usage_generation(file_key, generation)
    DEFERRABLE INITIALLY DEFERRED
) STRICT

-- table usage_source_chunk
CREATE TABLE usage_source_chunk (
  file_key BLOB NOT NULL CHECK(length(file_key) = 32),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  chunk_index INTEGER NOT NULL CHECK(chunk_index >= 0),
  covered_len INTEGER NOT NULL CHECK(covered_len BETWEEN 1 AND 1048576),
  sha256 BLOB NOT NULL CHECK(length(sha256) = 32),
  PRIMARY KEY(file_key, generation, chunk_index),
  FOREIGN KEY(file_key, generation)
    REFERENCES usage_generation(file_key, generation) ON DELETE CASCADE
) STRICT

-- table usage_time_rollup
CREATE TABLE usage_time_rollup (
  aggregate_generation INTEGER NOT NULL CHECK(aggregate_generation >= 0),
  dataset_kind TEXT NOT NULL CHECK(dataset_kind IN ('current','legacy')),
  bucket_width TEXT NOT NULL CHECK(bucket_width IN ('minute','hour')),
  bucket_start_seconds INTEGER NOT NULL,
  provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  dimension_kind TEXT NOT NULL CHECK(dimension_kind IN ('all','model','project')),
  dimension_value TEXT NOT NULL CHECK(length(CAST(dimension_value AS BLOB)) <= 512),
  event_count INTEGER NOT NULL CHECK(event_count > 0),
  input_known_count INTEGER NOT NULL CHECK(input_known_count BETWEEN 0 AND event_count),
  input_known_sum INTEGER NOT NULL CHECK(input_known_sum >= 0),
  cached_known_count INTEGER NOT NULL CHECK(cached_known_count BETWEEN 0 AND event_count),
  cached_known_sum INTEGER NOT NULL CHECK(cached_known_sum >= 0),
  output_known_count INTEGER NOT NULL CHECK(output_known_count BETWEEN 0 AND event_count),
  output_known_sum INTEGER NOT NULL CHECK(output_known_sum >= 0),
  reasoning_known_count INTEGER NOT NULL CHECK(reasoning_known_count BETWEEN 0 AND event_count),
  reasoning_known_sum INTEGER NOT NULL CHECK(reasoning_known_sum >= 0),
  total_known_count INTEGER NOT NULL CHECK(total_known_count BETWEEN 0 AND event_count),
  total_known_sum INTEGER NOT NULL CHECK(total_known_sum >= 0),
  fallback_model_count INTEGER NOT NULL CHECK(fallback_model_count BETWEEN 0 AND event_count),
  long_context_yes_count INTEGER NOT NULL CHECK(long_context_yes_count BETWEEN 0 AND event_count),
  long_context_no_count INTEGER NOT NULL CHECK(long_context_no_count BETWEEN 0 AND event_count),
  long_context_unavailable_count INTEGER NOT NULL CHECK(long_context_unavailable_count BETWEEN 0 AND event_count),
  activity_read INTEGER NOT NULL CHECK(activity_read >= 0),
  activity_edit_write INTEGER NOT NULL CHECK(activity_edit_write >= 0),
  activity_search INTEGER NOT NULL CHECK(activity_search >= 0),
  activity_git INTEGER NOT NULL CHECK(activity_git >= 0),
  activity_build_test INTEGER NOT NULL CHECK(activity_build_test >= 0),
  activity_web INTEGER NOT NULL CHECK(activity_web >= 0),
  activity_subagents INTEGER NOT NULL CHECK(activity_subagents >= 0),
  activity_terminal INTEGER NOT NULL CHECK(activity_terminal >= 0),
  PRIMARY KEY(aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds, provider_id,
              profile_id, dimension_kind, dimension_value),
  CHECK((bucket_width = 'minute' AND bucket_start_seconds % 60 = 0)
     OR (bucket_width = 'hour' AND bucket_start_seconds % 3600 = 0)),
  CHECK((dimension_kind = 'all' AND dimension_value = '')
     OR (dimension_kind = 'model' AND length(CAST(dimension_value AS BLOB)) BETWEEN 1 AND 64)
     OR dimension_kind = 'project')
) STRICT

-- trigger benefit_ack_no_update
CREATE TRIGGER benefit_ack_no_update
BEFORE UPDATE ON benefit_reminder_ack
BEGIN
  SELECT RAISE(ABORT, 'immutable benefit acknowledgement');
END

-- trigger benefit_change_no_update
CREATE TRIGGER benefit_change_no_update
BEFORE UPDATE ON benefit_change
BEGIN
  SELECT RAISE(ABORT, 'immutable benefit change');
END

-- trigger benefit_delivery_no_update
CREATE TRIGGER benefit_delivery_no_update
BEFORE UPDATE ON benefit_reminder_delivery
BEGIN
  SELECT RAISE(ABORT, 'immutable benefit delivery');
END

-- trigger benefit_lot_revision_no_update
CREATE TRIGGER benefit_lot_revision_no_update
BEFORE UPDATE ON benefit_lot_revision
BEGIN
  SELECT RAISE(ABORT, 'immutable benefit lot revision');
END

-- trigger benefit_state_no_delete
CREATE TRIGGER benefit_state_no_delete
BEFORE DELETE ON benefit_state
BEGIN
  SELECT RAISE(ABORT, 'benefit state is required');
END

-- trigger git_category_no_update
CREATE TRIGGER git_category_no_update
BEFORE UPDATE ON git_category_aggregate
BEGIN
  SELECT RAISE(ABORT, 'immutable Git category aggregate');
END

-- trigger git_day_category_no_update
CREATE TRIGGER git_day_category_no_update
BEFORE UPDATE ON git_day_category_aggregate
BEGIN
  SELECT RAISE(ABORT, 'immutable Git day category aggregate');
END

-- trigger git_day_no_update
CREATE TRIGGER git_day_no_update
BEFORE UPDATE ON git_day_aggregate
BEGIN
  SELECT RAISE(ABORT, 'immutable Git day aggregate');
END

-- trigger git_installation_state_no_delete
CREATE TRIGGER git_installation_state_no_delete
BEFORE DELETE ON git_installation_state
BEGIN
  SELECT RAISE(ABORT, 'Git installation state is required');
END

-- trigger git_warning_no_update
CREATE TRIGGER git_warning_no_update
BEFORE UPDATE ON git_warning
BEGIN
  SELECT RAISE(ABORT, 'immutable Git warning');
END

-- trigger quota_epoch_history_no_update
CREATE TRIGGER quota_epoch_history_no_update
BEFORE UPDATE ON quota_epoch_history
BEGIN
  SELECT RAISE(ABORT, 'immutable quota epoch');
END

-- trigger quota_sample_no_update
CREATE TRIGGER quota_sample_no_update
BEFORE UPDATE ON quota_sample
BEGIN
  SELECT RAISE(ABORT, 'immutable quota sample');
END

-- trigger quota_state_no_delete
CREATE TRIGGER quota_state_no_delete
BEFORE DELETE ON quota_state
BEGIN
  SELECT RAISE(ABORT, 'quota state is required');
END

-- trigger quota_transition_no_update
CREATE TRIGGER quota_transition_no_update
BEFORE UPDATE ON quota_transition
BEGIN
  SELECT RAISE(ABORT, 'immutable quota transition');
END

-- trigger quota_window_definition_no_update
CREATE TRIGGER quota_window_definition_no_update
BEFORE UPDATE ON quota_window_definition
BEGIN
  SELECT RAISE(ABORT, 'immutable quota definition');
END

-- trigger usage_event_aggregate_session_after_delete
CREATE TRIGGER usage_event_aggregate_session_after_delete
AFTER DELETE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  SELECT CASE WHEN (
    SELECT count(*) FROM usage_session_rollup
    WHERE aggregate_generation =
          (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
      AND dataset_kind = 'current'
      AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
      AND session_id = OLD.session_id
      AND (
        (dimension_kind = 'all' AND dimension_value = '')
        OR (dimension_kind = 'model' AND dimension_value = OLD.model)
        OR (dimension_kind = 'project'
            AND dimension_value = coalesce(OLD.project_alias, ''))
      )
  ) <> 3 THEN RAISE(ABORT, 'aggregate session rows unavailable') END;
  DELETE FROM usage_session_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND session_id = OLD.session_id AND event_count = 1
    AND (
      (dimension_kind = 'all' AND dimension_value = '')
      OR (dimension_kind = 'model' AND dimension_value = OLD.model)
      OR (dimension_kind = 'project'
          AND dimension_value = coalesce(OLD.project_alias, ''))
    );
  UPDATE usage_session_rollup
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
    AND session_id = OLD.session_id AND event_count > 1
    AND (
      (dimension_kind = 'all' AND dimension_value = '')
      OR (dimension_kind = 'model' AND dimension_value = OLD.model)
      OR (dimension_kind = 'project'
          AND dimension_value = coalesce(OLD.project_alias, ''))
    );
  UPDATE usage_session_rollup
  SET first_timestamp_seconds = (
        SELECT timestamp_seconds FROM usage_event
        WHERE provider_id = OLD.provider_id AND profile_id = OLD.profile_id
          AND session_id = OLD.session_id
        ORDER BY timestamp_seconds, timestamp_nanos, fingerprint LIMIT 1
      ),
      first_timestamp_nanos = (
        SELECT timestamp_nanos FROM usage_event
        WHERE provider_id = OLD.provider_id AND profile_id = OLD.profile_id
          AND session_id = OLD.session_id
        ORDER BY timestamp_seconds, timestamp_nanos, fingerprint LIMIT 1
      ),
      last_timestamp_seconds = (
        SELECT timestamp_seconds FROM usage_event
        WHERE provider_id = OLD.provider_id AND profile_id = OLD.profile_id
          AND session_id = OLD.session_id
        ORDER BY timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC LIMIT 1
      ),
      last_timestamp_nanos = (
        SELECT timestamp_nanos FROM usage_event
        WHERE provider_id = OLD.provider_id AND profile_id = OLD.profile_id
          AND session_id = OLD.session_id
        ORDER BY timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC LIMIT 1
      )
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND dimension_kind = 'all'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND session_id = OLD.session_id;
END

-- trigger usage_event_aggregate_session_after_insert
CREATE TRIGGER usage_event_aggregate_session_after_insert
AFTER INSERT ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  INSERT INTO usage_session_rollup(
    aggregate_generation, dataset_kind, provider_id, profile_id, session_id, dimension_kind,
    dimension_value, event_count, first_timestamp_seconds, first_timestamp_nanos,
    last_timestamp_seconds, last_timestamp_nanos,
    input_known_count, input_known_sum, cached_known_count, cached_known_sum,
    output_known_count, output_known_sum, reasoning_known_count, reasoning_known_sum,
    total_known_count, total_known_sum, fallback_model_count,
    long_context_yes_count, long_context_no_count, long_context_unavailable_count,
    activity_read, activity_edit_write, activity_search, activity_git,
    activity_build_test, activity_web, activity_subagents, activity_terminal
  )
  SELECT
    (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1),
    'current', NEW.provider_id, NEW.profile_id, NEW.session_id,
    dimension.kind, dimension.value, 1,
    CASE WHEN dimension.kind = 'all' THEN NEW.timestamp_seconds END,
    CASE WHEN dimension.kind = 'all' THEN NEW.timestamp_nanos END,
    CASE WHEN dimension.kind = 'all' THEN NEW.timestamp_seconds END,
    CASE WHEN dimension.kind = 'all' THEN NEW.timestamp_nanos END,
    CASE WHEN NEW.input_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.input_tokens, 0),
    CASE WHEN NEW.cached_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.cached_tokens, 0),
    CASE WHEN NEW.output_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.output_tokens, 0),
    CASE WHEN NEW.reasoning_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.reasoning_tokens, 0),
    CASE WHEN NEW.total_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.total_tokens, 0),
    NEW.fallback_model,
    CASE WHEN NEW.long_context = 'yes' THEN 1 ELSE 0 END,
    CASE WHEN NEW.long_context = 'no' THEN 1 ELSE 0 END,
    CASE WHEN NEW.long_context = 'unavailable' THEN 1 ELSE 0 END,
    NEW.activity_read, NEW.activity_edit_write, NEW.activity_search, NEW.activity_git,
    NEW.activity_build_test, NEW.activity_web, NEW.activity_subagents,
    NEW.activity_terminal
  FROM (
    SELECT 'all' AS kind, '' AS value
    UNION ALL SELECT 'model', NEW.model
    UNION ALL SELECT 'project', coalesce(NEW.project_alias, '')
  ) AS dimension
  WHERE true
  ON CONFLICT(
    aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
    dimension_kind, dimension_value
  ) DO UPDATE SET
    event_count = event_count + 1,
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
    activity_terminal = activity_terminal + excluded.activity_terminal;
END

-- trigger usage_event_aggregate_session_after_update
CREATE TRIGGER usage_event_aggregate_session_after_update
AFTER UPDATE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  SELECT CASE WHEN (
    SELECT count(*) FROM usage_session_rollup
    WHERE aggregate_generation =
          (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
      AND dataset_kind = 'current'
      AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
      AND session_id = OLD.session_id
      AND (
        (dimension_kind = 'all' AND dimension_value = '')
        OR (dimension_kind = 'model' AND dimension_value = OLD.model)
        OR (dimension_kind = 'project'
            AND dimension_value = coalesce(OLD.project_alias, ''))
      )
  ) <> 3 THEN RAISE(ABORT, 'aggregate session rows unavailable') END;
  DELETE FROM usage_session_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND session_id = OLD.session_id AND event_count = 1
    AND (
      (dimension_kind = 'all' AND dimension_value = '')
      OR (dimension_kind = 'model' AND dimension_value = OLD.model)
      OR (dimension_kind = 'project'
          AND dimension_value = coalesce(OLD.project_alias, ''))
    );
  UPDATE usage_session_rollup
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
    AND session_id = OLD.session_id AND event_count > 1
    AND (
      (dimension_kind = 'all' AND dimension_value = '')
      OR (dimension_kind = 'model' AND dimension_value = OLD.model)
      OR (dimension_kind = 'project'
          AND dimension_value = coalesce(OLD.project_alias, ''))
    );
  UPDATE usage_session_rollup
  SET first_timestamp_seconds = (
        SELECT timestamp_seconds FROM usage_event
        WHERE provider_id = OLD.provider_id AND profile_id = OLD.profile_id
          AND session_id = OLD.session_id
        ORDER BY timestamp_seconds, timestamp_nanos, fingerprint LIMIT 1
      ),
      first_timestamp_nanos = (
        SELECT timestamp_nanos FROM usage_event
        WHERE provider_id = OLD.provider_id AND profile_id = OLD.profile_id
          AND session_id = OLD.session_id
        ORDER BY timestamp_seconds, timestamp_nanos, fingerprint LIMIT 1
      ),
      last_timestamp_seconds = (
        SELECT timestamp_seconds FROM usage_event
        WHERE provider_id = OLD.provider_id AND profile_id = OLD.profile_id
          AND session_id = OLD.session_id
        ORDER BY timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC LIMIT 1
      ),
      last_timestamp_nanos = (
        SELECT timestamp_nanos FROM usage_event
        WHERE provider_id = OLD.provider_id AND profile_id = OLD.profile_id
          AND session_id = OLD.session_id
        ORDER BY timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC LIMIT 1
      )
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND dimension_kind = 'all'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND session_id = OLD.session_id;
  INSERT INTO usage_session_rollup(
    aggregate_generation, dataset_kind, provider_id, profile_id, session_id, dimension_kind,
    dimension_value, event_count, first_timestamp_seconds, first_timestamp_nanos,
    last_timestamp_seconds, last_timestamp_nanos,
    input_known_count, input_known_sum, cached_known_count, cached_known_sum,
    output_known_count, output_known_sum, reasoning_known_count, reasoning_known_sum,
    total_known_count, total_known_sum, fallback_model_count,
    long_context_yes_count, long_context_no_count, long_context_unavailable_count,
    activity_read, activity_edit_write, activity_search, activity_git,
    activity_build_test, activity_web, activity_subagents, activity_terminal
  )
  SELECT
    (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1),
    'current', NEW.provider_id, NEW.profile_id, NEW.session_id,
    dimension.kind, dimension.value, 1,
    CASE WHEN dimension.kind = 'all' THEN NEW.timestamp_seconds END,
    CASE WHEN dimension.kind = 'all' THEN NEW.timestamp_nanos END,
    CASE WHEN dimension.kind = 'all' THEN NEW.timestamp_seconds END,
    CASE WHEN dimension.kind = 'all' THEN NEW.timestamp_nanos END,
    CASE WHEN NEW.input_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.input_tokens, 0),
    CASE WHEN NEW.cached_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.cached_tokens, 0),
    CASE WHEN NEW.output_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.output_tokens, 0),
    CASE WHEN NEW.reasoning_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.reasoning_tokens, 0),
    CASE WHEN NEW.total_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.total_tokens, 0),
    NEW.fallback_model,
    CASE WHEN NEW.long_context = 'yes' THEN 1 ELSE 0 END,
    CASE WHEN NEW.long_context = 'no' THEN 1 ELSE 0 END,
    CASE WHEN NEW.long_context = 'unavailable' THEN 1 ELSE 0 END,
    NEW.activity_read, NEW.activity_edit_write, NEW.activity_search, NEW.activity_git,
    NEW.activity_build_test, NEW.activity_web, NEW.activity_subagents,
    NEW.activity_terminal
  FROM (
    SELECT 'all' AS kind, '' AS value
    UNION ALL SELECT 'model', NEW.model
    UNION ALL SELECT 'project', coalesce(NEW.project_alias, '')
  ) AS dimension
  WHERE true
  ON CONFLICT(
    aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
    dimension_kind, dimension_value
  ) DO UPDATE SET
    event_count = event_count + 1,
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
    activity_terminal = activity_terminal + excluded.activity_terminal;
END

-- trigger usage_event_aggregate_time_after_delete
CREATE TRIGGER usage_event_aggregate_time_after_delete
AFTER DELETE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
SELECT CASE WHEN (
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'all' AND dimension_value = '') +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'model' AND dimension_value = OLD.model) +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '')) +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'all' AND dimension_value = '') +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'model' AND dimension_value = OLD.model) +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, ''))
  ) <> 6 THEN RAISE(ABORT, 'aggregate time rows unavailable') END;
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'all' AND dimension_value = '';
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'model' AND dimension_value = OLD.model;
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '');
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'all' AND dimension_value = '';
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'model' AND dimension_value = OLD.model;
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '');
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'all' AND dimension_value = '';
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'model' AND dimension_value = OLD.model;
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '');
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'all' AND dimension_value = '';
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'model' AND dimension_value = OLD.model;
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '');
END

-- trigger usage_event_aggregate_time_after_insert
CREATE TRIGGER usage_event_aggregate_time_after_insert
AFTER INSERT ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  INSERT INTO usage_time_rollup(
    aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
    provider_id, profile_id,
    dimension_kind, dimension_value, event_count,
    input_known_count, input_known_sum, cached_known_count, cached_known_sum,
    output_known_count, output_known_sum, reasoning_known_count, reasoning_known_sum,
    total_known_count, total_known_sum, fallback_model_count,
    long_context_yes_count, long_context_no_count, long_context_unavailable_count,
    activity_read, activity_edit_write, activity_search, activity_git,
    activity_build_test, activity_web, activity_subagents, activity_terminal
  )
  SELECT
    (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1),
    'current', bucket.width,
    NEW.timestamp_seconds -
      (((NEW.timestamp_seconds % bucket.seconds) + bucket.seconds) % bucket.seconds),
    NEW.provider_id, NEW.profile_id, dimension.kind, dimension.value, 1,
    CASE WHEN NEW.input_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.input_tokens, 0),
    CASE WHEN NEW.cached_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.cached_tokens, 0),
    CASE WHEN NEW.output_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.output_tokens, 0),
    CASE WHEN NEW.reasoning_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.reasoning_tokens, 0),
    CASE WHEN NEW.total_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.total_tokens, 0),
    NEW.fallback_model,
    CASE WHEN NEW.long_context = 'yes' THEN 1 ELSE 0 END,
    CASE WHEN NEW.long_context = 'no' THEN 1 ELSE 0 END,
    CASE WHEN NEW.long_context = 'unavailable' THEN 1 ELSE 0 END,
    NEW.activity_read, NEW.activity_edit_write, NEW.activity_search, NEW.activity_git,
    NEW.activity_build_test, NEW.activity_web, NEW.activity_subagents,
    NEW.activity_terminal
  FROM (
    SELECT 'minute' AS width, 60 AS seconds
    UNION ALL SELECT 'hour', 3600
  ) AS bucket
  CROSS JOIN (
    SELECT 'all' AS kind, '' AS value
    UNION ALL SELECT 'model', NEW.model
    UNION ALL SELECT 'project', coalesce(NEW.project_alias, '')
  ) AS dimension
  WHERE true
  ON CONFLICT(
    aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
    provider_id, profile_id,
    dimension_kind, dimension_value
  ) DO UPDATE SET
    event_count = event_count + 1,
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
    activity_terminal = activity_terminal + excluded.activity_terminal;
END

-- trigger usage_event_aggregate_time_after_update
CREATE TRIGGER usage_event_aggregate_time_after_update
AFTER UPDATE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
SELECT CASE WHEN (
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'all' AND dimension_value = '') +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'model' AND dimension_value = OLD.model) +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '')) +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'all' AND dimension_value = '') +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'model' AND dimension_value = OLD.model) +
    (SELECT count(*) FROM usage_time_rollup
     WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current'
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, ''))
  ) <> 6 THEN RAISE(ABORT, 'aggregate time rows unavailable') END;
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'all' AND dimension_value = '';
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'model' AND dimension_value = OLD.model;
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '');
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'all' AND dimension_value = '';
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'model' AND dimension_value = OLD.model;
  DELETE FROM usage_time_rollup
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count = 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '');
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'all' AND dimension_value = '';
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'model' AND dimension_value = OLD.model;
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'minute' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '');
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'all' AND dimension_value = '';
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'model' AND dimension_value = OLD.model;
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
  WHERE aggregate_generation = (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current'
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND event_count > 1
    AND bucket_width = 'hour' AND bucket_start_seconds = OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600) AND dimension_kind = 'project' AND dimension_value = coalesce(OLD.project_alias, '');
  INSERT INTO usage_time_rollup(
    aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
    provider_id, profile_id,
    dimension_kind, dimension_value, event_count,
    input_known_count, input_known_sum, cached_known_count, cached_known_sum,
    output_known_count, output_known_sum, reasoning_known_count, reasoning_known_sum,
    total_known_count, total_known_sum, fallback_model_count,
    long_context_yes_count, long_context_no_count, long_context_unavailable_count,
    activity_read, activity_edit_write, activity_search, activity_git,
    activity_build_test, activity_web, activity_subagents, activity_terminal
  )
  SELECT
    (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1),
    'current', bucket.width,
    NEW.timestamp_seconds -
      (((NEW.timestamp_seconds % bucket.seconds) + bucket.seconds) % bucket.seconds),
    NEW.provider_id, NEW.profile_id, dimension.kind, dimension.value, 1,
    CASE WHEN NEW.input_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.input_tokens, 0),
    CASE WHEN NEW.cached_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.cached_tokens, 0),
    CASE WHEN NEW.output_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.output_tokens, 0),
    CASE WHEN NEW.reasoning_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.reasoning_tokens, 0),
    CASE WHEN NEW.total_tokens IS NULL THEN 0 ELSE 1 END,
    coalesce(NEW.total_tokens, 0),
    NEW.fallback_model,
    CASE WHEN NEW.long_context = 'yes' THEN 1 ELSE 0 END,
    CASE WHEN NEW.long_context = 'no' THEN 1 ELSE 0 END,
    CASE WHEN NEW.long_context = 'unavailable' THEN 1 ELSE 0 END,
    NEW.activity_read, NEW.activity_edit_write, NEW.activity_search, NEW.activity_git,
    NEW.activity_build_test, NEW.activity_web, NEW.activity_subagents,
    NEW.activity_terminal
  FROM (
    SELECT 'minute' AS width, 60 AS seconds
    UNION ALL SELECT 'hour', 3600
  ) AS bucket
  CROSS JOIN (
    SELECT 'all' AS kind, '' AS value
    UNION ALL SELECT 'model', NEW.model
    UNION ALL SELECT 'project', coalesce(NEW.project_alias, '')
  ) AS dimension
  WHERE true
  ON CONFLICT(
    aggregate_generation, dataset_kind, bucket_width, bucket_start_seconds,
    provider_id, profile_id,
    dimension_kind, dimension_value
  ) DO UPDATE SET
    event_count = event_count + 1,
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
    activity_terminal = activity_terminal + excluded.activity_terminal;
END

-- trigger usage_event_dataset_generation_after_delete
CREATE TRIGGER usage_event_dataset_generation_after_delete
AFTER DELETE ON usage_event
BEGIN
  SELECT CASE
    WHEN (SELECT count(*) FROM usage_archive_state WHERE singleton_id = 1) <> 1
      THEN RAISE(ABORT, 'dataset generation unavailable')
    WHEN (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1)
         = 9223372036854775807
      THEN RAISE(ABORT, 'dataset generation exhausted')
    WHEN (SELECT count(*) FROM usage_aggregate_state WHERE singleton_id = 1) <> 1
      THEN RAISE(ABORT, 'aggregate state unavailable')
    WHEN (SELECT current_event_count FROM usage_aggregate_state WHERE singleton_id = 1) = 0
      THEN RAISE(ABORT, 'aggregate event count underflow')
  END;
  UPDATE usage_archive_state
  SET dataset_generation = dataset_generation + 1
  WHERE singleton_id = 1;
  UPDATE usage_aggregate_state
  SET expected_dataset_generation =
        (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1),
      current_event_count = current_event_count - 1,
      state = CASE WHEN state = 'ready' THEN 'ready' ELSE 'rebuild_required' END,
      failure_code = NULL,
      rebuild_aggregate_generation = NULL,
      rebuild_dataset_kind = NULL,
      rebuild_cursor_fingerprint = NULL,
      rebuild_processed_events = 0,
      rebuild_total_events = current_event_count - 1 + legacy_event_count
  WHERE singleton_id = 1;
END

-- trigger usage_event_dataset_generation_after_insert
CREATE TRIGGER usage_event_dataset_generation_after_insert
AFTER INSERT ON usage_event
BEGIN
  SELECT CASE
    WHEN (SELECT count(*) FROM usage_archive_state WHERE singleton_id = 1) <> 1
      THEN RAISE(ABORT, 'dataset generation unavailable')
    WHEN (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1)
         = 9223372036854775807
      THEN RAISE(ABORT, 'dataset generation exhausted')
    WHEN (SELECT count(*) FROM usage_aggregate_state WHERE singleton_id = 1) <> 1
      THEN RAISE(ABORT, 'aggregate state unavailable')
    WHEN (SELECT current_event_count FROM usage_aggregate_state WHERE singleton_id = 1)
         = 9223372036854775807
      THEN RAISE(ABORT, 'aggregate event count exhausted')
  END;
  UPDATE usage_archive_state
  SET dataset_generation = dataset_generation + 1
  WHERE singleton_id = 1;
  UPDATE usage_aggregate_state
  SET expected_dataset_generation =
        (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1),
      current_event_count = current_event_count + 1,
      state = CASE WHEN state = 'ready' THEN 'ready' ELSE 'rebuild_required' END,
      failure_code = NULL,
      rebuild_aggregate_generation = NULL,
      rebuild_dataset_kind = NULL,
      rebuild_cursor_fingerprint = NULL,
      rebuild_processed_events = 0,
      rebuild_total_events = current_event_count + 1 + legacy_event_count
  WHERE singleton_id = 1;
END

-- trigger usage_event_dataset_generation_after_update
CREATE TRIGGER usage_event_dataset_generation_after_update
AFTER UPDATE ON usage_event
BEGIN
  SELECT CASE
    WHEN (SELECT count(*) FROM usage_archive_state WHERE singleton_id = 1) <> 1
      THEN RAISE(ABORT, 'dataset generation unavailable')
    WHEN (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1)
         = 9223372036854775807
      THEN RAISE(ABORT, 'dataset generation exhausted')
    WHEN (SELECT count(*) FROM usage_aggregate_state WHERE singleton_id = 1) <> 1
      THEN RAISE(ABORT, 'aggregate state unavailable')
  END;
  UPDATE usage_archive_state
  SET dataset_generation = dataset_generation + 1
  WHERE singleton_id = 1;
  UPDATE usage_aggregate_state
  SET expected_dataset_generation =
        (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1),
      state = CASE WHEN state = 'ready' THEN 'ready' ELSE 'rebuild_required' END,
      failure_code = NULL,
      rebuild_aggregate_generation = NULL,
      rebuild_dataset_kind = NULL,
      rebuild_cursor_fingerprint = NULL,
      rebuild_processed_events = 0,
      rebuild_total_events = current_event_count + legacy_event_count
  WHERE singleton_id = 1;
END

-- trigger usage_event_price_session_after_delete
CREATE TRIGGER usage_event_price_session_after_delete
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
      AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
      AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
  ) <> 1 THEN RAISE(ABORT, 'price session row unavailable') END;
  DELETE FROM usage_price_session_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND provider_id = OLD.provider_id
    AND profile_id = OLD.profile_id AND session_id = OLD.session_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1;
  UPDATE usage_price_session_rollup
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
    AND profile_id = OLD.profile_id AND session_id = OLD.session_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1;
END

-- trigger usage_event_price_session_after_insert
CREATE TRIGGER usage_event_price_session_after_insert
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
    NEW.model, coalesce(NEW.project_alias, ''), CASE WHEN NEW.service_tier IS NULL THEN 'standard_assumed' WHEN lower(NEW.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(NEW.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END, NEW.long_context,
    CASE WHEN NEW.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END,
    1, CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN 1 ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN NEW.input_tokens - NEW.cached_tokens ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN NEW.cached_tokens ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN
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
END

-- trigger usage_event_price_session_after_update
CREATE TRIGGER usage_event_price_session_after_update
AFTER UPDATE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  SELECT CASE WHEN (
    SELECT count(*) FROM usage_price_session_rollup
    WHERE aggregate_generation =
          (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
      AND dataset_kind = 'current' AND provider_id = OLD.provider_id
      AND profile_id = OLD.profile_id AND session_id = OLD.session_id AND model = OLD.model
      AND project_key = coalesce(OLD.project_alias, '')
      AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
      AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
  ) <> 1 THEN RAISE(ABORT, 'price session row unavailable') END;
  DELETE FROM usage_price_session_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND provider_id = OLD.provider_id
    AND profile_id = OLD.profile_id AND session_id = OLD.session_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1;
  UPDATE usage_price_session_rollup
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
    AND profile_id = OLD.profile_id AND session_id = OLD.session_id AND model = OLD.model
    AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1;
  INSERT INTO usage_price_session_rollup(
    aggregate_generation, dataset_kind, provider_id, profile_id, session_id,
    model, project_key, service_tier, long_context, reported_state,
    event_count, calculable_event_count, uncached_input_sum, cached_input_sum,
    billable_output_sum, reported_cost_count, reported_cost_sum
  ) VALUES (
    (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1),
    'current', NEW.provider_id, NEW.profile_id, NEW.session_id,
    NEW.model, coalesce(NEW.project_alias, ''), CASE WHEN NEW.service_tier IS NULL THEN 'standard_assumed' WHEN lower(NEW.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(NEW.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END, NEW.long_context,
    CASE WHEN NEW.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END,
    1, CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN 1 ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN NEW.input_tokens - NEW.cached_tokens ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN NEW.cached_tokens ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN
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
END

-- trigger usage_event_price_time_after_delete
CREATE TRIGGER usage_event_price_time_after_delete
AFTER DELETE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  SELECT CASE WHEN
    (SELECT count(*) FROM usage_price_time_rollup
     WHERE aggregate_generation =
           (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current' AND bucket_width = 'minute'
       AND bucket_start_seconds =
           OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60)
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
       AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
       AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END)
    +
    (SELECT count(*) FROM usage_price_time_rollup
     WHERE aggregate_generation =
           (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current' AND bucket_width = 'hour'
       AND bucket_start_seconds =
           OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
       AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
       AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END)
    <> 2 THEN RAISE(ABORT, 'price time rows unavailable') END;
  DELETE FROM usage_price_time_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND bucket_width = 'minute'
    AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1;
  DELETE FROM usage_price_time_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND bucket_width = 'hour'
    AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1;
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
    AND dataset_kind = 'current' AND bucket_width = 'minute'
    AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1;
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
    AND dataset_kind = 'current' AND bucket_width = 'hour'
    AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1;
END

-- trigger usage_event_price_time_after_insert
CREATE TRIGGER usage_event_price_time_after_insert
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
    CASE WHEN NEW.service_tier IS NULL THEN 'standard_assumed' WHEN lower(NEW.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(NEW.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END, NEW.long_context,
    CASE WHEN NEW.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END,
    1, CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN 1 ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN NEW.input_tokens - NEW.cached_tokens ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN NEW.cached_tokens ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN
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
END

-- trigger usage_event_price_time_after_update
CREATE TRIGGER usage_event_price_time_after_update
AFTER UPDATE ON usage_event
WHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'
BEGIN
  SELECT CASE WHEN
    (SELECT count(*) FROM usage_price_time_rollup
     WHERE aggregate_generation =
           (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current' AND bucket_width = 'minute'
       AND bucket_start_seconds =
           OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60)
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
       AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
       AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END)
    +
    (SELECT count(*) FROM usage_price_time_rollup
     WHERE aggregate_generation =
           (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
       AND dataset_kind = 'current' AND bucket_width = 'hour'
       AND bucket_start_seconds =
           OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)
       AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
       AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
       AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
       AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END)
    <> 2 THEN RAISE(ABORT, 'price time rows unavailable') END;
  DELETE FROM usage_price_time_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND bucket_width = 'minute'
    AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1;
  DELETE FROM usage_price_time_rollup
  WHERE aggregate_generation =
        (SELECT active_aggregate_generation FROM usage_aggregate_state WHERE singleton_id = 1)
    AND dataset_kind = 'current' AND bucket_width = 'hour'
    AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count = 1;
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
    AND dataset_kind = 'current' AND bucket_width = 'minute'
    AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 60) + 60) % 60)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1;
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
    AND dataset_kind = 'current' AND bucket_width = 'hour'
    AND bucket_start_seconds =
        OLD.timestamp_seconds - (((OLD.timestamp_seconds % 3600) + 3600) % 3600)
    AND provider_id = OLD.provider_id AND profile_id = OLD.profile_id
    AND model = OLD.model AND project_key = coalesce(OLD.project_alias, '')
    AND service_tier = CASE WHEN OLD.service_tier IS NULL THEN 'standard_assumed' WHEN lower(OLD.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(OLD.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END AND long_context = OLD.long_context
    AND reported_state = CASE WHEN OLD.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END
    AND event_count > 1;
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
    CASE WHEN NEW.service_tier IS NULL THEN 'standard_assumed' WHEN lower(NEW.service_tier) IN ('standard','default') THEN 'standard_reported' WHEN lower(NEW.service_tier) IN ('priority','fast') THEN 'priority' ELSE 'unknown' END, NEW.long_context,
    CASE WHEN NEW.reported_cost_usd_micros IS NULL THEN 'missing' ELSE 'present' END,
    1, CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN 1 ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN NEW.input_tokens - NEW.cached_tokens ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN NEW.cached_tokens ELSE 0 END,
    CASE WHEN NEW.input_tokens IS NOT NULL AND NEW.cached_tokens IS NOT NULL AND NEW.cached_tokens <= NEW.input_tokens AND ((NEW.total_tokens IS NOT NULL AND NEW.total_tokens >= NEW.input_tokens AND (NEW.output_tokens IS NULL OR NEW.reasoning_tokens IS NULL OR (NEW.output_tokens <= NEW.total_tokens - NEW.input_tokens AND NEW.reasoning_tokens = NEW.total_tokens - NEW.input_tokens - NEW.output_tokens))) OR (NEW.total_tokens IS NULL AND NEW.output_tokens IS NOT NULL AND NEW.reasoning_tokens IS NOT NULL AND NEW.output_tokens <= 9223372036854775807 - NEW.reasoning_tokens)) THEN
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
END

-- trigger usage_legacy_event_no_delete
CREATE TRIGGER usage_legacy_event_no_delete
BEFORE DELETE ON usage_legacy_event
BEGIN
  SELECT RAISE(ABORT, 'immutable legacy snapshot');
END

-- trigger usage_legacy_event_no_insert
CREATE TRIGGER usage_legacy_event_no_insert
BEFORE INSERT ON usage_legacy_event
BEGIN
  SELECT RAISE(ABORT, 'immutable legacy snapshot');
END

-- trigger usage_legacy_event_no_update
CREATE TRIGGER usage_legacy_event_no_update
BEFORE UPDATE ON usage_legacy_event
BEGIN
  SELECT RAISE(ABORT, 'immutable legacy snapshot');
END
