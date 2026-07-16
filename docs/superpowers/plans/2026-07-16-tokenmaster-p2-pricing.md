# TokenMaster P2-C Pricing Implementation Plan

**Design:** `docs/superpowers/specs/2026-07-16-tokenmaster-p2-pricing-design.md`

Use focused red/green tests for every task. Keep each commit independently reviewable.
Do not push or package without explicit user direction.

## Task 1: Pure fixed-point pricing engine

**Add:** `crates/pricing`

1. Write failing contracts for money/rate parsing, checked accumulation, one-time
   rounding, standard/cached/output calculation, reasoning-inclusive billable output,
   whole-request long context, priority, unsupported priority-long-context, exact
   aliases, missing models, and legitimate explicit zero.
2. Add the crate with no runtime dependencies outside the standard library and domain
   bounded identifiers where useful.
3. Embed the reviewed Codex/OpenAI catalog, catalog identity, retrieval date, and source
   metadata. Keep exact reviewed aliases only.
4. Run crate tests and strict crate Clippy.

**Commit:** `feat(pricing): add deterministic pinned cost engine`

## Task 2: Validated immutable overrides and cost selection

1. Write failing adversarial tests for maximum count/length/rate/threshold, strict
   decimal syntax, duplicates, incomplete new models, alias cycles/chains, atomic
   rejection, override provenance, and stable revision identity.
2. Implement immutable override snapshots capped at 512 entries.
3. Write failing fixtures for `auto`, `calculated`, `reported`, complete/partial/
   unavailable/zero, missing reasons, and the one-cent-plus-two-percent conflict rule.
4. Implement selection over bounded synthetic price-basis rows.
5. Prove Debug/errors contain no source URLs beyond catalog metadata, raw values, paths,
   SQL, prompts, commands, or reasoning text.

**Commit:** `feat(pricing): add bounded overrides and cost provenance`

## Task 3: Domain and schema-v9 source cost/price basis

1. Add failing domain tests for optional bounded source-reported USD microdollars.
2. Add failing v8-to-v9/current-schema/malformed-schema tests for the new event column,
   aggregate state version, time/session price tables, indexes, checks, and triggers.
3. Implement schema v9 migration without changing retained private-data policy.
4. Add failing write/replay/delete/replace tests for per-event tier/context/reported-state
   price facts and billable-output derivation.
5. Implement transactional current rollup maintenance.

**Commit:** `feat(store): add transactional price basis rollups`

## Task 4: Recovery rebuild and bounded store queries

1. Add failing current/legacy rebuild, cancel/resume, reopen, failure, cleanup, and
   generation-publication tests for both price tables.
2. Extend the existing page rebuild with constant per-event price rows; do not add an
   unbounded Rust map or a second raw-history pass.
3. Add failing range/segment/scope/session query tests for deterministic grouping,
   512-key cap, exact omitted counters, deadlines, read-only behavior, and identity.
4. Implement grouped price-basis capture APIs and index-plan assertions.

**Commit:** `feat(store): add bounded price basis reads`

## Task 5: Query facade cost integration

1. Add failing public contracts that attach cost to overview, series points, breakdown
   items, session pages/details, and exact current/legacy dataset identity.
2. Implement batched grouped cost reads; forbid one store query per series point.
3. Expose mode, availability, amount, provenance, catalog/override identity, counters,
   conflicts, and capped missing keys with path-private Debug/errors.
4. Verify calendar/timezone output is unchanged and price calculation is timezone-
   independent over the same immutable event set.

**Commit:** `feat(query): expose truthful usage cost estimates`

## Task 6: Release-scale and resource evidence

1. Extend the ignored million-event current/legacy gate with price rows and cold/cached
   overview, 400-point series, four breakdowns, 32 scopes, and session costs.
2. Keep total SQLite main/WAL/SHM amplification at or below 3.0x, cold queries below
   1 second, cached p95 below 250 ms, and session p95 below 100 ms.
3. Extend the Windows resource loop for catalog/override/query switches: no handle,
   thread, USER, or GDI growth and private-memory plateau growth at or below 2 MiB.
4. Add a production binary/string audit proving there is no pricing HTTP client or
   runtime LiteLLM/models.dev fetch path.

**Commit:** `test(pricing): close cost scale and resource gates`

## Task 7: Project truth and full acceptance

Update affected specification, data/API/security contracts, traceability, decisions,
feature parity, current state, handoff, roadmap, changelog, and project history. Record
facts and commands, never a current commit hash in tracked docs.

Run:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Then run the explicit ignored P2-C release gate, inspect the exact task-owned process
tree and temporary directories, perform a root-owned read-only critical review, and
commit only intentional files.

**Commit:** `docs(pricing): close P2-C project truth`

P2-C completion does not claim quota/reset inventory (P2-D), Git output metrics (P2-E),
joined UI snapshots (P2-F), P3 UI, P5 automation, M0 acceptance, packaging, signing, or
release.
