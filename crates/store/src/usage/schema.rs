pub const USAGE_SCHEMA_VERSION: i64 = 6;
pub(super) const V1_SCHEMA_VERSION: i64 = 1;
pub(super) const V2_SCHEMA_VERSION: i64 = 2;
pub(super) const V3_SCHEMA_VERSION: i64 = 3;
pub(super) const V4_SCHEMA_VERSION: i64 = 4;
pub(super) const V5_SCHEMA_VERSION: i64 = 5;

pub(super) struct TableContract {
    pub(super) name: &'static str,
    pub(super) columns: &'static [&'static str],
}

pub(super) struct IndexContract {
    pub(super) name: &'static str,
    pub(super) sql: &'static str,
}

pub(super) struct TriggerContract {
    pub(super) name: &'static str,
    pub(super) sql: &'static str,
}

pub(super) const V1_TABLE_COUNT: usize = 6;

pub(super) const V1_INDEX_CONTRACTS: &[IndexContract] = &[
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
        name: "usage_legacy_event_model_time",
        sql: "CREATE INDEX usage_legacy_event_model_time ON usage_legacy_event(snapshot_id, model, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC)",
    },
    IndexContract {
        name: "usage_legacy_event_time_desc",
        sql: "CREATE INDEX usage_legacy_event_time_desc ON usage_legacy_event(snapshot_id, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC)",
    },
    IndexContract {
        name: "usage_observation_fingerprint",
        sql: "CREATE INDEX usage_observation_fingerprint ON usage_observation(fingerprint, profile_id, file_key, generation, source_offset)",
    },
    IndexContract {
        name: "usage_replay_observation_children",
        sql: "CREATE INDEX usage_replay_observation_children ON usage_replay_observation(revision_id, provider_id, profile_id, parent_session_id, session_ordinal, disposition, session_id)",
    },
    IndexContract {
        name: "usage_replay_observation_disposition",
        sql: "CREATE INDEX usage_replay_observation_disposition ON usage_replay_observation(revision_id, disposition)",
    },
    IndexContract {
        name: "usage_replay_observation_fingerprint",
        sql: "CREATE INDEX usage_replay_observation_fingerprint ON usage_replay_observation(revision_id, fingerprint, disposition, file_key, generation, source_offset)",
    },
    IndexContract {
        name: "usage_replay_observation_parent",
        sql: "CREATE INDEX usage_replay_observation_parent ON usage_replay_observation(revision_id, provider_id, profile_id, session_id, session_ordinal)",
    },
    IndexContract {
        name: "usage_replay_revision_one_current",
        sql: "CREATE UNIQUE INDEX usage_replay_revision_one_current ON usage_replay_revision(status) WHERE status = 'current'",
    },
    IndexContract {
        name: "usage_replay_revision_one_staging",
        sql: "CREATE UNIQUE INDEX usage_replay_revision_one_staging ON usage_replay_revision(status) WHERE status = 'staging'",
    },
];

pub(super) const V5_INDEX_CONTRACTS: &[IndexContract] = &[
    IndexContract {
        name: "usage_scan_one_running_scope",
        sql: "CREATE UNIQUE INDEX usage_scan_one_running_scope ON usage_scan(provider_id, profile_id) WHERE completion_state = 'running'",
    },
    IndexContract {
        name: "usage_scan_set_one_running",
        sql: "CREATE UNIQUE INDEX usage_scan_set_one_running ON usage_scan_set(completion_state) WHERE completion_state = 'running'",
    },
    IndexContract {
        name: "usage_source_scope_missing",
        sql: "CREATE INDEX usage_source_scope_missing ON usage_source(provider_id, profile_id, missing, file_key)",
    },
];

