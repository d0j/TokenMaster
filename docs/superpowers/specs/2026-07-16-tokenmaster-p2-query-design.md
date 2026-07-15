# TokenMaster P2 Query Foundation Design

Status: approved for implementation.
Date: 2026-07-16.

## Goal

Create the single synchronous, bounded, provider-neutral read facade used later by the
Slint desktop, CLI JSON, and MCP adapters. Queries must be immutable, revision-exact,
read-only, deadline-aware, indexed, path-private, and fast enough that no frontend ever
opens SQLite directly or performs a full archive scan.

This P2-A foundation precedes aggregate, pricing, quota, reset-inventory, and Git-output
work. It does not add UI, CLI, MCP, provider mutation, arbitrary SQL, or a background
query daemon.

## Selected architecture

Add `tokenmaster-query` as a small synchronous crate. It depends on
`tokenmaster-store`, owns public read models and validation, and performs no writes.
`tokenmaster-store` adds a separate `UsageReadStore`, opened with SQLite read-only and
query-only enforcement. The writable `UsageStore` API is never exposed through the
query facade.

Every facade call:

1. validates page/filter/deadline bounds before SQLite work and samples its injected
   query clock;
2. starts one short deferred read transaction;
3. reads archive publication, dataset identity, exact scan completion/manifest, and the
   requested indexed payload inside that same SQLite snapshot;
4. rejects a caller-supplied stale dataset identity before returning continuation data;
5. commits/ends the read transaction;
6. returns only owned bounded immutable values, preferably `Arc<[T]>` for payloads.

No returned value retains a SQLite connection, transaction, statement, path, source
identity, writer guard, callback, runtime handle, or UI object.

## Two-dimensional identity

One scalar cannot represent both freshness updates and row-set continuity.

- `publicationGeneration` is the persisted archive generation. A consumer replaces its
  current header/snapshot only with a strictly newer publication generation.
- `datasetIdentity` is `empty`, `legacy_snapshot_v1`, or
  `replay_revision(<checked u64>)`. It changes only when the visible canonical row set
  can change.

No-change scans may advance publication generation, scan identity, `dataThrough`, and
freshness while retaining the same dataset identity. Keyset cursors are bound to the
dataset identity, not publication generation, so a freshness-only update does not reset
scroll position. A changed dataset rejects the continuation with fixed
`stale_snapshot`; the frontend discards the old page rather than mixing revisions.

## Query schema v1

Every `QueryEnvelope<T>` contains:

- fixed `schemaVersion = 1`;
- checked process-local `snapshotGeneration` for async consumer ordering;
- `publicationGeneration` and `datasetIdentity`;
- `generatedAtMs` from the facade-owned clock (system clock in production, deterministic
  clock in tests);
- exact optional `dataThroughMs` from the publication's complete scan set;
- freshness: `fresh`, `aging`, `stale`, or `unavailable`;
- quality: `authoritative`, `derived`, `estimated`, `partial`, `conflict`, or `unknown`;
- at most 32 explicitly applied provider/profile filter scopes; an empty list means
  all scopes, while the internal exact scan manifest remains independently bounded;
- at most 16 stable ASCII warning/reason codes;
- one bounded payload.

Missing values remain absent. `empty` is a successful query over an empty authoritative
dataset; it is not a fabricated zero when scan authority is unavailable. Legacy data is
`unknown`/`legacy_unverified`. `partial` and `recovery_pending` preserve their explicit
warning codes and never become authoritative merely because rows are readable.

Usage freshness v1 uses one documented product policy: fresh through 20 minutes,
aging through 2 hours, and stale afterward. This covers the 15-minute healthy poll plus
jitter without masking multi-hour failure. A wall-clock rollback (`generatedAtMs <
dataThroughMs`) yields `unavailable` plus `clock_discontinuity`, never negative age.
Quota/provider TTL policy remains separate and provider-defined in P2-D.

## First payload: latest activity

P2-A first implements `LatestActivityPage` because the existing
`usage_event_time_desc` index already supports exact composite keyset seek.

