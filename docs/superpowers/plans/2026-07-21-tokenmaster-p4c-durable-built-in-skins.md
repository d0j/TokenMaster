# TokenMaster P4-C Durable Built-in Skins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver three instant, durable, bounded production skin families while preserving one presentation owner, exact legacy settings/package compatibility, constant runtime capacity, and the existing Rust/Slint/SQLite authority boundaries.

**Architecture:** Extend the complete presentation selection from density-only to `{ density, skin }` across settings schema v3, typed packages, the application command worker, the sole Desktop presentation owner, and one root Slint palette value. Rust owns three immutable palettes; Slint only aliases semantic roles. Every UI request and persistence payload carries the complete selection, preventing mixed-axis lost updates.

**Tech Stack:** Rust 1.97.0, Slint 1.17.1, serde JSON, bundled SQLite, PowerShell 7/Pester 5.7, Cargo locked workspace.

## Global Constraints

- Follow `AGENTS.md` and read normative sources in its declared order before changing behavior.
- Implement exactly the approved design in `docs/superpowers/specs/2026-07-21-tokenmaster-p4c-durable-built-in-skins-design.md`.
- Work from branch `cx/tokenmaster-product-architecture`; do not rewrite or rebase shared history.
- Use one writer at a time in the shared worktree. Workers must not revert other edits.
- Use `C:\code\.tokenmaster-target\p4c` as `CARGO_TARGET_DIR`; never create a root `target` directory.
- Make every behavior change RED before GREEN. Record the exact failing assertion before implementation.
- Keep all presentation input, state, packages, models, and retained memory bounded.
- Add no dependency, process, thread, timer, polling loop, async runtime, channel, queue, cache, filesystem access, network access, SQL, unsafe code, additional window, or palette-specific heap allocation.
- Keep `Refined`, `Graphite`, and `Ember` as stable skin identities. Do not infer identities from the M0 probe.
- Keep skin and future `system|light|dark` color scheme as separate axes.
- Keep external skin files, inheritance, hot reload, provider plugins, CLI, MCP, and Hermes out of P4-C.
- Never persist or expose prompts, responses, reasoning, commands, source contents, credentials, raw incomplete lines, or absolute user paths.
- Do not put a current commit hash in tracked documentation.
- P4-C is developer evidence only; it cannot claim full P4, M0, package, signing, soak, or release acceptance.

## Task 0: Preflight and exact baseline

**Files:**

- Verify: `scripts/audit-clean-root.ps1`
- Verify: `scripts/audit-application-composition.ps1`
- Verify: `scripts/tests/audit-application-composition.Tests.ps1`
- Verify: `docs/superpowers/specs/2026-07-21-tokenmaster-p4c-durable-built-in-skins-design.md`

- [ ] **Step 1: Prove the branch identity and clean worktree**

Run:

```powershell
git branch --show-current
git status --short --branch
git rev-parse HEAD
git diff --check
```

Expected: branch `cx/tokenmaster-product-architecture`, no untracked or modified files, and no whitespace errors.

- [ ] **Step 2: Prove the repaired application composition gate**

Run:

```powershell
pwsh -NoProfile -File scripts\audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
Invoke-Pester -Path scripts\tests\audit-application-composition.Tests.ps1 -Output Detailed
```

Expected: source receipt `result=pass`; 85 tests pass and zero fail.

- [ ] **Step 3: Establish the isolated build target**

