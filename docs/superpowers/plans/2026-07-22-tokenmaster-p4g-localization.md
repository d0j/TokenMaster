# P4-G Unified Localization Implementation Plan

**Goal:** Ship bounded, persisted, hot English/Russian/pseudo localization for the
complete current production window through the existing presentation owner.

**Architecture:** Settings schema v7 and `DesktopPresentationSelection` gain one
closed locale axis. Slint fixed strings use built-in compile-time catalogs; desktop
projection localizes only display labels and preserves stable data fields.

## Task 1: Persisted locale axis

**Scope:** `crates/state/src/settings/**`, state tests, application/desktop
presentation DTOs and their focused tests.

1. Add failing tests for v7 default/round-trip, v1-v6 migration, strict invalid
   locale input, complete app mapping, and desktop locale index admission.
2. Run focused tests and record expected failures.
3. Implement the minimal closed enums, schema migration, mappings, and selection
   methods.
4. Re-run focused state/app/desktop tests and commit.

## Task 2: Complete production UI localization

**Scope:** `crates/desktop/build.rs`, `crates/desktop/translations/**`,
`crates/desktop/ui/**`, `crates/desktop/src/ui.rs`, focused desktop UI contracts.

1. Add failing contracts for bundled locale switching, catalog completeness,
   placeholder preservation, callback wiring, and absence of unwrapped visible
   production literals.
2. Enable bundled translations and convert all fixed visible/accessibility strings
   to `@tr`, including formatted text.
3. Add complete Russian and deterministic pseudo catalogs.
4. Localize Rust-generated display labels at the desktop projection boundary; keep
   stable keys/codes/source values byte-identical.
5. Wire the Settings selector through the existing presentation callback and prove
   hot switching without a new worker or owner.
6. Run focused desktop and app tests and commit.

## Task 3: Integration and release evidence

**Scope:** affected audit scripts and source-of-truth documentation only as required
by the implemented contract.

1. Update composition/shell audits for schema v7 and the fifth presentation axis;
   do not add speculative textual rules.
2. Run one bounded implementation review. Correct only production correctness,
   security/data-loss, or required acceptance-evidence findings, then one re-review.
3. Run focused suites, relevant aggregates, then the full baseline once.
4. Update `spec/TRACEABILITY.md`, `docs/CURRENT_STATE.md`, `docs/HANDOFF.md`,
   `docs/ROADMAP.md`, and `HISTORY.md` with separate product, evidence, release
   blocker, and Git state. Do not claim release acceptance.
5. Remove only task-owned disposable artifacts/processes, verify clean Git, and
   commit the handoff.

## Non-goals and loop guard

No runtime catalogs, OS inference, RTL, arbitrary provider/plugin localization,
query/store changes, typography/DPI remediation, or wording-only review rounds.
Two consecutive audit/test/doc-only corrections trigger `AUDIT_HARDENING_LOOP` and
an immediate return to the next release-critical product slice.
