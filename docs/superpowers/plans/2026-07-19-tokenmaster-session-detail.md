# TokenMaster P3-D.2b Exact Session Detail Implementation Plan

**Goal:** Deliver exact generation-safe session detail on the existing bounded worker
with no opaque UI identity, stale rapid-click result, or controller-restart alias.

## Task 1: Make backend replacement identity explicit

**Files:** `crates/desktop/src/bridge.rs`, `crates/desktop/src/presentation.rs`,
`crates/desktop/src/ui.rs`, bridge/presentation tests, `crates/app/src/application.rs`.

1. Add RED tests proving a higher `DesktopSnapshotEpoch` accepts a restarted lower/equal
   product generation, an older epoch is ignored, and epoch replacement clears selection.
2. Allocate one checked epoch per bridge, expose it, and bind the controller before live
   publication.
3. Preserve same-epoch newest-only behavior and existing capacity-one bridge semantics.
4. Run focused desktop bridge/presentation and application composition tests; commit.

## Task 2: Correlate one product detail result

**Files:** `crates/product/src/snapshot.rs`, `crates/product/src/reducer.rs`, product
reducer tests.

1. Add RED tests for selection generation/ordinal correlation, no cross-selection
   retained payload, older-result rejection, missing detail, and dataset invalidation.
2. Add bounded value types and specialized detail publish/fail methods.
3. Keep exactly one current product snapshot and no new dependency/authority.
4. Run product tests, strict product Clippy, and product-status audit; commit.

## Task 3: Multiplex detail on the existing worker

**Files:** `crates/desktop/src/controller.rs`, controller tests.

1. Add RED tests for exact ordinal-to-key resolution, stale epoch/product generation,
   missing row/detail, failure, cancellation, rapid-selection latest-wins, and combined
   refresh/detail coalescing.
2. Add one constant-state work slot (`refresh_pending`, latest/pending selection) and
   route both work kinds through the existing `RefreshWorker`.
3. Publish detail only for the latest selection; never expose or retain its opaque key.
4. Run focused controller/full desktop tests and strict desktop Clippy; commit.

## Task 4: Add bounded detail projection and UI

**Files:** `crates/desktop/src/sessions.rs`, `presentation.rs`, `ui.rs`, Slint models/
Sessions view/main window, projection/UI tests, application session-intent routing.

1. Add RED projection tests for idle/loading/ready/missing/unavailable, 32+32 breakdown
   caps, truncation, approved labels, values, and privacy.
2. Add the typed session-intent router and current-bundle application sink.
3. Add a selectable accessible list and one responsive summary/breakdown detail card.
4. Prove synchronous loading/highlight, rapid click, stale result rejection, same-window
   route switching, and no UI-thread query/model rebuild.
5. Run focused UI/application tests and strict package Clippy; commit.

## Task 5: Expand audits, synchronize truth, and verify

**Files:** desktop/application/product audits and mutation tests, affected `spec/`,
architecture/current state/handoff/roadmap/parity/changelog/project history.

1. Pin epoch binding, one pending selection, detail bounds/model/application path, and
   forbidden opaque/UI-query authority with RED mutations.
2. Mark P3-D.2b complete only after focused/release audits pass.
3. Run clean-root, format, strict workspace Clippy, and full locked workspace tests.
4. Verify a clean branch, zero task-owned processes, and no release/M0 overclaim; commit.
