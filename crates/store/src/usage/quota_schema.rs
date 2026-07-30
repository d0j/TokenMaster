use super::schema::{IndexContract, TableContract, TriggerContract};

pub(super) const V10_QUOTA_SCHEMA: &str = r#"
CREATE TABLE quota_state (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  revision INTEGER NOT NULL CHECK(revision >= 0),
  retained_sample_count INTEGER NOT NULL CHECK(retained_sample_count >= 0),
  retained_epoch_count INTEGER NOT NULL CHECK(retained_epoch_count >= 0),
  retained_transition_count INTEGER NOT NULL CHECK(retained_transition_count >= 0),
  last_published_at_ms INTEGER CHECK(last_published_at_ms IS NULL OR last_published_at_ms > 0),
  CHECK((revision = 0 AND last_published_at_ms IS NULL)
     OR (revision > 0 AND last_published_at_ms IS NOT NULL))
) STRICT;

INSERT INTO quota_state(
  singleton_id, revision, retained_sample_count, retained_epoch_count,
  retained_transition_count, last_published_at_ms
) VALUES (1, 0, 0, 0, 0, NULL);

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
) STRICT;

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
) STRICT;

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
) STRICT;

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
) STRICT;

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
) STRICT;

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
) STRICT;

CREATE INDEX quota_definition_scope_revision
  ON quota_window_definition(scope_id, window_id, revision DESC);
CREATE INDEX quota_sample_retention
  ON quota_sample(scope_id, window_id, observed_at_ms DESC, observation_id DESC);
CREATE INDEX quota_epoch_history_retention
  ON quota_epoch_history(scope_id, window_id, last_observed_at_ms DESC, epoch_id DESC);
CREATE UNIQUE INDEX quota_transition_window_sequence
  ON quota_transition(scope_id, window_id, sequence);
CREATE INDEX quota_window_current_scope
  ON quota_window_current(scope_id, window_id);

CREATE TRIGGER quota_state_no_delete
BEFORE DELETE ON quota_state
BEGIN
  SELECT RAISE(ABORT, 'quota state is required');
END;

CREATE TRIGGER quota_window_definition_no_update
BEFORE UPDATE ON quota_window_definition
BEGIN
  SELECT RAISE(ABORT, 'immutable quota definition');
END;

CREATE TRIGGER quota_sample_no_update
BEFORE UPDATE ON quota_sample
BEGIN
  SELECT RAISE(ABORT, 'immutable quota sample');
END;

CREATE TRIGGER quota_epoch_history_no_update
BEFORE UPDATE ON quota_epoch_history
BEGIN
  SELECT RAISE(ABORT, 'immutable quota epoch');
END;

CREATE TRIGGER quota_transition_no_update
BEFORE UPDATE ON quota_transition
BEGIN
  SELECT RAISE(ABORT, 'immutable quota transition');
END;
"#;