pub(super) const USAGE_TRIGGER_CONTRACTS: &[TriggerContract] = &[
    TriggerContract {
        name: "usage_legacy_event_no_delete",
        sql: "CREATE TRIGGER usage_legacy_event_no_delete BEFORE DELETE ON usage_legacy_event BEGIN SELECT RAISE(ABORT, 'immutable legacy snapshot'); END",
    },
    TriggerContract {
        name: "usage_legacy_event_no_insert",
        sql: "CREATE TRIGGER usage_legacy_event_no_insert BEFORE INSERT ON usage_legacy_event BEGIN SELECT RAISE(ABORT, 'immutable legacy snapshot'); END",
    },
    TriggerContract {
        name: "usage_legacy_event_no_update",
        sql: "CREATE TRIGGER usage_legacy_event_no_update BEFORE UPDATE ON usage_legacy_event BEGIN SELECT RAISE(ABORT, 'immutable legacy snapshot'); END",
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
            "projection_revision_id",
            "origin_revision_id",
            "retained",
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
        name: "usage_legacy_snapshot",
        columns: &[
            "snapshot_id",
            "source_schema_version",
            "quality_state",
            "event_count",
        ],
    },
    TableContract {
        name: "usage_legacy_event",
        columns: &[
            "snapshot_id",
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
    TableContract {
        name: "usage_replay_revision",
        columns: &[
            "revision_id",
            "status",
            "canonicalizer_version",
            "fingerprint_version",
            "replay_signature_version",
            "expected_source_count",
            "evidence_epoch",
            "sealed",
            "promoted",
        ],
    },
    TableContract {
        name: "usage_replay_source",
        columns: &["revision_id", "file_key", "generation", "state"],
    },
    TableContract {
        name: "usage_replay_session",
        columns: &[
            "revision_id",
            "provider_id",
            "profile_id",
            "session_id",
            "parent_session_id",
            "relation_conflict",
            "state",
            "completion_state",
            "first_relation_file_key",
            "first_relation_source_offset",
            "last_classified_ordinal",
            "evidence_epoch",
        ],
    },
    TableContract {
        name: "usage_replay_observation",
        columns: &[
            "revision_id",
            "file_key",
            "generation",
            "source_offset",
            "fingerprint",
            "provider_id",
            "profile_id",
            "session_id",
            "parent_session_id",
            "session_ordinal",
            "canonicalizer_version",
            "fingerprint_version",
            "replay_signature_version",
            "replay_signature",
            "evidence",
            "disposition",
            "declared_conflict",
            "evidence_epoch",
        ],
    },
    TableContract {
        name: "usage_replay_selection",
        columns: &[
            "revision_id",
            "fingerprint",
            "file_key",
            "generation",
            "source_offset",
            "canonicalizer_version",
            "fingerprint_version",
            "replay_signature_version",
        ],
    },
    TableContract {
        name: "usage_replay_work",
        columns: &[
            "revision_id",
            "work_kind",
            "provider_id",
            "profile_id",
            "session_id",
            "reason",
            "next_ordinal",
            "child_session_cursor",
            "expected_evidence_epoch",
        ],
    },
];

pub(super) const V5_SCAN_SET_CONTRACT: TableContract = TableContract {
    name: "usage_scan_set",
    columns: &[
        "scan_set_id",
        "started_at_ms",
        "completed_at_ms",
        "completion_state",
        "expected_scope_count",
    ],
};

pub(super) const V5_USAGE_SCAN_CONTRACT: TableContract = TableContract {
    name: "usage_scan",
    columns: &[
        "scan_id",
        "scan_set_id",
        "provider_id",
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
};

pub(super) const V5_REPLAY_REVISION_CONTRACT: TableContract = TableContract {
    name: "usage_replay_revision",
    columns: &[
        "revision_id",
        "status",
        "canonicalizer_version",
        "fingerprint_version",
        "replay_signature_version",
        "expected_source_count",
        "evidence_epoch",
        "sealed",
        "promoted",
        "scan_set_id",
    ],
};

pub(super) const V6_ARCHIVE_STATE_CONTRACT: TableContract = TableContract {
    name: "usage_archive_state",
    columns: &[
        "singleton_id",
        "archive_generation",
        "current_revision_id",
        "latest_complete_scan_set_id",
        "incremental_state",
    ],
};

pub(super) const PRE_V4_USAGE_EVENT_CONTRACT: TableContract = TableContract {
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
};

pub(super) const V1_SCHEMA: &str = r#"
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

pub(super) const REPLAY_AUX_SCHEMA: &str = r#"
CREATE TABLE usage_legacy_snapshot (
  snapshot_id INTEGER PRIMARY KEY CHECK(snapshot_id = 1),
  source_schema_version INTEGER NOT NULL CHECK(source_schema_version = 1),
  quality_state TEXT NOT NULL CHECK(quality_state = 'legacy_unverified'),
  event_count INTEGER NOT NULL CHECK(event_count >= 0)
) STRICT;

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
  activity_terminal INTEGER NOT NULL CHECK(activity_terminal >= 0),
  PRIMARY KEY(snapshot_id, fingerprint),
  FOREIGN KEY(snapshot_id) REFERENCES usage_legacy_snapshot(snapshot_id)
) STRICT;

CREATE INDEX usage_legacy_event_time_desc
  ON usage_legacy_event(snapshot_id, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC);
CREATE INDEX usage_legacy_event_model_time
  ON usage_legacy_event(snapshot_id, model, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC);
"#;

pub(super) const V2_REPLAY_REVISION_SCHEMA: &str = r#"
CREATE TABLE usage_replay_revision (
  revision_id INTEGER PRIMARY KEY CHECK(revision_id >= 0),
  status TEXT NOT NULL CHECK(status IN ('staging','current')),
  canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
  fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
  replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
  expected_source_count INTEGER NOT NULL CHECK(expected_source_count BETWEEN 1 AND 256),
  evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
  sealed INTEGER NOT NULL CHECK(sealed IN (0,1)),
  promoted INTEGER NOT NULL CHECK(promoted IN (0,1)),
  CHECK((status = 'staging' AND promoted = 0) OR
        (status = 'current' AND sealed = 1 AND promoted = 1))
) STRICT;
"#;

pub(super) const V3_REPLAY_REVISION_SCHEMA: &str = r#"
CREATE TABLE usage_replay_revision (
  revision_id INTEGER PRIMARY KEY CHECK(revision_id >= 0),
  status TEXT NOT NULL CHECK(status IN ('staging','current')),
  canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
  fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
  replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
  expected_source_count INTEGER NOT NULL CHECK(expected_source_count >= 1),
  evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
  sealed INTEGER NOT NULL CHECK(sealed IN (0,1)),
  promoted INTEGER NOT NULL CHECK(promoted IN (0,1)),
  CHECK((status = 'staging' AND promoted = 0) OR
        (status = 'current' AND sealed = 1 AND promoted = 1))
) STRICT;
"#;

pub(super) const V5_SCAN_SET_SCHEMA: &str = r#"
CREATE TABLE usage_scan_set (
  scan_set_id INTEGER PRIMARY KEY CHECK(scan_set_id >= 0),
  started_at_ms INTEGER NOT NULL,
  completed_at_ms INTEGER,
  completion_state TEXT NOT NULL CHECK(completion_state IN ('running','complete','partial','cancelled','failed','timed_out')),
  expected_scope_count INTEGER NOT NULL CHECK(expected_scope_count BETWEEN 1 AND 256),
  CHECK((completion_state = 'running' AND completed_at_ms IS NULL) OR
        (completion_state <> 'running' AND completed_at_ms IS NOT NULL)),
  CHECK(completed_at_ms IS NULL OR completed_at_ms >= started_at_ms)
) STRICT;
"#;

pub(super) const V5_USAGE_SCAN_SCHEMA: &str = r#"
CREATE TABLE usage_scan (
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
) STRICT;
"#;

pub(super) const V5_REPLAY_REVISION_SCHEMA: &str = r#"
CREATE TABLE usage_replay_revision (
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
) STRICT;
"#;

pub(super) const V6_ARCHIVE_STATE_SCHEMA: &str = r#"
CREATE TABLE usage_archive_state (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  archive_generation INTEGER NOT NULL CHECK(archive_generation >= 0),
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
) STRICT;
"#;

pub(super) const V4_USAGE_EVENT_SCHEMA: &str = r#"
CREATE TABLE usage_event (
  fingerprint BLOB PRIMARY KEY CHECK(length(fingerprint) = 32),
  event_id TEXT NOT NULL CHECK(length(CAST(event_id AS BLOB)) BETWEEN 1 AND 128),
  selected_file_key BLOB NOT NULL CHECK(length(selected_file_key) = 32),
  selected_generation INTEGER NOT NULL CHECK(selected_generation >= 0),
  selected_source_offset INTEGER NOT NULL CHECK(selected_source_offset >= 0),
  projection_revision_id INTEGER CHECK(projection_revision_id IS NULL OR projection_revision_id >= 0),
  origin_revision_id INTEGER CHECK(origin_revision_id IS NULL OR origin_revision_id >= 0),
  retained INTEGER NOT NULL CHECK(retained IN (0,1)) DEFAULT 0,
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
) STRICT;
"#;

pub(super) const REPLAY_CHILD_SCHEMA: &str = r#"
CREATE UNIQUE INDEX usage_replay_revision_one_current
  ON usage_replay_revision(status) WHERE status = 'current';
CREATE UNIQUE INDEX usage_replay_revision_one_staging
  ON usage_replay_revision(status) WHERE status = 'staging';

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
) STRICT;

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
) STRICT;

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
) STRICT;

