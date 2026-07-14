# TokenMaster P0 Authority Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every usage event pass through one provider-neutral TokenMaster canonicalizer before storage, with collision-safe versioned identity and no public construction bypass.

**Architecture:** `tokenmaster-domain` owns bounded `ObservationDraft` inputs. A new `tokenmaster-accounting` crate validates drafts and exclusively constructs opaque canonical events, fingerprint v2, replay signature v1, and replay evidence. Codex emits drafts; the reader carries drafts; the store accepts canonicalized events.

**Tech Stack:** Rust 1.97, serde, sha2, existing TokenMaster domain/Codex/store crates, Cargo tests and Clippy.

## Global Constraints

- Work on `cx/tokenmaster-product-architecture`, never `main`.
- Use red/green TDD for every behavior change and inspect the expected failure before production code.
- Do not add third-party dependencies; `serde` and `sha2` are already locked workspace dependencies.
- Provider code cannot construct an event fingerprint, replay signature, event ID, replay disposition, or canonical event.
- Fingerprint v2 includes provider/profile/session/ordinal/model/delta and excludes timestamp/source/display/activity.
- Replay signature v1 includes model/delta/optional cumulative; evidence is strong only when cumulative total is available.
- Reject self-parenting without an explicit conflict marker, all-zero/unavailable usage, inconsistent cumulative counts, and ordinal/source offsets above SQLite signed range.
- Keep every string and collection bounded and keep private source content/path data outside all new types.
- Update traceability, current state, history, handoff, roadmap, architecture, and changelog with exact verification truth.

---

### Task 1: Add provider-neutral observation drafts

**Files:**
- Modify: `crates/domain/src/usage.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/domain/tests/usage_contract.rs`

**Interfaces:**
- Produces: `UsageProviderId`, `ObservationVerification`, `ObservationDraftParts`, and `ObservationDraft`.
- `ObservationDraft::new(parts: ObservationDraftParts) -> Result<ObservationDraft, UsageError>` validates only bounded structural relations; accounting owns identity derivation.
- Accessors expose typed facts and never expose a mutable parts object.

- [x] **Step 1: write failing contract tests** for provider ID alphabet/bounds, source verification serialization, parent/session relation, zero-based ordinal, delta/cumulative access, redacted debug output, and absence of canonical identity fields.
- [x] **Step 2: run RED:** `cargo +1.97.0 test -p tokenmaster-domain --test usage_contract observation_draft_is_bounded_provider_neutral_and_private -- --exact`; unresolved imports were observed.
- [x] **Step 3: implement the minimal bounded draft types** in `usage.rs`, reusing existing text/token/activity value types. `Debug` prints identifiers and numeric metadata but replaces token values and source evidence with `[redacted]`.
- [x] **Step 4: run GREEN:** the exact test and locked domain suite pass.
- [x] **Step 5: run** format and diff checks; both pass.

