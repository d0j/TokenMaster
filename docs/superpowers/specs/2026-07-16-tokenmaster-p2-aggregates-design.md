# TokenMaster P2-B Transactional Aggregates Design

Status: approved and re-audited for implementation on 2026-07-16; Tasks 1-7 are
implemented and focused-query verified. Performance/resource evidence and final
project-wide closure remain open.

## Goal

Provide exact, bounded usage summaries for the desktop, future CLI, and future MCP
without grouping the complete event archive at view time. The contour covers current,
legacy, period, model, project, provider, profile, session, and activity facts. Pricing,
quota/reset inventory, Git output, UI composition, and automation remain later P2/P3
contours.

The design preserves the P1 append/replay authority and the P2-A query-only boundary:
the store owns transactional materialization, `tokenmaster-query` owns immutable public
values, and no frontend receives SQL, SQLite handles, raw paths, source contents, or
unbounded collections.

## Critical correction to P2-A identity

`usage_replay_revision.evidence_epoch` is replay/CAS evidence, not a dataset clock. It
can advance after a complete no-change scan even when the visible canonical row set is
unchanged. Therefore it cannot be the second component of a cursor identity.

Schema v7 adds `usage_archive_state.dataset_generation`:

- it is a checked non-negative signed-64 value;
- it changes inside the same SQLite transaction after every real `usage_event` insert,
  delete, or update;
- it does not change when only scan freshness, publication generation, replay evidence,
  or runtime health changes;
- overflow aborts the entire mutation;
- a current dataset identity is `(replay_revision_id, dataset_generation)`;
- empty and immutable legacy identities remain unchanged.

This is deliberately conservative: a provenance-only event update may invalidate a
cursor even if its visible fields are unchanged. A no-change scan must preserve it.
The generation and every aggregate mutation share one database transaction, so a
reader can never observe a new identity with old aggregates or the reverse.

Schema v7 is intentionally limited to this identity correction. Aggregate storage and
the provider-self-contained projection use schema v8. Schema versions are not treated
as scarce: separating the small cursor-correctness migration from the larger
materialization migration gives each an independent rollback and acceptance boundary.

## Schema-v8 aggregate authority boundary

### Self-contained provider identity

The current canonical `usage_event` is rebuilt once to add bounded non-null
`provider_id`. Previously the query path recovered provider through `usage_source`,
which made the supposedly self-contained projection depend on mutable source metadata.
The migration copies the exact source provider when present and the already-public
stable `unknown` value only for an old malformed/orphaned projection that P2-A would
also have reported as unknown. All new canonical writes must supply provider identity;
there is no default that can silently manufacture it.

The immutable v1 legacy table remains unchanged. Its one-time rollup derives provider
from the retained source row and uses explicit `unknown` when unavailable.

### Aggregate state

`usage_aggregate_state` is a singleton with:

- aggregate schema version;
- state: `ready`, `rebuild_required`, `rebuilding`, or `failed`;
- expected dataset generation for resumable rebuild;
- exact current and legacy event counts;
- path-free stable failure code;
- bounded rebuild cursor/progress counters.

Aggregate reads require `ready`. Other states return a stable unavailable/rebuilding
warning; they never fall back to a whole-history query. Event ingestion remains
authoritative even when aggregate rebuild is pending.

### Time rollup

`usage_time_rollup` stores UTC minute and UTC hour facts. Its key is:

`dataset_kind, bucket_width, bucket_start_seconds, provider_id, profile_id,
dimension_kind, dimension_value`.

`dataset_kind` is `current` or `legacy`. `bucket_width` is `minute` or `hour` and the
bucket start is alignment-checked. `dimension_kind` is `all`, `model`, or `project`.
For `all`, the value is empty; a model is non-empty; an empty project value means the
explicit unassociated bucket. Provider/profile breakdowns are derived from the `all`
rows, so they do not duplicate storage.

Every row stores:

- event count;
- input, cached, output, reasoning, and total token known-count plus known-sum;
- fallback-model and long-context yes/no/unavailable counts;
- read, edit/write, search, Git, build/test, web, subagent, and terminal activity
  counters.

The public availability algebra is exact:

- known count zero: `unavailable` and no value;
- known count equals event count: `known(sum)`;
- otherwise: `partial(known_sum, known_count, event_count)`.

No missing token component becomes zero. Counts and sums use checked SQLite INTEGER
constraints; overflow rolls back the source mutation.

### Session rollup

`usage_session_rollup` uses the same three dimension kinds per bounded
provider/profile/session key. The `all` row additionally stores exact first/last UTC
instants and supports composite keyset ordering. Model/project rows support an exact
bounded session detail without scanning the session's raw events. Session IDs remain
internal query keys; public Debug/wire values use an opaque bounded representation and
never expose transcript or source identity.

