# TokenMaster P4-F Board Preferences Implementation Plan

**Goal:** Close TM-FUNC-004 with durable reorder, hide, collapse, and reset behavior
for the six fixed Dashboard sections without changing data ownership.

**Architecture:** Add one validated fixed board manifest to the complete presentation
value and schema v6. Reuse the existing presentation admission/worker/persistence
path. Slint receives bounded editor and visible-slot models whose keys select only the
six compiled card components.

**Constraints:** Rust 1.97.0, Slint 1.17, one writer, focused RED before production
code, one implementation review plus at most one re-review, no drag-and-drop, no new
worker/query/timer/cache, and no speculative audit hardening.

---

## Task 1: Strict schema-v6 board manifest

**Files:**
- Modify: `crates/state/src/settings/value.rs`
- Modify: `crates/state/src/settings/migration.rs`
- Modify: `crates/state/src/settings/mod.rs`
- Modify: `crates/state/tests/settings_contract.rs`
- Modify: package/config compatibility tests that bind the schema version

1. Add RED tests for canonical v6 serialization, exact six-key permutation,
   duplicate/missing/unknown/all-hidden rejection, and v1-v5 default migration.
2. Run the exact new tests and confirm RED for the absent board contract.
3. Add the closed key enum, fixed row and manifest values, validating construction
   and deserialization, accessors, and canonical default.
4. Extend complete `PresentationSettings`; migrate v1-v5 in memory without startup
   writes; update exact package/config schema assertions.
5. Run `cargo +1.97.0 test -p tokenmaster-state --test settings_contract --locked`
   plus directly affected package/config tests.
6. Commit `feat(state): add durable board preferences`.

## Task 2: Complete application and desktop payload

**Files:**
- Modify: `crates/desktop/src/presentation_style.rs`
- Modify: `crates/desktop/src/reliable_state.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/app/src/command.rs`
- Modify: relevant Desktop/app focused tests and constructor call sites

1. Add RED tests for state/Desktop key mapping, complete payload preservation, row
   move, visibility, collapse, last-visible rejection, and reset.
2. Run the exact tests and confirm RED.
3. Add the fixed Desktop board value and edit methods. Preserve board state while
   selecting other presentation axes and carry the complete value through the
   existing `UpdatePresentation` operation.
4. Prove admission-before-apply and one-active/one-latest replacement remain intact;
   add no command, worker, queue, or persistence slot.
5. Run focused Desktop presentation/reliable-state and app command/state tests.
6. Commit `feat(app): carry complete board preferences`.

## Task 3: Compiled board editor and Dashboard rendering

**Files:**
- Modify: `crates/desktop/ui/models.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/ui/views/settings-view.slint`
- Modify: `crates/desktop/ui/views/dashboard-view.slint`
- Modify: `crates/desktop/src/ui.rs`
- Create or modify focused board UI contract tests

1. Add RED compiled-UI tests for six editor rows, Up/Down/Visible/Collapse/Reset
   callbacks, ordered visible slots, compact collapsed cards, last-visible safety,
   narrow stacking, and preservation of every P4-E wide template.
2. Run the new target and confirm RED.
3. Bind the fixed editor model and callbacks. Submit complete presentation values and
   update the window only after admission.
4. Render visible slots by closed key against the existing six payloads. Keep all six
   projection rows and payload models populated even when their cards are hidden or
   collapsed.
5. Add a bounded repeated-edit test and run board UI, Dashboard projection, existing
   density/skin/scheme/layout, and `ui_contract` targets.
6. Commit `feat(desktop): add customizable dashboard board`.

## Task 4: Source-of-truth and required receipts

**Files:**
- Modify only directly affected receipt scripts/tests when an existing exact schema
  or payload assertion fails
- Modify: `spec/SPECIFICATION.md`, `spec/DATA_CONTRACT.md`, `spec/API_CONTRACT.md`,
  `spec/SECURITY.md`, `spec/TRACEABILITY.md`, `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`, `docs/HANDOFF.md`, `docs/ROADMAP.md`,
  `docs/FEATURE_PARITY.md`, `docs/PROJECT_HISTORY.md`, `docs/CHANGELOG.md`

1. Update only stale schema/payload receipt anchors; add no new audit parser or
   mutation category unless a required existing receipt cannot validate v6.
2. Run affected receipt self-tests and receipts once.
3. Record P4-F product truth and explicitly retain locale/language,
   typography/accessibility/DPI/paint/resource, P5/P6, M0, packaging/signing/soak,
   and release acceptance as blockers. Do not store a commit hash.
4. Commit `docs: record P4-F board evidence`.

## Task 5: Review, gates, and closeout

1. Request one bounded read-only Sol High review for production correctness,
   migration/data loss, boundedness, UI semantics, and release-claim accuracy.
2. Correct only demonstrated production/security/data-loss or required-evidence
   findings with focused RED/GREEN tests; allow at most one re-review.
3. Run the directly affected receipts, then exactly once after corrections:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

4. Reconcile `product state`, `audit/evidence state`, `release blockers`, and `Git
   state`; clean only proven task-owned artifacts and stop task-owned processes and
   agents.

**Stop condition:** P4-F is developer-complete only when focused behavior, required
receipts, and the final baseline are green. It is not M0, a package,
release-candidate, or stable-release acceptance.