### Task 2: Add the exclusive accounting canonicalizer

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/accounting/Cargo.toml`
- Create: `crates/accounting/src/lib.rs`
- Create: `crates/accounting/src/hash.rs`
- Create: `crates/accounting/src/event.rs`
- Create: `crates/accounting/tests/canonicalizer_contract.rs`

**Interfaces:**
- Consumes: `tokenmaster_domain::ObservationDraft`.
- Produces: `Canonicalizer::canonicalize(&ObservationDraft) -> Result<CanonicalUsageEvent, CanonicalizationError>`.
- Produces opaque read-only `CanonicalUsageEvent`, `EventFingerprint`, `EventId`, `ReplaySignature`, `ReplayEvidence`, and `UsageLineage`.
- Constants: `CANONICALIZER_VERSION = 1`, `EVENT_FINGERPRINT_VERSION = 2`, `REPLAY_SIGNATURE_VERSION = 1`.

- [x] **Step 1: write failing tests** proving deterministic vectors, provider/session/ordinal separation, timestamp/source-insensitive duplicate identity, replay timestamp/source independence, strong/weak evidence, self-parent rejection, cumulative inconsistency rejection, empty usage rejection, and redacted digest debug output.
- [x] **Step 2: run RED:** the accounting package/target absence was observed.
- [x] **Step 3: create the crate and opaque event API.** Constructors for fingerprints, replay signatures, event IDs, lineage, and canonical events remain private to the crate. Public APIs are accessors plus `Canonicalizer::canonicalize`.
- [x] **Step 4: implement framed hashing.** Each hash starts with its ASCII-NUL domain tag; each text is encoded as `u32` big-endian length plus bytes; each ordinal/count uses fixed big-endian bytes; each `TokenCount` uses one availability byte plus `u64` when available.
- [x] **Step 5: run GREEN:** the accounting contract and locked suite pass.
- [x] **Step 6: compile-fail proof:** downstream construction examples fail to compile and accounting doc tests pass.
- [x] **Step 7: run** format and diff checks; both pass.

### Task 3: Make Codex emit drafts and persist lineage state

**Files:**
- Delete: `crates/codex/src/parser/fingerprint.rs`
- Modify: `crates/codex/src/parser/wire.rs`
- Modify: `crates/codex/src/parser/effects.rs`
- Modify: `crates/codex/src/parser/state.rs`
- Modify: `crates/codex/src/parser/mod.rs`
- Modify: `crates/codex/src/reader/framing.rs`
- Modify: `crates/codex/src/reader/mod.rs`
- Modify: `crates/codex/src/lib.rs`
- Modify: `crates/codex/tests/parser_usage_contract.rs`
- Modify: `crates/codex/tests/parser_state_contract.rs`
- Create: `crates/codex/tests/parser_lineage_contract.rs`
- Modify: `crates/codex/tests/reader_contract.rs`

**Interfaces:**
- `ParseOutcome::Emitted(ObservationDraft)` and `ReadBatch::events() -> &[ObservationDraft]`.
- `ParseContext` carries the fixed `codex` `UsageProviderId`.
- Parser resume schema v2 carries optional parent, conflict marker, and next usage ordinal. A v1 resume fails closed because its ordinal cannot be inferred safely; P0-D must rebuild it non-destructively instead of risking identity collision.
- Late ancestry is emitted as a separate bounded `SessionRelationDraft`, so metadata discovered after usage remains available for later reconciliation.

- [x] **Step 1: add failing lineage compatibility tests** for top-level/payload `forked_from_id`, top-level/payload `parent_thread_id`, structured subagent parent, equal duplicate declarations, conflicting parents, invalid types, controls/oversize, and ancestry arriving after earlier usage.
- [x] **Step 2: run RED:** the missing draft/relation API failure was observed.
- [x] **Step 3: decode bounded ancestry only.** The collector is precedence-independent: zero unique valid parents means none, one means that parent, and more than one sets conflict. Invalid values increment stable diagnostics without echoing source text.
- [x] **Step 4: persist resume v2 state** and increment ordinal only after a positive usage draft is emitted. Preserve the provider cumulative snapshot on the draft and keep unrelated JSON fields borrowed/ignored.
- [x] **Step 5: delete Codex fingerprinting** and return drafts plus separate late relations through parser/reader. Existing tests assert draft facts and canonicalize only through accounting when identity is under test.
- [x] **Step 6: run GREEN:** focused lineage/reader coverage and the complete locked Codex suite pass.
- [x] **Step 7: run** format, strict workspace Clippy, and diff checks; all pass.

### Task 4: Make the store accept only accounting output

**Files:**
- Modify: `crates/store/Cargo.toml`
- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/write.rs`
- Modify: `crates/store/tests/usage_ingest_contract.rs`
- Modify: `crates/domain/src/usage.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/domain/tests/usage_contract.rs`

**Interfaces:**
- `AppendBatchParts.events: Box<[tokenmaster_accounting::CanonicalUsageEvent]>`.
- Domain no longer exports `CanonicalUsageEvent`, `CanonicalUsageEventParts`, `EventFingerprint`, `EventId`, `ReplaySignature`, `ReplayEvidence`, or `UsageLineage`.

- [x] **Step 1: convert store fixtures to canonicalize drafts** and add compile-fail rustdocs proving store consumers cannot construct a canonical event directly.
- [x] **Step 2: run RED:** the locked store build rejected the missing dependency edge before migration.
- [x] **Step 3: update store imports and event accessors,** then remove the superseded canonical/replay authority types from domain. No schema change occurs in this task.
- [x] **Step 4: run GREEN:** domain, accounting, Codex, store, and workspace validation pass.
- [x] **Step 5: search authority bypasses:** no downstream constructor or Codex fingerprint implementation remains; only compile-fail examples intentionally mention forbidden constructors.

### Task 5: Record truth and close the slice

**Files:**
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `README.md`
- Modify: `README_RU.md`

- [x] **Step 1: update all status documents** to identify P0-A and its incorporated Codex lineage slice as implemented, with P0-C as next.
- [x] **Step 2: retain one traceability row for every normative `TM-*` requirement,** including explicit planned/open-evidence entries.
- [x] **Step 3: run a docs consistency scan** for nonexistent ADR references, stale next-slice text, unfinished markers, and normative requirement IDs missing from traceability; no findings.
- [x] **Step 4: run the quality gate:** clean-root audit, format, strict workspace Clippy, locked workspace tests, `git diff --check`, and tracked sensitive-content scan pass.
- [x] **Step 5: inspect `git status --short`** and commit only the intended authority/lineage files with `feat(accounting): enforce canonicalization authority`.

## Acceptance gate

- No crate outside `tokenmaster-accounting` can construct authoritative identity or a canonical event.
- Equal provider/profile/session/ordinal/model/delta facts share fingerprint v2 across source/timestamp copies; different provider, session, ordinal, model, or delta facts do not.
- Replay identity remains separate and its evidence strength is explicit.
- Codex parser and reader expose drafts only; store accepts canonicalized events only.
- No SQLite migration is attempted until P0-B lineage fixtures and P0-C classifier contracts exist.
- Focused tests, strict Clippy, locked workspace tests, privacy checks, clean-root audit, and diff checks pass before the slice is called complete.