An indexed canonical session/time access path supports exact first/last repair after a
delete or relevant update. No offset pagination is allowed.

### Current and legacy rows

Current rows are maintained transactionally. Legacy rows are built once from the
immutable snapshot and protected by additional immutability triggers. Query selection
uses the same dataset identity as P2-A. A legacy aggregate remains explicitly
`legacy_unverified`; materialization does not upgrade its quality.

## Mutation and rebuild protocol

SQLite triggers are the final invariant boundary because all existing append,
promotion, reconciliation, migration, and test write paths converge on
`usage_event`. Rust write helpers may batch work, but correctness does not depend on
remembering a second call site.

- every insert/delete/update advances `dataset_generation` exactly once;
- relevant metric/key/time updates subtract the old contribution and add the new one;
- insert/delete maintain current event count;
- when aggregates are `ready`, the same transaction maintains minute/hour/session
  rows;
- when they are not ready, ingestion advances canonical truth and marks/requires a
  rebuild rather than maintaining a known-incomplete projection;
- trigger or integer failure aborts the entire canonical transaction.

Migration does not impose an unbounded startup group-by. Non-empty archives enter
`rebuild_required`. Rebuild uses the already-held single-writer authority, a persisted
keyset cursor, fixed pages capped at 2,048 events, disk-backed generation-qualified
staging rows, and an expected `dataset_generation`. Each page transaction checks that
generation. Initial cleanup removes at most nine rollup rows per requested event page,
or 18,432 rows at the hard cap;
no call performs history-sized cleanup or allocates a history-sized Rust collection.
Final publication changes the active aggregate generation in one singleton update,
records exact counts, and moves to `ready` only if the dataset generation still
matches. Cancellation or crash leaves current canonical events untouched and
resumable staging private. A generation mismatch discards only unpublished rollups in
bounded cleanup pages and restarts; no stale aggregate is published.

Fresh empty archives start `ready`. A legacy-only archive builds its immutable facts
through the same bounded protocol. The UI later shows aggregate rebuild progress and
continues to expose health/latest-activity snapshots that do not require aggregates.

## Calendar and timezone semantics

Stored facts remain locale- and timezone-neutral. `tokenmaster-query` owns one
internal calendar boundary module pinned to `jiff = 0.2.32`; Jiff types do not enter
public APIs. On Windows its default feature set bundles IANA tzdb and maps the Windows
zone to IANA. The shipped tzdb version is recorded in build/release provenance and is
updated deliberately, never implicitly.

Requests use an explicit IANA zone or an explicitly resolved system-zone identity;
there is no silent UTC fallback. Local civil boundaries are converted to exact UTC
instants before database reads. Day, configurable-week, month, and custom-range
semantics are half-open `[start, end)`. Ambiguous/nonexistent boundaries use the
explicit Jiff `Compatible` rule: earlier instant for a fold and later instant for a
gap. A civil date skipped by a zone transition is therefore an explicit zero-duration
bucket rather than an error or a fabricated 24-hour day.

Hour rows cover aligned full UTC hours. Minute rows cover boundary hours split by a
local calendar boundary. This keeps a year view bounded to full-hour rows plus at most
120 minute rows per returned calendar bucket. If a historical zone boundary is not
minute-aligned, the request fails with stable `unsupported_time_boundary` rather than
rounding or scanning raw events. Contemporary IANA/DST fixtures, half-hour and
quarter-hour zones, leap days, 23/25-hour days, and week-start changes are mandatory.

The store boundary represents one exact calendar bucket as one to three ordered,
non-empty, adjacent UTC segments: an optional minute-aligned prefix, an optional
hour-aligned middle, and an optional minute-aligned suffix. Segment count, alignment,
continuity, scope count, and deadline are validated before SQLite access. The three
segments are summed inside the same deferred read transaction with checked arithmetic;
events on a segment boundary belong to exactly one half-open segment.

The first public limits are:

- at most 400 returned calendar points;
- at most 32 selected provider/profile scopes;
- at most four independently requested breakdown collections;
- at most 256 items in each breakdown/session page;
- at most a two-second normal query deadline;
- no caller-supplied SQL, expressions, timezone files, or unbounded cursor text.

Breakdowns are independent capped collections, not an arbitrary four-dimensional
cube. Supported P2-B breakdowns are model, project, provider, and profile. This avoids
combinatorial row growth while covering the reference products' useful analysis.
The store returns them in canonical kind order. Each collection is ordered by known
total-token sum, then event count and identity, reads at most 257 groups, retains at
most 256, and reports truncation explicitly. Project absence is a typed unassociated
identity rather than an empty display label.

One analytics capture binds overview, up to 400 ordered series points, and every
requested breakdown to the same publication and active aggregate generation. Series
points form an exact adjacent partition of the overview range. A zero-duration point
is allowed only at a minute-aligned boundary so a skipped civil date remains visible
without a fabricated bucket or database read.

