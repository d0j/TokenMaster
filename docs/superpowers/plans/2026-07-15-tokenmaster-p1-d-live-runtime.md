# TokenMaster P1-D Live Runtime Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement
> this plan task-by-task. Use `superpowers:test-driven-development` for every behavior
> change and `superpowers:verification-before-completion` before each completion claim.

**Status:** Active. P1-D.4 is the immediate execution slice. Work remains root-only
because the available task-name-only child surface cannot prove configured role/model
routing; `MODEL_ROUTING_DRIFT` remains explicit.

**Progress:** Tasks 1-7 (P1-D.0/P1-D.1/P1-D.2/P1-D.3) are implemented. Task 5 applies events
and late relations in one bounded store transaction with one epoch advance. Task 6
adds the real Codex bootstrap adapter, strict checkpoint codec, and checked store
archive bridge. Task 7 adds schema-v6 publication generations, exact scan admission,
paired-CAS replay-aware tail append, bounded partial recovery, and durable rebuild
selection. Focused store/runtime tests cover zero-byte unchanged, exact append,
multi-batch, new/missing sources, cancellation/deadline resume, replacement/truncation,
and transaction faults. No watcher, OS lease, lifecycle assembly, P1-E, M0 acceptance,
or release is claimed.

**Goal:** Compose a live built-in Codex runtime whose normal refresh is tail-only,
whose memory and watcher state are fixed/bounded, and whose writer/recovery behavior
is safe across threads, processes, failures, and restart.

**Architecture:** Repair the engine's real per-file streaming seam first, then make
store event/relation application atomic, add a separate `tokenmaster-runtime`
composition crate, add replay-aware incremental archive generation, implement a Rust
std OS file lease, and only then attach a pathless coalescing watcher/scheduler. The
existing `OneShotExecutor` remains bootstrap/rebuild, not the steady-state append
path. The binding rationale and rejected alternatives are in
`docs/superpowers/specs/2026-07-15-tokenmaster-p1-d-live-runtime-design.md`.

**Tech Stack:** Rust 1.97 stable, synchronous object-safe ports, standard-library
threads/channels/file locks, exact `notify = 8.2.0` in the runtime crate only, bundled
SQLite through the existing store, synthetic Codex JSONL contracts, PowerShell
quality/audit scripts.

---

## Task 1: P1-D.0 RED — make engine identity unique per logical file

**Files:**

- Modify: `crates/engine/tests/port_values_contract.rs`
- Modify: `crates/engine/tests/port_batch_contract.rs`
- Modify: `crates/engine/tests/port_traits_contract.rs`
- Modify: `crates/engine/tests/one_shot_executor_contract.rs` (constructor-only
  compatibility in this task)
- Modify: `crates/engine/src/values.rs`
- Modify: `crates/engine/src/batch.rs`

### Step 1: Write the failing per-file identity contract

Change the test helper to construct two identities with the same scope and provider
source ID but different fixed logical file keys:

```rust
let first = SourceIdentity::new(scope.clone(), "sessions", [1; 32])?;
let second = SourceIdentity::new(scope, "sessions", [2; 32])?;
assert_ne!(first, second);
assert_eq!(first.source_id(), second.source_id());
assert_eq!(first.logical_file_key(), &[1; 32]);
assert!(!format!("{first:?}").contains("1, 1"));
```

Add a batch contract proving a batch constructed for `first` exposes only the sealed
identity accessor and is rejected when a replay sink expects `second`, even though
their provider source IDs are equal.

### Step 2: Run the focused RED test

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test port_values_contract --locked
```

Expected: compile failure because `SourceIdentity::new` has no logical-file-key
argument/accessor and `AdapterBatch` does not retain its sealed source identity.

### Step 3: Implement the minimal identity change

- Add `[u8; 32] logical_file_key` to `SourceIdentity`.
- Require it in `SourceIdentity::new`; include it in `Eq`, `Hash`, and ordering where
  required; expose a fixed-byte accessor; keep `Debug` fully redacted.
- Remove the duplicate logical-identity storage from `DiscoveredSource`; its accessor
  delegates to `SourceIdentity`.
- Store a cloned `SourceIdentity` inside `AdapterBatch` and `CanonicalBatch`; expose a
  read-only redacted identity accessor and validate it at the executor boundary.
- Mechanically update engine fixtures with deterministic fixed keys. Do not create a
  compatibility constructor that silently invents one key for multiple files.

### Step 4: Run focused GREEN tests

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test port_values_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --test port_batch_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --locked
```

