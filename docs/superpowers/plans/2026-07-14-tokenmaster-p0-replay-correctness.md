# TokenMaster P0 Replay Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make canonical TokenMaster totals fail-safe against Codex fork/subagent transcript-prefix replay, including timestamp rewrites and nested ancestry, before analytics or product UI consume the archive.

**Architecture:** The Codex adapter emits provider-neutral usage observations with bounded lineage evidence. The store persists every observation, classifies its contribution through an explicit session-prefix state machine, and admits only `eligible` observations to the canonical event table. Unknown parent tails and conflicts remain visible as non-canonical states until later complete-scan reconciliation proves a safe result.

**Tech Stack:** Rust 1.97, serde/serde_json, sha2, rusqlite with bundled SQLite, existing TokenMaster domain/Codex/store crates, Cargo tests and Clippy.

**Execution status:** Tasks 1 and 2 are historical and complete. Tasks 3 onward in
this document are permanently superseded and must not be executed. The approved
replacement sequence starts with
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-authority-boundary.md`; the complete
delivery order and resolved ambiguities are in `docs/AUDIT_AND_MASTER_PLAN.md`.

## Global Constraints

- Work only on `cx/tokenmaster-product-architecture` or a successor `cx/` feature branch, never directly on `main`.
- Apply test-driven development to every behavior task: add one focused failing test, run it and inspect the expected failure, implement the smallest behavior, then run the focused test and its crate suite.
- Keep the existing event fingerprint unchanged. Replay identity is a separate versioned signature because timestamps and source identities may legitimately differ across copied prefixes.
- Store every bounded observation. Replay classification controls canonical contribution; it must not erase source evidence.
- Only explicit Codex ancestry fields establish a parent. Do not infer a fork from timestamp proximity, filename similarity, equal cost, or equal token counts alone.
- Treat an unproved parent tail as `pending`, not as a new canonical event. Treat cycles, conflicting parent declarations, and ambiguous weak-prefix matches as `conflict`.
- Bound append batches at the existing 256 events. Bound ancestry traversal at 32 sessions and re-evaluation fanout at 256 direct children per transaction.
- Keep prompt, response, reasoning text, tool arguments, command output, paths, raw JSON, and credentials out of domain objects, SQLite, diagnostics, and test failure messages.
- Keep domain and store contracts provider-neutral. Codex JSON fields stay inside `tokenmaster-codex`; no Codex-specific path or wire type may leak into domain/store APIs.
- Codex local files remain the only implemented source. The later engine plan introduces `SourceCatalog`, `SourceReader`, and `ProviderDecoder` ports over these neutral contracts; this slice does not add unused runtime polymorphism or executable plugins.
- Do not add third-party dependencies. `sha2`, `serde`, `serde_json`, and `rusqlite` already cover the slice; one existing workspace crate may be added as a test-only dependency for the final pipeline proof.
- After every task, run `cargo fmt --all -- --check`, the focused crate test, and `git diff --check` before committing the named files only.
- After the final task, update `spec/TRACEABILITY.md`, `docs/CURRENT_STATE.md`, `docs/PROJECT_HISTORY.md`, and the affected data/security/decision documents with exact commands and remaining unverified boundaries.

## Contract and file map

| Concern | Existing boundary | Planned change |
| --- | --- | --- |
| Canonical usage object | `crates/domain/src/usage.rs` | Add bounded replay signature, evidence, session ordinal, and optional parent to every emitted event. |
| Public domain exports | `crates/domain/src/lib.rs` | Export the new provider-neutral value types. |
| Codex wire decoding | `crates/codex/src/parser/wire.rs` | Decode `payload.forked_from_id` and structured `payload.source.subagent.thread_spawn.parent_thread_id` without retaining unrelated content. |
| Parser metadata/state | `crates/codex/src/parser/effects.rs`, `state.rs` | Retain one bounded parent identity and the next usage ordinal; bump the resume schema. |
| Replay signature | new `crates/codex/src/parser/replay.rs` | Hash a version tag, normalized model, delta usage, and cumulative snapshot when present. |
| Event emission | `crates/codex/src/parser/mod.rs` | Attach lineage after model and usage normalization; advance ordinal only after an event is emitted. |
| SQLite contract | `crates/store/src/usage/schema.rs` | Add strict replay-session and replay-observation tables and bounded indexes; migrate v1 fail-closed. |
| Append/canonical selection | `crates/store/src/usage/write.rs` | Persist replay metadata, classify sessions, re-evaluate bounded descendants, and filter canonical selection. |
| Store value boundary | `crates/store/src/usage/types.rs` | Add only typed status/count read models needed by tests and later quality reporting. |
| Public evidence | `crates/codex/tests`, `crates/store/tests` | Synthetic direct/nested fork fixtures with rewritten timestamps, legitimate equal values, pending tails, conflicts, and migration coverage. |

---

## Task 1: Normalize authoritative contracts and source-adapter seam

**Files:**

- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/superpowers/specs/2026-07-14-tokenmaster-product-architecture-design.md`

