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
failure discard without changing production dependency direction. P1-C.1 adds the
constant-state provider-neutral refresh coordinator and P1-C.2 adds sealed bounded
values, scope-exact batches, and object-safe adapter/archive/clock/writer-lease ports.
P1-C.3 adds the synchronous one-shot executor over those contracts. P1-C.4 completes
that plan with the bounded deterministic worker shell. P1-D.0 is complete under
`docs/superpowers/plans/2026-07-15-tokenmaster-p1-d-live-runtime.md`: it repairs exact
logical-file identity and replaces archive-page descriptor recovery with two linear
passes and one temporary descriptor-bound reader. P1-D.1 is also complete: one bounded
replay fact batch atomically applies canonical events, late relations, derived replay
state, checkpoint, and one epoch increment. P1-D.2 is complete: `tokenmaster-runtime`
now supplies the real built-in Codex bootstrap adapter, strict 32-KiB path-free
checkpoint codec, checked store bridge, real Windows replacement/reopen/300-file
contracts, and exact staging cleanup. The immediate next task is P1-D.3 replay-aware
incremental tail refresh, before the portable writer lease, watcher/periodic hints,
and lifecycle cancellation.
P2 now also has an approved separate banked-reset inventory/expiry/reminder/activation
design in `docs/superpowers/plans/2026-07-15-tokenmaster-banked-reset-inventory.md`.
It does not change the immediate P1-D gate and no current provider discovery,
notification delivery, or activation capability is claimed.
P1-A is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p1-retained-projection.md`: strict
schema v4, exact v1/v2/v3 migration, and atomic retained projection now handle
complete truncation/replacement without retaining obsolete generations. The complete
approved order is in `docs/AUDIT_AND_MASTER_PLAN.md`.

P1-B.1 through P1-B.3 are implemented under
`docs/superpowers/plans/2026-07-15-tokenmaster-p1-b-scan-authority.md`: strict schema
v5 migrates exact v1-v4 archives and adds a bounded provider/profile-qualified scan
set. Only a complete child derives presence from exact last-seen membership. Partial
or failed outcomes, append, and late registration cannot invent presence. Parent
creation and missing-state finalization have injected rollback proofs. Production
replay now stores one complete set ID, stages only its exact present membership, and
revalidates it through continuation, seal, and promotion. Zero-source retention-only
promotion preserves missing-source generations. The real synthetic Codex driver
exercises this path and closes cancelled enumeration partial. P1-B.3 keeps 32 closed
sets per scope, removes at most 64 whole unreferenced sets per transaction, preserves
running/source/replay references, and proves bounded backlog recovery, parent/child
ID exhaustion, reopen, and rollback after an injected pruning fault.

The `tokenmaster-engine` crate owns checked monotonic IDs/deadlines, one active permit,
one highest-urgency merged follow-up, cooperative atomic cancellation, sealed bounded
runtime values, scope-exact adapter/canonical batches, and object-safe synchronous
adapter/archive/clock/writer-lease contracts. `OneShotExecutor` now acquires the lease
first, streams discoveries directly into one exact scan set, enforces current-scope
ownership, starts replay only after complete close, canonicalizes bounded batches,
validates exact handle progress, and promotes or discards the last confirmed staging
handle. `RefreshWorker` adds one owned named thread, capacity-one wake/latest-result
channels, checked result supersession, cooperative cancel/wake/join shutdown and
`Drop`, stable stale/closed/faulted errors, redacted panic containment, and a compile
guard against incompatible `panic=abort`. Ten thousand hints remain one pending
aggregate; checkpoints cap at 32 KiB, event/relation batches at 256,
chunk updates at 18, and continuation calls at 4,096 per execution. P1-D.0 removes
the engine replay-page API: full rebuild now validates fixed per-file identity and
lends one temporary reader at a time, while exact store seal remains completeness
authority. Do not yet claim
Codex is composed into this worker, live scheduling exists, or the OS lease is
implemented.

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
cargo +1.97.0 test -p tokenmaster-store --test scan_contract --locked
cargo +1.97.0 test -p tokenmaster-accounting --locked
cargo +1.97.0 test -p tokenmaster-accounting --test replay_classifier_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --locked
cargo +1.97.0 test -p tokenmaster-engine --locked
cargo +1.97.0 test -p tokenmaster-platform --locked
cargo +1.97.0 test --workspace --locked
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
```

For M0 developer evidence, Pester 5.7.1 and a validated GNU linker are required:

```powershell
pwsh -NoProfile -File scripts\verify-m0.ps1 -RepositoryRoot (Get-Location).Path
```

The P0 authority/lineage/classifier, P0-D archive, P0-D.1 scalable manifest, P0-E
transactional composition, P1-A retained projection, and complete P1-B scoped scan
and replay authority slices passed focused contracts. P0-D.1 evidence includes exact populated-v2
migration and three injected rollback boundaries, 300-source set-based begin, a
two-page seal/promotion/reopen lifecycle, late-source seal rejection, and exact discard.
P0-E adds persisted physical-identity reconstruction, bounded staging/chunk reads,
exact source preparation, seven real-JSONL pipeline contracts, 300-file and 300-event
bounds, reopen after batch one, and Windows atomic replacement. P1-A adds exact
v1/v2/v3-to-v4 migration, truth-table retention, provenance/fault rollback, and
successful complete-line truncation carry-forward; cancellation, malformed data,
incomplete tails, and pending evidence remain fail-closed.
P1-B.1 adds exact v4-to-v5 populated migration plus create/copy/drop rollback,
provider-qualified lifecycle, complete-only presence, late-registration safety, and
two lifecycle fault rollback contracts. P1-B.2 adds exact multi-provider binding,
bidirectional membership revalidation, stale-scan rejection, two begin fault
boundaries, zero-source reopen/promotion, and the seven scan-bound Codex pipeline
contracts. P1-B.3 adds the repeated-scan 32-row plateau, 64+remainder backlog passes,
running/replay-reference preservation, checked ID exhaustion, and close+prune rollback.
P1-C.1/P1-C.2 add coordinator and port contracts; P1-C.3 adds the provider-neutral
one-shot executor; P1-C.4 adds ten worker burst/backpressure/stale/deadline/shutdown/
drop/panic/lock-order contracts. P1-D.0 brings the executor suite to 23 contracts,
including same-source-ID logical files, cross-file batches, extra/omitted second-pass
files, incomplete second-pass quality, and repeated 300-file/one-live-reader proof.
P1-D.1 adds a 47-test replay suite with two atomic fault boundaries, a 256 relation
cap, one epoch advance regardless of relation count, and the seven green real-JSONL
pipeline contracts without the prior per-relation commit loop. P1-D.2 adds three
checkpoint-codec and seven production-bootstrap contracts, including 300 files,
zero/missing profiles, reopen, Windows replacement, truncation retention, cancellation,
and exact post-begin discard. This does not claim incremental tail refresh, watcher
scheduling, or a real OS lease.
Clean-root, formatting, strict workspace Clippy, and the full locked workspace passed;
see the P1-A history entry for exact commands and focused counts. The
one-million-row M0 scale test remains explicitly ignored in the normal workspace run.
This does not accept M0, prove interactive Windows behavior, or package a product
release. See `M0_ACCEPTANCE.md`.