CREATE INDEX usage_replay_observation_parent
  ON usage_replay_observation(revision_id, provider_id, profile_id, session_id, session_ordinal);
CREATE INDEX usage_replay_observation_children
  ON usage_replay_observation(revision_id, provider_id, profile_id, parent_session_id, session_ordinal, disposition, session_id);
CREATE INDEX usage_replay_observation_disposition
  ON usage_replay_observation(revision_id, disposition);
CREATE INDEX usage_replay_observation_fingerprint
  ON usage_replay_observation(revision_id, fingerprint, disposition, file_key, generation, source_offset);

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
) STRICT;

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
) STRICT;
"#;

pub(super) const LEGACY_IMMUTABILITY_TRIGGERS: &str = r#"
CREATE TRIGGER usage_legacy_event_no_insert
BEFORE INSERT ON usage_legacy_event
BEGIN
  SELECT RAISE(ABORT, 'immutable legacy snapshot');
END;

CREATE TRIGGER usage_legacy_event_no_update
BEFORE UPDATE ON usage_legacy_event
BEGIN
  SELECT RAISE(ABORT, 'immutable legacy snapshot');
END;

CREATE TRIGGER usage_legacy_event_no_delete
BEFORE DELETE ON usage_legacy_event
BEGIN
  SELECT RAISE(ABORT, 'immutable legacy snapshot');
