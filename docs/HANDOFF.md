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
contracts, and exact staging cleanup. P1-D.3 is complete: schema v6 adds the checked
current publication generation; runtime exact-scan/preflight/tail passes read zero
payload bytes when unchanged, append from persisted checkpoints, admit multiple new
or empty sources, retain missing history, resume deadline-partial work, and durably
mark replacement/truncation or profile-scope changes `recovery_pending`. Full rebuild
recovers any unadmitted provisional source without trusting its abandoned checkpoint.
P1-D.4 is complete: a persistent empty local sidecar and Rust 1.97 `File::try_lock`
provide one non-blocking process-owned writer guard. Contention alone maps to `busy`;
normal exit, forced termination, and guard drop release the lock; paths and OS errors
remain private. P1-D.5 is complete: a fixed pathless atomic hint aggregate,
capacity-one wake, one scheduler thread, 250 ms quiet window, 15 minute healthy/60
second degraded reconciliation, checked clock rollback, and bounded `notify = 8.2.0`
root generations now feed only refresh urgency into the existing worker. P1-D.6 is
complete: `LiveRuntime` performs lease-first startup recovery, owns the Codex adapter,
archive, worker, scheduler and watcher lifecycle, selects incremental versus rebuild,
and implements admission-safe pause/resume plus ordered joined shutdown. P1-E.1 is now
complete: one startup-seeded immutable `EngineSnapshot` advances only for a strictly
newer archive generation and exposes exact revision/scan/data-through/quality plus
fixed checked diagnostics. Equal/older and writer-busy candidates cannot replace it;
focused store/runtime tests are green. P1-E.2 now closes no-change, pause/resume,
restart, malformed-truncation recovery, canonical-retention, and successful-repair
publication behavior. P1-E.3 completes the Windows 8+ static capacity-one power
callback, narrow runtime command, forced resume reconciliation, and 4,096-cycle
resource proof without helper thread/window or callback heap state. P1 is implemented.
P2-A adds the new `tokenmaster-query` crate with checked two-axis identity,
immutable bounded headers/envelopes/pages, stable path-free errors, injected clock
sampling, and redacted cursors. `UsageReadStore` is read-only/no-migration, query-only/
defensive, fixed-cache and no-checkpoint-on-close; one short deferred transaction owns
exact publication/scan/current-or-legacy keyset capture, stale cursor rejection,
deadline cancellation/cleanup, and `pageSize + 1`. `QueryService` now maps exact
freshness/quality and accounting-version staleness, preserves cursors across no-change
publication, rejects changed replay revision or dataset generation, and publishes only
successful strictly newer owned envelopes. The 100K latency and 256-cycle Windows
resource contracts pass. P2-A plus the audited schema-v7 dataset-generation correction
are implemented; exact migration rollback, insert/update/delete/overflow, real
no-change scan, and append contracts pass. P2-B schema-v8 provider materialization,
transactional UTC/session rollups, and bounded resumable generation publication are
also implemented and focused-store verified. The first fixed overview read now
captures publication, ready generation, and exact metrics in one transaction over at
most three adjacent aligned UTC segments and 32 scopes; boundary, stale/rebuild,
deadline, and raw-plan contracts pass. A combined capture now adds an exact partitioned
400-point series and four independently capped 256-item model/project/provider/profile
breakdowns in that same transaction, including explicit skipped-date zero points and
truncation. All-time current/legacy session pages now use mixed-order 256+1 keyset
continuation and exact dataset-bound opaque keys; detail uses only capped model/project
session rollups. Raw session identity has no getter or Debug projection.
Private calendar composition and immutable public query values are now complete:
explicit/resolved IANA zones, configurable week starts, exact DST/skipped-date
boundaries, known/partial/unavailable metrics, optional 400-point daily series, four
breakdowns, and owned session page/detail mappings pass. Public session cursors bind
dataset plus canonical scopes so a filter change cannot skip keyset rows. Jiff 0.2.32
is private; the locked Windows chain bundles tzdb 2026c. P2-B Task 8 now passes on
deterministic current/legacy million-event fixtures: rebuild 13,240/12,324 events/s,
page p95 246.558/268.305 ms, amplification 1.483x/1.568x, cold overview below 179 ms,
cached overview p95 below 0.55 ms, full 400-point/four-breakdown/32-scope analytics
below 166 ms, session page p95 below 0.75 ms, and repeated Windows resource plateaus.
The measured rebuild cap is 2,048 events; the former 256-event cap failed throughput.
P2-C is complete. Schema v9 adds transactional time/session price-basis facts and exact
v8 migration/recovery without persisting calculated cost. `tokenmaster-pricing` owns
release-pinned integer USD-micro calculation, immutable 512-entry overrides, exact
aliases, three selection modes, provenance/conflict/missing evidence, and no runtime
network. The query facade attaches dataset-exact cost to overview, 400-point series,
four breakdowns, session pages, and detail through 401/256-target batches with a
global 512-key detail cap, never one query per visible row. The final current/legacy
million gate passed: amplification 1.862x/2.010x, full p95 148.168/156.080 ms,
32-scope 158.588/162.504 ms, session page below 14 ms, detail below 1 ms. Resource and
production pricing-network audits pass. P2-D quota history core Task 1 is complete:
the floating-point `QuotaTarget` placeholder is gone, and exact bounded quota IDs,
parts-per-million ratios, optional units, provider thresholds, definitions, samples,
reset evidence, validated serde, and redacted observation IDs are implemented and
focused-verified. The immediate next slice is Task 2 in
`docs/superpowers/plans/2026-07-16-tokenmaster-p2-quota-core.md`: pure deterministic
reset evaluation and identities. Schema v10, persistence, retention, reads, public
query values, permitted Codex quota transport, banked-reset inventory/reminders, and
UI are not implemented. Do not replace aggregates with view-time full scans, infer
quota from local token/cost totals, or relabel whole-session totals as period totals.
The post-Task-1 complete baseline passes. During that gate, the existing query
resource binary was corrected from a multi-thread test-harness/single-sample
`PrivateUsage` measurement to an isolated `harness = false` process with two
retained-return windows. The original 1 MiB open/drop and 2 MiB aggregate/rebuild
budgets plus per-sample handle/thread/USER/GDI bounds remain unchanged.
The 2026-07-16 closure review also freezes the remaining plan ambiguities: P3 is the
complete UI, P4 presentation/localization, P5 read-only automation, and P6 the
canonical MSVC signed portable release. It selects the Slint attribution route,
forbids private/browser quota integration, keeps pricing release-pinned, expands
`docs/FEATURE_PARITY.md` into a blocking row-level ledger, and defines the full
supply-chain gate. See
`docs/superpowers/specs/2026-07-16-tokenmaster-plan-closure-design.md` and ADR-024.
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
and exact post-begin discard. P1-D.3 adds seven store and eleven real runtime contracts:
schema-v6 migration, paired revision/archive CAS, exact scan freshness/admission,
zero-byte unchanged, exact tail bytes, 300-event multi-batch, multiple new/empty and
missing sources, full-rebuild source admission, profile-scope and bounded-admission
recovery, changed provisional identity, cancellation, deadline resume, durable rebuild
state, and four
transaction rollback boundaries. P1-D.4 adds eight platform lease integration
contracts, one Windows drive-classification unit contract, and two runtime lease
contracts covering same/cross-process contention, normal/forced process release,
canonical aliasing, empty persistence, unsupported/remote locations, privacy, and
bridge mapping; the eighth integration contract proves no handle-count growth across
4,096 acquire/drop cycles. P1-D.5 adds five scheduler and five watcher contracts:
10,000 hints collapse into one aggregate and one real worker follow-up, healthy/
degraded timing and clock rollback are deterministic, create/append/rename is reduced
to a pathless hint, missing/invalid/oversized generations fail closed, and 32 Windows
root replacements return process handles and threads to baseline. P1-D.6 adds four
startup-recovery and three combined live contracts: lease contention blocks before
SQLite creation, orphan scans close, exact staging resumes/discards, append plus
new-source publication works, 10,000 hints remain bounded, current partial state
resumes without duplicates, replacement/truncation rebuilds preserve prior truth,
pause/resume/reopen succeeds, and combined Windows handles/threads return to baseline
after shutdown.
P1-E.2 adds no-change/resume/restart and failed-truncation recovery contracts. Blocking
malformed/incomplete/oversized adapter diagnostics now fail before checkpoint or batch
commit; the archive remains `recovery_pending`, prior canonical truth stays readable,
and a later valid rebuild returns the immutable publication to `complete`.
P1-E.3 adds actual OS callback register/unregister, deterministic last-event-wins power
semantics, resume-without-suspend recovery, duplicate/shutdown behavior, and a
4,096-cycle plateau for private bytes, handles, threads, USER, and GDI objects. It does
not replace the later frozen-candidate interactive hibernation or soak receipts.
An explicit `x86_64-pc-windows-msvc` check currently stops before TokenMaster code:
the installed Rust target has no `cl.exe`, Visual Studio installation, or `vswhere` in
this environment. GNU remains the developer gate; MSVC setup/comparison remains P6.
Clean-root, formatting, strict workspace Clippy, and the full locked workspace passed;
see the P1-A history entry for exact commands and focused counts. The
one-million-row M0 scale test remains explicitly ignored in the normal workspace run.
This does not accept M0, prove interactive Windows behavior, or package a product
release. See `M0_ACCEPTANCE.md`.