- [x] Add a P0 accounting requirement that defines observation, canonical contribution, explicit ancestry, divergence, `pending`, and `conflict` states.
- [x] State that canonical totals include only `eligible` observations; pending/conflict counts remain available to future quality snapshots and never become zero silently.
- [x] Record the source-adapter seam: bounded source catalog, sequential reader, provider decoder, and optional quota adapter; only the local Codex adapter is implemented in 1.0.
- [x] State the security boundary: statically linked allowlisted adapters, no arbitrary filesystem/network/command surface, opaque size-bounded checkpoints, engine-owned cancellation/backpressure.
- [x] Record the deliberate P0 boundary: proving a child outgrew a still-open parent requires complete-scan/session-finalization evidence from the following staging/runtime-engine slice.

**Verification:**

- [x] Build a PowerShell marker regex from split string fragments and scan the changed contracts for unfinished drafting markers; no matches were found.
- [x] Run `pwsh -NoLogo -NoProfile -File scripts/audit-clean-root.ps1`; returned `TM-CLEAN-PASS`.
- [x] Run `git diff --check`; exited with code 0.

**Commit:**

- [x] Commit the reviewed contract normalization as `docs: define replay-safe accounting contract`.

## Task 2: Add provider-neutral replay value types

**Files:**

- Modify: `crates/domain/src/usage.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/domain/tests/usage_contract.rs`

- [x] Add failing domain tests that require deterministic serialization, redacted debug output, exact 32-byte signatures, parent/session bounds, and zero-based ordinals.
- [x] Run `cargo test -p tokenmaster-domain --test usage_contract replay_lineage_is_bounded_serializable_and_private`; observed the expected unresolved-import failure because the replay types did not exist.
- [x] Add these source-neutral types:

```rust
#[derive(Clone, Copy, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ReplaySignature([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayEvidence {
    StrongCumulative,
    WeakUsageOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UsageLineage {
    parent_session_id: Option<UsageSessionId>,
    session_ordinal: u64,
    signature: ReplaySignature,
    evidence: ReplayEvidence,
    declared_conflict: bool,
}
```

- [x] Implement `ReplaySignature::new`, `as_bytes`, and private `Debug`; implement `UsageLineage::new` plus accessors. Construction does not accept a parent equal to the current event session ID unless `declared_conflict` is true.
- [x] Run the focused test again; one focused test passed.
- [x] Run `cargo test -p tokenmaster-domain`; all domain tests passed.

**Commit:**

- [x] Commit the domain implementation, contract test, and required project-truth updates as `feat(domain): add replay lineage contract`.

## Task 3: Decode explicit Codex ancestry without retaining source payloads

**Files:**

- Modify: `crates/codex/src/parser/wire.rs`
- Modify: `crates/codex/src/parser/value.rs`
- Modify: `crates/codex/src/parser/effects.rs`
- Modify: `crates/codex/src/parser/effects.rs` internal tests
- Modify: `crates/codex/src/parser/wire.rs` internal tests

- [ ] Add failing internal parser tests for both public Codex ancestry shapes:

```json
{"type":"session_meta","payload":{"id":"child","forked_from_id":"parent","source":"cli"}}
{"type":"session_meta","payload":{"id":"child","source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent"}}}}}
```

