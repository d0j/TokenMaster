# TokenMaster P0-D Replay Archive Design

Status: approved design recorded for implementation on 2026-07-14.

## 1. Goal

P0-D makes the SQLite archive capable of retaining, restarting, reconciling, and
atomically promoting replay-safe accounting state. It preserves the existing v1
canonical history as an immutable `legacy_unverified` snapshot, builds replay-aware
state in an invisible staging revision, and admits only `eligible` observations to
the promoted canonical view.

P0-D is a store/accounting milestone. It does not implement the Codex-to-store
runtime pipeline, filesystem event engine, UI snapshots, quota transport, analytics,
or provider plugin host. Those remain P0-E and later milestones.

## 2. Binding inputs

This design implements the following existing contracts without weakening them:

- `TM-FUNC-002`: incremental archive writes remain checkpoint/CAS protected.
- `TM-FUNC-007`: copied ancestry prefixes do not contribute twice.
- `TM-DATA-002`: only `tokenmaster-accounting` constructs canonical identities.
- `TM-DATA-004`: staging is invisible and promotion is atomic.
- `TM-DATA-005`: strict, versioned, bounded SQLite remains mandatory.
- `TM-DATA-007`: only `eligible` observations enter canonical totals.
- `TM-SEC-002`: providers cannot supply canonical or replay authority fields.
- `TM-SEC-004`: failed migration, rebuild, reconciliation, or promotion rolls back.

The fixed classifier limits remain:

- maximum ancestry depth per bounded traversal: 32;
- maximum direct descendants reconsidered per transaction: 256;
- maximum canonical events accepted by one store batch: 256;
- maximum source manifest entries per replay revision: 256.

## 3. Alternatives

### 3.1 Delete v1 canonical rows and rebuild in place

Rejected. If original sources are unavailable, the product can become empty after a
technically successful migration. It also turns migration into destructive
reconciliation before new evidence exists.

### 3.2 Duplicate the complete archive into independent v2 source/checkpoint tables

Rejected for P0-D. It isolates schemas but duplicates checkpoints, chunk coverage,
observations, and indexes, increasing migration time, disk use, and code paths. The
existing source generation and observation tables already provide an invisible
staging boundary.

### 3.3 Immutable legacy snapshot plus replay overlay on staging observations

Selected. The v1 user-visible canonical projection is copied once into an immutable
legacy snapshot. New replay metadata references observations in existing staging
source generations. A replay revision owns session state, observation disposition,
canonical selection, expected source coverage, and durable continuation work.

## 4. Archive states

The public store exposes an `ArchiveState` value rather than making callers infer
quality from table contents:

- `Empty`: no legacy snapshot and no promoted replay revision;
- `LegacyUnverified`: canonical reads come from the immutable v1 snapshot;
- `ReplayVerified`: a current replay revision matches the compiled accounting
  versions;
- `ReplayVersionStale`: a previous replay revision remains readable, but its version
  tuple differs from the compiled tuple and it cannot accept new writes;
- `RebuildStaging`: a staging revision exists while reads continue from the previous
  current replay revision or legacy snapshot.

`RebuildStaging` is an orthogonal activity flag in the typed state. It never changes
which revision readers see.

## 5. Accounting restart seam

The current classifier accepts `CanonicalUsageEvent` references. SQLite cannot
reconstruct that opaque type after restart without reintroducing a public canonical
constructor. P0-D therefore adds a non-authoritative borrowed fact projection:

```rust
pub struct ReplayEventFacts<'a> {
    provider_id: &'a str,
    profile_id: &'a str,
    session_id: &'a str,
    parent_session_id: Option<&'a str>,
    session_ordinal: u64,
    replay_signature: &'a [u8; 32],
    evidence: ReplayEvidence,
    declared_conflict: bool,
}
```

`ReplayClassificationInput` consumes `ReplayEventFacts` for child and parent.
`ReplayEventFacts::from_event(&CanonicalUsageEvent)` is the live path. The store
constructs facts only after validating persisted text bounds, digest lengths,
version fields, and enum values.

