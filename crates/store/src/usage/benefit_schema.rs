use super::schema::{IndexContract, TableContract, TriggerContract};

pub(super) const V11_BENEFIT_SCHEMA: &str = r#"
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
) STRICT;

INSERT INTO benefit_state(
  singleton_id, revision, current_lot_count, retained_change_count,
  pending_due_count, retained_delivery_count, last_published_at_ms
) VALUES (1, 0, 0, 0, 0, 0, NULL);

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
) STRICT;

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
) STRICT;

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
) STRICT;

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
) STRICT;

CREATE TABLE benefit_reminder_profile (
  profile_kind TEXT NOT NULL CHECK(profile_kind IN ('global','scope')),
  profile_scope_id BLOB NOT NULL
    CHECK((profile_kind = 'global' AND length(profile_scope_id) = 0)
       OR (profile_kind = 'scope' AND length(profile_scope_id) = 32)),
  revision INTEGER NOT NULL CHECK(revision > 0),
  channel_in_app INTEGER NOT NULL CHECK(channel_in_app IN (0,1)),
  channel_os_scheduled INTEGER NOT NULL CHECK(channel_os_scheduled IN (0,1)),
  PRIMARY KEY(profile_kind, profile_scope_id)
) STRICT;

INSERT INTO benefit_reminder_profile(
  profile_kind, profile_scope_id, revision, channel_in_app, channel_os_scheduled
) VALUES ('global', x'', 1, 1, 0);

CREATE TABLE benefit_reminder_threshold (
  profile_kind TEXT NOT NULL CHECK(profile_kind IN ('global','scope')),
  profile_scope_id BLOB NOT NULL,
  threshold_seconds INTEGER NOT NULL CHECK(threshold_seconds BETWEEN 60 AND 31536000),
  PRIMARY KEY(profile_kind, profile_scope_id, threshold_seconds),
  FOREIGN KEY(profile_kind, profile_scope_id)
    REFERENCES benefit_reminder_profile(profile_kind, profile_scope_id) ON DELETE CASCADE
) STRICT;

INSERT INTO benefit_reminder_threshold(profile_kind, profile_scope_id, threshold_seconds)
VALUES
  ('global', x'', 604800),
  ('global', x'', 86400),
  ('global', x'', 43200),
  ('global', x'', 21600),
  ('global', x'', 3600);

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
) STRICT;

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
) STRICT;

CREATE UNIQUE INDEX benefit_change_scope_sequence
  ON benefit_change(scope_id, sequence);
CREATE INDEX benefit_delivery_scope_time
  ON benefit_reminder_delivery(scope_id, delivered_at_ms DESC, delivery_id DESC);
CREATE INDEX benefit_due_next
  ON benefit_reminder_due(due_at_ms, expiry_at_ms, scope_id, lot_id);
CREATE INDEX benefit_lot_current_expiry
  ON benefit_lot_current(scope_id, state, conservative_expiry_at_ms, lot_id);
CREATE INDEX benefit_lot_revision_retention
  ON benefit_lot_revision(scope_id, lot_id, lot_revision DESC);
CREATE INDEX benefit_profile_scope
  ON benefit_reminder_profile(profile_scope_id, profile_kind);

CREATE TRIGGER benefit_state_no_delete
BEFORE DELETE ON benefit_state
BEGIN
  SELECT RAISE(ABORT, 'benefit state is required');
END;

CREATE TRIGGER benefit_lot_revision_no_update
BEFORE UPDATE ON benefit_lot_revision
BEGIN
  SELECT RAISE(ABORT, 'immutable benefit lot revision');
END;

CREATE TRIGGER benefit_change_no_update
BEFORE UPDATE ON benefit_change
BEGIN
  SELECT RAISE(ABORT, 'immutable benefit change');
END;

CREATE TRIGGER benefit_delivery_no_update
BEFORE UPDATE ON benefit_reminder_delivery
BEGIN
  SELECT RAISE(ABORT, 'immutable benefit delivery');
END;
"#;

