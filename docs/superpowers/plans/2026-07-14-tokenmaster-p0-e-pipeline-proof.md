# TokenMaster P0-E Transactional Pipeline Proof Implementation Plan

> **Execution mode:** root-only, test-first, one writer, current feature branch. The
> plan is implemented inline because the user explicitly asked to continue
> autonomously and model/role routing is not provable on the available spawn surface.

**Goal:** Prove that real synthetic Codex JSONL files flow through discovery,
streaming enumeration, restart-safe reader batches, accounting canonicalization, and
the transactional replay archive with exact atomic outcomes and bounded working
state.

**Architecture:** Keep all production dependency edges unchanged. Add three narrow
restart seams (`PhysicalFileIdentity::from_persisted_bytes`, one exact staging
generation read, one exact chunk read), then implement a development-only integration
driver in `tokenmaster-codex`. The driver streams two enumeration passes, holds one
reader/canonical batch at a time, uses only public production APIs, and is never
compiled into a product scheduler.

**Stack:** Rust 2024, Codex provider/reader, accounting canonicalizer, SQLite through
`tokenmaster-store`, Cargo integration tests, rustfmt, strict Clippy.

---

## Task 1: Persisted physical identity reconstruction

**Files:**

- Modify: `crates/platform/src/lib.rs`
- Test: `crates/platform/tests/physical_identity_contract.rs`

**Step 1: Write the failing contract test**

Add a test that opens a temporary file, obtains its live identity, reconstructs it
from `*identity.as_bytes()`, and proves equality plus redacted Debug output:

```rust
let live = PhysicalFileIdentity::from_file(&file).expect("identity");
let restored = PhysicalFileIdentity::from_persisted_bytes(*live.as_bytes());
assert_eq!(restored, live);
assert!(!format!("{restored:?}").contains(&fixture_path));
```

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-platform --test physical_identity_contract persisted_identity_round_trips_without_exposing_os_fields --locked
```

Expected RED: `from_persisted_bytes` does not exist.

**Step 2: Add the minimal constructor**

```rust
/// Reconstructs an opaque identity from its controlled persistent representation.
#[must_use]
pub const fn from_persisted_bytes(bytes: [u8; 32]) -> Self {
    Self(bytes)
}
```

Do not add variable-length parsing, OS identity fields, paths, serialization, or a
public raw constructor for any other platform value.

**Step 3: Run the focused platform contract**

Run the focused command above, then:

```powershell
cargo +1.97.0 test -p tokenmaster-platform --locked
```

Expected GREEN: all platform tests pass.

**Step 4: Commit**

```powershell
git add crates/platform/src/lib.rs crates/platform/tests/physical_identity_contract.rs
git commit -m "feat(platform): restore persisted file identity"
```

## Task 2: Exact restart-safe replay reads

**Files:**

- Modify: `crates/store/src/usage/read.rs`
- Test: `crates/store/tests/replay_archive_contract.rs`

**Step 1: Write failing staging-generation tests**

Cover these contracts:

- the exact `(revision_id, source_key)` returns the staging `GenerationSnapshot`;
- a current-only source, wrong source, discarded revision, promoted revision, or wrong
  revision returns `StoreErrorCode::StaleRevision` rather than falling back to current;
- stored enum/integer/digest validation is reused from `RawGeneration::validate`;
- Debug output contains no private sentinel.

Target signature:

```rust
pub fn replay_generation_snapshot(
    &self,
    revision_id: ReplayRevisionId,
    source_key: SourceKey,
) -> Result<GenerationSnapshot, StoreError>
```

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract replay_generation_snapshot --locked
```

Expected RED: method missing.

**Step 2: Implement one ownership-constrained query**

Join `usage_replay_revision`, `usage_replay_source`, and `usage_generation`; require
`revision.status = 'staging'`, `sealed = 0`, matching revision/source/generation, and
`generation.status = 'staging'`. Read the same columns as `generation_snapshot`, then
call the existing `RawGeneration::validate(source_key)`. Map no row to
`StaleRevision`.

**Step 3: Write failing single-chunk tests**

Cover exact hit, exact absence, wrong generation absence, zero/oversized length
tampering fail-closed, digest-length tampering fail-closed, and redacted Debug.

Target signature:

```rust
pub fn source_chunk(
    &self,
    source_key: SourceKey,
    generation: u64,
    chunk_index: u64,
) -> Result<Option<StoredSourceChunk>, StoreError>
```

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract source_chunk --locked
```

Expected RED: method missing.

**Step 4: Implement a bounded single-row query**

Reject generation/index above SQLite's signed ceiling before querying. Select only
`covered_len, sha256` for the exact primary key. Convert all stored values with checked
helpers, require exactly 32 digest bytes, and construct `StoredSourceChunk::new`.
Return `Ok(None)` only for a genuinely absent key.

**Step 5: Run store gates**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
```