END;
"#;

pub(super) const LEGACY_COPY_SQL: &str = r#"
INSERT INTO usage_legacy_snapshot(
  snapshot_id, source_schema_version, quality_state, event_count
)
SELECT 1, 1, 'legacy_unverified', count(*) FROM usage_event;

INSERT INTO usage_legacy_event(
  snapshot_id, fingerprint, event_id, selected_file_key, selected_generation,
  selected_source_offset, profile_id, session_id, source_id, timestamp_seconds,
  timestamp_nanos, model, raw_model, input_tokens, cached_tokens, output_tokens,
  reasoning_tokens, total_tokens, fallback_model, long_context, service_tier,
  project_alias, originator, activity_read, activity_edit_write, activity_search,
  activity_git, activity_build_test, activity_web, activity_subagents,
  activity_terminal
)
SELECT
  1, fingerprint, event_id, selected_file_key, selected_generation,
  selected_source_offset, profile_id, session_id, source_id, timestamp_seconds,
  timestamp_nanos, model, raw_model, input_tokens, cached_tokens, output_tokens,
  reasoning_tokens, total_tokens, fallback_model, long_context, service_tier,
  project_alias, originator, activity_read, activity_edit_write, activity_search,
  activity_git, activity_build_test, activity_web, activity_subagents,
  activity_terminal
FROM usage_event;
"#;
