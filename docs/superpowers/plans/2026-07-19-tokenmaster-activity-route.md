# TokenMaster P3-D.5 Recent Activity Route Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or
> superpowers:subagent-driven-development. Every behavior task is red-green-refactor.

**Goal:** Replace the Activity placeholder with a truthful, responsive, identity-free
12-row Recent activity route over the already-published latest page without adding a
query or background owner.

**Architecture:** Selectively project the existing `LatestActivityPage` into one
bounded Desktop/Slint model containing only UTC timestamp, canonical model, and five
explicit token facts. Preserve page/evidence truth and keep future rhythm aggregation
separate.

**Tech stack:** Rust 1.97.0, Slint 1.17, bundled SQLite, existing query/product/desktop
packages, PowerShell contract audits.

## Global constraints

- TokenMaster remains the only product; pinned references remain non-runtime inputs.
- Keep the existing one `LatestActivityRequest::first(12)` and one capacity-one worker.
- Never expose scope, provider/profile, event ID, cursor/fingerprint, dataset identity,
  source/session/project/path/content, prompts, responses, commands, or credentials.
- Keep query results, product snapshots, projections, and Slint models bounded.
- Route selection performs presentation-only work and never rebuilds the model.
- Add no dependency, crate, schema, thread, timer, queue, cache, watcher, connection,
  arbitrary query, or export surface.
- Do not label the route as rhythm/hourly/day-of-week or complete archive.

---

### Task 1: Add the bounded identity-free Activity projection

**Files:**
- Create: `crates/desktop/src/activity.rs`
- Modify: `crates/desktop/src/dashboard.rs` only for a crate-private exact token mapper.
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/presentation.rs`
- Create: `crates/desktop/tests/activity_projection_contract.rs`
- Modify test support only for deterministic public fixtures.

**Interfaces:**
- `MAX_ACTIVITY_ROWS = 12`.
- `DesktopRecentActivityRow`: timestamp, canonical model, input/cached/output/reasoning/
  total typed token facts only.
- `DesktopActivityProjection`: state/reasons, freshness/quality, optional `has_more`,
  one `Arc` row list.

**Acceptance:**
- [x] Start with compile-failing tests for initial truth and the public API.
- [x] Add ready, authoritative-empty, degraded-retained, unavailable, partial evidence,
  `has_more`, and frontend-cap fixtures.
- [x] Prove newest-first order and exact five-component token availability/zero truth.
- [x] Prove scope/event/cursor/fingerprint/private identity never enters the projection.
- [x] Prove aggregate rebuild does not disable Activity route truth.
- [x] Prove 10,000 snapshot replacements release the old row list.
- [x] Add the projection to `DesktopProjection` and run focused tests green.

### Task 2: Mount the real responsive Slint Recent activity view

**Files:**
- Modify: `crates/desktop/ui/models.slint`
- Create: `crates/desktop/ui/views/activity-view.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/ui_contract.rs`

**Acceptance:**
- [x] First add a failing compiled UI test for visibility, UTC timestamp, canonical
  model, all token facts, page status, evidence, and one window/model identity.
- [x] Add `RecentActivityRow`, Activity properties, and one `activity-visible` branch.
- [x] Wide and narrow layouts preserve all five token facts from the same bounded model.
- [x] Waiting/unavailable/degraded/empty/ready/incomplete-page states remain explicit.
- [x] Full accessible labels retain UTC time, model, and every token component.
- [x] Route-only selection does not apply/rebuild the Activity model, query, recreate
  the window, or schedule work.
- [x] Run the complete Desktop package tests green.

### Task 3: Extend deterministic architecture and privacy audits

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`

**Acceptance:**
- [x] Update exact production file counts and add mutations for the 12-row cap, exact
  existing request, one model/application site, real mount, UTC context, full token
  mix, accessibility, route-only behavior, and forbidden fields.
- [x] Assert no second Activity query, worker, timer, queue, cache, connection, callback,
  cursor, raw-event export, or rhythm claim is introduced.
- [x] Run source/release audits and all Desktop audit Pester tests green.

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
- [x] Record the exact Recent activity boundary, 12-row cap, privacy surface, page
  completeness, aggregate-rebuild readiness, and delivery evidence.
- [x] Keep full rhythm aggregation, pagination, Activity detail/filter/export,
  Notifications/Help/P3-E, P4/P5, M0, packaging, signing, and release incomplete.
- [x] Do not store a current commit hash in tracked documents.

### Task 5: Independent review and closeout

- [x] Run focused Desktop/audit tests first.
- [x] Run `git diff --check`.
- [x] Run clean-root, Rust formatting, warnings-as-errors workspace Clippy, and locked
  workspace tests exactly as required by `AGENTS.md`.
- [x] Request one independent read-only high-rigor correctness/security/performance
  review and resolve every Critical/Important issue.
- [x] Audit and stop task-owned agents, tests, diagnostics, and temporary processes.
- [x] Stage only this slice and create an English conventional commit.
- [x] Do not push, merge, package, sign, or claim release acceptance without authority.