- [ ] Add internal adversarial cases for oversized/control-character parent IDs, a scalar/object type mismatch, and conflicting ancestry fields. Self-parent validation belongs to parser state in Task 4. Tests must assert bounded diagnostic codes and must not echo the rejected value.
- [ ] Run `cargo test -p tokenmaster-codex parser::effects::tests::explicit_parent_metadata_is_bounded_and_shape_compatible`; expect failure because parent metadata is not decoded.
- [ ] Replace the current source-only scalar field with a custom bounded visitor that extracts only:
  - a bounded display alias when `source` is a scalar string;
  - `subagent.thread_spawn.parent_thread_id` when `source` is an object;
  - no other object members or raw JSON.
- [ ] Add bounded `forked_from_id` decoding. Resolve a parent only when one valid field exists or both valid fields agree. Conflicting valid fields produce a stable `InvalidMetadata` diagnostic and no accepted parent.
- [ ] Extend `MetadataUpdate` with `parent_session_id: Option<UsageSessionId>` but do not retain it in `ParserState` until Task 4; keep ancestry resolution pure and unit-testable.
- [ ] Run the focused internal tests and `cargo test -p tokenmaster-codex`; expect all Codex tests pass.

**Commit:**

- [ ] Commit the wire/value/effects and test files as `feat(codex): decode bounded session ancestry`.

## Task 4: Version and persist parser lineage state

**Files:**

- Modify: `crates/codex/src/parser/state.rs`
- Modify: `crates/codex/src/parser/effects.rs`
- Modify: `crates/codex/src/parser/mod.rs`
- Modify: `crates/codex/tests/parser_state_contract.rs`
- Modify: `crates/codex/tests/reader_contract.rs`
- Modify: `crates/codex/tests/parser_adversarial_contract.rs`

- [ ] Add failing resume tests requiring `parent_session_id`, `next_usage_ordinal`, and `lineage_conflict` to survive a serialize/restore cycle exactly.
- [ ] Add invalid-state tests for a self-parent, an ordinal above `i64::MAX`, a parent change after emission, and unknown resume fields.
- [ ] Run `cargo test -p tokenmaster-codex --test parser_state_contract resume_preserves_bounded_lineage_state`; expect failure because resume schema v1 lacks the fields.
- [ ] Rename the public resume type to `ParserResumeStateV2`, set `PARSER_SCHEMA_VERSION` to `2`, and update reader checkpoint types/exports. Do not silently accept v1 parser state; a stored v1 checkpoint must trigger the existing parser-schema reparse path.
- [ ] Add to `ParserState`:

```rust
parent_session_id: Option<UsageSessionId>,
next_usage_ordinal: u64,
lineage_conflict: bool,
```

- [ ] Apply parent metadata before usage emission. The first accepted parent is retained. Self-parenting or a later different parent sets `lineage_conflict`; it never replaces the retained parent silently.
- [ ] Increment `next_usage_ordinal` only after a positive usage event is emitted. Metadata-only, tool-only, zero-usage, malformed, and rejected lines do not consume an ordinal.
- [ ] Include the new state in `ParserState::MAX_RETAINED_TEXT_BYTES` accounting and keep the serialized resume payload under `MAX_RESUME_BYTES`.
- [ ] Run the focused tests, `cargo test -p tokenmaster-codex --test reader_contract`, and `cargo test -p tokenmaster-codex`; expect pass.

**Commit:**

- [ ] Commit parser state/reader compatibility changes as `feat(codex): persist replay lineage state`.

## Task 5: Generate strong and weak replay signatures

**Files:**

- Create: `crates/codex/src/parser/replay.rs`
- Modify: `crates/codex/src/parser/mod.rs`
- Modify: `crates/domain/src/usage.rs`
- Modify: `crates/domain/tests/usage_contract.rs`
- Modify: `crates/codex/tests/parser_usage_contract.rs`
- Create: `crates/codex/tests/parser_replay_contract.rs`
- Modify: `crates/store/tests/usage_ingest_contract.rs`
- Modify: `crates/store/src/usage/write.rs` unit-test fixtures

