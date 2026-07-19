# TokenMaster P3-D.3 Models Route Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or
> superpowers:subagent-driven-development. Every behavior task is red-green-refactor.

**Goal:** Replace the Models placeholder with a truthful, responsive, bounded recent
30-day model-usage route without adding a query or background owner.

**Architecture:** Enrich the existing recent History request with Model and Project
breakdowns, project the shared immutable envelope into at most 64 model rows, and mount
one compiled Slint Models view. History keeps its 30 daily rows; Models and the future
Projects view share the same range, timezone, freshness, and dataset identity.

**Tech stack:** Rust 1.97.0, Slint 1.17, bundled SQLite, existing query/product/desktop
packages, PowerShell contract audits.

## Global constraints

- TokenMaster remains the only product; pinned references remain non-runtime inputs.
- Keep query results, product snapshots, desktop projections, and Slint models bounded.
- Never expose content, credentials, private IDs, absolute paths, cursors, or authority.
- Preserve the one capacity-one query worker and one immutable publication path.
- Route selection performs presentation-only work and never rebuilds bounded models.
- Add no dependency, crate, schema, thread, timer, queue, cache, watcher, or connection.
- Do not widen into interactive ranges, aliases, filters, drill-down, export, or P4/P5.

---

### Task 1: Share the recent analytics envelope with Models and Projects

**Files:**
- Modify: `crates/desktop/src/controller.rs`
- Modify: `crates/desktop/tests/controller_contract.rs`
- Modify: `crates/product/src/route.rs`
- Modify: `crates/product/tests/route_contract.rs`

**Acceptance:**
- [ ] Write failing tests proving the refresh still makes exactly `today` then
  `recent_days`, and the latter requests exactly Model and Project breakdowns.
- [ ] Write a failing route test proving Models follows recent usage while Dashboard
  continues to follow today analytics.
- [ ] Add the two breakdown kinds to the existing History request only.
- [ ] Derive Models readiness from `history_ready`; derive Projects from
  `history_ready + git_ready` without changing other route semantics.
- [ ] Run controller and product route tests with `--locked` and record green output.

### Task 2: Add a bounded Models desktop projection

**Files:**
- Create: `crates/desktop/src/models.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/presentation.rs`
- Create: `crates/desktop/tests/models_projection_contract.rs`
- Modify test support only where deterministic public fixtures require it.

**Interfaces:**
- `MAX_MODEL_ROWS = 64`.
- `DesktopModelUsageRow`: canonical model label, events, input, cached, output,
  reasoning, total, and cost.
- `DesktopModelsProjection`: section state/reasons, exact range/timezone/evidence,
  shared overview, rows, token maximum, and truncation.

**Acceptance:**
- [ ] Start with compile-failing tests for initial unavailable truth and public API.
- [ ] Add deterministic ready/empty/partial/truncated/mismatched-identity fixtures.
- [ ] Prove stable backend order is retained and more than 64 rows cannot cross the
  desktop boundary.
- [ ] Prove backend truncation and desktop truncation are both visible.
- [ ] Prove no provider/profile/source/session/project/path/key/cursor field exists.
- [ ] Add the projection to `DesktopProjection` and run focused tests green.

### Task 3: Mount the real responsive Slint Models view

**Files:**
- Modify: `crates/desktop/ui/models.slint`
- Create: `crates/desktop/ui/views/models-view.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/ui_contract.rs`

**Acceptance:**
- [ ] First add a failing compiled UI test for Models visibility, real fixture labels,
  all token components, range/timezone/evidence, completeness, and one window identity.
- [ ] Add `ModelUsageRow`, Models properties, and one `models-visible` branch.
- [ ] Wide layout shows all columns; narrow layout keeps all component meaning in one
  owned row model and provides an accessible full-row label.
- [ ] Empty/unavailable/degraded/partial/truncated states remain explicit.
- [ ] Route-only selection does not invoke `apply_models_projection`, replace its model,
  query, create a window, or schedule work.
- [ ] Run the complete desktop package tests green.

### Task 4: Add deterministic architecture/privacy audits

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify additional existing audit only if its current ownership is the exact fit.

**Acceptance:**
- [ ] Add mutation tests for the Models cap, single mapping/application site, real view
  mount, route-only behavior, shared-query breakdowns, and forbidden frontend fields.
- [ ] Assert no third analytics request/section/worker/timer/cache is introduced.
- [ ] Assert the Slint view has wide/narrow branches and every token component.
- [ ] Run audit Pester tests and the production desktop audit green.

### Task 5: Synchronize durable project truth

**Files:**
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`

**Acceptance:**
- [ ] Record the shared recent-usage decision, exact bounds, privacy surface, and
  Models delivery evidence.
- [ ] Keep interactive ranges, aliases, filters, Projects, Activity, P4/P5, parity,
  M0, packaging, signing, and release incomplete.
- [ ] Do not store a current commit hash in tracked documents.

### Task 6: Independent review and closeout

- [ ] Run focused product/desktop/audit tests first.
- [ ] Run `git diff --check` and clean-root.
- [ ] Run Rust formatting, warnings-as-errors workspace Clippy, and locked workspace
  tests exactly as required by `AGENTS.md`.
- [ ] Request one independent read-only high-rigor correctness/security/performance
  review and resolve every Critical/Important issue.
- [ ] Audit and stop task-owned agents, tests, diagnostics, and temporary processes.
- [ ] Stage only this slice and create an English conventional commit.
- [ ] Do not push, merge, package, sign, or claim release acceptance without authority.
