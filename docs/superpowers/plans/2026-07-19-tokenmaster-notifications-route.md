# TokenMaster P3-D.6 Notifications Route Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or
> superpowers:subagent-driven-development. Every behavior task is red-green-refactor.

**Goal:** Replace the Notifications placeholder with a truthful, responsive,
identity-free expiry-safety route over the already-published benefit overview without
adding backend work or falsely acknowledging runtime deliveries.

**Architecture:** Project the existing bounded `BenefitOverviewSnapshot` into one
32-row reminder-profile model and one 256-row current-lot model. Preserve expiry
precision, evidence, policy source/coverage, ordering, and explicit truncation.

**Tech stack:** Rust 1.97.0, Slint 1.17, bundled SQLite, existing query/product/desktop
packages, PowerShell contract audits.

## Global constraints

- TokenMaster remains the only product; pinned references remain non-runtime inputs.
- Keep the existing all-current benefit overview and capacity-one query worker.
- Never call the reminder delivery take/ack/release APIs from route projection or
  navigation.
- Never expose provider/account/workspace/scope/lot/delivery/window identity, target,
  path, content, cursor, SQL, credential, receipt, or activation authority.
- Keep 32 scopes, 256 lots, and eight leads as hard frontend caps.
- Preserve exact/bounded/local/date/unknown expiry truth and available zero values.
- Route selection is presentation-only and never rebuilds either model.
- Add no dependency, crate, schema, worker, thread, timer, queue, cache, watcher,
  connection, polling, runtime mutation, or export surface.

---

### Task 1: Add the bounded Notifications projection

**Files:**
- Create: `crates/desktop/src/notifications.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/presentation.rs`
- Create: `crates/desktop/tests/notifications_projection_contract.rs`

**Interfaces:**
- `MAX_NOTIFICATION_SCOPES = 32`, `MAX_NOTIFICATION_LOTS = 256`,
  `MAX_NOTIFICATION_LEADS = 8`.
- `DesktopReminderScopeRow`: ordinal, profile source/coverage/revision, leads,
  completeness/evidence/warnings, nearest due/expiry, current-lot count.
- `DesktopBenefitExpiry`: exact UTC, bounded UTC, provider local, provider date,
  unknown; no lossy exact-time conversion.
- `DesktopBenefitLotRow`: ordinal, provider-neutral kind/quantity/state/label,
  granted time, typed expiry, source/confidence/detail only.
- `DesktopNotificationsProjection`: state/reasons, two `Arc` row arrays, explicit
  truncation.

**Acceptance:**
- [x] Start with compile-failing public API and initial-state tests.
- [x] Add ready, authoritative-empty, retained failure, unavailable, partial evidence,
  warning, and frontend-cap fixtures.
- [x] Prove 32/256/8 caps, query order, distinct lots, exact kinds/states/quantities,
  effective coverage/source, and every expiry precision.
- [x] Prove private/opaque identities and delivery/activation authority are absent.
- [x] Prove 10,000 snapshot replacements release old arrays.
- [x] Add the projection to `DesktopProjection` and run focused tests green.

### Task 2: Mount the responsive Slint Notifications view

**Files:**
- Modify: `crates/desktop/ui/models.slint`
- Create: `crates/desktop/ui/views/notifications-view.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/ui_contract.rs`

**Acceptance:**
- [x] First add a failing compiled UI test for visibility, counts, coverage/source,
  leads, next due/expiry, separate lot rows, expiry precision, evidence, and accessibility.
- [x] Add one `notifications-visible` mount with one scope and one lot model.
- [x] Wide and narrow layouts preserve complete meaning from the same bounded models.
- [x] Waiting/unavailable/degraded/empty/ready/truncated states remain explicit.
- [x] Accessible labels retain scope association, benefit kind/quantity/state, expiry,
  and evidence without color-only meaning.
- [x] Route-only selection performs no model application, query/runtime call, ack,
  scheduling, or window recreation.
- [x] Run the complete Desktop package tests green.

### Task 3: Extend deterministic architecture and privacy audits

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`

**Acceptance:**
- [x] Update exact production file counts and mutations for 32/256/8 caps, existing
  overview reuse, two models/application sites, real mount, complete wide/narrow/
  accessibility meaning, and route-only behavior.
- [x] Assert zero new query/worker/timer/queue/cache/connection/polling and zero delivery
  take/ack/release, runtime/store/query authority, private IDs, or activation controls.
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
- [x] Record the exact expiry-center boundary, caps, privacy, expiry precision,
  effective-profile truth, no-delivery-ack rule, and performance budget.
- [x] Keep GUI presentation receipts, settings synchronization/editing, snooze/quiet
  hours, OS delivery, usage alerts, activation, Help/P3-E, P4/P5, M0, packaging,
  signing, and release incomplete.
- [x] Do not store a current commit hash in tracked documents.

### Task 5: Independent review and closeout

- [x] Run focused Desktop/audit tests first.
- [x] Run `git diff --check`.
- [x] Run clean-root, Rust formatting, warnings-as-errors workspace Clippy, and locked
  workspace tests exactly as required by `AGENTS.md`.
- [x] Request one independent read-only high-rigor correctness/security/performance
  review and resolve every Critical/Important issue.
- [x] Audit and stop task-owned agents, tests, diagnostics, and temporary processes.
- [x] Stage only this slice and create English conventional commits.
- [x] Do not push, merge, package, sign, or claim release acceptance without authority.