## Immutable query values

P2-B adds validated request/value families:

- range: today, day, week, month, or bounded custom half-open range;
- timezone identity and week start;
- explicit scopes and independently selected breakdowns;
- total/series token availability values;
- activity counters and fallback/long-context facts;
- bounded model/project/provider/profile collections;
- keyset-paged session summaries and bounded session detail.

Every result uses the P2-A header, publication/dataset identity, freshness, quality,
warning limits, snapshot ordering, deadline policy, and path-private errors. A service
call reads header plus all requested aggregate payloads in one deferred SQLite read
transaction. It returns owned immutable data and releases the transaction before the
frontend receives it.

## Performance and resource gates

- no aggregate query plan may read `usage_event` or `usage_legacy_event`;
- no aggregate query uses `OFFSET`, caller SQL, or a returned live transaction;
- incremental maintenance is measured at 1, 32, and 256 appended events against the
  same append with aggregate publication unavailable;
- on the reference machine the p95 budgets are below 25 ms for the normal one-event
  append, below 50 ms for a 32-event catch-up, and below 250 ms for the maximum
  256-event catch-up; ready aggregate maintenance must also stay within 1.5 times its
  matching baseline;
- a cached one-million-event overview is p95 below 250 ms and cold below one second on
  the reference machine;
- 400-point daily series plus four capped breakdowns and all 32 scopes complete below
  one second, while session first/cursor page p95 remains below 100 ms;
- current and immutable-legacy rebuild throughput remains at least 5,000 events/s and
  rebuild-page p95 below 500 ms;
- repeated query/snapshot replacement and rebuild cancellation/restart have stable
  private-memory, handle, thread, USER, and GDI plateaus;
- database size amplification includes the main SQLite file, WAL, and SHM, is measured
  and documented, and remains at or below 3.0x before P2-B acceptance.

## Fault and integrity matrix

Tests inject failure after schema creation, canonical-table copy, state seeding,
trigger creation, rebuild-page write, generation mismatch, final swap, and before
commit. Reopen must yield either exact schema v7 truth or exact schema v8 truth, never
a hybrid. Trigger failures must preserve the prior event, dataset generation,
aggregate rows, and archive publication.

Validation checks exact tables, columns, STRICT/foreign-key/index/trigger SQL,
singleton state, legal states, aligned buckets, non-negative counts, known-count
bounds, dimension sentinels, and generation/count coherence without doing a full raw
history scan on every startup.

## Rejected alternatives

- Replay evidence epoch as dataset identity: freshness-only scans can change it.
- Publication generation as cursor identity: unnecessary cursor resets on no-change
  publication.
- View-time `GROUP BY usage_event`: violates million-row latency and UI isolation.
- Only UTC-day rows: cannot answer arbitrary IANA local days exactly.
- Only minute rows: all-time/year reads and database amplification are needlessly high.
- Precomputing every timezone: unbounded and invalid after tzdb updates.
- Storing local-time buckets for the current setting: timezone changes require
  destructive rematerialization and break CLI/MCP parity.
- A long-lived read snapshot during rebuild: retains WAL state and can grow storage.
- An arbitrary multi-dimensional cube: combinatorial storage/write amplification.
- Silent UTC fallback or rounded historical offsets: produces plausible but wrong
  reports.
- Rust-call-site-only aggregate maintenance: one missed replay/promotion path can
  silently corrupt totals.

## Self-review closure

The plan is approved because the following ambiguities are closed explicitly:

- cursor identity is separate from freshness/replay evidence;
- provider identity is self-contained for current canonical rows;
- missing token values have an algebra, not a display convention;
- current and legacy aggregates have distinct quality and mutation rules;
- rebuild is resumable, generation-bound, and not a blocking full startup scan;
- timezone/DST behavior is exact within declared minute-aligned boundaries and fails
  closed outside them;
- requested breakdowns are independent bounded projections, not a cube;
- all mutation paths converge on transactional triggers;
- query, memory, collection, response, and deadline bounds are fixed;
- performance, storage amplification, migration rollback, privacy, and resource gates
  are acceptance requirements, not later cleanup.

No known product or architecture decision remains open for P2-B implementation. This
approval is not a claim that implementation is complete; every task remains subject to
RED/GREEN tests and the full workspace gate.

## Primary time-library references

- Jiff 0.2.32 crate/platform features: <https://docs.rs/jiff/0.2.32/jiff/>;
- timezone database and Windows behavior: <https://docs.rs/jiff/0.2.32/jiff/tz/>;
- explicit gap/fold policy:
  <https://docs.rs/jiff/0.2.32/jiff/tz/enum.Disambiguation.html>.