Run:

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
```

Expected: `TM-CLEAN-PASS`; no root `target` appears.

---

## Task 1: Settings schema v3 and exact v1/v2 migration

**Files:**

- Modify: `crates/state/src/settings/value.rs`
- Modify: `crates/state/src/settings/migration.rs`
- Modify: `crates/state/src/settings/mod.rs`
- Modify: `crates/app/src/state.rs`
- Modify: `crates/app/src/state_tests.rs`
- Test: `crates/state/tests/settings_contract.rs`
- Test: `crates/state/tests/restore_contract.rs`

- [ ] **Step 1: Write failing schema-v3 value tests**

Add tests named `presentation_skin_serialization_contract` and
`settings_schema_v3_contract` proving:

- `SETTINGS_SCHEMA_VERSION == 3`;
- `PresentationSkin` serializes exactly `refined|graphite|ember`;
- `PresentationSettings` has exactly `density` and `skin`;
- `PresentationSettings::refined()` is Comfortable plus Refined;
- unknown, missing, duplicate, or malformed skin fields reject;
- canonical JSON is stable and does not add top-level/device fields.

Run:

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-state --test settings_contract --locked presentation_skin_serialization_contract -- --exact
cargo +1.97.0 test -p tokenmaster-state --test settings_contract --locked settings_schema_v3_contract -- --exact
```

Expected RED: missing `PresentationSkin`, schema version still 2, and v3 payload rejected.

- [ ] **Step 2: Implement the bounded state values**

Implement:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationSkin {
    Refined,
    Graphite,
    Ember,
}

pub struct PresentationSettings {
    density: PresentationDensity,
    skin: PresentationSkin,
}
```

Expose only checked constructors/getters required by existing state/app code:

```rust
pub const fn new(density: PresentationDensity, skin: PresentationSkin) -> Self;
pub const fn refined() -> Self;
pub const fn density(self) -> PresentationDensity;
pub const fn skin(self) -> PresentationSkin;
```

Do not add `Default` if it could hide explicit migration/default decisions.
Update every existing `PresentationSettings::new` caller in the same task. The
temporary density command must preserve `current.presentation().skin()` rather than
resetting an already durable non-Refined skin. Test fixtures must pass an explicit
skin. The task may not commit a workspace that fails to compile.

- [ ] **Step 3: Write failing v1/v2/v3 dispatch tests**

Prove:

- v1 retains reminder/backup truth and defaults Comfortable plus Refined in memory;
- v2 retains density and defaults only skin to Refined;
- v3 retains both values;
- versions 0 and 4+ reject;
- decode performs no startup write;
- next successful mutation writes canonical v3;
- A/B fallback and downgrade rejection remain exact.

Expected RED: migration currently supports only v1/v2 and has no isolated v2 wire type.

- [ ] **Step 4: Implement isolated legacy wire types**

Keep exact private wire types for schema v1 and v2. Do not deserialize legacy JSON into the current v3 struct. Dispatch by version, migrate in memory, and preserve `source_version` separately from current canonical version.

- [ ] **Step 5: Run state schema GREEN gate**

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-state --test settings_contract --locked
cargo +1.97.0 test -p tokenmaster-state --lib --locked settings
cargo +1.97.0 test -p tokenmaster-state --test restore_contract --locked
cargo +1.97.0 test -p tokenmaster-app --lib --locked state_tests
```

Expected: all selected state tests pass.

- [ ] **Step 6: Commit Task 1**

```powershell
git add -- crates/state/src/settings/value.rs crates/state/src/settings/migration.rs crates/state/src/settings/mod.rs crates/state/tests/settings_contract.rs crates/state/tests/restore_contract.rs crates/app/src/state.rs crates/app/src/state_tests.rs
git commit -m "feat(state): add durable presentation skin"
```

---

## Task 2: Typed config, backup, and restore compatibility

**Files:**

- Modify: `crates/state/src/settings/preview.rs`
- Modify: `crates/state/src/package/manifest.rs`
- Modify: `crates/state/src/package/reader.rs`
- Test: `crates/state/tests/package_contract.rs`
- Test: `crates/state/tests/package_adversarial_contract.rs`
- Test: `crates/state/tests/restore_contract.rs`
- Modify: `crates/state/tests/package_support/mod.rs`
- Modify: `scripts/audit-reliable-state.ps1`
- Modify: `scripts/tests/audit-reliable-state.Tests.ps1`
- Modify: `scripts/audit-backup-package.ps1`
- Modify: `scripts/tests/audit-backup-package.Tests.ps1`

