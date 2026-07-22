# TokenMaster Provider Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow a second built-in provider to supply usage and quota/benefit data through provider-neutral runtime modules without changing TokenMaster storage, query, product, desktop, or UI consumers.

**Architecture:** Keep Codex as the native default adapter, but move provider choice to the application composition root. The shared runtime consumes boxed provider-neutral usage and quota ports. Existing SQLite source-progress columns remain unchanged: their bounded `resume_payload` is provider-owned opaque resume state, while the concrete descriptor-bound reader alone reconstructs its complete `AdapterCheckpoint`. External Wasm hosting, package installation, permissions, and SDK work remain deferred to 1.1.

**Tech Stack:** Rust 1.97, bundled SQLite through rusqlite, existing synchronous engine ports, focused contract tests, PowerShell release audits.

## Global Constraints

- Codex-only startup and idle operation MUST NOT load Wasmtime, start another process, add a timer, or add an unbounded collection.
- Engine, archive, query, automation, and UI code MUST depend on provider-neutral observations and snapshots rather than Codex paths or JSONL wire shapes.
- Providers emit bounded drafts and opaque checkpoints; only `tokenmaster-accounting` constructs canonical events.
- Adapter checkpoints remain path-private, redacted, versioned, and capped at 32 KiB.
- Observation and relation batches remain capped at 256; chunk updates remain capped at 18.
- No prompt, response, reasoning, command, source content, credential, raw incomplete line, absolute user path, raw provider payload, or inner provider error may cross persistence, diagnostics, query, product, desktop, or UI boundaries.
- One writer owns the worktree. Each behavior change follows focused RED/GREEN tests. One implementation review and one final re-review are the normal maximum.
- This slice does not implement Wasmtime, WIT packages, `.tmplugin` installation, marketplace, native DLL loading, arbitrary executable providers, network authority, provider mutation, custom UI, SQL, CLI, or MCP.

---

### Task 1: Restore provider checkpoints without Codex archive authority

**Files:**
- Modify: `crates/engine/src/values.rs`
- Modify: `crates/engine/src/batch.rs`
- Modify: `crates/engine/src/ports.rs`
- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/read.rs`
- Modify: `crates/store/src/usage/incremental.rs`
- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/runtime/src/codex_adapter.rs`
- Modify: `crates/runtime/src/store_archive.rs`
- Test: `crates/runtime/tests/provider_checkpoint_contract.rs`
- Test: affected engine/store/runtime checkpoint and migration contracts

**Interfaces:**
- Consumes: existing `AdapterCheckpoint`, `SourceIdentity`, `AdapterBatch`, `StoredCheckpoint`, and schema-v13 source generations.
- Produces: `AdapterSourceProgress` with bounded provider-owned opaque resume state and archive operations that never decode or construct a provider checkpoint.

- [ ] **Step 1: Write the failing non-Codex checkpoint contract**

Create a real-store runtime contract whose synthetic adapter returns a checkpoint beginning with `synthetic-provider-v1`, plus a valid bounded `AdapterSourceProgress` containing the provider-owned resume bytes needed to reconstruct it. Prove full rebuild, process-style store reopen, and one incremental append return the exact opaque checkpoint to the synthetic reader. Assert debug/query surfaces do not expose those bytes.

```rust
let checkpoint = AdapterCheckpoint::new(b"synthetic-provider-v1\0page-0001".to_vec().into_boxed_slice())?;
let progress = AdapterSourceProgress::new(AdapterSourceProgressParts {
    schema_version: 1,
    physical_identity: None,
    logical_identity: [7; 32],
    committed_offset: 1,
    scan_offset: 1,
    observed_extent: 1,
    modified_time_ns: None,
    anchor_start: 0,
    anchor_len: 0,
    anchor_sha256: [0; 32],
    provider_resume: b"page-0001".to_vec().into_boxed_slice(),
    discarding_oversized_record: false,
    incomplete_tail: false,
    verification: AdapterVerification::Full,
})?;
```

- [ ] **Step 2: Run the focused contract and verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-runtime --test provider_checkpoint_contract --locked`

Expected: compilation or assertion failure because the source-progress API and descriptor-bound checkpoint restoration do not exist and `StoreArchive` constructs `CodexCheckpointV1` itself.

- [ ] **Step 3: Add the provider-neutral progress value**

Add validated, non-serializable, redacted `AdapterSourceProgress` and `AdapterVerification` engine values. Carry the progress beside `AdapterCheckpoint` in initial source discovery and every `AdapterBatch`/`CanonicalBatch`. Validate logical identity, offsets/extents, anchors, incomplete/discard state, provider resume at the existing 32-KiB bound, and fixed bounds in the constructor.

```rust
pub struct AdapterSourceState {
    checkpoint: AdapterCheckpoint,
    progress: AdapterSourceProgress,
}

