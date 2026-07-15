# TokenMaster handoff

## First five minutes

1. Read `AGENTS.md`, then the `spec/` files in its declared order.
2. Confirm `git status --short` is empty and use a feature branch or worktree.
3. Run the clean-root audit and the focused test for the next requirement.
4. Do not infer an interactive or release result from developer-only evidence.

## Current implementation boundary

P0-A and the incorporated P0-B Codex-lineage surface are complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-authority-boundary.md`. The P0-C pure
classifier is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-c-replay-classifier.md`. P0-D schema
v2, exact-v1 immutable migration, explicit archive state, fixed-manifest staging, and
classified replay append, durable continuation, exact seal, rollback-safe promotion,
and staging recovery are implemented under
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-d-replay-archive.md`. P0-D.1 is also
complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-scalable-replay-manifest.md`: strict
schema v3, exact non-destructive v2 migration, SQLite-owned all-source begin, checked
`u64` counts, and 256-row keyset validation remove the historical product cap. P0-E is
complete under `docs/superpowers/plans/2026-07-14-tokenmaster-p0-e-pipeline-proof.md`:
the test-only driver proves the real synthetic Codex-to-archive path with more than
256 files/events, restart, append, atomic replacement, exact totals/quality, and
failure discard without changing production dependency direction. The immediate next
task is P1-B scan epochs/source-set finalization, not expansion of the test driver.
P1-A is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p1-retained-projection.md`: strict
schema v4, exact v1/v2/v3 migration, and atomic retained projection now handle
complete truncation/replacement without retaining obsolete generations. The complete
approved order is in `docs/AUDIT_AND_MASTER_PLAN.md`.

Tasks 3+ in `2026-07-14-tokenmaster-p0-replay-correctness.md` are superseded. Do not
execute its Codex-owned fingerprint/signature or destructive migration steps. Do not
add Wasmtime to the GUI or current Codex path.

Codex parser resume v1 is deliberately rejected: it cannot recover a trustworthy
event ordinal. Do not reinterpret it as ordinal zero. Immutable legacy rows remain
read-only; rebuild current schema state in a separate staging generation.

Do not expose a staging revision as current truth. Replay append advances a
store-owned evidence epoch and must reject stale CAS, altered duplicate observations,
mixed accounting versions, or stale durable work atomically. Late explicit relations
use deterministic first-source identity; conflicting parents and confirmed cycles are
permanent conflict. Seal requires exact complete manifest evidence. Promotion requires
zero pending rows and a valid prior projection owner. It installs eligible selections,
suppresses replay-only prior contributions, carries absent/conflict-only
replay-verified events, and swaps all visible state in one transaction. Unrebuilt
legacy rows stay in the immutable legacy snapshot instead of entering replay-verified
totals. A blocked/obsolete staging revision is recovered only through the
exact revision/epoch discard API; never delete archive rows or the database manually.
An untouched staging source must first be prepared with its exact revision/epoch and
a validated zero-offset adapter checkpoint. Do not copy or invent opaque resume state.
Reader truncation/replacement classification is not deletion authority. Only a
complete sealed overlay may invoke the P1-A carry-forward policy; partial, cancelled,
pending, stale, or invalid rebuilds remain blocked.

## Commands

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
cargo +1.97.0 test -p tokenmaster-accounting --locked
cargo +1.97.0 test -p tokenmaster-accounting --test replay_classifier_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --locked
cargo +1.97.0 test -p tokenmaster-platform --locked
cargo +1.97.0 test --workspace --locked
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
```

For M0 developer evidence, Pester 5.7.1 and a validated GNU linker are required:

```powershell
pwsh -NoProfile -File scripts\verify-m0.ps1 -RepositoryRoot (Get-Location).Path
```

The P0 authority/lineage/classifier, P0-D archive, P0-D.1 scalable manifest, P0-E
transactional composition, and P1-A retained projection slices passed focused
contracts. P0-D.1 evidence includes exact populated-v2
migration and three injected rollback boundaries, 300-source set-based begin, a
two-page seal/promotion/reopen lifecycle, late-source seal rejection, and exact discard.
P0-E adds persisted physical-identity reconstruction, bounded staging/chunk reads,
exact source preparation, seven real-JSONL pipeline contracts, 300-file and 300-event
bounds, reopen after batch one, and Windows atomic replacement. P1-A adds exact
v1/v2/v3-to-v4 migration, truth-table retention, provenance/fault rollback, and
successful complete-line truncation carry-forward; cancellation, malformed data,
incomplete tails, and pending evidence remain fail-closed.
Clean-root, formatting, strict workspace Clippy, and the full locked workspace passed;
see the P1-A history entry for exact commands and focused counts. The
one-million-row M0 scale test remains explicitly ignored in the normal workspace run.
This does not accept M0, prove interactive Windows behavior, or package a product
release. See `M0_ACCEPTANCE.md`.
