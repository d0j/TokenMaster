pub const USAGE_SCHEMA_VERSION: i64 = 1;

pub(super) struct TableContract {
    pub(super) name: &'static str,
    pub(super) columns: &'static [&'static str],
}

pub(super) struct IndexContract {
    pub(super) name: &'static str,
    pub(super) sql: &'static str,
}

pub(super) const USAGE_INDEX_CONTRACTS: &[IndexContract] = &[
    IndexContract {
        name: "usage_event_model_time",
        sql: "CREATE INDEX usage_event_model_time ON usage_event(model, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC)",
    },
    IndexContract {
        name: "usage_event_time_desc",
        sql: "CREATE INDEX usage_event_time_desc ON usage_event(timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC)",
    },
    IndexContract {
        name: "usage_generation_one_current",
        sql: "CREATE UNIQUE INDEX usage_generation_one_current ON usage_generation(file_key) WHERE status = 'current'",
    },
    IndexContract {
        name: "usage_generation_one_staging",
        sql: "CREATE UNIQUE INDEX usage_generation_one_staging ON usage_generation(file_key) WHERE status = 'staging'",
    },
    IndexContract {
        name: "usage_observation_fingerprint",
        sql: "CREATE INDEX usage_observation_fingerprint ON usage_observation(fingerprint, profile_id, file_key, generation, source_offset)",
    },
];

pub(super) const USAGE_TABLE_CONTRACTS: &[TableContract] = &[
    TableContract {
        name: "usage_scan",
        columns: &[
            "scan_id",
            "profile_id",
            "started_at_ms",
            "completed_at_ms",
            "completion_state",
            "sources_seen",
            "files_read",
            "bytes_read",
            "events_observed",
            "diagnostics",
        ],
    },
    TableContract {
        name: "usage_source",
        columns: &[
            "file_key",
            "provider_id",
            "profile_id",
            "source_id",
            "source_kind",
            "logical_identity",
            "physical_identity",
            "current_generation",
            "last_seen_scan_id",
            "missing",
            "last_error_code",
            "verification_level",
            "diagnostic_count",
        ],
    },
    TableContract {
        name: "usage_generation",
        columns: &[
            "file_key",
            "generation",
            "status",
            "parser_schema_version",
            "physical_identity",
            "logical_identity",
            "committed_offset",
            "scan_offset",
            "observed_file_length",
            "modified_time_ns",
            "anchor_start",
            "anchor_len",
            "anchor_sha256",
            "resume_payload",
            "discarding_oversized_line",
            "incomplete_tail",
            "verification_level",
        ],
    },
    TableContract {
        name: "usage_source_chunk",
        columns: &[
            "file_key",
            "generation",
            "chunk_index",
            "covered_len",
            "sha256",
        ],
    },
    TableContract {
        name: "usage_observation",
        columns: &[
            "file_key",
            "generation",
            "source_offset",
            "fingerprint",
            "event_id",
            "profile_id",
            "session_id",
            "source_id",
            "timestamp_seconds",
            "timestamp_nanos",
            "model",
            "raw_model",
            "input_tokens",
            "cached_tokens",
            "output_tokens",
            "reasoning_tokens",
            "total_tokens",
            "fallback_model",
            "long_context",
            "service_tier",
            "project_alias",
            "originator",
            "activity_read",
            "activity_edit_write",
            "activity_search",
            "activity_git",
            "activity_build_test",
            "activity_web",
            "activity_subagents",
            "activity_terminal",
        ],
    },
    TableContract {
        name: "usage_event",
        columns: &[
            "fingerprint",
            "event_id",
            "selected_file_key",
            "selected_generation",
            "selected_source_offset",
            "profile_id",
            "session_id",
            "source_id",
            "timestamp_seconds",
            "timestamp_nanos",
            "model",
            "raw_model",
            "input_tokens",
            "cached_tokens",
            "output_tokens",
            "reasoning_tokens",
            "total_tokens",
            "fallback_model",
            "long_context",
            "service_tier",
            "project_alias",
            "originator",
            "activity_read",
            "activity_edit_write",
            "activity_search",
            "activity_git",
            "activity_build_test",
            "activity_web",
            "activity_subagents",
            "activity_terminal",
        ],
    },
];

pub(super) const USAGE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS usage_scan (
  scan_id INTEGER PRIMARY KEY CHECK(scan_id >= 0),
  profile_id TEXT NOT NULL CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  started_at_ms INTEGER NOT NULL,
  completed_at_ms INTEGER,
  completion_state TEXT NOT NULL CHECK(completion_state IN ('running','complete','partial','cancelled','failed')),
  sources_seen INTEGER NOT NULL DEFAULT 0 CHECK(sources_seen >= 0),
  files_read INTEGER NOT NULL DEFAULT 0 CHECK(files_read >= 0),
  bytes_read INTEGER NOT NULL DEFAULT 0 CHECK(bytes_read >= 0),
  events_observed INTEGER NOT NULL DEFAULT 0 CHECK(events_observed >= 0),
  diagnostics INTEGER NOT NULL DEFAULT 0 CHECK(diagnostics >= 0)
) STRICT;

CREATE TABLE IF NOT EXISTS usage_source (
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
) STRICT;

CREATE TABLE IF NOT EXISTS usage_generation (
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
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS usage_generation_one_current
  ON usage_generation(file_key) WHERE status = 'current';
CREATE UNIQUE INDEX IF NOT EXISTS usage_generation_one_staging
  ON usage_generation(file_key) WHERE status = 'staging';

CREATE TABLE IF NOT EXISTS usage_source_chunk (
  file_key BLOB NOT NULL CHECK(length(file_key) = 32),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  chunk_index INTEGER NOT NULL CHECK(chunk_index >= 0),
  covered_len INTEGER NOT NULL CHECK(covered_len BETWEEN 1 AND 1048576),
  sha256 BLOB NOT NULL CHECK(length(sha256) = 32),
  PRIMARY KEY(file_key, generation, chunk_index),
  FOREIGN KEY(file_key, generation)
    REFERENCES usage_generation(file_key, generation) ON DELETE CASCADE
) STRICT;

CREATE TABLE IF NOT EXISTS usage_observation (
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
  activity_terminal INTEGER NOT NULL CHECK(activity_terminal >= 0),
  PRIMARY KEY(file_key, generation, source_offset, fingerprint),
  FOREIGN KEY(file_key, generation)
    REFERENCES usage_generation(file_key, generation) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS usage_observation_fingerprint
  ON usage_observation(fingerprint, profile_id, file_key, generation, source_offset);

CREATE TABLE IF NOT EXISTS usage_event (
  fingerprint BLOB PRIMARY KEY CHECK(length(fingerprint) = 32),
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
  activity_terminal INTEGER NOT NULL CHECK(activity_terminal >= 0),
  FOREIGN KEY(selected_file_key, selected_generation, selected_source_offset, fingerprint)
    REFERENCES usage_observation(file_key, generation, source_offset, fingerprint)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE INDEX IF NOT EXISTS usage_event_time_desc
  ON usage_event(timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC);
CREATE INDEX IF NOT EXISTS usage_event_model_time
  ON usage_event(model, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC);
"#;
