# P2-A Immutable Query Foundation Plan

Status: approved; implementation not started.

## Goal

Implement the approved query foundation in
`docs/superpowers/specs/2026-07-16-tokenmaster-p2-query-design.md` with focused RED/GREEN
contracts and no UI, analytics, pricing, quota, CLI, MCP, or arbitrary SQL scope.

## Task 1 — Add the bounded public query values

Files:

- add `crates/query/Cargo.toml` and `crates/query/src/lib.rs`;
- add `crates/query/src/error.rs`, `clock.rs`, `identity.rs`, and `activity.rs`;
- add `crates/query/tests/value_contract.rs`;
- add `crates/query` to the root workspace.

RED contracts:

- schema v1, two-dimensional identity, freshness/quality/warning values;
- checked snapshot generation and strict newer predicate;
- page size 0/257 rejected and 1/256 accepted;
- at most 32 scopes and 16 warnings;
- cursor Debug redacts fingerprint and every public Debug/error is path-free;
- owned page uses at most 256 items and has no SQLite/runtime/platform types.

Implement the minimum validated values and stable errors. Do not open SQLite yet.

## Task 2 — Add a separate query-only store

Files:

- add `crates/store/src/usage/query.rs`;
- export `UsageReadStore` and store-only query capture values;
- add `crates/store/tests/usage_query_contract.rs`;
- enable the pinned `rusqlite` `hooks` feature for deadline progress handling.

RED contracts:

- open existing schema-v6 archive read-only and leave file/WAL content unchanged;
- missing archive, old/new schema, malformed state, and wrong SQLite version fail with
  stable codes and never migrate;
- `PRAGMA query_only=ON`, bounded cache, foreign keys, busy timeout, and mmap policy;
- publication, dataset, exact scan completion/scopes, and latest activity page are read
  in one deferred transaction;
- `pageSize + 1` lookahead proves `hasMore`; cursor uses composite index seek and no
  offset/full count;
- expected dataset mismatch fails before returning rows;
- progress cancellation interrupts work and is cleared for the next query.

## Task 3 — Compose `QueryService`

Files:

- add `crates/query/src/service.rs`;
- add `crates/query/tests/service_contract.rs`.

RED contracts:

- empty/current/partial/recovery/legacy quality mapping is truthful;
- exact `dataThrough`, scopes, and generated-at freshness policy;
- wall-clock rollback becomes unavailable/clock-discontinuity;
- no-change publication advances header generation but preserves dataset/cursor;
- changed revision rejects an old cursor with `stale_snapshot`;
- latest and continuation pages map explicit token availability without zeros;
- every result is owned after the transaction and remains valid after service drop.

## Task 4 — Add bounded consumer ordering without a query daemon

Files:

- add `crates/query/src/publication.rs` and focused tests;
- keep the facade synchronous; document one bounded desktop query worker as P3 shell
  ownership and direct synchronous calls for CLI/MCP.

Contracts:

- older asynchronous response cannot replace a newer snapshot generation;
- equal response coalesces without retained history;
- 10,000 candidates retain one current immutable envelope/header;
- overflow fails closed; no event/page history is retained.

## Task 5 — Performance, privacy, and resource evidence

- `EXPLAIN QUERY PLAN` proves composite keyset search;
- 100K fixture latest/cursor page stays bounded and records elapsed evidence;
- repeated open/query/drop returns handles and private memory to a stable plateau;
- Debug/error/serialized fixtures contain no archive path, source ID, fingerprint,
  SQLite text, prompt, response, command, or reasoning;
- normal query deadline remains at most two seconds.

The one-million-row cached dashboard is P2-B materialized-aggregate evidence, not a
P2-A event-page blocker or permission to full-scan from a frontend.

## Task 6 — Project truth and full gate

Update `spec/API_CONTRACT.md`, `spec/DATA_CONTRACT.md`, `spec/SECURITY.md`,
`spec/TRACEABILITY.md`, `spec/DECISIONS.md`, `docs/CURRENT_STATE.md`,
`docs/HANDOFF.md`, `docs/ROADMAP.md`, `docs/AUDIT_AND_MASTER_PLAN.md`,
`docs/FEATURE_PARITY.md`, `docs/CHANGELOG.md`, and `docs/PROJECT_HISTORY.md`.

Final gate:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

P2-A completion does not claim indexed aggregates, pricing, quota/reset inventory,
complete UI, automation, M0 acceptance, packaging, signing, or release.
