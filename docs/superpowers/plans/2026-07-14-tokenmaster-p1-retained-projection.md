# TokenMaster P1-A Retained Canonical Projection Implementation Plan

**Status:** Approved; implementation pending.

> **Execution mode:** root-only, test-first, one writer, current feature branch. The
> available task-name-only child surface cannot prove requested model routing, so no
> delegated writer is used.

**Goal:** Allow a complete replay revision to preserve previously accounted events
that disappear because a source was truncated or replaced, without retaining obsolete
source generations, fabricating observations, or weakening atomic promotion.

**Architecture:** Upgrade the private archive to schema v4. Make the indexed canonical
projection self-contained and record publishing/origin revision plus retained state.
Promotion materializes the deterministic union of new eligible selections and safe
prior rows in one transaction. Replay-only evidence suppresses; absent/conflicting
evidence carries; pending remains blocked.

**Stack:** Rust 1.97, Rust 2024, rusqlite 0.40 with bundled SQLite, strict tables,
deferred foreign keys, Cargo contract tests, rustfmt, strict Clippy.

---

## Task 1: Freeze the schema-v4 and migration contract

**Files:**

- Modify: `crates/store/tests/usage_schema_contract.rs`
- Modify: `crates/store/src/usage/schema.rs`
- Modify: `crates/store/src/usage/migration.rs`

**Step 1: Write failing schema tests**

Add contracts that require version 4 and the exact `usage_event` provenance columns:
`projection_revision_id`, `origin_revision_id`, and `retained`. Require strict checks,
the deferred publishing-revision foreign key, and absence of the old observation
foreign key.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract schema_is_strict_path_free_and_has_exact_usage_tables --locked
```

Expected RED: schema remains version 3 and the columns do not exist.

**Step 2: Write failing v3 migration tests**

Create exact populated v3 fixtures for both states:

- legacy projection with no current replay revision;
- promoted current replay revision with direct selections.

After open, require exact row count and logical event values. Legacy rows have null
revision provenance and `retained = 0`; replay rows bind both revision IDs to the
current revision and `retained = 0`. Reopen must validate v4.

Add fault-injected create/copy/drop boundaries in internal migration tests and prove
the original v3 archive, user version, foreign-key policy, and event values survive
each rollback.

**Step 3: Implement exact schema definitions**

Keep separate v1/v2/v3 table contracts so old archives are validated against their
real schema. Add the final v4 `usage_event` schema and use it for fresh archives and
current validation. Do not weaken any existing string, numeric, digest, or privacy
constraint.

**Step 4: Implement create/copy/drop/rename migration**

Validate the source schema before mutation. Create `usage_event_v4`, copy every
logical column plus derived controlled provenance, compare old/new counts and logical
rows, drop the old table, rename, recreate indexes, set user version, run full schema
and foreign-key validation, and commit once.

Route v1 and v2 through their existing exact legacy/revision migrations and then the
same event-table upgrade. A newer or malformed archive still fails closed.

**Step 5: Run schema gates**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store migration --locked
```

**Step 6: Commit**

```powershell
git add crates/store/src/usage/schema.rs crates/store/src/usage/migration.rs crates/store/tests/usage_schema_contract.rs
git commit -m "feat(store): migrate canonical projection provenance"
```

## Task 2: Prove the retention truth table RED

**Files:**

- Modify: `crates/store/tests/replay_archive_contract.rs`
- Modify: `crates/store/src/usage/replay.rs`

**Step 1: Replace the obsolete missing-prior expectation**

Change the existing truncation contract from “promotion fails” to:

- old current page remains unchanged during staging;
- complete sealed promotion succeeds;
- an omitted prior event remains visible and has `retained = 1` under the new
  projection revision;
- its original source key/generation/offset and origin revision are unchanged;
- obsolete source generation rows are removed;
- reopen returns the same event and provenance.

Expected RED: the schema-v3 prior-coverage guard returns `IncompleteManifest`.

**Step 2: Add the complete truth-table fixture**