**Step 6: Commit**

```powershell
git add crates/store/src/usage/read.rs crates/store/tests/replay_archive_contract.rs
git commit -m "feat(store): expose bounded replay restart reads"
```

## Task 3: Development-only pipeline driver and baseline proof

**Files:**

- Modify: `crates/codex/Cargo.toml`
- Create: `crates/codex/tests/support/pipeline.rs`
- Create: `crates/codex/tests/pipeline_contract.rs`

**Step 1: Add only the development dependency**

```toml
[dev-dependencies]
tempfile.workspace = true
tokenmaster-accounting = { path = "../accounting" }
tokenmaster-store = { path = "../store" }
```

Confirm `cargo tree -p tokenmaster-codex --edges normal` does not contain store or
accounting.

**Step 2: Write the failing baseline integration test**

Use a temporary direct Codex root with real JSONL lines. Discover with
`CodexProvider`, stream enumeration, register zero checkpoints in pass one, call
`begin_replay_revision_all_sources`, and rebuild in pass two. The fixture must prove:

- staging is invisible before promotion;
- replay-equal parent/child cumulative evidence contributes once;
- a divergent suffix contributes once;
- canonical page event IDs/count/explicit total tokens match an oracle derived from
  canonicalized fixture drafts before archive reads;
- replay quality has exact eligible/replay/pending/conflict counts;
- store close/reopen preserves the result;
- Debug output never contains the temporary path or privacy sentinel.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract baseline --locked
```

Expected RED: support driver and store dependency are absent.

**Step 3: Implement controlled conversion helpers**

The test support module may convert only bounded typed values:

- Codex `SourceKind` to store `SourceKind` by exhaustive match;
- reader checkpoints to `StoredCheckpoint` via serde JSON for
  `ParserResumeState` and explicit verification mapping;
- stored checkpoints back through `PhysicalFileIdentity::from_persisted_bytes`,
  `LogicalFileIdentity::from_bytes`, `BoundaryAnchor::new`, serde JSON decode, and
  `ReaderCheckpointV1::new`;
- reader chunks to `StoredSourceChunk::new` and stored chunks to
  `SourceChunkDigest::from_persisted_parts`;
- drafts only through `Canonicalizer::canonicalize`.

Every conversion returns a small test error enum; it must not format paths, JSON
contents, SQL, or source bytes.

**Step 4: Implement the bounded driver**

The driver must:

1. require one available discovered profile and complete enumeration;
2. derive `SourceKey` from `logical_file_identity`;
3. register each emitted descriptor immediately in pass one and drop its initial
   reader batch;
4. begin the disk-backed all-source revision only after complete enumeration;
5. in pass two, obtain the exact staging snapshot, repeatedly read/canonicalize/apply
   at most `MAX_BATCH_EVENTS`, then apply relations in emitted order with exact epoch;
6. verify a complete prefix using `source_chunk` one chunk at a time and apply an
   otherwise unchanged full-prefix checkpoint;
7. run bounded continuation until `remaining_work == false`, then exact seal/promote;
8. discard exact-epoch staging on any injected cancellation/failure.

No complete descriptor list, event history, chunk history, raw line, or SQL is retained.

**Step 5: Run baseline and dependency gates**

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract baseline --locked
cargo +1.97.0 tree -p tokenmaster-codex --edges normal
```

**Step 6: Commit**

```powershell
git add crates/codex/Cargo.toml crates/codex/tests/support/pipeline.rs crates/codex/tests/pipeline_contract.rs Cargo.lock
git commit -m "test(codex): prove transactional pipeline baseline"
```

## Task 4: Restart, scale, replacement, and failure matrix

**Files:**

- Modify: `crates/codex/tests/support/pipeline.rs`
- Modify: `crates/codex/tests/pipeline_contract.rs`

**Step 1: Write RED tests for bounded restart and scale**

Add fixtures that prove:

- more than 256 observations cross multiple reader/store batches;
- the store closes after the first batch and resumes only from persisted staging
  checkpoint/chunk reads without duplicate or skipped events;
- more than 256 JSONL files register and promote through the disk-backed manifest;
- maximum observed batch/page size is at most 256 and no test helper keeps a complete
  event or chunk history.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract restart --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract more_than_256_files --locked