pub(super) const V11_BENEFIT_TABLE_CONTRACTS: &[TableContract] = &[
    TableContract {
        name: "benefit_state",
        columns: &[
            "singleton_id",
            "revision",
            "current_lot_count",
            "retained_change_count",
            "pending_due_count",
            "retained_delivery_count",
            "last_published_at_ms",
        ],
    },
    TableContract {
        name: "benefit_scope",
        columns: &[
            "scope_id",
            "provider_id",
            "account_id",
            "workspace_id",
            "inventory_revision",
            "last_change_sequence",
            "observation_id",
            "observed_at_ms",
            "fresh_until_ms",
            "stale_after_ms",
            "completeness",
            "current_lot_count",
        ],
    },
    TableContract {
        name: "benefit_lot_revision",
        columns: &[
            "scope_id",
            "lot_id",
            "lot_revision",
            "kind",
            "quantity",
            "state",
            "target_kind",
            "target_window_id",
            "granted_at_ms",
            "expiry_kind",
            "expiry_exact_at_ms",
            "expiry_local_year",
            "expiry_local_month",
            "expiry_local_day",
            "expiry_local_hour",
            "expiry_local_minute",
            "expiry_local_second",
            "expiry_local_millisecond",
            "expiry_time_zone",
            "expiry_bounded_earliest_at_ms",
            "expiry_bounded_latest_at_ms",
            "source",
            "confidence",
            "detail_kind",
            "label_key",
        ],
    },
    TableContract {
        name: "benefit_lot_current",
        columns: &[
            "scope_id",
            "lot_id",
            "lot_revision",
            "kind",
            "quantity",
            "state",
            "detail_kind",
            "conservative_expiry_at_ms",
        ],
    },
    TableContract {
        name: "benefit_change",
        columns: &[
            "change_id",
            "scope_id",
            "sequence",
            "lot_id",
            "lot_revision",
            "kind",
            "before_revision",
            "after_revision",
            "observed_at_ms",
        ],
    },
    TableContract {
        name: "benefit_reminder_profile",
        columns: &[
            "profile_kind",
            "profile_scope_id",
            "revision",
            "channel_in_app",
            "channel_os_scheduled",
        ],
    },
    TableContract {
        name: "benefit_reminder_threshold",
        columns: &["profile_kind", "profile_scope_id", "threshold_seconds"],
    },
    TableContract {
        name: "benefit_reminder_due",
        columns: &[
            "delivery_id",
            "scope_id",
            "lot_id",
            "lot_revision",
            "threshold_seconds",
            "channel",
            "due_at_ms",
            "expiry_at_ms",
            "profile_revision",
        ],
    },
    TableContract {
        name: "benefit_reminder_delivery",
        columns: &[
            "delivery_id",
            "scope_id",
            "lot_id",
            "lot_revision",
            "threshold_seconds",
            "channel",
            "due_at_ms",
            "expiry_at_ms",
            "delivered_at_ms",
        ],
    },
];

pub(super) const V11_BENEFIT_INDEX_CONTRACTS: &[IndexContract] = &[
    IndexContract {
        name: "benefit_change_scope_sequence",
        sql: "CREATE UNIQUE INDEX benefit_change_scope_sequence ON benefit_change(scope_id, sequence)",
    },
    IndexContract {
        name: "benefit_delivery_scope_time",
        sql: "CREATE INDEX benefit_delivery_scope_time ON benefit_reminder_delivery(scope_id, delivered_at_ms DESC, delivery_id DESC)",
    },
    IndexContract {
        name: "benefit_due_next",
        sql: "CREATE INDEX benefit_due_next ON benefit_reminder_due(due_at_ms, expiry_at_ms, scope_id, lot_id)",
    },
    IndexContract {
        name: "benefit_lot_current_expiry",
        sql: "CREATE INDEX benefit_lot_current_expiry ON benefit_lot_current(scope_id, state, conservative_expiry_at_ms, lot_id)",
    },
    IndexContract {
        name: "benefit_lot_revision_retention",
        sql: "CREATE INDEX benefit_lot_revision_retention ON benefit_lot_revision(scope_id, lot_id, lot_revision DESC)",
    },
    IndexContract {
        name: "benefit_profile_scope",
        sql: "CREATE INDEX benefit_profile_scope ON benefit_reminder_profile(profile_scope_id, profile_kind)",
    },
];

pub(super) const V11_BENEFIT_TRIGGER_CONTRACTS: &[TriggerContract<'static>] = &[
    TriggerContract {
        name: "benefit_change_no_update",
        sql: "CREATE TRIGGER benefit_change_no_update BEFORE UPDATE ON benefit_change BEGIN SELECT RAISE(ABORT, 'immutable benefit change'); END",
    },
    TriggerContract {
        name: "benefit_delivery_no_update",
        sql: "CREATE TRIGGER benefit_delivery_no_update BEFORE UPDATE ON benefit_reminder_delivery BEGIN SELECT RAISE(ABORT, 'immutable benefit delivery'); END",
    },
    TriggerContract {
        name: "benefit_lot_revision_no_update",
        sql: "CREATE TRIGGER benefit_lot_revision_no_update BEFORE UPDATE ON benefit_lot_revision BEGIN SELECT RAISE(ABORT, 'immutable benefit lot revision'); END",
    },
    TriggerContract {
        name: "benefit_state_no_delete",
        sql: "CREATE TRIGGER benefit_state_no_delete BEFORE DELETE ON benefit_state BEGIN SELECT RAISE(ABORT, 'benefit state is required'); END",
    },
];
