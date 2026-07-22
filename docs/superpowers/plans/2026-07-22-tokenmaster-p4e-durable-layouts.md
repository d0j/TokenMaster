# TokenMaster P4-E Durable Layouts Implementation Plan

**Goal:** Deliver three instant, durable production layout presets without adding
runtime authority or unbounded retained state.

**Architecture:** Extend the single presentation value and existing replaceable
operation payload from three axes to four. Schema v5 owns the fixed layout enum;
legacy v1-v4 migrate to Refined. The sole Slint window receives one layout index and
the Dashboard rearranges its existing bounded inputs declaratively.

**Constraints:** Rust 1.97.0, Slint 1.17, one writer, focused RED before production
code, one implementation review plus at most one re-review, no speculative audit
hardening.

---

## Task 1: Strict schema-v5 layout state

**Files:**
- Modify: `crates/state/src/settings/value.rs`
- Modify: `crates/state/src/settings/migration.rs`
- Modify: `crates/state/src/package.rs`
- Modify: `crates/state/src/lib.rs`
- Modify: `crates/state/tests/settings_contract.rs`

1. Add RED tests for exact v5 serialization, unknown/partial layout rejection, and
   v1-v4 migration to Refined.
2. Run the exact new state tests and confirm RED for the missing layout contract.
3. Add `PresentationLayout::{Refined, ControlCenter, Workbench}`, extend complete
   `PresentationSettings`, and migrate schema v1-v4 without startup writes.
4. Run `cargo +1.97.0 test -p tokenmaster-state --test settings_contract --locked`.
5. Commit `feat(state): add durable presentation layout`.

## Task 2: Complete four-axis application payload

**Files:**
- Modify: `crates/desktop/src/presentation_style.rs`
- Modify: `crates/desktop/src/reliable_state.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/app/src/command.rs`
- Modify: relevant Desktop/app focused tests and constructor call sites

1. Add RED tests proving every layout maps exactly across Desktop/state and the
   one-active/one-latest payload preserves the complete 81-combination selection.
2. Run the exact tests and confirm RED.
3. Extend the typed Desktop selection, reliable-state projection, state conversion,
   and existing operation payload. Do not add another worker or slot.
4. Run focused Desktop style/projection and app command/operation/state tests.
5. Commit `feat(app): carry complete presentation layout`.

## Task 3: Visible production layout switching

**Files:**
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/ui/views/settings-view.slint`
- Modify: `crates/desktop/ui/views/dashboard-view.slint`
- Modify: `crates/desktop/src/ui.rs`
- Create: `crates/desktop/tests/presentation_layout_ui_contract.rs`
- Modify: relevant existing presentation UI tests

1. Add RED compiled-UI tests for hydration, selector admission, three visible wide
   compositions, narrow fallback, invalid-index neutrality, same window/routes/models,
   and 10,000 switches.
2. Run the new target and confirm RED.
3. Add one fixed Settings selector and callback. Bind layout before presentation
   metadata and implement the three Dashboard branches over existing inputs.
4. Run the new layout target plus existing density/skin/scheme UI targets and
   `ui_contract`.
5. Commit `feat(desktop): add instant layout presets`.

## Task 4: Source-of-truth and required receipts

**Files:**
- Modify only directly affected receipt scripts/tests if the existing exact
  three-axis assertions fail
- Modify: `spec/SPECIFICATION.md`, `spec/DATA_CONTRACT.md`, `spec/API_CONTRACT.md`,
  `spec/SECURITY.md`, `spec/TRACEABILITY.md`, `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`, `docs/HANDOFF.md`, `docs/ROADMAP.md`,
  `docs/FEATURE_PARITY.md`, `docs/PROJECT_HISTORY.md`, `docs/CHANGELOG.md`

1. Update only stale exact-schema/exact-payload receipt assertions; add no unrelated
   regex or mutation categories.
2. Run affected receipt self-tests and receipts once.
3. Record P4-E product truth, evidence limits, remaining board customization, locale,
   typography/accessibility/DPI/paint/resource, P5/P6/M0/package/signing/soak blockers,
   and the audit-loop disposition. Do not store a commit hash.
4. Commit `docs: record P4-E layout evidence`.

## Task 5: Review and final acceptance

1. Request one bounded read-only Sol High review for production correctness,
   migration/data loss, boundedness, UI meaning, and release-claim accuracy.
2. Correct only demonstrated production/security/data-loss or required-evidence
   findings using focused RED/GREEN tests; allow one re-review.
3. Run exactly once after corrections:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

4. Reconcile `product state`, `audit/evidence state`, `release blockers`, and `Git
   state`; clean only proven task-owned disposable artifacts and stop task-owned
   processes/agents.

**Stop condition:** P4-E is developer-complete only when all focused behavior and
required receipts pass and the final baseline is green. It is not M0, package,
release-candidate, or stable-release acceptance.