- [ ] **Step 1: Write failing package-version tests**

Add exact fixtures for source versions 1, 2, and 3. Prove:

- manifests admit only settings source versions 1..=3;
- entry source version must equal manifest source version;
- v1/v2 previews keep their original source version and migrated complete selection;
- v3 config/backup round-trips all three skins;
- resealing a changed manifest or entry version rejects;
- version 0/4+, duplicate fields, unknown skin, missing skin, and one-bit corruptions reject;
- confirmed import/restore replaces density and skin together;
- cancelled preview and data-only restore cannot change either axis.

Run:

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-state --test package_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test package_adversarial_contract --locked
```

Expected RED: v3 source version is outside the current accepted range and fixtures lack skin.

- [ ] **Step 2: Implement package compatibility without raw authority**

Extend only the existing typed settings/package path. Preserve manifest/entry equality, verified-source binding, streaming bounds, encryption behavior, and no-extraction guarantees. Do not add generic JSON/package writers.

- [ ] **Step 3: Extend mutation audits RED then GREEN**

First add audit mutation tests that remove the skin field, widen schema ranges, bypass source-version equality, or accept unknown versions. Observe each fail against the old audits. Then update exact anchors and counts to schema v3.

Run:

```powershell
Invoke-Pester -Path scripts\tests\audit-reliable-state.Tests.ps1,scripts\tests\audit-backup-package.Tests.ps1 -Output Detailed
pwsh -NoProfile -File scripts\audit-reliable-state.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
pwsh -NoProfile -File scripts\audit-backup-package.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
```

- [ ] **Step 4: Run package/restore GREEN gate**

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-state --test package_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test package_adversarial_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test restore_contract --locked
```

- [ ] **Step 5: Commit Task 2**

Stage only Task 2 files and commit:

```powershell
git add -- crates/state/src/settings/preview.rs crates/state/src/package/manifest.rs crates/state/src/package/reader.rs crates/state/tests/package_contract.rs crates/state/tests/package_adversarial_contract.rs crates/state/tests/restore_contract.rs crates/state/tests/package_support/mod.rs scripts/audit-reliable-state.ps1 scripts/tests/audit-reliable-state.Tests.ps1 scripts/audit-backup-package.ps1 scripts/tests/audit-backup-package.Tests.ps1
git commit -m "test(state): bind skin package compatibility"
```

---

## Task 3: Immutable production skin and complete presentation owner

**Files:**

- Create: `crates/desktop/src/skin.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/presentation_style.rs`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/ui/tokens.slint`
- Test: `crates/desktop/tests/skin_palette_contract.rs`
- Test: `crates/desktop/tests/presentation_style_contract.rs`
- Create: `crates/desktop/tests/presentation_skin_ui_contract.rs`

- [ ] **Step 1: Write failing skin identity/palette tests**

Define tests for:

- exact keys and indices: Refined/`refined`/0, Graphite/`graphite`/1, Ember/`ember`/2;
- invalid indices reject without fallback;
- exactly fifteen semantic roles per palette;
- exact RGB values from the approved design;
- minimum meaningful foreground/surface contrast greater than 6.8:1;
- values are `Copy`, fixed-size, path-free, serialization-free, and contain no strings;
- no M0 probe dependency or inferred probe theme identity.

In `presentation_skin_ui_contract.rs`, add RED compiled/source contracts before
changing `tokens.slint`. They must prove one exported `UiPalette` value contains all
fifteen roles, the existing `UiTokens` role properties derive from that value, and
Slint contains no named family table or palette-selection branch. Selector and Rust
application tests are added to the same file later in Task 5.

Run:

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-desktop --test skin_palette_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_skin_ui_contract --locked
```

Expected RED: `DesktopSkin`, `DesktopRgb`, and `DesktopColorTokens` do not exist.