This projection cannot create an event fingerprint, replay signature, event ID,
canonical event, or archive write. Store write APIs still accept only
`CanonicalUsageEvent`, so providers gain no authority bypass.

## 6. Schema version 2

`USAGE_SCHEMA_VERSION` becomes 2. The six existing v1 tables remain the source,
checkpoint, observation, and materialized canonical payload tables. P0-D adds the
following strict tables.

### 6.1 `usage_legacy_snapshot`

One row records the immutable fallback projection:

- `snapshot_id` fixed to 1;
- `source_schema_version` fixed to 1 for migrated databases;
- `quality_state` fixed to `legacy_unverified`;
- `event_count` checked against the copied row count;
- no path, content, credential, or transcript fields.

Fresh v2 databases have no legacy snapshot row.

### 6.2 `usage_legacy_event`

Contains the exact bounded user-visible columns of v1 `usage_event`, keyed by
`(snapshot_id, fingerprint)`. Migration copies rows with `INSERT ... SELECT` inside
the migration transaction. Insert, update, and delete triggers reject later writes
with a stable path-free error.

The snapshot preserves user-visible history, not raw source bytes, incomplete tails,
or private paths.

### 6.3 `usage_replay_revision`

One row per replay rebuild:

- non-negative `revision_id`;
- status `staging` or `current`;
- `canonicalizer_version`;
- `fingerprint_version`;
- `replay_signature_version`;
- bounded expected-source count;
- bounded evidence epoch used for continuation CAS;
- sealed flag and promoted flag constrained by status.

Partial unique indexes permit at most one staging and one current revision. A new
revision always records the compiled version tuple; callers cannot choose versions.

### 6.4 `usage_replay_source`

The replay manifest is fixed when a revision begins:

- `(revision_id, file_key)` primary key;
- expected staging generation;
- state `pending` or `complete`;
- a foreign key to `usage_source`;
- a deferred foreign key to the expected `usage_generation` row once created.

The manifest accepts at most 256 unique source keys. Seal requires the stored count to
equal the revision count and every source state to be `complete`.

### 6.5 `usage_replay_session`

The session key is `(revision_id, provider_id, profile_id, session_id)`. Each row
stores:

- optional explicit parent session;
- relation conflict marker;
- session state `root`, `matching`, `diverged`, `pending`, or `conflict`;
- completion state `open` or `sealed_complete`;
- first relation source identity and offset for deterministic conflict handling;
- last classified ordinal and evidence epoch.

Only an explicit observation or `SessionRelationDraft` supplies a parent. Two
different explicit parents, self-parenting, or an explicit conflict marker locks the
session to `conflict`.

### 6.6 `usage_replay_observation`

This is a one-to-one replay overlay for a staged `usage_observation`. Its primary key
is `(revision_id, file_key, generation, source_offset, fingerprint)`. It stores:

- provider, profile, session, optional parent, and zero-based ordinal;
- canonicalizer, fingerprint, and replay-signature versions;
- 32-byte replay signature;
- evidence `strong_cumulative` or `weak_usage_only`;
- disposition `eligible`, `replay`, `pending`, or `conflict`;
- declared conflict marker and evidence epoch.

Every row has a foreign key to the underlying observation and revision. All three
version fields must equal the owning revision. The store verifies that provider,
profile, session, source, source offset, event ID, and fingerprint agree with the
canonical event being appended.

Required indexes include:

- parent lookup: revision, provider, profile, session, ordinal;
- descendant lookup: revision, provider, profile, parent, ordinal, disposition,
  session;
- disposition counts: revision, disposition;
- fingerprint selection: revision, fingerprint, disposition, file key, generation,
  source offset.

### 6.7 `usage_replay_selection`

One row per selected eligible fingerprint in a revision. It references both the
replay overlay and underlying observation. Selection order is deterministic:
profile, file key, generation, source offset. A refresh may insert zero or one row;
zero is valid when no eligible observation remains.