- [ ] Add failing tests proving that:
  - rewritten timestamps and different source IDs produce the same replay signature for the same normalized model/delta/cumulative snapshot;
  - different cumulative snapshots produce different strong signatures even when the emitted delta is equal;
  - non-cumulative usage produces `WeakUsageOnly`;
  - event fingerprints still differ when timestamps differ;
  - ordinals advance exactly once per emitted event.
- [ ] Run `cargo test -p tokenmaster-codex --test parser_replay_contract rewritten_timestamps_keep_strong_replay_identity`; expect failure because no lineage is emitted.
- [ ] Preserve `LineEffect::baseline_update` long enough to generate a signature after model resolution. Hash an unambiguous fixed-width stream:

```text
tokenmaster.replay.v1\0
model-byte-length:u64-be | model-bytes
delta token fields: one availability byte plus u64-be value each
cumulative-present byte
cumulative token fields when present
```

- [ ] Use `StrongCumulative` only when `total_token_usage` is present. Otherwise use `WeakUsageOnly`. Do not hash timestamp, profile, session, source, offset, display metadata, or activity.
- [ ] Add `lineage: UsageLineage` to `CanonicalUsageEventParts` and a `CanonicalUsageEvent::lineage()` accessor without changing fingerprint or event-ID behavior. Update all workspace event constructors with deterministic fixture lineage.
- [ ] Attach `UsageLineage` to parser output using the retained parent, current ordinal, signature, evidence, and `lineage_conflict`. If conflict is set, preserve bounded parent/signature evidence; the store detects the session-level conflict in Task 8.
- [ ] Run the focused replay contract, `cargo test -p tokenmaster-codex --test parser_usage_contract`, and `cargo test -p tokenmaster-codex`; expect pass.
- [ ] Run `cargo test --workspace --all-targets --no-run`; expect every workspace target compiles with the expanded canonical-event contract.

**Commit:**

- [ ] Commit parser signature generation and tests as `feat(codex): emit replay signatures`.

## Task 6: Add the pure bounded prefix classifier

**Files:**

- Create: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Create: `crates/store/tests/usage_replay_classifier_contract.rs`

- [ ] Add table-driven failing tests for root, strong match, strong mismatch/divergence, missing parent ordinal, weak pre-divergence match, post-divergence weak event, cycle, conflicting parent, ancestry depth 33, and fanout 257.
- [ ] Run `cargo test -p tokenmaster-store --test usage_replay_classifier_contract strong_prefix_match_is_replay_and_mismatch_diverges`; expect a compile failure because the classifier does not exist.
- [ ] Implement a pure state transition with these stable states:

```rust
enum ReplayDisposition { Eligible, Replay, Pending, Conflict }
enum SessionReplayState { Root, Matching, Diverged, Pending, Conflict }
```

- [ ] Enforce the transition table:

| Session state / evidence | Parent ordinal | Result |
| --- | --- | --- |
| root/no parent | not applicable | `eligible` |
| matching + strong equal | present | `replay`, continue matching |
| matching + strong different | present | `eligible`, lock divergence at this ordinal |
| matching + weak | present or absent | `pending` unless the session already diverged |
| matching + parent ordinal absent and parent open | absent | `pending` |
| diverged | any | `eligible` |
| cycle/conflicting parent/bound exceeded | any | `conflict` |

- [ ] Make divergence irreversible for a fixed session-parent relation. A later apparent equality cannot return a diverged session to matching.
- [ ] Run the focused classifier test and its complete test target; expect pass.

**Commit:**

- [ ] Commit the pure store classifier as `feat(store): add bounded replay classifier`.

## Task 7: Migrate SQLite v1 to replay-aware schema v2

**Files:**

- Modify: `crates/store/src/usage/schema.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/tests/usage_schema_contract.rs`

- [ ] Add failing schema tests requiring version 2, eight exact strict tables, exact columns/indexes, foreign keys, status checks, path/privacy exclusions, and a transactional v1 migration.
- [ ] Create a real v1 fixture with canonical events, open it through `UsageStore`, and assert:
  - observations remain intact;
  - old canonical rows are removed because their lineage was never captured;
  - no v1 row is silently treated as replay-safe;
  - a later reparse can attach replay metadata to an existing observation.