- [ ] **Step 2: Implement immutable palette DTOs**

Use fixed values such as:

```rust
pub struct DesktopRgb { red: u8, green: u8, blue: u8 }
pub struct DesktopColorTokens {
    background: DesktopRgb,
    surface: DesktopRgb,
    surface_raised: DesktopRgb,
    surface_subtle: DesktopRgb,
    border: DesktopRgb,
    text_primary: DesktopRgb,
    text_secondary: DesktopRgb,
    accent: DesktopRgb,
    accent_subtle: DesktopRgb,
    accent_secondary: DesktopRgb,
    accent_tertiary: DesktopRgb,
    ready: DesktopRgb,
    waiting: DesktopRgb,
    degraded: DesktopRgb,
    unavailable: DesktopRgb,
}
pub enum DesktopSkin { Refined, Graphite, Ember }
```

Provide `const` constructors/getters, stable key/index conversion, and `DesktopSkin::color_tokens()`. Keep Slint generated types out of `skin.rs`.

- [ ] **Step 3: Write failing complete-selection style tests**

Replace density-only expectations with `DesktopPresentationSelection { density, skin }`. Prove:

- one current and one persisted complete selection;
- one checked revision for either-axis mutation;
- selecting the same complete value retries without revision advance;
- invalid density/skin index and revision overflow preserve all fields;
- Saving/Saved/NotSaved apply to the complete selection;
- stale success/failure/cancel cannot confirm or overwrite newer mixed-axis selection;
- config import and portable restore override both axes atomically;
- data-only restore and unrelated projection leave both axes unchanged.

- [ ] **Step 4: Implement the sole complete presentation owner**

Keep one `DesktopPresentationStyle`. Do not create a second skin state, callback owner, or palette cache. Admission closures receive the complete resulting selection.

Update the existing `ui.rs` constructor call in this same task so the workspace never
depends on a removed density-only style constructor. Until Task 4 projects durable
skin through `DesktopPresentationSettings`, construct the complete initial selection
explicitly from the projected density plus `DesktopSkin::Refined`. This is a temporary
call-site value, not a compatibility constructor or a second defaulting API. Task 4
must replace it with the complete persisted projection before its commit.

- [ ] **Step 5: Convert Slint tokens to one palette input**

Export one `UiPalette` struct with the fifteen roles. Give `UiTokens` one complete palette property and keep existing role names as aliases. Do not put `Refined|Graphite|Ember` tables or selection branches in Slint.

