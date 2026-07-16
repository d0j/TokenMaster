use super::schema::{IndexContract, TableContract, TriggerContract};

pub(super) const V13_GIT_SCHEMA: &str = r#"
CREATE TABLE git_installation_state (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  installation_salt BLOB NOT NULL CHECK(length(installation_salt) = 32),
  publication_revision INTEGER NOT NULL CHECK(publication_revision >= 0),
  repository_count INTEGER NOT NULL CHECK(repository_count BETWEEN 0 AND 32),
  association_count INTEGER NOT NULL CHECK(association_count BETWEEN 0 AND 4096),
  last_published_at_ms INTEGER CHECK(last_published_at_ms IS NULL OR last_published_at_ms > 0),
  CHECK((publication_revision = 0 AND last_published_at_ms IS NULL)
     OR (publication_revision > 0 AND last_published_at_ms IS NOT NULL))
) STRICT;

INSERT INTO git_installation_state(
  singleton_id, installation_salt, publication_revision,
  repository_count, association_count, last_published_at_ms
) VALUES (1, randomblob(32), 0, 0, 0, NULL);

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
) STRICT;

CREATE TABLE git_activity_association (
  association_id BLOB PRIMARY KEY CHECK(length(association_id) = 32),
  repository_id BLOB NOT NULL CHECK(length(repository_id) = 32),
  project_key BLOB CHECK(project_key IS NULL OR length(project_key) = 32),
  first_activity_at_ms INTEGER NOT NULL CHECK(first_activity_at_ms > 0),
  last_activity_at_ms INTEGER NOT NULL CHECK(last_activity_at_ms >= first_activity_at_ms),
  FOREIGN KEY(repository_id) REFERENCES git_repository(repository_id) ON DELETE CASCADE
) STRICT;

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
) STRICT;

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
) STRICT;

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
) STRICT;

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
) STRICT;

CREATE INDEX git_association_repository_activity
  ON git_activity_association(repository_id, last_activity_at_ms DESC, association_id);
CREATE INDEX git_day_repository_range
  ON git_day_aggregate(repository_id, aggregate_generation, day_index);
CREATE INDEX git_day_category_repository_range
  ON git_day_category_aggregate(repository_id, aggregate_generation, day_index, category);
CREATE INDEX git_repository_observed
  ON git_repository(observed_at_ms DESC, repository_id);

CREATE TRIGGER git_category_no_update
BEFORE UPDATE ON git_category_aggregate
BEGIN
  SELECT RAISE(ABORT, 'immutable Git category aggregate');
END;
CREATE TRIGGER git_day_no_update
BEFORE UPDATE ON git_day_aggregate
BEGIN
  SELECT RAISE(ABORT, 'immutable Git day aggregate');
END;
CREATE TRIGGER git_day_category_no_update
BEFORE UPDATE ON git_day_category_aggregate
BEGIN
  SELECT RAISE(ABORT, 'immutable Git day category aggregate');
END;
CREATE TRIGGER git_installation_state_no_delete
BEFORE DELETE ON git_installation_state
BEGIN
  SELECT RAISE(ABORT, 'Git installation state is required');
END;
CREATE TRIGGER git_warning_no_update
BEFORE UPDATE ON git_warning
BEGIN
  SELECT RAISE(ABORT, 'immutable Git warning');
END;
"#;

pub(super) const V13_GIT_TABLE_CONTRACTS: &[TableContract] = &[
    TableContract {
        name: "git_installation_state",
        columns: &[
            "singleton_id",
            "installation_salt",
            "publication_revision",
            "repository_count",
            "association_count",
            "last_published_at_ms",
        ],
    },
    TableContract {
        name: "git_repository",
        columns: &[
            "repository_id",
            "active_generation",
            "scan_revision",
            "object_format",
            "heads_fingerprint",
            "mailmap_fingerprint",
            "author_fingerprint",
            "category_version",
            "shallow",
            "observed_at_ms",
            "data_through_ms",
            "quality",
            "unavailable_reason",
            "publication_state",
            "commits",
            "merge_commits",
            "added_lines",
            "removed_lines",
            "binary_files",
            "submodule_changes",
            "omitted_commits",
            "omitted_paths",
        ],
    },
    TableContract {
        name: "git_activity_association",
        columns: &[
            "association_id",
            "repository_id",
            "project_key",
            "first_activity_at_ms",
            "last_activity_at_ms",
        ],
    },
    TableContract {
        name: "git_day_aggregate",
        columns: &[
            "repository_id",
            "aggregate_generation",
            "day_index",
            "commits",
            "merge_commits",
            "added_lines",
            "removed_lines",
        ],
    },
    TableContract {
        name: "git_category_aggregate",
        columns: &[
            "repository_id",
            "aggregate_generation",
            "category",
            "added_lines",
            "removed_lines",
        ],
    },
    TableContract {
        name: "git_day_category_aggregate",
        columns: &[
            "repository_id",
            "aggregate_generation",
            "day_index",
            "category",
            "added_lines",
            "removed_lines",
        ],
    },
    TableContract {
        name: "git_warning",
        columns: &["repository_id", "aggregate_generation", "warning"],
    },
];

pub(super) const V13_GIT_INDEX_CONTRACTS: &[IndexContract] = &[
    IndexContract {
        name: "git_association_repository_activity",
        sql: "CREATE INDEX git_association_repository_activity ON git_activity_association(repository_id, last_activity_at_ms DESC, association_id)",
    },
    IndexContract {
        name: "git_day_category_repository_range",
        sql: "CREATE INDEX git_day_category_repository_range ON git_day_category_aggregate(repository_id, aggregate_generation, day_index, category)",
    },
    IndexContract {
        name: "git_day_repository_range",
        sql: "CREATE INDEX git_day_repository_range ON git_day_aggregate(repository_id, aggregate_generation, day_index)",
    },
    IndexContract {
        name: "git_repository_observed",
        sql: "CREATE INDEX git_repository_observed ON git_repository(observed_at_ms DESC, repository_id)",
    },
];

pub(super) const V13_GIT_TRIGGER_CONTRACTS: &[TriggerContract<'static>] = &[
    TriggerContract {
        name: "git_category_no_update",
        sql: "CREATE TRIGGER git_category_no_update BEFORE UPDATE ON git_category_aggregate BEGIN SELECT RAISE(ABORT, 'immutable Git category aggregate'); END",
    },
    TriggerContract {
        name: "git_day_category_no_update",
        sql: "CREATE TRIGGER git_day_category_no_update BEFORE UPDATE ON git_day_category_aggregate BEGIN SELECT RAISE(ABORT, 'immutable Git day category aggregate'); END",
    },
    TriggerContract {
        name: "git_day_no_update",
        sql: "CREATE TRIGGER git_day_no_update BEFORE UPDATE ON git_day_aggregate BEGIN SELECT RAISE(ABORT, 'immutable Git day aggregate'); END",
    },
    TriggerContract {
        name: "git_installation_state_no_delete",
        sql: "CREATE TRIGGER git_installation_state_no_delete BEFORE DELETE ON git_installation_state BEGIN SELECT RAISE(ABORT, 'Git installation state is required'); END",
    },
    TriggerContract {
        name: "git_warning_no_update",
        sql: "CREATE TRIGGER git_warning_no_update BEFORE UPDATE ON git_warning BEGIN SELECT RAISE(ABORT, 'immutable Git warning'); END",
    },
];
