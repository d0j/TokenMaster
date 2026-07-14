# TokenMaster P0-C Pure Replay Classifier Plan

> **Execution:** follow this plan with test-driven development. This slice changes no SQLite schema and performs no archive migration.

**Goal:** classify one canonical observation at a time as `eligible`, `replay`, `pending`, or `conflict` through a deterministic, allocation-free, provider-neutral state transition.

**Architecture:** replay identity already belongs to `tokenmaster-accounting`, so the pure classifier belongs there as well. Store/runtime code will later supply parent-ordinal and traversal facts; it must not own a second interpretation of replay semantics. P0-D persists the result and durable continuation state only after this contract passes.

**Tech stack:** Rust 1.97, existing `tokenmaster-accounting` and domain types, no new dependency.

## Binding decisions

- `MAX_REPLAY_DEPTH = 32` and `MAX_REPLAY_FANOUT = 256` are accounting contract constants.
- Cycles, self/conflicting ancestry, corrupt state combinations, and mismatched parent ordinals are `conflict`.
- Depth or fanout exhaustion is `pending`, never `conflict`; P0-D will persist a continuation.
- A root observation is always `eligible` when root facts are internally consistent.
- Before divergence, replay is proved only when child and parent evidence are both strong and signatures match.
- Before divergence, strong mismatch or a completed parent tail proves divergence and makes the current observation `eligible`.
- Weak evidence is `pending` for that observation but leaves the session in `matching`, so a later strong mismatch can still prove divergence.
- Missing data from an open parent makes the session `pending`; callers must replay that session from `matching` when new parent evidence arrives.
- Divergence is irreversible for one fixed child-parent relation. Later observations remain `eligible`.
- The classifier accepts no provider content, paths, timestamps, token payloads, SQL handles, collections, or callbacks.

## Transition table

| Prior state / facts | Parent ordinal | Disposition | Next state |
| --- | --- | --- | --- |
| `root`, no parent | not applicable | `eligible` | `root` |
| `matching`, both strong, equal signature | present at same ordinal | `replay` | `matching` |
| `matching`, both strong, different signature | present at same ordinal | `eligible` | `diverged` |
| `matching`, either side weak | present at same ordinal | `pending` | `matching` |
| `matching` | missing, parent open | `pending` | `pending` |
| `matching` | missing, parent complete | `eligible` | `diverged` |
| `diverged` | any structurally valid parent state | `eligible` | `diverged` |
| `pending` | any | `pending` | `pending` |
| any conflict/cycle/corrupt combination | any | `conflict` | `conflict` |
| any depth/fanout exhaustion without conflict | any | `pending` | `pending` |

## Task 1 — RED classifier contract

**Files:**

- Create `crates/accounting/tests/replay_classifier_contract.rs`.

- [x] Build canonical child/parent fixtures only through `Canonicalizer`.
- [x] Add table-driven cases for root, strong replay, strong divergence, weak pending followed by strong divergence, missing-open, missing-complete, irreversible divergence, declared conflict, cycle, relation conflict, wrong scope/session/ordinal, depth 33, and fanout 257.
- [x] Run the focused test and observe unresolved classifier imports.

## Task 2 — GREEN pure implementation

**Files:**

- Create `crates/accounting/src/replay.rs`.
- Modify `crates/accounting/src/lib.rs`.

- [x] Add `ReplayDisposition`, `SessionReplayState`, `ParentOrdinal`, `ReplayTraversalFacts`, `ReplayClassificationInput`, `ReplayClassification`, and zero-sized `ReplayClassifier`.
- [x] Keep input fields private and expose constructors/accessors; invalid cross-field combinations fail closed as `conflict` rather than panicking.
- [x] Implement the table exactly with no allocation, recursion, I/O, global state, or provider-specific branch.
- [x] Run the focused contract, complete accounting suite, rustdoc compile-fail proofs, format, and strict accounting Clippy.

## Task 3 — truth and quality gate

**Files:**

- Modify `spec/TRACEABILITY.md`.
- Modify `docs/CURRENT_STATE.md`.
- Modify `docs/PROJECT_HISTORY.md`.
- Modify `docs/HANDOFF.md`.
- Modify `docs/ROADMAP.md`.
- Modify `docs/CHANGELOG.md`.
- Modify `docs/AUDIT_AND_MASTER_PLAN.md`.

- [x] Mark only the pure classifier implemented; keep persistence, continuation queue, reclassification, and replay-safe totals pending.
- [x] Run clean-root, format, strict workspace Clippy, full locked workspace tests, authority/privacy/docs consistency scans, and `git diff --check`.
- [x] Commit the intended P0-C slice as `feat(accounting): add bounded replay classifier`.

## Acceptance gate

- Every transition in the table has a public contract test.
- Weak evidence never suppresses usage and does not prevent later proven divergence.
- Bounds exhaustion never becomes false conflict or false eligibility.
- Impossible state/parent combinations fail closed without panic or allocation.
- No store/schema/runtime behavior changes and no replay-safe totals claim is made.