impl AdapterSourceState {
    pub fn new(
        checkpoint: AdapterCheckpoint,
        progress: AdapterSourceProgress,
    ) -> Result<Self, EngineError>;
    pub const fn checkpoint(&self) -> &AdapterCheckpoint;
    pub const fn progress(&self) -> &AdapterSourceProgress;
}
```

Change `SourceSink` and replay source callbacks to receive this complete state rather than an unpaired checkpoint. Extend the descriptor-bound `SourceBatchReader` with `restore_checkpoint(&AdapterSourceProgress, &OperationControl) -> Result<AdapterCheckpoint, PortError>`. The default must fail closed; only the concrete provider reader interprets `provider_resume` and constructs its checkpoint. Engine and archive code never interpret those bytes.

- [ ] **Step 4: Map progress through the existing store schema**

Do not change `USAGE_SCHEMA_VERSION` or canonical SQLite SQL. Map `AdapterSourceProgress.provider_resume` to the existing bounded `StoredCheckpoint.resume` / `usage_generation.resume_payload` column and map the remaining common progress fields exactly. `StoreArchive` reads and writes only this provider-neutral progress projection.

Move checkpoint reconstruction to the concrete `SourceBatchReader`. `CodexSourceBatchReader` reconstructs `CodexCheckpointV1` from the stored progress; a synthetic reader reconstructs its arbitrary checkpoint from the same common progress plus opaque resume. Remove all `tokenmaster_codex` checkpoint imports and encode/decode helpers from `StoreArchive`. No schema migration or legacy fallback is introduced.

- [ ] **Step 5: Adapt Codex and verify GREEN**

Have `CodexAdapter` project its existing reader checkpoint into `AdapterSourceProgress` and keep `CodexCheckpointV1` encoding/decoding private to the adapter/reader. Run:

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test provider_checkpoint_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --locked
cargo +1.97.0 test -p tokenmaster-store --test incremental_replay_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test incremental_contract --locked
```

Expected: all pass, including exact synthetic checkpoint restoration, unchanged schema-v13 evidence, and existing Codex checkpoint/replay behavior.

- [ ] **Step 6: Commit**

```powershell
git add crates/engine crates/store crates/runtime
git commit -m "refactor(runtime): persist opaque provider checkpoints"
```

---

### Task 2: Inject provider usage modules into the live runtime

**Files:**
- Create: `crates/runtime/src/provider.rs`
- Modify: `crates/runtime/src/lib.rs`
- Modify: `crates/runtime/src/live.rs`
- Modify: `crates/runtime/src/codex_adapter.rs`
- Modify: `crates/app/src/application.rs`
- Test: `crates/runtime/tests/provider_live_runtime_contract.rs`
- Test: affected live runtime and application composition contracts

**Interfaces:**
- Consumes: Task 1 `AdapterSourceState`, existing object-safe `Adapter`, bounded watcher roots, and optional sealed Git hint ingress.
- Produces: `UsageProviderFactory`, `LiveProviderAdapter`, `CodexUsageProviderFactory`, and provider-injected `LiveRuntime` construction while retaining current Codex convenience constructors.

- [ ] **Step 1: Write the failing provider injection contract**

Create a synthetic factory whose adapter uses provider ID `synthetic`, arbitrary opaque checkpoints, no watch roots, and no repository-hint capability. Start the real `LiveRuntime`, wait for the first completed publication, query the store, and assert synthetic usage appears without constructing `CodexAdapter` or a Codex discovery request.

```rust
pub trait UsageProviderFactory: Send + 'static {
    fn descriptor(&self) -> &ProviderDescriptor;
    fn build(
        self: Box<Self>,
        repository_hints: Option<GitRepositoryHintIngress>,
    ) -> Result<Box<dyn LiveProviderAdapter>, RuntimeError>;
}

pub trait LiveProviderAdapter: Adapter {
    fn watch_roots(&self) -> ProviderWatchRoots;
}
```

- [ ] **Step 2: Run the focused contract and verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-runtime --test provider_live_runtime_contract --locked`

Expected: compilation failure because `UsageProviderFactory`, `LiveProviderAdapter`, and provider-injected `LiveRuntime` construction do not exist.

- [ ] **Step 3: Implement the minimal runtime seam**

Add a bounded `ProviderWatchRoots` value capped by the existing watcher-root limit. Store `Box<dyn LiveProviderAdapter>` in `LiveExecution`. Add provider-injected guarded/notified constructors and retain the current Codex constructors as thin wrappers around `CodexUsageProviderFactory`. Build the adapter only after the existing Git ingress exists; pass `None` unless `ProviderCapability::RepositoryActivity` is declared.

The runtime continues to own one worker, one scheduler, one watcher, one archive, one writer lease, and one current publication. This task supports one configured usage provider per runtime; a multi-provider scheduler/registry is not introduced.

- [ ] **Step 4: Make application composition explicit and verify GREEN**

Change application bootstrap to construct the built-in Codex factory explicitly and call the provider-injected runtime constructor. Run:

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test provider_live_runtime_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test live_runtime_contract --locked
cargo +1.97.0 test -p tokenmaster-app --lib --locked
```

