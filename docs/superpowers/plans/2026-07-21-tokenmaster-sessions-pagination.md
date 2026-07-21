# TokenMaster P3-D.2c Bounded Sessions Pagination Implementation Plan

**Goal:** Deliver generation-safe replace-only Sessions pagination on the existing
desktop worker with no cursor exposure, page accumulation, or stale detail.

**Architecture:** Query pages carry only an identity-free newest/continuation marker.
Slint emits a direction-only intent; the controller resolves the opaque cursor from its
current product snapshot, coalesces one pending request, and replaces the single 64-row
page. Refresh remains newest-authoritative and page replacement clears exact detail.

## Task 1: Make page kind and detail invalidation explicit

**Files:** `crates/query/src/session.rs`, `crates/product/src/reducer.rs`,
`crates/product/src/snapshot.rs`, their inline tests.

1. Add RED query tests proving first pages report newest, continuation pages report
   continuation, and no public debug/output contract reveals cursor contents.
2. Add RED product tests proving every Sessions success/failure replacement clears the
   page-relative selection and exact detail while unrelated section publication does not.
3. Add the minimal page-kind value and specialized Sessions reducer methods; retain the
   existing 256 query maximum and 64 desktop maximum.
4. Run `cargo +1.97.0 test -p tokenmaster-query session --locked` and
   `cargo +1.97.0 test -p tokenmaster-product reducer --locked`, then strict package
   Clippy. Commit the green task.

## Task 2: Add the constant-state controller navigation path

**Files:** `crates/desktop/src/controller.rs`,
`crates/desktop/tests/controller_contract.rs`,
`crates/desktop/tests/session_pagination_controller_contract.rs`.

1. Add RED tests for next/newest requests, missing continuation, stale epoch/product/
   navigation generations, refresh supersession, cancellation, query failure, and page
   success clearing detail.
2. Add stress RED proving 10,000 rapid requests retain one pending intent and execute only
   the latest eligible request without adding a worker/thread/queue.
3. Implement one checked navigation generation and one replace-only pending slot on the
   existing capacity-one worker. Resolve `next_cursor` only inside worker execution.
4. Make ordinary refresh supersede not-started page work and publish the first page.
5. Run the new controller test, existing controller/detail contracts, full desktop tests,
   and strict desktop Clippy. Commit the green task.

## Task 3: Project and route typed navigation without cursor identity

**Files:** `crates/desktop/src/sessions.rs`, `crates/desktop/src/presentation.rs`,
`crates/desktop/src/bridge.rs`, `crates/desktop/src/ui.rs`,
`crates/desktop/tests/sessions_projection_contract.rs`,
`crates/desktop/tests/bridge_event_loop_contract.rs`,
`crates/app/src/application.rs`, `crates/app/src/application_tests.rs`.

1. Add RED projection tests for newest/continuation/unavailable states, synchronous
   pending admission, epoch replacement, rejected handoff, bounds, and privacy.
2. Add RED application tests proving weak current-bundle routing, nonblocking contention,
   safe-mode/missing/stale/closed rejection, and restart epoch isolation.
3. Add a direction-only intent/router/sink and identity-free projection facts. Keep the
   cursor type absent from public desktop/app APIs.
4. Run focused projection, bridge, and application tests plus strict desktop/app Clippy.
   Commit the green task.

## Task 4: Add accessible replace-only Sessions controls

**Files:** `crates/desktop/ui/models.slint`,
`crates/desktop/ui/views/sessions-view.slint`, `crates/desktop/ui/main.slint`,
`crates/desktop/src/ui.rs`, `crates/desktop/tests/ui_contract.rs`,
`crates/desktop/tests/sessions_projection_contract.rs`.

1. Add RED structural and live Slint tests for exact labels, enablement, pending status,
   pointer/Enter/Space dispatch, explicit Tab bindings, and recovery to newest.
2. Add a mutation proving append-style model updates or cursor properties fail the UI
   contract.
3. Bind the two typed actions and replace the single 64-row model exactly once per newer
   snapshot; route-only switching must remain query/model inert.
4. Run focused UI/projection tests and full desktop tests under strict Clippy. Commit the
   green task.

## Task 5: Pin architecture, synchronize source-of-truth, and verify

**Files:** `spec/API_CONTRACT.md`, `spec/TRACEABILITY.md`, `spec/DECISIONS.md`,
`docs/CURRENT_STATE.md`, `docs/HANDOFF.md`, `docs/ROADMAP.md`,
`docs/PROJECT_HISTORY.md`, affected parity/changelog documents,
`scripts/audit-desktop-shell.ps1`, `scripts/audit-application-composition.ps1`,
`scripts/audit-product-status.ps1`, and their Pester mutation tests.

1. Add RED audit mutations pinning one worker/slot, refresh precedence, 64-row replacement,
   no cursor/public identity, exact application wiring, and detail invalidation.
2. Update the authoritative contracts and project state without storing a current commit
   hash or overclaiming P3-E/P4/P5/P6/M0/release acceptance.
3. Run focused Rust/Pester tests, then:

   ```powershell
   pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
   cargo +1.97.0 fmt --all -- --check
   $env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
   cargo +1.97.0 test --workspace --locked
   ```

4. Request independent read-only Sol High review; fix verified findings through focused
   RED/green tests and rerun affected gates.
5. Audit Git status, task-owned processes, temporary files, and heavy build artifacts;
   remove only safe task-owned residue. Commit the verified closeout.
