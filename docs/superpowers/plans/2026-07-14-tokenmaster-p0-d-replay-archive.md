# TokenMaster P0-D Replay Archive Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task by task. Do not delegate unless the operator explicitly re-enables agents.

**Goal:** Persist replay classification in strict SQLite, migrate v1 history into an immutable fallback snapshot, reconcile bounded late ancestry across restarts, and atomically promote only eligible replay selections.

**Architecture:** Keep canonical accounting authority in `tokenmaster-accounting`. Add a borrowed `ReplayEventFacts` projection for validated restart data, then extend `tokenmaster-store` to schema v2 with an immutable legacy snapshot and an invisible replay revision overlay on staging generations. Reconciliation is keyset-bounded and durable; sealing proves the fixed source manifest; promotion swaps materialized events, source generations, and revision state in one immediate transaction.

**Tech Stack:** Rust 1.97.0, rusqlite with bundled SQLite, strict SQLite tables and deferred foreign keys, existing TokenMaster domain/accounting/store crates, PowerShell quality scripts.

## Global constraints

- Work only on `cx/tokenmaster-product-architecture`; do not touch `main`.
- Preserve the existing six-section UI and all non-store behavior.
- Test-first: every behavior task starts with a focused failing test and records the expected failure.
- Providers never construct fingerprints, replay signatures, canonical events, dispositions, versions, revision IDs, or evidence epochs.
- Never store paths, raw JSON, prompts, responses, reasoning, commands, output, credentials, source contents, or incomplete-tail bytes.
- Public collections cap at 256; replay traversal caps remain depth 32 and fanout 256.
- All store mutations use immediate transactions, compare-and-swap inputs, and fail closed.
- Staging data is never visible through canonical read APIs.
- Preserve unrelated changes and never revert user work.

---

### Task 1: Add the restart-safe accounting fact seam

**Files:**

- Modify: `crates/accounting/tests/replay_classifier_contract.rs`
- Modify: `crates/accounting/src/replay.rs`
- Modify: `crates/accounting/src/lib.rs`

**Step 1: Write the failing contract test**

Add `persisted_replay_facts_match_live_event_classification` and construct equivalent child/parent inputs in two ways:

- `ReplayEventFacts::from_event(&event)` for the live path;
- `ReplayEventFacts::new(provider_id, profile_id, session_id, parent_session_id, session_ordinal, signature, evidence, declared_conflict)` for validated persisted fields.

Change the test helper to pass fact values to `ReplayClassificationInput::new`, and prove both inputs return the same `ReplayClassification` for replay, divergence, weak evidence, missing parent, and conflict cases.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-accounting --test replay_classifier_contract persisted_replay_facts_match_live_event_classification --locked
```

Expected: compile failure because `ReplayEventFacts` does not exist.

**Step 2: Implement the minimum accounting API**

Add a borrowed, `Clone + Copy` `ReplayEventFacts<'a>` with private fields and these public methods:

- `new(provider_id: &'a str, profile_id: &'a str, session_id: &'a str, parent_session_id: Option<&'a str>, session_ordinal: u64, replay_signature: &'a [u8; 32], evidence: ReplayEvidence, declared_conflict: bool) -> Self` for already-validated persisted facts;
- `from_event(&CanonicalUsageEvent) -> Self`;
- read-only getters for every fact.

Change `ParentOrdinal::Present` to contain `ReplayEventFacts<'a>` by value and change `ReplayClassificationInput` to contain child facts by value. Update structural validation and matching classification to use fact getters. Re-export `ReplayEventFacts` from `crates/accounting/src/lib.rs`.

The constructor must not create an event ID, fingerprint, signature, canonical event, or write authority.

**Step 3: Verify the accounting seam**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-accounting --test replay_classifier_contract --locked
cargo +1.97.0 test -p tokenmaster-accounting --locked
```

Expected: all accounting tests pass.

**Step 4: Commit**

Stage only the accounting files and commit:

```text
feat(accounting): support replay facts after restart
```

---

### Task 2: Introduce the exact v2 schema and immutable v1 migration

**Files:**

- Modify: `crates/store/tests/usage_schema_contract.rs`
- Modify: `crates/store/src/usage/schema.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/error.rs`
- Add: `crates/store/src/usage/migration.rs`

**Step 1: Write failing schema and migration tests**

Add focused tests that prove:

- a fresh database has schema version 2, all 14 strict usage tables, exact named indexes, and three immutable legacy triggers;
- schema SQL contains none of the forbidden private/content column names;
- a hand-built exact v1 fixture migrates all `usage_event` rows into `usage_legacy_event`, records one `legacy_unverified` snapshot, preserves the original `usage_event`, and reopens successfully;
- insert, update, and delete against `usage_legacy_event` fail after migration;
- malformed v1 schema and a newer version roll back without creating any v2 object.

Create the v1 fixture from a dedicated test constant copied from the current reviewed v1 schema, not by calling the v2 initializer.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
```