pub(super) const V10_QUOTA_TABLE_CONTRACTS: &[TableContract] = &[
    TableContract {
        name: "quota_state",
        columns: &[
            "singleton_id",
            "revision",
            "retained_sample_count",
            "retained_epoch_count",
            "retained_transition_count",
            "last_published_at_ms",
        ],
    },
    TableContract {
        name: "quota_window_definition",
        columns: &[
            "scope_id",
            "window_id",
            "revision",
            "provider_id",
            "account_id",
            "workspace_id",
            "label_key",
            "presentation",
            "semantics",
            "nominal_duration_seconds",
            "maximum_post_reset_used_ppm",
            "minimum_post_reset_remaining_ppm",
            "minimum_used_ratio_drop_ppm",
        ],
    },
    TableContract {
        name: "quota_sample",
        columns: &[
            "observation_id",
            "scope_id",
            "window_id",
            "definition_revision",
            "observed_at_ms",
            "fresh_until_ms",
            "stale_after_ms",
            "provider_epoch_id",
            "used_ratio_ppm",
            "remaining_ratio_ppm",
            "unit_id",
            "used_units",
            "remaining_units",
            "capacity_units",
            "advertised_resets_at_ms",
            "quality",
            "source",
            "confidence",
            "reset_evidence",
            "reset_occurred_at_ms",
        ],
    },
    TableContract {
        name: "quota_epoch_current",
        columns: &[
            "scope_id",
            "window_id",
            "epoch_id",
            "epoch_definition_revision",
            "definition_revision",
            "first_observation_id",
            "last_observation_id",
            "first_observed_at_ms",
            "last_observed_at_ms",
            "maximum_used_ratio_ppm",
            "maximum_used_ratio_observation_id",
            "maximum_unit_id",
            "maximum_used_units",
            "maximum_remaining_units",
            "maximum_capacity_units",
            "maximum_used_units_observation_id",
            "provider_epoch_id",
            "advertised_resets_at_ms",
            "last_transition_sequence",
        ],
    },
    TableContract {
        name: "quota_epoch_history",
        columns: &[
            "epoch_id",
            "scope_id",
            "window_id",
            "epoch_definition_revision",
            "definition_revision",
            "first_observation_id",
            "last_observation_id",
            "first_observed_at_ms",
            "last_observed_at_ms",
            "maximum_used_ratio_ppm",
            "maximum_used_ratio_observation_id",
            "maximum_unit_id",
            "maximum_used_units",
            "maximum_remaining_units",
            "maximum_capacity_units",
            "maximum_used_units_observation_id",
            "final_provider_epoch_id",
            "final_advertised_resets_at_ms",
            "closing_transition_sequence",
        ],
    },
    TableContract {
        name: "quota_transition",
        columns: &[
            "transition_id",
            "scope_id",
            "window_id",
            "definition_revision",
            "sequence",
            "kind",
            "previous_epoch_id",
            "current_epoch_id",
            "pre_observation_id",
            "post_observation_id",
            "maximum_used_ratio_ppm",
            "maximum_used_ratio_observation_id",
            "maximum_unit_id",
            "maximum_used_units",
            "maximum_remaining_units",
            "maximum_capacity_units",
            "maximum_used_units_observation_id",
            "old_resets_at_ms",
            "new_resets_at_ms",
            "allowance_change_kind",
            "allowance_old_unit_id",
            "allowance_old_used_units",
            "allowance_old_remaining_units",
            "allowance_old_capacity_units",
            "allowance_new_unit_id",
            "allowance_new_used_units",
            "allowance_new_remaining_units",
            "allowance_new_capacity_units",
            "source",
            "confidence",
            "detection_time_kind",
            "exact_at_ms",
            "after_ms",
            "at_or_before_ms",
        ],
    },
    TableContract {
        name: "quota_window_current",
        columns: &[
            "scope_id",
            "window_id",
            "definition_revision",
            "sample_observation_id",
            "epoch_id",
            "observed_at_ms",
            "fresh_until_ms",
            "stale_after_ms",
            "quality",
            "source",
            "confidence",
            "last_transition_sequence",
        ],
    },
];

pub(super) const V10_QUOTA_INDEX_CONTRACTS: &[IndexContract] = &[
    IndexContract {
        name: "quota_definition_scope_revision",
        sql: "CREATE INDEX quota_definition_scope_revision ON quota_window_definition(scope_id, window_id, revision DESC)",
    },
    IndexContract {
        name: "quota_epoch_history_retention",
        sql: "CREATE INDEX quota_epoch_history_retention ON quota_epoch_history(scope_id, window_id, last_observed_at_ms DESC, epoch_id DESC)",
    },
    IndexContract {
        name: "quota_sample_retention",
        sql: "CREATE INDEX quota_sample_retention ON quota_sample(scope_id, window_id, observed_at_ms DESC, observation_id DESC)",
    },
    IndexContract {
        name: "quota_transition_window_sequence",
        sql: "CREATE UNIQUE INDEX quota_transition_window_sequence ON quota_transition(scope_id, window_id, sequence)",
    },
    IndexContract {
        name: "quota_window_current_scope",
        sql: "CREATE INDEX quota_window_current_scope ON quota_window_current(scope_id, window_id)",
    },
];

pub(super) const V10_QUOTA_TRIGGER_CONTRACTS: &[TriggerContract<'static>] = &[
    TriggerContract {
        name: "quota_epoch_history_no_update",
        sql: "CREATE TRIGGER quota_epoch_history_no_update BEFORE UPDATE ON quota_epoch_history BEGIN SELECT RAISE(ABORT, 'immutable quota epoch'); END",
    },
    TriggerContract {
        name: "quota_sample_no_update",
        sql: "CREATE TRIGGER quota_sample_no_update BEFORE UPDATE ON quota_sample BEGIN SELECT RAISE(ABORT, 'immutable quota sample'); END",
    },
    TriggerContract {
        name: "quota_state_no_delete",
        sql: "CREATE TRIGGER quota_state_no_delete BEFORE DELETE ON quota_state BEGIN SELECT RAISE(ABORT, 'quota state is required'); END",
    },
    TriggerContract {
        name: "quota_transition_no_update",
        sql: "CREATE TRIGGER quota_transition_no_update BEFORE UPDATE ON quota_transition BEGIN SELECT RAISE(ABORT, 'immutable quota transition'); END",
    },
    TriggerContract {
        name: "quota_window_definition_no_update",
        sql: "CREATE TRIGGER quota_window_definition_no_update BEFORE UPDATE ON quota_window_definition BEGIN SELECT RAISE(ABORT, 'immutable quota definition'); END",
    },
];
