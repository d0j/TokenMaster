# TokenMaster P3-D.2a Sessions Route Implementation Plan

**Goal:** Render a real privacy-safe first Sessions page without adding a worker,
route-time query, cache, dependency, or private UI identity.

**Architecture:** Increase the existing sessions query page independently to 64, keep
Dashboard's 12-row copy cap, add one pure desktop Sessions projection, and map it into
one responsive Slint model. Exact detail remains P3-D.2b under the generation-bound
selection design.

## Task 1: Prove independent controller bounds

**Files:** `crates/desktop/src/controller.rs`,
`crates/desktop/tests/controller_contract.rs`

1. Add a RED assertion that Sessions requests 64 rows while Activity remains 12.
2. Introduce `MAX_SESSION_ROWS = 64` and keep the overview/activity cap at 12.
3. Run the focused controller contract and commit.

## Task 2: Add the bounded Sessions projection

**Files:** `crates/desktop/src/sessions.rs`, `crates/desktop/src/presentation.rs`,
`crates/desktop/src/lib.rs`, `crates/desktop/src/dashboard.rs`,
`crates/desktop/tests/sessions_projection_contract.rs`

1. Add RED tests for waiting truth, 64-row cap, order, values, evidence, and `has_more`.
2. Reuse crate-private token/cost/evidence mapping helpers.
3. Add `DesktopSessionsProjection` to the one current desktop projection.
4. Run focused projection and desktop package tests and commit.

## Task 3: Render the real responsive route

**Files:** `crates/desktop/ui/models.slint`, `crates/desktop/ui/main.slint`,
`crates/desktop/ui/views/sessions-view.slint`, `crates/desktop/src/ui.rs`,
`crates/desktop/tests/ui_contract.rs`

1. Add RED UI assertions for visibility, 64-row model, newest row, `has_more`, evidence,
   narrow/wide layout, and same-window route switching.
2. Add one Sessions row model and a responsive overview/table view.
3. Map only copied aggregate facts; add no callbacks or keys in this slice.
4. Run focused UI and desktop package tests and commit.

## Task 4: Expand the authority audit

**Files:** `scripts/audit-desktop-shell.ps1`,
`scripts/tests/audit-desktop-shell.Tests.ps1`

1. Add RED mutations for the 64-row cap and route-triggered Sessions rebuild.
2. Update exact production file counts and receipt fields.
3. Require one Sessions model replacement/application site and zero polling/authority.
4. Run Pester and the release desktop audit and commit with closure docs.

## Task 5: Synchronize project truth and verify

**Files:** affected `spec/` contracts plus `docs/CURRENT_STATE.md`, `docs/HANDOFF.md`,
`docs/ROADMAP.md`, `docs/FEATURE_PARITY.md`, `docs/CHANGELOG.md`,
`docs/PROJECT_HISTORY.md`

1. Mark only P3-D.2a complete and P3-D.2b exact detail next.
2. Run clean-root, format, strict workspace Clippy, and full locked workspace tests.
3. Check clean Git state and zero task-owned processes; do not claim M0/release.