In one prior projection create distinct events whose next overlay is eligible,
replay-only, conflict-only, or absent. After promotion require:

- eligible is direct and selected from the new generation;
- replay-only is absent from the canonical page;
- conflict-only and absent are retained from prior provenance;
- quality counts still report overlay conflict/replay values;
- no pending overlay can promote.

**Step 3: Add rollback and tamper contracts**

Every existing promotion fault boundary must restore event values, provenance,
generations, and revision status exactly. Invalid projection provenance or a publishing
revision mismatch must fail on reopen or promotion with a stable path-free code.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract carry_forward --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract promotion --locked
```

## Task 3: Implement atomic retained projection

**Files:**

- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/src/usage/read.rs`
- Modify: `crates/store/src/usage/types.rs` only if a bounded provenance read model is
  required by a public contract test

**Step 1: Replace destructive rematerialization**

Within the existing immediate promotion transaction:

1. validate the complete manifest, overlay, versions, zero pending state, and durable
   work as today;
2. calculate the expected fingerprint union in SQLite before mutation;
3. remove prior rows represented only by replay disposition;
4. mark surviving prior rows retained under the new publishing revision;
5. upsert deterministic eligible selections with direct provenance;
6. validate count, direct-selection ownership, retained-state checks, and publishing
   revision before swapping generations;
7. perform the existing generation/revision swap and foreign-key check.

Do not load the canonical page into Rust, add a history-sized vector, retain old
generations, or copy an old event into `usage_observation`.

**Step 2: Preserve exact rollback**

Keep fault boundaries after materialization, generation swap, and revision status.
All projection changes remain inside the same transaction. Discard continues to
remove only unpublished staging.

**Step 3: Run store gates**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

**Step 4: Commit**

```powershell
git add crates/store/src/usage/replay.rs crates/store/src/usage/read.rs crates/store/src/usage/types.rs crates/store/tests/replay_archive_contract.rs
git commit -m "feat(store): retain prior canonical evidence"
```

## Task 4: Update the real Codex pipeline contract

**Files:**

- Modify: `crates/codex/tests/pipeline_contract.rs`
- Modify: `crates/codex/tests/support/pipeline.rs` only if the oracle needs a bounded
  provenance assertion

**Step 1: Change the truncation fixture**

The real JSONL truncation rebuild must now promote successfully while preserving the
omitted historical event exactly once. Atomic replacement that supplies the event
directly must remain direct. Staging and every injected failure still leave the old
page visible.

**Step 2: Prove restart and memory shape**

Close/reopen around the carried projection and confirm totals, ordering, and replay
quality. Continue to hold at most one reader/store/page batch; no complete event list
is introduced in production code.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --locked
```

**Step 3: Commit**

```powershell
git add crates/codex/tests/pipeline_contract.rs crates/codex/tests/support/pipeline.rs
git commit -m "test(codex): prove retained truncation recovery"
```

## Task 5: Documentation, audit, and verification

**Files:**

- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/RECOVERY_PLAYBOOK.md`
- Modify: this plan status

**Step 1: Record the exact authority boundary**

Document the schema-v4 provenance fields and truth table. State that carry-forward
preserves accounted usage; it is not proof that a source still exists. Conflict
retains prior usage while quality remains conflict. Replay classification, not reader
deletion/replacement, is the only automatic suppression authority in P1-A.

Mark P1-A complete only after every gate below passes. Keep P1-B scan finalization and
P1-C+ engine work explicitly unimplemented.

**Step 2: Run the complete gate**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Also audit production dependency direction, forbidden retained-content identifiers,
absolute private paths, secrets, unexpected generated artifacts, and task-owned
processes. Report the ignored million-row gate rather than treating it as passed.

**Step 3: Commit and push**

```powershell
git add spec docs
git commit -m "docs: record retained projection authority"
git push origin cx/tokenmaster-product-architecture
```

No command in this plan accepts M0, proves interactive Windows product behavior,
packages, signs, or releases TokenMaster.
