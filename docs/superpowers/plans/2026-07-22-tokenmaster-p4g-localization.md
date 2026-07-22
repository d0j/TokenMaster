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

## Task 2a1: Hot locale shell and Settings presentation strip

**Scope:** `crates/desktop/build.rs`, `crates/desktop/translations/**`,
`crates/desktop/ui/main.slint`, the Settings presentation strip,
`crates/desktop/src/ui.rs`, and focused desktop UI contracts.

1. Add failing contracts for bundled locale switching, catalog completeness,
   placeholder preservation, callback wiring, and absence of unwrapped visible
   literals in the named shell/control scope.
2. Enable bundled translations and convert the bounded shell/control scope to
   `@tr`, with complete Russian and deterministic pseudo catalogs.
3. Wire the Settings selector through the existing presentation callback and prove
   hot switching without a new worker or owner.
4. Run focused desktop and app tests and commit. Record this as partial localization,
   never as unified production-window completion.

## Task 2a2: Shared component localization

**Scope:** the nine production files under `crates/desktop/ui/components/**`, the
existing bundled catalogs, and focused component/catalog source contracts.

1. Add a failing scoped contract that inventories every fixed linguistic
   visible/accessibility component literal and requires complete Russian/pseudo
   entries with equal placeholders. Locale-invariant punctuation and spacing
   separators may remain raw only through an exact scoped allowlist.
2. Convert only the nine shared components to `@tr` and extend both catalogs with
   human Russian and deterministic visibly-expanded pseudo translations.
3. Run the localization contract, affected component/UI contracts, and strict
   desktop Clippy; commit and continue to Task 2b without claiming unified locale.

## Task 2b: Complete views and projection localization

**Scope:** remaining `crates/desktop/ui/views/**`, the remaining Settings content,
the catalogs, `crates/desktop/src/ui.rs`, a narrow closed display-label resolver,
and focused desktop contracts.

1. Add failing per-surface catalog/source contracts and classify Rust literals into
   translatable display labels versus invariant keys, codes, evidence, paths,
   timestamps, numbers, and source data.
2. Convert every remaining fixed visible/accessibility string to `@tr` and complete
   the Russian and deterministic pseudo catalogs with placeholder equality.
3. Localize only the closed Rust display-label set at the existing projection
   boundary; prove invariant fields remain byte-identical.
4. Run focused desktop contracts, existing presentation UI contracts, and strict
   desktop Clippy; commit only when no production view remains mixed-language.

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