Expected: failures because the schema is still v1.

**Step 2: Define v1 and v2 contracts separately**

In `schema.rs`:

- set `USAGE_SCHEMA_VERSION` to 2;
- retain `V1_TABLE_CONTRACTS` and `V1_INDEX_CONTRACTS` for pre-migration validation;
- define exact v2 table, index, and trigger contracts;
- split SQL into `FRESH_V2_SCHEMA`, `V2_REPLAY_SCHEMA`, and `LEGACY_IMMUTABILITY_TRIGGERS` so migration can copy before triggers exist.

Use strict `CHECK` constraints for enum text, booleans, digest lengths, non-negative counters, version equality shape, and state combinations. Use deferred foreign keys wherever a generation/replay overlay/selection is swapped in one transaction.

**Step 3: Implement transactional migration**

Move migration logic to `migration.rs` and make `migrate_schema`:

1. reject `user_version > 2` before mutation;
2. for version 0 with no usage objects, create fresh v2;
3. for version 1, validate exact v1, create replay and legacy tables, insert snapshot metadata, copy v1 canonical rows, verify copied counts and `foreign_key_check`, create immutability triggers, set version 2, validate exact v2, commit;
4. for version 2, validate only and never silently repair.

Map migration constraint failures to stable path-free store codes. Add only error variants needed by the design: `StaleRevision`, `AccountingVersionMismatch`, `IncompleteManifest`, `UnsealedRevision`, `PendingContinuation`, and `ArchiveModeMismatch`.

**Step 4: Verify schema and rollback**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

Expected: schema/migration contracts and all prior store tests pass.

**Step 5: Commit**

Commit:

```text
feat(store): migrate usage archive to schema v2
```

---

### Task 3: Expose archive state, replay identity, and bounded quality reads

**Files:**

- Add: `crates/store/tests/replay_archive_contract.rs`
- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/read.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/lib.rs`

**Step 1: Write failing read-state tests**

Add tests for:

- fresh v2 -> `ArchiveMode::Empty`, no active revision, no rebuild activity;
- migrated v1 -> `ArchiveMode::LegacyUnverified` and canonical pages served from the immutable snapshot;
- compatible promoted fixture -> `ReplayVerified`;
- different compiled version tuple -> `ReplayVersionStale` but still readable;
- one staging revision sets `rebuild_staging=true` without changing page results;
- `replay_quality(revision_id)` returns eligible/replay/pending/conflict counts and rejects a nonexistent revision;
- page size and quality values remain bounded and validated after reopen.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract archive_state --locked
```

Expected: compile failure because archive read types do not exist.

**Step 2: Implement typed read models**

Add:

- `AccountingVersions` with a compiled-only constructor inside the store and read-only getters;
- `ReplayRevisionId` and `ReplayEpoch` checked non-negative wrappers;
- `ArchiveMode::{Empty, LegacyUnverified, ReplayVerified, ReplayVersionStale}`;
- `ArchiveState { mode, active_revision, rebuild_staging }`;
- `ReplayQualityCounts` with four non-negative counters.

Expose `UsageStore::archive_state()` and `UsageStore::replay_quality(revision_id)`. Change `event_page_before` to choose exactly one visible source: current compatible/stale materialization, immutable legacy snapshot, or empty. It must never inspect a staging selection.

**Step 3: Verify reads**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

Expected: all archive-state and legacy read tests pass.

**Step 4: Commit**

Commit:

```text
feat(store): expose replay archive state
```

---

### Task 4: Begin a fixed replay revision and stage source generations

**Files:**

- Modify: `crates/store/tests/replay_archive_contract.rs`
- Modify: `crates/store/src/usage/types.rs`
- Add: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/lib.rs`

**Step 1: Write failing lifecycle tests**

Prove:

- an empty manifest and a manifest above 256 are rejected with the right stable code/limit;
- duplicate source keys are rejected before any row is created;
- callers cannot choose accounting versions, revision IDs, or epochs;
- begin creates exactly one staging revision with the compiled version tuple and fixed source count;
- every manifest source gets one generation greater than current with status `staging`;
- a second staging revision is rejected and canonical pages/counts remain unchanged;
- any manifest/source-generation fault rolls the entire begin transaction back.

Run the focused lifecycle test and observe failure before implementation.

**Step 2: Add bounded manifest types**

Add `ReplayManifest::new(Box<[SourceKey]>)` with sort/duplicate validation and cap 256. Its `Debug` output reports only count. Add `ReplayRevisionSnapshot` containing revision ID, epoch, status, versions, source count, sealed state, and promoted state; redact key material.

**Step 3: Implement begin and staging generation creation**

Implement `UsageStore::begin_replay_revision(&ReplayManifest)` in `replay.rs` using one immediate transaction. Read existing registered sources, assign the next checked revision ID, insert manifest rows, and insert a new staging generation for each source from its current checkpoint identity with zero offsets. Do not update `usage_source.current_generation`.

**Step 4: Verify lifecycle**

Run the focused contract, then all store tests. Commit:

```text
feat(store): stage fixed replay revisions
```

---

### Task 5: Persist replay observations and deterministic eligible selections

**Files:**

- Modify: `crates/store/tests/replay_archive_contract.rs`
- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/src/usage/write.rs`
- Modify: `crates/store/src/lib.rs`