Expected: PASS; two real files under one provider source remain distinct and no debug
surface reveals either key.

### Step 5: Commit

```powershell
git add -- crates/engine/src/values.rs crates/engine/src/batch.rs crates/engine/tests/port_values_contract.rs crates/engine/tests/port_batch_contract.rs crates/engine/tests/port_traits_contract.rs crates/engine/tests/one_shot_executor_contract.rs
git commit -m "fix(engine): identify logical source files"
```

## Task 2: P1-D.0 RED — replace archive-page descriptor recovery with temporary readers

**Files:**

- Modify: `crates/engine/src/ports.rs`
- Modify: `crates/engine/src/archive.rs`
- Modify: `crates/engine/src/lib.rs`
- Modify: `crates/engine/tests/port_traits_contract.rs`

### Step 1: Write the failing object-safety/lifetime contract

Define test fakes for the intended public seam:

```rust
trait SourceBatchReader {
    fn read_batch(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError>;
}

trait ReplaySourceSink {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_checkpoint: AdapterCheckpoint,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError>;
}
```

Extend `Adapter` with object-safe `visit_replay_sources`. Change `Archive` preparation
to accept the exact `DiscoveredSource` and fresh initial checkpoint. Remove public
archive cursor/page-driven replay recovery from the contract.

The contract must prove:

- `Box<dyn Adapter>`, `&mut dyn SourceBatchReader`, `&mut dyn ReplaySourceSink`, and
  `Box<dyn Archive>` are object-safe;
- the temporary reader is invoked while the adapter owns the descriptor and is not
  returned or retained;
- a reader batch for another logical file is rejected with `invalid_data`;
- errors/debug output remain path-free and payload-free.

### Step 2: Run RED

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test port_traits_contract --locked
```

Expected: compile failure because the temporary reader/replay sink methods are absent.

### Step 3: Implement the minimal port seam

- Add `SourceBatchReader` and `ReplaySourceSink` to `ports.rs` and re-export them.
- Add `Adapter::visit_replay_sources`.
- Replace `Archive::replay_source_page` and `ReplaySourcePage` preparation with exact
  preparation of the currently streamed `DiscoveredSource` plus its fresh zero-offset
  checkpoint.
- Remove `ArchiveSourceCursor`, `ReplaySource`, `ReplaySourcePage`, and
  `MAX_REPLAY_SOURCES_PER_PAGE` only after all production/test callers move.
- Keep all callbacks synchronous. Add no generic payload, path, async trait, channel,
  or retained descriptor type.

### Step 4: Run GREEN

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test port_traits_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --test port_batch_contract --locked
```

### Step 5: Keep the RED/GREEN seam uncommitted until Task 3

The public ports and executor are one compilation unit. Do not create an intermediate
commit that passes only `port_traits_contract` while the full engine crate is broken.
Proceed directly to Task 3 and commit the port plus executor transition together after
the complete engine crate passes.

## Task 3: P1-D.0 RED — drive rebuild through two linear streaming passes

**Files:**

- Modify: `crates/engine/src/executor.rs`
- Modify: `crates/engine/tests/one_shot_executor_contract.rs`

### Step 1: Rewrite the executor fake around the second pass

The fake adapter must count discovery-pass and replay-pass descriptors separately and
lend one `FakeSourceReader` per replay callback. The fake archive prepares by logical
file key and rejects a source not in the exact scan set.

Add RED contracts for:

1. two files with the same provider source ID and different logical keys both append
   and promote;