```

**Step 2: Implement reopen and bounded paging**

Represent restart state only by the SQLite path plus scalar revision/epoch/source key.
Reopen `UsageStore`, re-read the exact staging snapshot, and lazily fetch expected
chunks. Page visible events with `event_page_before(page.last().map(StoredUsageEvent::cursor), 256)`
and retain only the independent scalar count/total oracle plus event IDs required by
the bounded fixture.

**Step 3: Write RED tests for append, truncate, and replace**

Prove the previous page remains visible during every rebuild, append adds only the
suffix after promotion, truncate removes superseded evidence only after promotion,
atomic replacement is classified by the reader as `IdentityChanged`, and rebuilds
produce exact new totals after reopen.

**Step 4: Implement fixture transitions using only filesystem test setup**

Use complete-line append/truncate boundaries. For replacement, write a sibling file
and use a same-filesystem atomic rename. Probe the old current checkpoint before a new
full rebuild to assert the reader classification; never use classification itself as
permission for destructive archive mutation.

**Step 5: Write RED tests for cancellation and malformed/incomplete input**

Inject enumeration cancellation, reader cancellation, malformed JSON, and incomplete
tail. Assert none can seal/promote/change the current page; exact discard succeeds;
a clean retry then promotes. Partial/cancelled enumeration must fail before begin.

**Step 6: Run all focused P0-E tests**

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --locked
cargo +1.97.0 test -p tokenmaster-accounting --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

**Step 7: Commit**

```powershell
git add crates/codex/tests/support/pipeline.rs crates/codex/tests/pipeline_contract.rs
git commit -m "test(codex): cover transactional pipeline recovery"
```

## Task 5: Traceability and operational truth

**Files:**

- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/RECOVERY_PLAYBOOK.md`
- Modify: `docs/superpowers/specs/2026-07-14-tokenmaster-p0-e-pipeline-proof-design.md`

**Step 1: Update contracts and ADR**

Record the three restart seams, their privacy/bounds, the test-only dependency edge,
and the fact that P0-E proves composition but does not implement a runtime engine.
Do not put the current commit hash in tracked documents.

**Step 2: Update status and handoff**

Mark P0-E complete only if every focused and full gate below passes. Make P1 the exact
next slice and preserve explicit non-claims for watchers, scan finalization, UI,
automation, M0, interactive Windows, package, and release.

**Step 3: Run documentation scans**

```powershell
rg -n "P0-E|pipeline|restart|from_persisted_bytes|replay_generation_snapshot|source_chunk" spec docs
rg -n "M0 accepted|RELEASED|interactive Windows.*pass" spec docs
```

**Step 4: Commit**

```powershell
git add spec/TRACEABILITY.md spec/DATA_CONTRACT.md spec/SECURITY.md spec/DECISIONS.md docs/CURRENT_STATE.md docs/PROJECT_HISTORY.md docs/HANDOFF.md docs/ROADMAP.md docs/RECOVERY_PLAYBOOK.md docs/superpowers/specs/2026-07-14-tokenmaster-p0-e-pipeline-proof-design.md
git commit -m "docs: record transactional pipeline proof"
```

## Task 6: Full verification and closeout

**Step 1: Focused-first verification**

```powershell
cargo +1.97.0 test -p tokenmaster-platform --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
```

**Step 2: Repository gates**

```powershell
powershell -NoProfile -File scripts/check-clean-root.ps1
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
Remove-Item Env:RUSTFLAGS
cargo +1.97.0 test --workspace --locked
```

Report the pre-existing ignored million-row M0 scale gate exactly as ignored. Do not
run package scripts or claim M0/release/interactive Windows.

**Step 3: Security, privacy, and dependency audit**

```powershell
git diff --check origin/main...HEAD
git diff --name-only origin/main...HEAD
git diff origin/main...HEAD | rg -n "(?i)(api[_-]?key|secret|token\s*=|password|BEGIN [A-Z ]+PRIVATE KEY)"
git ls-files '*.go' '*.js' '*.ts' '*.py'
cargo +1.97.0 tree -p tokenmaster-codex --edges normal
Get-Process cargo,rustc -ErrorAction SilentlyContinue
```

Inspect every match; a literal test privacy sentinel is allowed only inside the test
fixture and must not appear in generated artifacts or Debug output.

**Step 4: Final commit and push**

If formatting creates a mechanical diff, commit it intentionally. Verify clean state,
local/upstream identity, and push the feature branch without force:

```powershell
git status --short --branch
git push -u origin cx/tokenmaster-product-architecture
git rev-parse HEAD
git rev-parse '@{u}'
```

Stop with P0-E complete and P1 next only if every required gate is green. Otherwise
leave the branch truthful and report the exact failing command and smallest next fix.