Each item contains only product-safe provider/profile, event ID, timestamp, normalized
model, and explicit optional token components required by future views. It contains no
path, source ID, session transcript, prompt, response, command, reasoning, checkpoint,
or raw provider payload. The opaque cursor includes timestamp plus canonical
fingerprint, but Debug/serialization never exposes the fingerprint bytes directly.

Page size is clamped or rejected according to the common 1..=256 bound; the query crate
uses rejection for invalid external requests and exact caller-selected sizes for valid
requests. `hasMore` is proven by fetching at most `pageSize + 1`, never by `COUNT(*)`.

## Read-only and deadline boundary

`UsageReadStore` opens an existing archive with SQLite read-only flags, sets
`query_only=ON`, `foreign_keys=ON`, `busy_timeout=250`, `mmap_size=0`, and a bounded
4 MiB cache. It disables trusted-schema and double-quoted SQL compatibility, enables
defensive mode, query-planner stability and no-checkpoint-on-close, then validates the
exact bundled SQLite version and schema version without migration. Missing, old, new,
malformed, or policy-mismatched archives fail with stable codes and are never modified.

Queries install a SQLite progress handler tied to a facade-owned monotonic deadline,
then clear it on every success/error path. The normal public maximum is two seconds.
P2 tests use an injected deterministic query clock and operation-budget cancellation in
addition to elapsed-time bench evidence. The query crate remains synchronous by design:
P3 owns one bounded desktop query worker, while CLI/MCP call the same facade directly;
Slint callbacks never execute SQLite.

## Performance and memory invariants

- maximum page: 256 items plus one transient lookahead row;
- maximum applied scope filters: 32; internal scan scopes: 256; warnings: 16;
  requested breakdown dimensions later: 4;
- one read connection per facade instance and no retained result history;
- one short read transaction per call; no writer lease or schema mutation;
- keyset seek must show the expected index in `EXPLAIN QUERY PLAN`;
- latest-page and cursor-page SQL may not contain offset pagination;
- one million-row cached dashboard remains a materialized-aggregate P2-B gate, not a
  reason to group-scan `usage_event` from UI code;
- repeated page/snapshot replacement retains only the caller's current immutable data;
- public errors and Debug exclude SQLite text, archive path, fingerprints, and inner
  errors.

## P2 rail after the foundation

1. **P2-A:** exact query identity, read-only store, latest activity keyset page,
   deadlines, ordering, and privacy.
2. **P2-B:** schema-v7 transactional materialized daily/model/project/session/activity
   aggregates with availability counts; no view-time full scans.
3. **P2-C:** embedded release-pinned pricing catalog, validated overrides, provenance,
   and cost availability/conflict semantics.
4. **P2-D:** permitted Codex quota transport, immutable windows/epochs/full resets,
   banked-reset inventory, reminders, and read-only activation evidence.
5. **P2-E:** bounded Git output metrics derived from already normalized event activity,
   never shell execution from query code.
6. **P2-F:** joined immutable overview/route snapshots and performance evidence ready
   for P3 UI consumption.

## Rejected alternatives

- Sharing the live writer connection with UI: couples paint/read latency to mutations
  and violates ownership.
- Opening ordinary `UsageStore` for reads: it may apply policy/migrations and exposes
  write methods.
- Treating archive generation as cursor identity: resets stable paging on freshness-
  only scans.
- Returning a long-lived SQLite transaction as a snapshot: retains WAL pages and can
  grow storage/memory over time.
- Offset paging: becomes progressively slower and unstable under publication changes.
- Grouping the full event table for every dashboard refresh: cannot meet the million-
  row latency contract.
- A query daemon or async runtime: unnecessary processes/threads and larger memory
  surface for a local synchronous bounded facade.

## Acceptance

P2-A is complete only when tests prove exact transaction identity, no-change cursor
continuity, changed-revision rejection, read-only/no-migration behavior, index seek,
lookahead bounds, deadline cancellation/cleanup, path-private errors/Debug, strict
consumer ordering, and resource return under repeated open/query/drop. It does not
claim P2 analytics/pricing/quota completion or any P3/P5/release result.