2. second-pass cross-scope or cross-logical batch identity fails before archive append;
3. a second-pass extra source fails exact prepare and discards the latest epoch;
4. an omitted source reaches store seal as incomplete and is discarded;
5. second-pass partial/cancelled/failed quality never seals or promotes;
6. cancellation/deadline checks run before every replay callback and batch pull;
7. a repeated non-terminal checkpoint fails before append.

### Step 2: Run RED

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test one_shot_executor_contract --locked
```

Expected: compile failures from the changed adapter/archive traits, followed by
behavior failures until executor replay flow is replaced.

### Step 3: Implement the second-pass executor

- Iterate the fixed scope manifest, calling `visit_replay_sources` once per scope.
- In the replay sink validate exact scope and batch logical identity.
- Call archive prepare with the fresh initial checkpoint, update only the returned
  exact replay handle, pull/append bounded batches, and drop the reader before the next
  descriptor.
- Require complete second-pass quality for every scope. Treat any other quality as a
  replay failure and exact-discard the latest confirmed handle.
- Preserve the 4,096 continuation cap, lease-only `busy`, cancellation precedence,
  deadline checks, no-publication failure behavior, and cleanup reporting.
- Do not collect discovered identities to prove completeness; archive preparation and
  final seal remain the disk-backed exact membership proof.

### Step 4: Run GREEN and the complete engine crate

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test one_shot_executor_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --locked
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-engine --all-targets --locked
```

### Step 5: Commit

```powershell
git add -- crates/engine/src/ports.rs crates/engine/src/archive.rs crates/engine/src/lib.rs crates/engine/src/executor.rs crates/engine/tests/port_traits_contract.rs crates/engine/tests/port_batch_contract.rs crates/engine/tests/one_shot_executor_contract.rs
git commit -m "fix(engine): rebuild from linear source streams"
```

## Task 4: P1-D.0 bounds, compatibility, and project truth

**Files:**

- Modify: `crates/engine/tests/one_shot_executor_contract.rs`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/RECOVERY_PLAYBOOK.md`
- Modify: `docs/superpowers/plans/2026-07-15-tokenmaster-p1-c-engine-core.md`
- Modify: this plan

### Step 1: Add the 300-file shared-root contract

Create 300 fake logical file identities under the same provider/profile/source ID.
Assert two discovery passes, exactly 300 temporary reader callbacks, maximum one live
reader, 300 archive appends, no replay page/cursor, and exact promotion. Repeat the
fixture enough times to catch retained state growth; the fake may retain counters for
assertions but the engine API must expose no descriptor collection.

### Step 2: Run focused and dependency/privacy checks

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test one_shot_executor_contract --locked
cargo +1.97.0 tree -p tokenmaster-engine --edges normal
rg -n "tokenmaster[_-]codex|tokenmaster[_-]platform|rusqlite|slint|tokio|wasmtime|notify" crates/engine Cargo.toml Cargo.lock
rg -n "PathBuf|SourceFileDescriptor|UsageStore|raw_source|prompt|response|reasoning|command_output" crates/engine/src
```

Expected: engine normal dependency tree remains domain/accounting only; forbidden
runtime/provider/platform/UI dependencies are absent; privacy search has no new
production boundary violation.

### Step 3: Update project truth without over-claiming

Record that P1-D.0 repaired the real multi-file seam, while live Codex adapter,
incremental store, OS lease, watcher, lifecycle assembly, M0 acceptance, packaging,
and release remain unimplemented. Record the old page-based P1-C assumption as
superseded, not as historical concealment.

### Step 4: Run P1-D.0 root gate

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

### Step 5: Commit and push the completed slice

```powershell
git add -- spec/API_CONTRACT.md spec/DATA_CONTRACT.md spec/SECURITY.md spec/TRACEABILITY.md spec/DECISIONS.md docs/CURRENT_STATE.md docs/HANDOFF.md docs/ROADMAP.md docs/CHANGELOG.md docs/PROJECT_HISTORY.md docs/RECOVERY_PLAYBOOK.md docs/superpowers/plans/2026-07-15-tokenmaster-p1-c-engine-core.md docs/superpowers/plans/2026-07-15-tokenmaster-p1-d-live-runtime.md crates/engine
git commit -m "docs(engine): close real-source port repair"
git push origin cx/tokenmaster-product-architecture
```

