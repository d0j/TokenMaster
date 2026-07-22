# TokenMaster P4-D Independent Color Scheme Implementation Plan

> Execute with focused red/green tests. Root owns the critical path, integration, one
> implementation review, final verification, documentation, and stopping conditions.

**Goal:** Add durable, independent `system|light|dark` presentation schemes across all
existing densities and built-in skins without new authority, workers, polling, or growing
memory.

**Architecture:** Settings schema v4 persists one complete density/skin/scheme triple.
Desktop retains one complete presentation selection and resolves `system` from Slint's
reactive `Palette.color-scheme`; Rust owns six exact immutable palettes. The existing
capacity-one application worker persists only complete latest selections.

**Tech stack:** Rust 1.97, Slint 1.17, serde strict JSON, bundled SQLite, PowerShell gates.

**Design:** `docs/superpowers/specs/2026-07-22-tokenmaster-p4d-color-scheme-design.md`

## Guardrails

- Keep skin and color scheme independent.
- Fresh default is `system`; v1/v2/v3 migration is `dark` for visual compatibility.
- Use Slint's existing system observation; add no registry reader, watcher, timer, poller,
  thread, channel, history, or extra pending payload.
- Construct and persist complete triples only; never expose per-axis application commands.
- Palette roles are Rust-owned and applied before scheme metadata.
- Update textual audits only for the changed executable contract; do not broaden the threat
  model or add speculative regex hardening.
- Stop after one implementation review and one final re-review unless a new Critical
  production/security/data-loss defect has a focused reproducer.

## Task 1: Strict schema-v4 state contract

**Files:**

- Modify: `crates/state/src/settings/value.rs`
- Modify: `crates/state/src/settings/migration.rs`
- Modify: `crates/state/src/settings/mod.rs`
- Modify: `crates/state/tests/settings_contract.rs`
- Modify: `crates/state/tests/package_support/mod.rs`
- Modify: `crates/state/tests/package_contract.rs`
- Modify: `crates/state/tests/package_adversarial_contract.rs`

1. Add failing tests for `PresentationColorScheme`, fresh `system`, exact v4 round trip,
   v1/v2/v3 `dark` migration, and rejection of missing/duplicate/unknown/wrong-type scheme.
2. Run the focused state tests and confirm the expected failures.
3. Implement the closed enum, complete triple, exact v3 compatibility wire, and v4 wire.
4. Update package fixtures/source-version bounds to 1..=4 while retaining strict source
   schema binding.
5. Re-run focused state/package tests to green.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-state --test settings_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test package_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test package_adversarial_contract --locked
```

## Task 2: Complete desktop selection and six palettes

**Files:**

- Modify: `crates/desktop/src/presentation_style.rs`
- Modify: `crates/desktop/src/skin.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/tests/presentation_style_contract.rs`
- Modify: `crates/desktop/tests/skin_palette_contract.rs`
- Add: `crates/desktop/tests/presentation_color_scheme_contract.rs`

1. Add failing tests for requested/effective resolution, invalid selector rejection,
   admission-first selection, system observation without persistence/revision churn, total
   six-palette selection, contrast/distinctness, and dark palette compatibility.
2. Run the focused desktop tests and confirm the expected failures.
3. Implement requested/effective enums, the complete selection, observation handling, and
   exact light palettes without adding retained collections or resources.
4. Re-run focused tests to green.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_style_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test skin_palette_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_color_scheme_contract --locked
```

## Task 3: Reliable state and application latest-only persistence

**Files:**

- Modify: `crates/desktop/src/reliable_state.rs`
- Modify: `crates/app/src/command.rs`
- Modify: `crates/app/src/state.rs`
- Modify: `crates/app/src/operation.rs` only if its existing complete payload type requires
  a field-access update
- Modify the nearest existing inline/application presentation tests

1. Add failing tests that complete triples cross reliable state and application mapping,
   stale completions cannot replace a newer visible selection, and 10,000 mixed switches
   retain one active plus at most one latest pending payload.
2. Run the narrow app/desktop tests and confirm the expected failures.
3. Map each enum independently, construct one complete selection, and reuse the sole
   replaceable worker/payload path.
4. Re-run the focused tests to green.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-app presentation --locked
cargo +1.97.0 test -p tokenmaster-desktop reliable_state --locked
```

## Task 4: Reactive Slint bridge and Settings control

**Files:**

- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/ui/views/settings-view.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/presentation_contract.rs`
- Modify: `crates/desktop/tests/presentation_skin_ui_contract.rs` only for complete-payload
  assertions shared with the new axis
- Add: `crates/desktop/tests/presentation_color_scheme_ui_contract.rs`

1. Add failing compiled-UI tests for the three-entry selector, requested/effective keys,
   exact callback wiring, palette-before-metadata application, system observation, and
   10,000 mixed-axis switches.
2. Run the focused UI tests and confirm the expected failures.
3. Import Slint `Palette`, expose the bounded system observation, wire callbacks, and add
   the Settings selector. Do not derive palette colors in Slint.
4. Re-run the focused UI tests to green.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_color_scheme_ui_contract --locked
```

## Task 5: Contract audits, documentation, review, and final gates

**Files:**

- Modify only affected anchors in `scripts/audit-application-composition.ps1` and the
  existing presentation UI/source audit
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/PROJECT_HISTORY.md`

1. Update only version/complete-triple/system-observation anchors required by the product
   change and run those focused audits once.
2. Run one independent read-only implementation review. Fix only demonstrated product,
   security, data-loss, or required acceptance-evidence defects with focused reproducers.
3. Run affected crate tests, then the baseline once:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

4. Reconcile product state, audit/evidence state, release blockers, and Git state in the
   required docs without writing a current commit hash.
5. Run one final read-only re-review, `git diff --check`, task-owned process/artifact audit,
   commit intentionally, and prove the final worktree clean.

## Completion boundary

P4-D completes only independent built-in color schemes. Layouts, locale, typography/row
sizing, external skins, interactive accessibility/DPI/paint/resource evidence, P5, P6,
M0, package/signing/soak, and release remain open. A green developer baseline is not a
release acceptance receipt.
