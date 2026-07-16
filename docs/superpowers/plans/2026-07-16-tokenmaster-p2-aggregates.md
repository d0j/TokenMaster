# TokenMaster P2-B Transactional Aggregates Implementation Plan

Status: complete and fully verified on 2026-07-16.

Design authority:
`docs/superpowers/specs/2026-07-16-tokenmaster-p2-aggregates-design.md`.

## Task 1 — Correct dataset identity before adding aggregate consumers

Status: complete and verified on 2026-07-16.

- Add schema-v7 `dataset_generation` with exact insert/delete/update triggers.
- Replace P2-A evidence-epoch identity with replay revision plus dataset generation.
- Prove tail/replay mutation invalidation, freshness-only/no-change continuity,
  overflow rollback, concurrent snapshot consistency, and path-private errors.
- Keep replay `evidence_epoch` unchanged as replay/CAS authority.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_query_contract --locked
cargo +1.97.0 test -p tokenmaster-query --test value_contract --locked
cargo +1.97.0 test -p tokenmaster-query --test service_contract --locked
```

Stop if a no-change scan mutates dataset generation or any event mutation can commit
without exactly one generation advance per affected row.

## Task 2 — Make the current canonical event provider-self-contained

Status: complete and focused-store verified on 2026-07-16.

- Start the independently rollback-safe schema-v8 aggregate migration.
- RED migration/current-write/read contracts for bounded non-null `provider_id`.
- Rebuild schema-v7 current event table transactionally and preserve every other bit.
- Update append, replay promotion, retained projection, and direct writer SQL.
- Keep immutable legacy bytes/table stable and preserve explicit unknown-provider
  behavior only for old orphaned legacy/source evidence.

Stop on row/count/value drift, weakened constraints, hybrid migration, or any Codex
type crossing into store/query public values.

## Task 3 — Add aggregate schema, algebra, and invariant triggers

Status: complete and focused-store verified on 2026-07-16.

- Add exact STRICT aggregate state, time rollup, and session rollup tables/indexes.
- Add known-count/sum and activity/fallback/long-context algebra.
- Add current mutation triggers plus legacy immutability triggers.
- Prove insert/delete/relevant update/non-relevant update, rollback, integer overflow,
  empty-bucket deletion, unassociated project, and aggregate-not-ready behavior.
- Measure 1/32/256 event trigger overhead before proceeding.

Measured release p95 on the reference machine is 1.814 ms for one event, 19.888 ms for
32 events, and 230.620 ms for 256 events with aggregates ready. The matching disabled
baselines are 2.718 ms, 18.311 ms, and 159.787 ms. The corrected blocking gates are
therefore one event below 25 ms, 32 events below 50 ms, 256 events below 250 ms, and no
aggregate-ready result above 1.5 times its matching baseline. Storage evidence counts
the main database, WAL, and SHM rather than only the main file.

## Task 4 — Implement bounded resumable aggregate rebuild

Status: complete and focused-store verified on 2026-07-16.

- Persist rebuild expected generation, keyset cursor, and page progress.
- Populate disk-backed staging rollups in fixed event pages under writer authority.
- Resume after reopen; cancel cooperatively; restart on generation mismatch.
- Atomically publish only complete exact staging facts.
- Build legacy-only facts without upgrading legacy quality.
- Inject faults at every state/page/swap boundary.

The original 256-event cap retained only about 2,850 events/s in the deterministic
current-million red run (912,128 events after 346.44 seconds wall). The audited hard
cap is now 2,048: it retains the same cursor/generation/crash boundary and at most
18,432 derived rows per call while the scale gate separately caps page p95 at 500 ms.

Stop if rebuild needs a long-lived read transaction, retains a history-sized Rust map,
blocks startup on a full group-by, or can publish across a generation change.

## Task 5 — Add internal calendar/timezone boundary module

Status: complete and focused-query verified on 2026-07-16. The store remains UTC-only;
Jiff 0.2.32 and bundled tzdb 2026c stay private to `tokenmaster-query`.

- Pin `jiff = 0.2.32` behind private query types.
- Resolve explicit IANA/system zone identity without silent UTC fallback.
- Produce half-open day/week/month/custom UTC boundaries with configurable week start.
- Compose full UTC hours and boundary UTC minutes.
- Reject non-minute-aligned historical boundaries with a stable code.
- Record bundled tzdb provenance for Windows packaging.

Fixtures: UTC, system-zone failure, Asia/Jerusalem, America/New_York,
Australia/Lord_Howe, Asia/Kathmandu, leap day, month/year edge, spring gap, fall fold,
23/25-hour day, configurable week start, and historical non-minute boundary.

## Task 6 — Add bounded aggregate and session store reads

Status: complete and focused-store verified on 2026-07-16. Exact overview,
partitioned series, independently capped model/project/provider/profile breakdowns,
and opaque keyset session page/detail reads are implemented.

- Read header/state/payload in one deferred transaction. Overview now does this and
  requires a `ready` generation matching the captured dataset identity.
- Accept at most three adjacent aligned UTC minute/hour segments per calendar bucket,
  so DST boundaries compose exactly without raw-event reads or double counting.
- Add overview/series plus model/project/provider/profile breakdown queries. Complete:
  one capture binds all requested payloads to one transaction/generation, including a
  typed zero-duration series point for skipped civil dates.
- Add session first/cursor pages and exact bounded session detail. Complete: mixed
  timestamp/identity ordering, exact dataset-bound opaque keys, all-time semantics,
  typed missing detail, model/project dimensions, and 256+1 lookahead pass for current
  and rebuilt legacy data.
- Enforce 400 points, 32 scopes, four breakdowns, 256 rows, and two seconds.
- Prove aggregate plans never touch raw event tables and never use offset pagination.
  Complete for overview, analytics, session pages, and session detail with real
  `EXPLAIN QUERY PLAN`, forced cancellation cleanup, and concurrent-state fixtures.

## Task 7 — Add immutable public query values and facade mapping

Status: complete and focused-query verified on 2026-07-16. Exact calendar analytics,
availability values, optional daily series, fixed breakdowns, opaque scope-bound
session pages, and exact detail now share the P2-A envelope/facade.

- Add range/timezone/week-start/breakdown requests and validated immutable values.
- Map known/partial/unavailable token facts without zero fabrication.
- Preserve P2-A identity, freshness, quality, warnings, ordering, privacy, and owned
  result behavior.
- Prove replacement ordering and stale-dataset rejection. A rebuild cannot return a
  truthful analytics payload, so this call returns stable `unavailable` without
  consuming snapshot generation while activity remains readable; the future joined
  P2-F status snapshot owns the visible `aggregate_rebuilding` warning.

## Task 8 — Performance, storage, privacy, and resource evidence

Status: complete and reference-machine verified on 2026-07-16.

- Generate deterministic one-million-event current and legacy fixtures.
- Measure cold/cached overview, 400-point series, four breakdowns, session pages,
  append overhead, rebuild throughput/resume, and database amplification.
- Run repeated query/drop/snapshot/rebuild-cancel cycles and record memory/handle/
  thread/USER/GDI plateaus.
- Scan public Debug/errors/serialized test projections for paths, source IDs,
  fingerprints, SQLite text, prompts, responses, commands, and reasoning.

Acceptance: cached overview p95 <250 ms, cold <1 s, one-event append p95 <25 ms,
32-event append p95 <50 ms, 256-event append p95 <250 ms, aggregate-ready append no
more than 1.5 times its matching baseline, every collection bounded, no raw-table
aggregate plan, and no retained resource growth.

Release-mode current/legacy million-event results: rebuild 75.528/81.142 s at
13,240/12,324 events/s, 490 resumable calls, page p95 246.558/268.305 ms, database
amplification 1.483x/1.568x including main/WAL/SHM, cold overview 174.318/178.241 ms,
cached overview p95 0.543/0.365 ms, full 400-point/four-breakdown p95
151.043/141.192 ms, maximum-32-scope full analytics 165.120/139.040 ms, and session
first/cursor p95 below 0.75 ms. Repeated 400-point snapshot/session replacement and
cooperative rebuild reopen cycles retain handle/thread/USER/GDI plateaus and stay
within a 2 MiB private-memory delta after warm-up. Existing public Debug/error privacy
fixtures cover paths, source/session identities, fingerprints, SQLite text, prompts,
responses, commands, and reasoning; there is no serialized P2-B wire surface yet.

## Task 9 — Synchronize project truth and run the complete gate

Status: complete and fully verified on 2026-07-16.

Update specification, API/data/security contracts, decisions, traceability, feature
parity, current state, roadmap, audit/master plan, handoff, changelog, and project
history. Do not write a current commit hash into tracked documents.

Final gate:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

P2-B completion does not claim pricing, quotas/reset inventory, Git output, complete
desktop UI, automation, M0 acceptance, packaging, signing, or release.