## Task 5: P1-D.1 atomic replay event/relation batch

**Files:**

- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/tests/replay_archive_contract.rs`
- Modify: `crates/codex/tests/support/pipeline.rs`
- Modify: `crates/codex/tests/pipeline_contract.rs`

### RED/GREEN contract

Extend `ReplayAppendBatchParts` with at most 256 `SessionRelationDraft` values. Add
fault injection after event overlay work and after relation work. Prove that events,
relations, chunks, checkpoint, replay selection, work queue, and evidence epoch all
roll back together; success advances the epoch exactly once regardless of relation
count. Refactor P0-E to submit relations in that one batch and remove its per-relation
commit loop.

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
```

Commit: `fix(store): apply replay facts atomically`.

## Task 6: P1-D.2 bootstrap Codex composition

**Completed:** implemented with three checkpoint-codec contracts and seven real
bootstrap contracts. The production path covers 300 shared-source logical files,
zero/missing profiles, reopen, append, Windows atomic replacement, truncation
carry-forward, cancellation, and exact post-begin discard. It remains explicitly a
bootstrap/full-rebuild path.

**Files:**

- Create: `crates/runtime/Cargo.toml`
- Create: `crates/runtime/src/lib.rs`
- Create: `crates/runtime/src/clock.rs`
- Create: `crates/runtime/src/codex_adapter.rs`
- Create: `crates/runtime/src/store_archive.rs`
- Create: `crates/runtime/src/error.rs`
- Create: `crates/runtime/tests/bootstrap_contract.rs`
- Create: `crates/codex/src/checkpoint_codec.rs`
- Modify: `crates/codex/src/reader/mod.rs`
- Modify: `crates/codex/src/reader/source.rs`
- Modify: `crates/codex/src/lib.rs`
- Create: `crates/codex/tests/checkpoint_codec_contract.rs`
- Modify: root `Cargo.toml`

### RED/GREEN contract

Add a cheap `initialize_source_checkpoint` open/probe and the strict path-free bounded
`CodexCheckpointV1` codec. Create local runtime wrappers implementing engine ports;
map all store/provider errors to stable runtime/port codes without formatting inner
messages. Prove baseline, 300 files sharing source IDs, zero-source, append rebuild,
atomic replacement, truncation retention, cancellation, reopen, exact cleanup, and
Debug privacy over real synthetic JSONL. Do not expose this rebuild as the live
watcher path.

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test checkpoint_codec_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test bootstrap_contract --locked
cargo +1.97.0 tree -p tokenmaster-runtime --edges normal
```

Commit: `feat(runtime): compose Codex bootstrap rebuild`.

## Task 7: P1-D.3 replay-aware incremental archive

**Completed:** implemented with seven focused store contracts, eleven real runtime
contracts, 20 store unit tests including four current-append fault boundaries, exact
v5-to-v6 rollback, explicit full-rebuild source admission, profile-scope recovery,
bounded-admission fallback, and the root quality gate.

**Files:**

- Modify: `crates/store/src/usage/schema.rs`
- Modify: `crates/store/src/usage/migration.rs`
- Modify: `crates/store/src/usage/types.rs`
- Create: `crates/store/src/usage/incremental.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/usage/read.rs`
- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/src/usage/write.rs`
- Modify: `crates/store/tests/usage_schema_contract.rs`
- Create: `crates/store/tests/incremental_replay_contract.rs`
- Create: `crates/runtime/src/incremental.rs`
- Create: `crates/runtime/tests/incremental_contract.rs`

### RED/GREEN contract

Implement strict schema v6 singleton archive generation with exact v5 migration and
fault rollback. Add current replay-aware source admission, atomic current tail append,
bounded current continuation, exact generation/epoch CAS, affected-fingerprint
materialization, pending recovery, and complete-scan freshness. Disable the old
canonical-only append path in replay-verified mode.