- [ ] Run `cargo test -p tokenmaster-store --test usage_schema_contract v1_migration_preserves_observations_and_fails_canonical_closed`; expect failure because schema version is 1.
- [ ] Set `USAGE_SCHEMA_VERSION` to `2` and add strict tables:

```sql
usage_replay_session(
  profile_id, session_id, parent_session_id, state,
  matched_prefix_len, divergence_ordinal, conflict_code,
  PRIMARY KEY(profile_id, session_id)
)

usage_replay_observation(
  file_key, generation, source_offset, fingerprint,
  profile_id, session_id, parent_session_id, session_ordinal,
  replay_signature, evidence, disposition,
  PRIMARY KEY(file_key, generation, source_offset, fingerprint),
  FOREIGN KEY(...) REFERENCES usage_observation(...) ON DELETE CASCADE
)
```

- [ ] Constrain session/relation text by byte length, signatures to exactly 32 bytes, ordinals/counters to non-negative signed-SQL ranges, evidence to `strong_cumulative|weak_usage_only`, dispositions to `eligible|replay|pending|conflict`, and session states to `root|matching|diverged|pending|conflict`.
- [ ] Add indexes on `(profile_id, session_id, session_ordinal)` and `(profile_id, parent_session_id, disposition)`. Validate exact normalized index SQL through the existing schema contract mechanism.
- [ ] In the v1-to-v2 transaction, create new tables/indexes and delete existing `usage_event` rows. Keep `usage_observation`, generation checkpoints, and source identities unchanged. Set `user_version=2` only after schema validation succeeds.
- [ ] Run the focused migration test and `cargo test -p tokenmaster-store --test usage_schema_contract`; expect pass.

**Commit:**

- [ ] Commit schema and migration tests as `feat(store): migrate replay metadata schema`.

## Task 8: Make append and canonical selection replay-aware

**Files:**

- Modify: `crates/store/src/usage/write.rs`
- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/read.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/tests/usage_ingest_contract.rs`
- Create: `crates/store/tests/usage_replay_ingest_contract.rs`

- [ ] Add failing transaction tests for a normal root, child strong-prefix replay, rewritten child timestamps, first mismatch divergence, pending parent tail, duplicate observations, and an injected rollback after replay metadata insertion.
- [ ] Run `cargo test -p tokenmaster-store --test usage_replay_ingest_contract rewritten_child_prefix_never_enters_canonical_totals`; expect failure because canonical selection ignores replay state.
- [ ] Change append order inside the existing immediate transaction:
  1. insert or retain `usage_observation`;
  2. validate/upsert the session-parent relation;
  3. classify and upsert `usage_replay_observation`;
  4. refresh every affected fingerprint;
  5. update chunks/checkpoint;
  6. commit.
- [ ] Change `REFRESH_CANONICAL_SQL` to use an inner join to `usage_replay_observation` with `disposition='eligible'`. If no eligible current observation remains, deleting the canonical row is a valid result; change `refresh_canonical` to accept zero or one inserted row and reject more than one.
- [ ] When the same observation primary key already exists after v1 migration, verify its stable event fields and insert/update only its replay row. Any mismatch fails the whole batch as `InvalidStoredValue`.
- [ ] Expose bounded counts for `eligible`, `replay`, `pending`, and `conflict` through a typed store read model; do not expose paths or arbitrary SQL.
- [ ] Run the focused replay ingest target, existing `usage_ingest_contract`, and `cargo test -p tokenmaster-store`; expect pass.

**Commit:**

- [ ] Commit transactional classification/canonical filtering as `feat(store): filter canonical replay observations`.

## Task 9: Re-evaluate descendants and close conflict paths

**Files:**

- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/src/usage/write.rs`
- Modify: `crates/store/tests/usage_replay_ingest_contract.rs`