**Step 1: Write failing append/classification tests**

Create a bounded `ReplayAppendBatch` contract and tests for:

- root -> eligible;
- matching strong signature -> replay;
- first strong mismatch -> eligible/diverged and later descendants inherit divergence;
- weak evidence -> pending;
- missing parent while open -> pending;
- declared conflict, self-parent, cycle, or different explicit parent -> conflict;
- duplicate observation is idempotent only when every stable fact matches;
- provider/profile/session/source/offset/fingerprint mismatch rolls back observations, overlays, selections, checkpoints, chunks, session state, work, and epoch;
- selection chooses the smallest `(profile_id, file_key, generation, source_offset)` eligible observation per fingerprint;
- staging append never changes canonical page results.

Run the narrowest test first and confirm it fails because replay append does not exist.

**Step 2: Define replay append input**

Reuse `AppendBatch` checkpoint/chunk/CAS fields rather than duplicate their validators. Add a replay wrapper containing expected revision ID, expected replay epoch, the existing append payload, and bounded canonical events. The store derives overlay versions, signatures, evidence, relation facts, and dispositions from canonical events; none are caller-provided.

**Step 3: Implement one atomic append path**

Refactor only the transaction-local reusable pieces of `write.rs`; preserve the existing P0-B append API and tests. In one immediate transaction:

1. validate staging revision/source/generation/checkpoint/epoch CAS;
2. insert/verify observations;
3. upsert deterministic session relations;
4. construct `ReplayEventFacts` from canonical events or validated persisted parent rows;
5. classify and write one overlay per observation;
6. refresh affected eligible selections;
7. coalesce continuation work where ancestry is unresolved;
8. update chunk/checkpoint/source metadata and increment the epoch;
9. validate foreign keys and commit.

**Step 4: Verify append behavior**

Run replay append tests, existing ingest tests, and all store/accounting tests. Commit:

```text
feat(store): classify staged replay observations
```

---

### Task 6: Reconcile late ancestry with durable bounded continuation

**Files:**

- Modify: `crates/store/tests/replay_archive_contract.rs`
- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/src/lib.rs`

**Step 1: Write failing late-relation tests**

Prove:

- a late explicit parent changes a prior root/pending session and invalidates its old selection in the same transaction;
- nested descendants are reconsidered in deterministic session/ordinal keyset order;
- at most 256 direct descendants and 32 unresolved ancestry links are processed per transaction;
- fanout/depth exhaustion stays pending and leaves one coalesced durable work row;
- processing continuation after close/reopen resumes from its cursor without skipping or duplicating descendants;
- stale work epoch writes nothing;
- an explicit parent disagreement remains permanently conflict.

Run focused tests and record the expected missing-API failure.

**Step 2: Add bounded relation and work APIs**

Add a `ReplayRelation` input derived from validated `SessionRelationDraft` fields: provider/profile/session, optional parent, conflict flag, source key, source offset, expected revision, and expected epoch. Add `UsageStore::apply_replay_relation(&ReplayRelation) -> ReplayEpoch` and `UsageStore::continue_replay(revision_id: ReplayRevisionId, expected_epoch: ReplayEpoch) -> ReplayContinuationResult`. The result reports only processed count, remaining work flag, and new epoch.

**Step 3: Implement keyset reconciliation**

Persist first relation identity deterministically; never retain a path. Coalesce work on `(revision, kind, provider, profile, session)`. Each immediate transaction processes one bounded page, recalculates session state from the earliest affected ordinal, updates overlays/selections, advances or deletes the cursor, increments epoch, and commits. Confirmed cycles/conflicts fail closed; bounds produce pending continuation.

**Step 4: Verify restart and bounds**

Run late-relation tests twice, including reopen, then all store tests. Commit:

```text
feat(store): persist bounded replay reconciliation
```

---

### Task 7: Seal complete evidence and atomically promote eligible state

**Files:**

- Modify: `crates/store/tests/replay_archive_contract.rs`
- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/src/usage/read.rs`
- Modify: `crates/store/src/lib.rs`