Expected: all pass; existing Codex behavior remains unchanged and the synthetic provider reaches the real store through the same runtime.

- [ ] **Step 5: Commit**

```powershell
git add crates/runtime crates/app
git commit -m "refactor(runtime): inject usage provider modules"
```

---

### Task 3: Generalize quota and benefit polling behind one provider port

**Files:**
- Create: `crates/runtime/src/provider_quota.rs`
- Modify: `crates/runtime/src/quota/execution.rs`
- Modify: `crates/runtime/src/quota/health.rs`
- Modify: `crates/runtime/src/quota/runtime.rs`
- Modify: `crates/runtime/src/quota/mod.rs`
- Modify: `crates/product/src/runtime.rs`
- Modify: `crates/product/src/reducer.rs`
- Modify: `crates/app/src/application.rs`
- Test: `crates/runtime/tests/provider_quota_runtime_contract.rs`
- Test: affected quota, benefit, product reducer, and application contracts

**Interfaces:**
- Consumes: existing provider-neutral `QuotaSample`, `BenefitInventoryObservation`, store publishers, scheduler, cancellation, deadline, and count-only health.
- Produces: `ProviderQuotaSource`, `ProviderQuotaPoll`, `ProviderQuotaRuntime`, provider-neutral health enums, and a Codex source adapter preserving the current public compatibility aliases during this slice.

- [ ] **Step 1: Write the failing synthetic quota/benefit contract**

Create a source for provider ID `synthetic` returning one quota observation and one benefit observation. Run the real scheduler/publication path and assert both appear through existing store/query/product projections. Add a failure case proving stable provider-neutral transport/unavailable codes contain no raw provider error.

```rust
pub struct ProviderQuotaPoll {
    observed_at_ms: i64,
    quota: Box<[QuotaObservation]>,
    benefits: Option<BenefitInventoryObservation>,
}

pub trait ProviderQuotaSource: Send + 'static {
    fn poll(&mut self, observed_at_ms: i64) -> Result<ProviderQuotaPoll, ProviderPollErrorCode>;
}
```

- [ ] **Step 2: Run the focused contract and verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-runtime --test provider_quota_runtime_contract --locked`

Expected: compilation failure because quota execution accepts only `CodexQuotaSnapshot` and Codex-specific source/failure types.

- [ ] **Step 3: Extract the provider-neutral runtime types**

Move scheduler/publication control to provider-neutral names and values. The Codex transport/normalizer stays in `tokenmaster-codex`; a small `CodexQuotaSource` converts its snapshot into `ProviderQuotaPoll` before shared runtime publication. Preserve behavior and bounds: at most 32 quota windows, bounded benefits, one scheduler, one worker, I/O before writer lease, exact partial publication counts, stable redacted failures, and joined shutdown.

Keep temporary `pub type CodexQuotaRuntime = ProviderQuotaRuntime<CodexQuotaSource>`-style compatibility aliases only where they avoid unrelated churn; new product/app code must consume provider-neutral runtime health.

- [ ] **Step 4: Verify GREEN and compatibility**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test provider_quota_runtime_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test quota_runtime_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test quota_runtime_resource_contract --locked
cargo +1.97.0 test -p tokenmaster-product --locked
cargo +1.97.0 test -p tokenmaster-app --lib --locked
```

Expected: synthetic and Codex paths pass through one shared quota runtime; resource counts remain bounded.

- [ ] **Step 5: Commit**

```powershell
git add crates/runtime crates/product crates/app
git commit -m "refactor(runtime): generalize provider quota polling"
```

---

### Task 4: Synchronize contracts, evidence, and release state

**Files:**
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/CHANGELOG.md`

**Interfaces:**
- Consumes: verified implementation and receipts from Tasks 1-3.
- Produces: exact provider-readiness truth without claiming `.tmplugin`, WIT host, M0, package, RC, or release acceptance.

- [ ] **Step 1: Update normative and operational truth**

Record the implemented opaque-checkpoint, usage-factory, and quota-source boundaries. Keep external package hosting planned for 1.1. Separately state product behavior, evidence, remaining plugin-host work, release blockers, and Git state. Do not record a current commit hash in tracked documents.

- [ ] **Step 2: Run focused source and contract audits**

Run the existing provider/runtime/privacy audits named by traceability plus:

```powershell
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-engine -p tokenmaster-store -p tokenmaster-runtime -p tokenmaster-product -p tokenmaster-app --all-targets --locked
```

Expected: all pass without adding a new textual audit category.

- [ ] **Step 3: Run one final baseline**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Expected: exact baseline passes once after focused corrections. Do not rerun a long gate after an audit-only documentation correction.

- [ ] **Step 4: Commit**

```powershell
git add spec docs
git commit -m "docs: record provider-ready runtime contracts"
```