- [ ] **Step 6: Run Desktop model GREEN gate**

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-desktop --test skin_palette_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_style_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_skin_ui_contract --locked
```

- [ ] **Step 7: Commit Task 3**

```powershell
git add -- crates/desktop/src/skin.rs crates/desktop/src/lib.rs crates/desktop/src/presentation_style.rs crates/desktop/src/ui.rs crates/desktop/ui/tokens.slint crates/desktop/tests/skin_palette_contract.rs crates/desktop/tests/presentation_style_contract.rs crates/desktop/tests/presentation_skin_ui_contract.rs
git commit -m "feat(ui): own immutable built-in skins"
```

---

## Task 4: Full presentation command and latest-complete worker payload

**Files:**

- Modify: `crates/desktop/src/reliable_state.rs`
- Modify: `crates/app/src/command.rs`
- Modify: `crates/app/src/command_tests.rs`
- Modify: `crates/app/src/state.rs`
- Modify: `crates/app/src/state_tests.rs`
- Modify: `crates/app/src/operation.rs`
- Modify: `crates/app/src/operation_tests.rs`
- Modify: `crates/app/src/application.rs`
- Modify: `crates/app/src/application_tests.rs`
- Modify: `crates/desktop/tests/reliable_state_projection_contract.rs`
- Modify: `crates/desktop/src/ui.rs`

- [ ] **Step 1: Write failing full-payload tests**

Replace density-only command assertions with:

```rust
ApplicationCommand::UpdatePresentation
ApplicationOperationPayload::Presentation(ApplicationPresentationUpdate)
DesktopIntent::UpdatePresentation(DesktopPresentationSelection)
```

Prove the payload contains both axes, has redacted `Debug`, admits no partial constructor, and maps all nine density/skin combinations exactly. Add a failing
`reliable_state_projection_contract.rs` regression proving that a non-Refined durable
skin reaches the Desktop complete selection and initial UI owner.

- [ ] **Step 2: Implement full command conversion**

Map Desktop density/skin to state density/skin through explicit exhaustive matches. Avoid numeric/transmute/serde shortcuts across crate boundaries. Replace Task 3's
temporary Refined UI construction with the complete persisted
`DesktopPresentationSettings` selection.

- [ ] **Step 3: Write failing worker/coalescing tests**

Prove:

- presentation remains replaceable/latest-wins;
- at most one active plus one latest pending complete payload exists;
- a 10,000 mixed-axis burst publishes the last complete pair;
- no persisted pair combines density from one request with skin from another;
- cancel, shutdown, panic, retry, and terminal publication return to bounded idle state.

- [ ] **Step 4: Generalize the existing worker slot**

Rename density-only symbols to complete-presentation symbols. Reuse the same coordinator and operation worker. Do not add another wake, payload slot, or thread.

- [ ] **Step 5: Write failing durable-state mutation tests**

Prove `update_presentation`:

- returns early only for exact complete durable equality;
- preserves reminder, backup, and device settings;
- crosses the existing irreversible boundary before the one save;
- writes canonical schema v3 once;
- reports failure without fabricating durable truth.

- [ ] **Step 6: Implement one complete settings mutation**

Reconstruct portable settings from existing typed getters and the new complete presentation value. Keep application/state authority boundaries unchanged.

- [ ] **Step 7: Run application GREEN gate**

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-app --lib --locked command_tests
cargo +1.97.0 test -p tokenmaster-app --lib --locked state_tests
cargo +1.97.0 test -p tokenmaster-app --lib --locked operation_tests
cargo +1.97.0 test -p tokenmaster-app --lib --locked application_tests
cargo +1.97.0 test -p tokenmaster-desktop --test reliable_state_projection_contract --locked
```

- [ ] **Step 8: Commit Task 4**

```powershell
git add -- crates/desktop/src/reliable_state.rs crates/desktop/src/ui.rs crates/desktop/tests/reliable_state_projection_contract.rs crates/app/src/command.rs crates/app/src/command_tests.rs crates/app/src/state.rs crates/app/src/state_tests.rs crates/app/src/operation.rs crates/app/src/operation_tests.rs crates/app/src/application.rs crates/app/src/application_tests.rs
git commit -m "feat(app): persist complete presentation selection"
```

---

## Task 5: One-palette Slint application and Skin selector

**Files:**

- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/ui/views/settings-view.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/presentation_density_ui_contract.rs`
- Modify: `crates/desktop/tests/presentation_skin_ui_contract.rs`

- [ ] **Step 1: Write failing compiled-UI contract tests**

Prove:

- Settings has separate fixed Density and Skin selectors;
- labels are `Refined`, `Graphite`, `Ember` English fallbacks;
- both selectors submit the complete current selection;
- invalid indices do not call admission or mutate UI state;
- one root `UiPalette` assignment changes all fifteen semantic roles;
- the same `MainWindow`, route selection, product models, and geometry remain;
- the existing persistence label represents the complete presentation selection;
- 10,000 mixed density/skin switches end at the expected complete pair;
- no UI-thread I/O/query/scan/timer/worker/window/cache appears.
- palette assignment occurs before skin/density/revision metadata and before the
  production window can be shown;
- no `invoke_from_event_loop`, timer, callback, await, or other event-loop yield can
  occur between assigning the complete palette and the remaining presentation
  metadata.

Expected RED: no skin property/callback/model and no palette setter exist.

- [ ] **Step 2: Add root skin bindings and selector**

Expose one stable skin index/key on `MainWindow`, add one `select-presentation-skin(int)` callback, and add a fixed ComboBox in Settings. Keep the English strings local fallback only; do not create the locale system in this task.

- [ ] **Step 3: Apply one immutable palette from Rust**

Convert `DesktopRgb` to `slint::Color::from_rgb_u8` at the UI boundary and construct one generated `UiPalette`. Assign the whole palette before updating root presentation metadata and before showing the window. Do not set fifteen unrelated global fields from multiple callbacks.

- [ ] **Step 4: Wire admission-before-apply for both selectors**

Each callback derives a complete selection from the sole style owner, submits it once, then applies the optimistic complete selection only when admitted. Release the style mutex before calling the sink or Slint setters.

- [ ] **Step 5: Reconcile durable/import/restore truth**

On accepted reliable state, reconcile the complete selection once. Preserve newer optimistic revisions on stale terminal events and atomically override both axes only for confirmed config import/portable restore.

- [ ] **Step 6: Run compiled UI GREEN gate**

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_density_ui_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_skin_ui_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
```

- [ ] **Step 7: Commit Task 5**

```powershell
git add -- crates/desktop/ui/main.slint crates/desktop/ui/views/settings-view.slint crates/desktop/src/ui.rs crates/desktop/tests/presentation_density_ui_contract.rs crates/desktop/tests/presentation_skin_ui_contract.rs
git commit -m "feat(ui): switch durable built-in skins"
```

---

## Task 6: Exact audits and authoritative documentation

**Files:**

- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify: `scripts/audit-application-composition.ps1`
- Modify: `scripts/tests/audit-application-composition.Tests.ps1`
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/CHANGELOG.md`

- [ ] **Step 1: Add audit mutation tests before audit changes**

Mutations must reject:

- a fourth skin or changed stable key/index;
- missing/extra semantic role or changed exact palette value;
- a Slint-owned named family table;
- second presentation owner/palette slot/callback;
- partial density-only or skin-only application payload;
- UI mutation before admission;
- event-loop scheduling/yield between complete palette assignment and presentation
  metadata, or any window-show path before the first Rust palette application;
- schema range widened beyond 1..=3;
- missing v2 migration default;
- added worker/thread/timer/queue/channel/cache/window/query/SQL/filesystem/network/unsafe authority.

- [ ] **Step 2: Update source audits to exact P4-C contracts**

Keep lexical scanning robust to raw identifiers, Unicode escapes, comments, and multiline Rust signatures. Do not rely only on a global occurrence count when a specific owner/binding can be pinned.

- [ ] **Step 3: Run audit GREEN gate**

```powershell
pwsh -NoProfile -File scripts\audit-reliable-state.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
pwsh -NoProfile -File scripts\audit-backup-package.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
pwsh -NoProfile -File scripts\audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
pwsh -NoProfile -File scripts\audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
Invoke-Pester -Path scripts\tests\audit-reliable-state.Tests.ps1,scripts\tests\audit-backup-package.Tests.ps1,scripts\tests\audit-desktop-shell.Tests.ps1,scripts\tests\audit-application-composition.Tests.ps1 -Output Detailed
```

- [ ] **Step 4: Update normative and operational truth**

Record schema v3, the complete presentation contract, exact skin keys/palettes, one owner, performance/security exclusions, test evidence, and remaining P4/P5/P6/M0/release work. Keep P4-C `partial` where the parent requirement still includes layouts/schemes/locales/interactive acceptance.

- [ ] **Step 5: Run documentation consistency scan**

```powershell
rg -n "schema v2|schema-v2|density-only|UpdatePresentationDensity|PresentationDensity\(" spec docs scripts crates
rg -n "TBD|TODO|FIXME|maybe|likely|implement later" docs\superpowers\plans\2026-07-21-tokenmaster-p4c-durable-built-in-skins.md spec docs
git diff --check
```

Every remaining v2/density-only match must describe a legacy format, historical event, or deliberately deferred boundary.

- [ ] **Step 6: Commit Task 6**

```powershell
git add -- scripts/audit-desktop-shell.ps1 scripts/tests/audit-desktop-shell.Tests.ps1 scripts/audit-application-composition.ps1 scripts/tests/audit-application-composition.Tests.ps1 spec/SPECIFICATION.md spec/DATA_CONTRACT.md spec/API_CONTRACT.md spec/SECURITY.md spec/TRACEABILITY.md spec/DECISIONS.md docs/CURRENT_STATE.md docs/HANDOFF.md docs/ROADMAP.md docs/FEATURE_PARITY.md docs/PROJECT_HISTORY.md docs/CHANGELOG.md
git commit -m "docs: record durable built-in skin contract"
```

---

## Task 7: Full verification, independent review, and cleanup

- [ ] **Step 1: Run the focused product gate**

```powershell
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 test -p tokenmaster-state --locked
cargo +1.97.0 test -p tokenmaster-desktop --locked
cargo +1.97.0 test -p tokenmaster-app --locked
```

- [ ] **Step 2: Run the exact workspace baseline sequentially**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
$env:CARGO_TARGET_DIR = 'C:\code\.tokenmaster-target\p4c'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Do not overlap full Cargo commands. Capture exit codes and counts. Ignored tests are not accepted evidence.

- [ ] **Step 3: Run all P4-C source/mutation audits**

Repeat the four source-only audits and four Pester suites from Task 6 against the final committed tree.

- [ ] **Step 4: Obtain independent read-only review**

The reviewer inspects the design parent through P4-C HEAD for:

- schema/package compatibility and data loss;
- mixed-axis races and stale reconciliation;
- memory/resource growth;
- UI-thread blocking and paint atomicity;
- security/authority expansion;
- source-audit bypasses;
- public/API compatibility;
- documentation overclaims.

Acceptance: Critical 0 and Important 0. Fix or explicitly record every Minor, then rerun affected gates.

- [ ] **Step 5: Final identity and process audit**

```powershell
git status --short --branch
git diff --check
Get-CimInstance Win32_Process | Where-Object {
    $_.ExecutablePath -like 'C:\code\.tokenmaster-target\p4c*' -or
    $_.CommandLine -like '*tokenmaster-target\p4c*'
} | Select-Object ProcessId, ParentProcessId, Name, ExecutablePath, CommandLine
```

Expected: clean tree and no task-owned process.

- [ ] **Step 6: Remove only task-owned build output**

Resolve `C:\code\.tokenmaster-target\p4c`, prove its parent is exactly
`C:\code\.tokenmaster-target`, and clean only that child after the process audit:

```powershell
cargo +1.97.0 clean --target-dir 'C:\code\.tokenmaster-target\p4c'
```

Never clean the shared parent or another task's target. An empty target directory may
remain; zero files and zero retained build bytes are the cleanup acceptance condition.

- [ ] **Step 7: Re-run post-cleanup truth checks**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
git status --short --branch
Test-Path 'C:\code\tokenmaster\target'
$remaining = Get-ChildItem -LiteralPath 'C:\code\.tokenmaster-target\p4c' -File -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum
$remaining | Select-Object Count, Sum
```

Expected: clean root/worktree, root target path is `False`, and P4-C target reports
`Count=0` with no retained bytes.

## Completion boundary

P4-C is complete only after every checkbox above has direct evidence from the final committed tree, independent review is Critical/Important 0/0, the worktree is clean, and task-owned processes/artifacts are gone. Completion advances production built-in skins only. Layouts, color scheme, locale, typography/row sizing, interactive accessibility/DPI/paint/resource acceptance, external skins, P5, P6, M0, package/signing/soak, and release remain open.
