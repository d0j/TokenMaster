# TokenMaster P3-D.4 Projects Route Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or
> superpowers:subagent-driven-development. Every behavior task is red-green-refactor.

**Goal:** Replace the Projects placeholder with a truthful, responsive, bounded
recent-project usage route enriched by separately labelled today Git evidence without
adding a query or background owner.

**Architecture:** Project the existing recent History Project breakdown and existing
UTC-today Git envelope into at most 32 usage-centric rows. Join only exact safe aliases,
keep the two periods explicit, aggregate same-alias repositories with checked
arithmetic, and mount one compiled Slint Projects view.

**Tech stack:** Rust 1.97.0, Slint 1.17, bundled SQLite, existing query/product/desktop
packages, PowerShell contract audits.

## Global constraints

- TokenMaster remains the only product; pinned references remain non-runtime inputs.
- Keep query results, product snapshots, desktop projections, and Slint models bounded.
- Never expose paths, content, credentials, private IDs, scopes, dataset identities,
  absolute paths, cursors, or authority.
- Preserve the one capacity-one query worker and one immutable publication path.
- Route selection performs presentation-only work and never rebuilds bounded models.
- Add no dependency, crate, schema, thread, timer, queue, cache, watcher, or connection.
- Never combine recent local-time usage and UTC-today Git into one unlabeled period.
- Do not widen into range controls, aliases, filters, detail, export, or P4/P5.

---

### Task 1: Add a bounded two-window Projects desktop projection

**Files:**
- Create: `crates/desktop/src/projects.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/presentation.rs`
- Create: `crates/desktop/tests/projects_projection_contract.rs`
- Modify test support only where deterministic public fixtures require it.

**Interfaces:**
- `MAX_PROJECT_ROWS = 32`.
- `DesktopProjectUsageRow`: safe alias/unassociated label, events, complete token mix,
  typed cost, relative total, and optional typed UTC-today Git facts.
- `DesktopProjectsProjection`: combined state/reasons, recent usage range/evidence and
  overview, bounded rows/truncation, plus separate Git range/evidence/coverage.

**Acceptance:**
- [ ] Start with compile-failing tests for initial truth and the public API.
- [ ] Add ready, empty, partial, retained-failure, unassociated, unmatched,
  mismatched-identity, and 32-row cap fixtures.
- [ ] Prove recent usage range/timezone and Git UTC-today range remain separate.
- [ ] Prove exact alias matching, stable backend order, and no Git-only rows.
- [ ] Prove same-alias multi-repository checked sums and project cost is counted once.
- [ ] Prove backend Project and Git lookahead truncation remain explicit.
- [ ] Prove 10,000 snapshot replacements release the old Projects row list.
- [ ] Add the projection to `DesktopProjection` and run focused tests green.

### Task 2: Mount the real responsive Slint Projects view

**Files:**
- Modify: `crates/desktop/ui/models.slint`
- Create: `crates/desktop/ui/views/projects-view.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/ui_contract.rs`

**Acceptance:**
- [ ] First add a failing compiled UI test for Projects visibility, real safe aliases,
  recent usage labels, separate UTC-today Git labels, token/cost evidence, and one
  window/model identity.
- [ ] Add `ProjectUsageRow`, Projects properties, and one `projects-visible` branch.
- [ ] Wide and narrow layouts preserve full recent token mix and today code meaning
  from the same bounded model.
- [ ] Empty/unavailable/degraded/partial/unmatched/truncated states remain explicit.
- [ ] Full accessible labels name `Recent usage` and `Today code` separately.
- [ ] Route-only selection does not invoke `apply_projects_projection`, replace its
  model, query, create a window, or schedule work.
- [ ] Run the complete desktop package tests green.

### Task 3: Extend deterministic architecture and privacy audits

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify another existing audit only if its current ownership is the exact fit.

**Acceptance:**
- [ ] Add mutation tests for the 32-row cap, one mapping/application site, real view
  mount, two explicit time windows, exact alias-only join, and forbidden fields.
- [ ] Assert no extra analytics/Git query, worker, timer, cache, connection, or route
  callback is introduced.
- [ ] Assert the Slint view has wide/narrow branches, all token components, and all
  checked Git metrics.
- [ ] Run audit Pester tests and the production desktop audit green.

### Task 4: Synchronize durable project truth

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
- [ ] Record the exact two-window decision, bounds, alias join, privacy surface, and
  Projects delivery evidence.
- [ ] Keep interactive ranges, aliases, project detail, Activity, P4/P5, parity, M0,
  packaging, signing, and release incomplete.
- [ ] Do not store a current commit hash in tracked documents.

### Task 5: Independent review and closeout

- [ ] Run focused product/desktop/audit tests first.
- [ ] Run `git diff --check` and clean-root.
- [ ] Run Rust formatting, warnings-as-errors workspace Clippy, and locked workspace
  tests exactly as required by `AGENTS.md`.
- [ ] Request one independent read-only high-rigor correctness/security/performance
  review and resolve every Critical/Important issue.
- [ ] Audit and stop task-owned agents, tests, diagnostics, and temporary processes.
- [ ] Stage only this slice and create an English conventional commit.
- [ ] Do not push, merge, package, sign, or claim release acceptance without authority.