Selection rows repeat the three accounting versions and must equal the owning
revision. Promotion reads only this table when materializing `usage_event`.

### 6.8 `usage_replay_work`

Durable continuation items are coalesced by revision, work kind, provider, profile,
and session. A row contains:

- kind `classify_session` or `scan_children`;
- reason `late_relation`, `missing_parent`, `parent_changed`, `depth_bound`, or
  `fanout_bound`;
- next ordinal for session classification;
- optional child-session keyset cursor for descendant scans;
- expected evidence epoch for stale-work rejection.

No continuation stores source content or an unbounded path. Processing is keyset
paged and updates/deletes the work item in the same transaction as classification.

## 7. Migration

Migration is one `BEGIN IMMEDIATE` transaction:

1. Read `PRAGMA user_version`.
2. Reject versions newer than 2.
3. For version 1, validate the exact v1 tables, columns, indexes, strict flags, and
   foreign keys before creating anything.
4. Create v2 tables, indexes, and legacy snapshot metadata.
5. Copy every v1 `usage_event` into `usage_legacy_event`.
6. Verify copied count and foreign keys.
7. Create the legacy immutability triggers.
8. Set `user_version=2`.
9. Validate the exact complete v2 schema and commit.

For a fresh database, create the complete v2 schema directly without a legacy row.
For an already-current v2 database, validation may not silently repair missing or
changed schema objects. Any validation or commit failure leaves the original file
unchanged.

The v1 `usage_event` rows are not deleted during migration. They remain the previous
live projection until a replay revision is successfully promoted.

## 8. Staging lifecycle

### 8.1 Begin

`begin_replay_revision(&ReplayManifest)`:

- rejects duplicate or more than 256 source keys;
- rejects a second staging revision;
- assigns the next non-negative revision ID in the transaction;
- records compiled versions and expected source rows;
- changes no current generation and no canonical read.

### 8.2 Stage sources

Each manifest source receives one new `usage_generation` with status `staging` and a
generation number greater than its current generation. Source chunks, checkpoints,
and observations are written only to that generation. Stale revision, generation,
offset, scan offset, identity, chunk proof, or evidence epoch writes nothing.

### 8.3 Append facts

One replay append transaction performs this order:

1. validate revision and source manifest CAS;
2. insert/verify staged observations;
3. persist or reconcile explicit session relations;
4. insert replay overlay facts with compiled versions;
5. classify the bounded batch through `ReplayClassifier`;
6. refresh affected staging selections;
7. enqueue/coalesce affected continuation work;
8. update chunks/checkpoint and evidence epoch;
9. commit.

Duplicate observation keys must match every stable stored value. A mismatch is
`InvalidStoredValue` and rolls back the complete transaction.

### 8.4 Late ancestry and descendants

A late relation can change a root/pending session into matching or conflict. The same
transaction persists the relation, invalidates affected selections, and enqueues the
session. A continuation transaction processes at most 256 direct children and at most
32 unresolved ancestry links.

Depth or fanout exhaustion remains `pending` and advances durable work. It never
becomes conflict solely because a bound was reached. Confirmed cycles, self-parenting,
or different explicit parents are conflict.

### 8.5 Parent completion

Callers cannot set a session-complete boolean directly. A session becomes
`sealed_complete` only while sealing a revision whose fixed manifest proves every
source complete. Each source must have:

- a matching staging generation;
- `full_prefix` verification;
- no incomplete tail;
- no oversized-line discard in progress;
- `scan_offset == committed_offset == observed_file_length`;
- a checkpoint and chunk state that pass existing CAS validation.

Before seal, a missing parent ordinal is `MissingOpen` and the child remains pending.
During seal reconciliation, a genuinely absent parent ordinal becomes
`MissingComplete`, which proves divergence for that child and all later ordinals.

### 8.6 Seal

Seal runs bounded continuation transactions until no work remains, then performs one
final immediate transaction that verifies:

- all manifest sources are complete;
- no pending/conflict-unsafe invariant is hidden by unfinished work;
- every staged observation has exactly one replay row;
- every selection points to an eligible row;
- version tuples match;
- foreign-key check is empty.