Contracts prove unchanged reads zero historical event bytes, a one-line append starts
at the persisted offset, multi-batch tails stay bounded, new sources join only through
the exact complete scan, pending/conflict remain conservative, missing sources remain,
restart resumes current checkpoints, and replacement/truncation returns a typed
`rebuild_required` without destructive writes. Use deterministic byte/offset counters
in normal CI; record p95 timing only in the dedicated performance harness.

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test incremental_replay_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test incremental_contract --locked
```

Commit: `feat(store): add replay-aware incremental archive`.

## Task 8: P1-D.4 portable process-owned writer lease

**Files:**

- Create: `crates/platform/src/lease.rs`
- Modify: `crates/platform/src/lib.rs`
- Create: `crates/platform/tests/writer_lease_contract.rs`
- Create: `crates/platform/src/bin/lease_fixture.rs`
- Create: `crates/runtime/src/lease.rs`
- Create: `crates/runtime/tests/lease_bridge_contract.rs`

### RED/GREEN contract

Implement persistent empty sidecar plus Rust 1.97 `File::try_lock`. Prove independent
same-process handles, independent child process contention, normal exit, forced child
termination, reacquisition, canonical parent alias behavior, no payload, redacted
Debug, and stable WouldBlock/error mapping. Do not delete the sidecar on unlock.

```powershell
cargo +1.97.0 test -p tokenmaster-platform --test writer_lease_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test lease_bridge_contract --locked
```

Commit: `feat(platform): add portable writer lease`.

## Task 9: P1-D.5 bounded scheduler and filesystem hints

**Files:**

- Modify: root `Cargo.toml` with exact `notify = "=8.2.0"`
- Modify: `crates/runtime/Cargo.toml`
- Create: `crates/runtime/src/hints.rs`
- Create: `crates/runtime/src/scheduler.rs`
- Create: `crates/runtime/src/watcher.rs`
- Create: `crates/runtime/tests/scheduler_contract.rs`
- Create: `crates/runtime/tests/watcher_contract.rs`

### RED/GREEN contract

Implement the fixed atomic hint aggregate, capacity-one wake, one scheduler thread,
250 ms quiet window, 15 minute healthy poll, 60 second degraded poll, checked clock
discontinuity, bounded root watch generations, and watcher error/overflow force flag.
The callback must discard paths before touching shared state.

Use a fake monotonic clock and fake watcher for deterministic quiet/periodic tests.
Use the real watcher only for a bounded synthetic create/append/rename hint test; it is
not source authority. Ten thousand hints must retain one aggregate and at most one
engine follow-up. Shutdown joins the scheduler and drops the watcher.

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test scheduler_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test watcher_contract --locked
cargo +1.97.0 tree -p tokenmaster-runtime --edges normal
```

Commit: `feat(runtime): add bounded refresh scheduling`.

## Task 10: P1-D.6 live assembly, recovery, lifecycle, and acceptance

**Files:**

- Create: `crates/runtime/src/live.rs`
- Create: `crates/runtime/src/recovery.rs`
- Create: `crates/runtime/src/lifecycle.rs`
- Create: `crates/runtime/tests/live_runtime_contract.rs`
- Create: `crates/runtime/tests/recovery_contract.rs`
- Modify all source-of-truth, current-state, roadmap, changelog, history, recovery,
  traceability, and this plan files affected by the completed behavior.

### RED/GREEN contract

Assemble startup recovery, incremental/rebuild selection, worker/scheduler/watcher,
pause/resume/shutdown, and exact lease lifetime. Prove real synthetic startup, append,
new source, burst, concurrent hint, replacement, truncation, orphan scan close, exact
staging resume/discard, current incremental resume, sleep-style pause/resume, reopen,
and no task-owned thread/handle after shutdown. Run dependency, privacy, generated-file,
and process audits.

### Final P1-D verification

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

P1-D completion still does not claim P1-E immutable query snapshots/resource races,
M0 acceptance, packaging, or a release. Commit project truth only after the exact
focused and root gates pass; push the feature branch without force.
