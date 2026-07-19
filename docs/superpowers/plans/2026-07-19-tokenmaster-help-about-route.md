# TokenMaster P3-D.7 Help/About Route Implementation Plan

> Every behavior task follows red-green-refactor. The standard pinned `AboutSlint`
> widget is required; do not replace it with custom attribution.

**Goal:** Replace the Help/About placeholder with one truthful, responsive, accessible,
fixed-memory guide and attribution surface without adding runtime authority.

**Architecture:** Mount one static six-card Slint view, pass only the compile-time
package version once during window construction, and keep all current/future capability
boundaries explicit.

**Tech stack:** Rust 1.97.0, Slint 1.17.1 standard widgets, existing Desktop shell,
PowerShell mutation audits.

## Global constraints

- Keep Help/About ready without archive or live runtime.
- Add no projection, list model, query, worker, thread, timer, animation, queue, cache,
  connection, polling, dynamic diagnostics, filesystem/network/process/SQL owner, or
  TokenMaster callback.
- Use `env!("CARGO_PKG_VERSION")` once; never infer commit/package/signing truth.
- Mount exactly one pinned standard `AboutSlint`; add no product `Platform.open-url`,
  URL property, browser/session automation, or arbitrary link.
- Preserve current English fallback; P4 owns unified en/ru/pseudo locale switching.
- Do not claim P5 automation or P6 notices/SBOM/MSVC/signing/release completion.

---

### Task 1: Add RED compiled Help/About contracts

**Files:**
- Modify: `crates/desktop/tests/ui_contract.rs`

**Acceptance:**
- [x] Initial test requires the real Help/About mount, ready-without-archive state,
  compile-time version, six sections, wide/narrow switch, required content, standard
  Slint attribution accessibility, and unchanged window identity.
- [x] The test fails because the current route is still the placeholder.

### Task 2: Implement the static responsive route

**Files:**
- Create: `crates/desktop/ui/views/help-about-view.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/ui.rs`

**Acceptance:**
- [x] Add one `help-about-visible` mount and exclude it from the placeholder.
- [x] Set the package version exactly once during `DesktopShell` construction.
- [x] Instantiate six fixed accessible cards once and reflow by position only.
- [x] Mount exactly one standard `AboutSlint`.
- [x] Keep source, privacy, health, automation-unavailable, and license truth complete.
- [x] Run focused real-Slint and complete Desktop package tests green.

### Task 3: Extend deterministic architecture/license/privacy audits

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`

**Acceptance:**
- [x] Update exact Slint production file count.
- [x] Assert one real mount, six sections, one version setter, one standard attribution,
  responsive/accessibility content, and no placeholder fallback.
- [x] Assert zero model/query/runtime/store/worker/thread/timer/animation/queue/cache/
  connection/polling/dynamic-diagnostic/callback/product-open-URL authority.
- [x] Reject false signed/MSVC/SBOM/release/provider/automation claims.
- [x] Add RED/GREEN mutations for each boundary and run source/release plus Pester green.

### Task 4: Synchronize durable project truth

**Files:**
- Modify: `spec/SPECIFICATION.md`
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
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`

**Acceptance:**
- [x] Record the fixed-content/version/attribution/privacy/no-runtime boundary.
- [x] Keep P4 localization/presentation, P5 automation, P6 notices/SBOM/package/signing,
  M0, and release acceptance incomplete.
- [x] Store no current commit hash.

### Task 5: Independent review and closeout

- [x] Run focused Desktop/audit tests first.
- [x] Run `git diff --check`.
- [x] Run clean-root, Rust formatting, warnings-as-errors workspace Clippy, and locked
  workspace tests exactly as required by `AGENTS.md`.
- [x] Resolve every Critical/Important independent-review finding and re-review.
- [x] Audit task-owned agents/processes, stage only this slice, and create English
  conventional commits.
- [x] Do not push, merge, package, sign, or claim release acceptance without authority.