**Step 1: Write failing seal blockers**

Prove each condition independently blocks seal or promotion without mutation:

- omitted manifest source;
- staging generation missing;
- verification not `full_prefix`;
- incomplete tail or oversized discard;
- scan/committed/observed length mismatch;
- invalid checkpoint/chunk coverage;
- unfinished continuation;
- missing replay overlay;
- pending observations at promotion;
- stale revision or epoch;
- accounting version mismatch;
- foreign-key failure.

Prove missing parent is `MissingOpen` before seal reconciliation and becomes `MissingComplete` only after the complete manifest is proven.

**Step 2: Implement seal**

Implement `UsageStore::seal_replay_revision(revision_id, expected_epoch)`:

- reject if durable work remains;
- verify every fixed manifest source and full-prefix checkpoint;
- mark sessions complete only inside this transaction;
- reclassify missing parent ordinals as complete divergence;
- refresh overlays/selections and quality counts;
- validate exact overlay coverage, version tuple, and foreign keys;
- record sealed state and increment epoch.

Conflicts may seal for reporting. Pending evidence may seal for reporting, but promotion remains blocked.

**Step 3: Write and pass promotion rollback tests**

Add a test-only promotion fault seam inside the store crate and prove faults after materialization, generation status changes, and revision status changes all preserve the prior canonical page, current generation pointers, legacy snapshot, and revision state.

Implement `UsageStore::promote_replay_revision(revision_id, expected_epoch)` in one immediate transaction. Revalidate seal, versions, manifest, work, pending count, selections, and foreign keys; rebuild `usage_event` from eligible selections; atomically swap source generations; replace the current revision; retain the immutable legacy snapshot; commit.

**Step 4: Prove success is atomic**

Prove after reopen that:

- archive state is `ReplayVerified`;
- canonical pages contain only eligible selected rows;
- replay/pending/conflict rows are excluded;
- all source current pointers refer to promoted generations;
- no staging revision remains;
- legacy rows remain byte-for-byte unchanged;
- a repeated or stale promotion fails without mutation.

Run focused promotion tests and all store tests. Commit:

```text
feat(store): seal and promote replay archive
```

---

### Task 8: Close traceability, security, and full quality gates

**Files:**

- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DATA_CONTRACT.md` only if implementation details require a contract clarification
- Modify: `spec/SECURITY.md` only if a new enforced boundary is not already documented
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: relevant `docs/operations/*.md` if a new recovery procedure is required

**Step 1: Update durable project truth**

Record:

- P0-D implemented behavior and exact non-goals;
- v1-to-v2 immutable fallback migration;
- archive states and accounting version mismatch behavior;
- bounded continuation, seal, promotion, and rollback gates;
- exact commands run and ignored/unverified gates;
- P0-E as the next critical path: Codex discovery/enumerator/reader/accounting/store orchestration with truncate/replace recovery.

Do not write the current commit hash into tracked documents.

**Step 2: Run focused and full verification from a fresh command**

```powershell
cargo +1.97.0 test -p tokenmaster-accounting --test replay_classifier_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
Remove-Item Env:RUSTFLAGS
cargo +1.97.0 test --workspace --locked
git diff --check
```

Expected: every command passes. The existing one-million-row test may remain ignored because it is an explicit M0 scale gate; report it rather than claiming it ran.

**Step 3: Run privacy and repository checks**

Search schema, debug implementations, changed files, and Git history scope for forbidden raw-content/path fields and secret-like material. Confirm `git status --short`, `git diff --stat`, and `git diff --name-only` contain only intentional P0-D files.

**Step 4: Independent read-only review**

Because agents are deferred, perform a separate root review pass against the design sections and traceability rows. If the operator later re-enables agents before this step, route only this bounded read-only review to Sol High and verify its findings locally.

**Step 5: Commit and push**

Commit documentation and any review fixes with:

```text
docs: record replay archive milestone
```

Push `cx/tokenmaster-product-architecture` only after the complete gate passes. Do not claim M0 accepted, package, release, or interactive Windows verification.

## Stop conditions

Stop implementation and report exact evidence if:

- the baseline or a previously green focused contract becomes red for an unexplained reason;
- exact v1 migration cannot be proven non-destructive;
- persisted facts require exposing canonical constructors or provider authority;
- any staging row becomes visible through canonical reads;
- reconciliation cannot preserve the 32/256 bounds across restart;
- promotion cannot be made one rollback-safe transaction;
- a required change conflicts with an earlier source-of-truth contract.

Do not weaken a contract to make a test pass. Update the hypothesis, preserve the last green commit, and record the smallest unresolved blocker.