- [ ] Add failing tests where a child arrives before its parent, a nested grandchild arrives before both ancestors, a parent is later extended, a session declares two parents, a cycle is formed, and limits are reached.
- [ ] Run `cargo test -p tokenmaster-store --test usage_replay_ingest_contract late_parent_reclassifies_bounded_nested_descendants`; expect failure because pending descendants are not revisited.
- [ ] After inserting a parent ordinal, find at most 256 direct pending children for that `(profile_id, parent_session_id, session_ordinal)` and re-run classification. Traverse at most 32 ancestry levels in one transaction using an explicit fixed-capacity queue.
- [ ] For every changed disposition, refresh that observation's fingerprint. Re-evaluation must use raw replay-observation sequences, including rows already classified as `replay`, so nested children compare against the parent's complete observed stream rather than only canonical events.
- [ ] If a session-parent relation changes, self-references, cycles, exceeds depth, or exceeds bounded fanout, mark the entire affected session `conflict`, mark all its replay observations `conflict`, and remove their canonical contributions in the same transaction.
- [ ] Keep a child ordinal beyond the currently observed open parent as `pending`. This task does not infer end-of-parent from timing or current file length.
- [ ] Run the focused late-parent/nested/conflict tests and `cargo test -p tokenmaster-store`; expect pass.

**Commit:**

- [ ] Commit descendant re-evaluation as `feat(store): reconcile bounded replay ancestry`.

## Task 10: Prove the P0 slice end to end and record truth

**Files:**

- Modify: `crates/store/Cargo.toml`
- Create: `crates/store/tests/fixtures/codex-replay/README.md`
- Create: `crates/store/tests/replay_pipeline_contract.rs`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`

- [ ] Build synthetic inline JSONL fixtures only; do not copy user transcripts or upstream fixture assets. Cover root, direct child with rewritten timestamps, nested child, legitimate repeated delta after divergence, parent-late ordering, conflicting parents, truncation/reparse, and restart from parser resume state.
- [ ] Add `tokenmaster-codex` as a path-only dev dependency of `tokenmaster-store` so the public parser-to-store pipeline can be tested without changing production dependencies.
- [ ] Add one pipeline test that parses each synthetic source through `tokenmaster-codex`, appends events through the public `tokenmaster-store` API, and verifies exact canonical totals plus replay/pending/conflict counts.
- [ ] Add a bounded-memory assertion using existing batch limits: the test feeds more than one append batch and verifies no API requires a retained whole-history vector. Runtime allocation/working-set evidence remains part of the following engine gate.
- [ ] Run `cargo test -p tokenmaster-store --test replay_pipeline_contract`; expect pass.
- [ ] Run all focused and broad verification:

```powershell
cargo fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
Remove-Item Env:RUSTFLAGS
cargo test --workspace --all-targets --all-features --locked
pwsh -NoLogo -NoProfile -File scripts/audit-clean-root.ps1
git diff --check
```

- [ ] Expect format, Clippy, all workspace tests, clean-root audit (`TM-CLEAN-PASS`), and diff check to pass. Record exact commands and results in project history and handoff.
- [ ] Update traceability with concrete implementation/test rows. Mark complete-scan finalization of an unproved child tail and working-set soak evidence as not implemented until the staging/runtime-engine slice.
- [ ] Run a sensitive-content scan over tracked files for private paths, credentials, prompt/response/reasoning markers, and raw transcript content; expect no secret or personal data findings.

**Commit:**

- [ ] Commit the end-to-end proof and documentation as `test: prove replay-safe canonical accounting`.

## Acceptance gate

P0 replay correctness is accepted only when all statements below are supported by committed tests on one clean feature-branch commit:

- Direct and nested copied prefixes with rewritten timestamps contribute exactly once.
- A legitimate equal-valued event after proven divergence remains canonical.
- Missing parent evidence, weak pre-divergence evidence, conflicting ancestry, cycles, and limit exhaustion never enter canonical totals.
- Late parent observations reclassify bounded pending descendants atomically.
- v1 migration preserves observations but does not retain unproved canonical rows.
- Event fingerprints and existing source/checkpoint CAS behavior remain stable.
- No parser/store path retains raw transcripts or an unbounded history vector.
- Domain/store APIs are provider-neutral and keep future source adapters outside analytics and UI.
- All focused tests, locked workspace tests, strict Clippy, formatting, privacy scan, clean-root audit, and diff check pass.

The child-beyond-open-parent case is intentionally left `pending` until the next
staging/runtime-engine plan supplies authoritative complete-scan session finalization.
That boundary is a fail-closed correctness result, not a release-complete accounting
claim.