Conflict observations may remain as explicit quality data. Pending observations may
remain only when evidence is inherently unresolved, not because continuation work was
abandoned. A revision containing unresolved pending observations can be sealed for
quality reporting but cannot be promoted to canonical current until product policy
explicitly permits a partial archive. P0-D promotion requires zero pending rows.

## 9. Promotion and rollback

Promotion is one `BEGIN IMMEDIATE` transaction and requires the expected staging
revision ID and evidence epoch:

1. revalidate seal, versions, source manifest, pending work, pending counts, and
   foreign keys;
2. delete and rebuild `usage_event` from `usage_replay_selection` joined to staged
   observations;
3. clear each source's old current generation reference;
4. delete old current source generations only after their user-visible projection is
   known to exist in `usage_legacy_event` or a previously promoted revision;
5. mark staging generations current and update `usage_source.current_generation`;
6. replace the prior current replay revision, if any;
7. mark the staging revision current and commit.

Deferred foreign keys make the generation swap atomic. Any injected failure,
constraint error, stale epoch, or commit failure rolls back the materialized events,
source pointers, generation statuses, and revision state together.

The immutable legacy snapshot is never deleted by promotion. A future explicit
maintenance policy may compact it, but that is outside P0-D and cannot be automatic.

## 10. Read behavior

Canonical readers never join staging tables:

- current compatible replay revision: page `usage_event` and report
  `ReplayVerified`;
- current incompatible replay revision: page its materialized `usage_event`, report
  `ReplayVersionStale`, and reject writes until rebuild;
- no replay current but legacy snapshot exists: page `usage_legacy_event` and report
  `LegacyUnverified`;
- neither exists: return an empty page and `Empty`.

Reads remain keyset-paged to 256 rows. Quality counts expose eligible, replay,
pending, and conflict for one explicit revision. No arbitrary SQL or path is exposed.

## 11. Error and privacy behavior

New stable store error codes distinguish:

- stale revision/epoch;
- incompatible accounting version;
- incomplete manifest;
- unsealed revision;
- pending continuation;
- archive mode mismatch.

Errors remain path-free and never wrap SQLite or OS messages. Debug output for
manifest keys, fingerprints, signatures, checkpoints, and relation source identities
is redacted. No schema field contains prompt, response, reasoning, command, output,
raw JSON, file contents, credentials, absolute paths, or incomplete tails.

## 12. Required tests

P0-D must prove through focused red/green tests:

1. exact v2 strict schema, indexes, triggers, and privacy exclusions;
2. real v1 migration preserves canonical rows in an immutable snapshot;
3. malformed v1, disk/constraint fault, and newer version fail without partial
   migration;
4. staging is invisible to canonical readers;
5. persisted replay facts classify identically before and after reopen;
6. root, strong replay, first divergence, weak evidence, missing-open parent,
   missing-complete parent, conflict, and version mismatch;
7. late ancestry and nested descendants update selections transactionally;
8. depth/fanout exhaustion creates durable continuation and resumes by keyset;
9. manifest omission, incomplete tail, stale epoch, pending work, and pending rows
   block promotion;
10. injected promotion failure preserves the previous current and legacy snapshot;
11. successful promotion changes all visible state atomically;
12. every public collection and transaction remains bounded;
13. debug/error/schema scans remain path- and content-private.

The final gate includes focused store/accounting tests, all locked workspace tests,
strict Clippy with warnings denied, formatting, clean-root audit, traceability and ADR
consistency, privacy/secret scan, and `git diff --check`.

## 13. P0-D completion boundary

P0-D is complete when schema migration, replay persistence, restart reconstruction,
bounded reconciliation, durable continuation, seal, promotion, rollback, archive
state, and quality counts have committed tests.

P0-D does not claim the full parser-to-store pipeline, scan scheduling, truncate and
replacement orchestration, source discovery epochs, long-run working-set proof, M0
acceptance, or product release. P0-E and P1 own those gates.
