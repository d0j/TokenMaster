# TokenMaster P4-B Portable Presentation Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist and import/export the production density axis through strict settings schema version 2 while preserving version-1 records/packages, same-window hot switching, bounded memory, and off-UI-thread reliable-state I/O.

**Architecture:** `tokenmaster-state` owns strict portable `PresentationSettings` and exact v1-to-v2 migration; the typed package reader binds each settings entry to the manifest-declared source schema. `tokenmaster-app` maps durable density to/from the presentation boundary and reuses the existing replaceable application operation slot. `tokenmaster-desktop` applies an admitted selection immediately, retains one current/persisted style state, and reconciles newest reliable-state projections without rolling an unsaved newer selection back.

**Tech Stack:** Rust 1.97.0, edition 2024, Serde strict JSON, existing SHA-256 A/B record store, existing typed Zstandard package v1 container, Slint 1.17.1 software renderer, PowerShell/Pester deterministic audits.

**Design:** `docs/superpowers/specs/2026-07-21-tokenmaster-p4b-portable-presentation-settings-design.md`

## Global Constraints

- Settings schema version 2 adds only the already implemented density axis; no placeholder skin, layout, color-scheme, locale, provider, pricing, notification-delivery, path, credential, prompt, response, command, or source-content field is permitted.
- Stable density keys are exactly `comfortable`, `compact`, and `ultra_compact`; their Slint indices remain exactly `0`, `1`, and `2`.
- Version-1 records and portable candidates migrate strictly in memory to comfortable density; startup performs no hidden settings write.
- New records and packages are canonical schema version 2. Versions 0 and 3 or newer fail as unsupported, not as defaults or partial values.
- Package container/manifest version remains 1. Readers accept settings entry schema 1 or 2 only and require the entry source schema to equal the manifest field.
- Portable import/restore preserves device-local route state. Reminder and backup updates preserve presentation; presentation updates preserve every other settings class.
- A supported density switch performs no serialization, filesystem/SQLite access, source scan, query, product-model replacement, window recreation, renderer change, or UI-thread blocking I/O.
- Repeated saves reuse the existing single operation worker and replaceable latest-payload slot. No new thread, channel, queue, timer, watcher, cache, database table, dependency, or background daemon is permitted.
- Retained presentation state is one visible density, one persisted density, one checked `u64` revision, and one three-value persistence status; no history or request payload collection is permitted.
- An admitted selection applies synchronously. Rejected/invalid/exhausted selection leaves visible density/revision unchanged. Save failure retains valid visible density as `not_saved`; only `saved` truth survives restart.
- Existing software-renderer, one-window, privacy, bounded model, reliable-state, backup, recovery, and downgrade-protection contracts remain binding.
- Work only on `cx/tokenmaster-product-architecture`; do not push, package, sign, claim M0 acceptance, or claim a release.
- Use `C:\code\.tokenmaster-target\p4b` for Cargo output and delete only that verified task-owned directory after all evidence and reviews are complete.

---

### Task 1: Strict settings schema v2 and semantic v1 migration

**Files:**
- Modify: `crates/state/src/settings/value.rs`
- Modify: `crates/state/src/settings/migration.rs`
- Modify: `crates/state/src/settings/preview.rs`
- Modify: `crates/state/src/settings/mod.rs`
- Modify: `crates/state/src/lib.rs`
- Modify: `crates/state/tests/settings_contract.rs`
- Modify: settings-construction call sites reported by `rg -n "PortableSettings::new" crates`

**Interfaces:**
- Produces: `SETTINGS_SCHEMA_VERSION: u16 = 2` and crate-private `MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION: u16 = 1`.
- Produces: `PresentationDensity::{Comfortable,Compact,UltraCompact}` with strict snake-case Serde and `stable_key()`.
- Produces: `PresentationSettings::new(PresentationDensity)`, `comfortable()`, and `density()`.
- Changes: `PortableSettings::new(ReminderPolicy, BackupPolicy, PresentationSettings)` requires all owned groups.
- Produces: `DecodedPortableSettings { portable, source_schema_version }` inside the settings module.
- Adds: `SettingsChangeCategory::Presentation` after the existing three categories.

- [ ] **Step 1: Write failing exact schema and migration tests**

Add focused tests to `crates/state/tests/settings_contract.rs` with these assertions:

```rust
#[test]
fn settings_schema_v2_serializes_only_owned_portable_presentation() {
    let defaults = SettingsValue::safe_defaults();
    let encoded = serde_json::to_value(&defaults).expect("settings json");
    assert_eq!(encoded["schema_version"], 2);
    assert_eq!(encoded["portable"]["presentation"]["density"], "comfortable");
    assert!(encoded["portable"].get("skin").is_none());
    assert!(encoded["portable"].get("layout").is_none());
    assert!(encoded["portable"].get("color_scheme").is_none());
    assert!(encoded["portable"].get("locale").is_none());
}

#[test]
fn schema_v1_record_migrates_in_memory_and_explicit_save_writes_v2() {
    let (root, directory) = fixture();
    let payload = serde_json::to_vec(&legacy_v1_settings_json("projects"))
        .expect("legacy settings");
    std::fs::write(root.path().join("settings-a.tms"), encode_record(7, &payload))
        .expect("legacy record");
    let store = SettingsStore::new(&directory).expect("settings store");
    let loaded = store.load().expect("migrated load");
    assert_eq!(loaded.generation(), Some(7));
    assert_eq!(loaded.value().device().last_route(), DeviceRoute::Projects);
    assert_eq!(
        loaded.value().portable().presentation().density(),
        PresentationDensity::Comfortable
    );
    store.save(loaded.value()).expect("explicit v2 save");
    let newest = store.load().expect("v2 reread");
    assert_eq!(newest.generation(), Some(8));
    assert_eq!(newest.value(), loaded.value());
}

#[test]
fn portable_v1_migration_has_canonical_v2_digest_and_preview_category() {
    let (_root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    let legacy = legacy_v1_portable_json();
    let migrated = store.preview_import(&legacy).expect("legacy preview");
    assert_eq!(migrated.changed_category_count(), 0);
    let receipt = store.commit_import(&migrated).expect("migrated commit");
    let canonical = PortableSettingsCandidate::new(
        SettingsValue::safe_defaults().portable().clone()
    ).expect("canonical current candidate");
    assert_eq!(receipt.portable_digest(), canonical.digest());

    let candidate = PortableSettingsCandidate::new(PortableSettings::new(
        SettingsValue::safe_defaults().portable().reminders().clone(),
        SettingsValue::safe_defaults().portable().backup().clone(),
        PresentationSettings::new(PresentationDensity::Compact),
    )).expect("compact candidate");
    let preview = store.preview_candidate(candidate).expect("preview");
    assert_eq!(preview.categories(), &[SettingsChangeCategory::Presentation]);
    assert_eq!(preview.changed_field_count(), 1);
}
```

Also update the unsupported-version tests to use `schema_version: 3`, and add complete candidate/record cases for version `0`, version `3`, missing `presentation`, unknown `skin`, duplicate `presentation`, invalid density, and wrong density type. Preserve the existing two-invalid-slot and downgrade-no-overwrite assertions.

- [ ] **Step 2: Run RED and record the expected cause**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-state --test settings_contract settings_schema_v2_serializes_only_owned_portable_presentation --locked
cargo +1.97.0 test -p tokenmaster-state --test settings_contract schema_v1_record_migrates_in_memory_and_explicit_save_writes_v2 --locked
```

Expected: compilation fails because `PresentationDensity`, `PresentationSettings`, and schema-v2 accessors do not exist; the migration test must not be accepted as RED if it fails only because of malformed fixture encoding.

- [ ] **Step 3: Add the minimal current semantic types**

Implement in `value.rs`:

```rust
pub const SETTINGS_SCHEMA_VERSION: u16 = 2;
pub(crate) const MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationDensity {
    Comfortable,
    Compact,
    UltraCompact,
}

impl PresentationDensity {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
            Self::UltraCompact => "ultra_compact",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationSettings {
    density: PresentationDensity,
}

impl PresentationSettings {
    #[must_use]
    pub const fn new(density: PresentationDensity) -> Self { Self { density } }
    #[must_use]
    pub const fn comfortable() -> Self { Self::new(PresentationDensity::Comfortable) }
    #[must_use]
    pub const fn density(self) -> PresentationDensity { self.density }
}
```

Add `presentation: PresentationSettings` to `PortableSettings`; make its constructor require the value and add `presentation()`. Use comfortable in `safe_defaults()`. Re-export only the public semantic types from `settings/mod.rs` and `lib.rs`; expose the version constants only crate-internally where the package module needs them.

- [ ] **Step 4: Implement strict version dispatch without startup writes**

Move record/candidate source-version handling into `migration.rs`. Use separate `#[serde(deny_unknown_fields)]` v1 wire structs that contain only reminders/backup and separate current v2 wire structs. The dispatcher shape is exact:

```rust
pub(super) struct DecodedPortableSettings {
    pub(super) portable: PortableSettings,
    pub(super) source_schema_version: u16,
}

pub(super) fn decode_portable_candidate(
    bytes: &[u8],
) -> Result<DecodedPortableSettings, StateError> {
    enforce_payload_bound(bytes)?;
    let probe: VersionProbe = serde_json::from_slice(bytes)
        .map_err(|_| StateError::invalid_input())?;
    match probe.schema_version {
        1 => decode_portable_v1(bytes),
        SETTINGS_SCHEMA_VERSION => decode_portable_v2(bytes),
        _ => Err(StateError::unsupported_version()),
    }
}
```

Add the analogous complete-record dispatcher returning current `SettingsValue`. `RecordValue::decode_json` must map only unsupported versions to `RecordValueError::UnsupportedVersion`; malformed supported inputs remain `Invalid`. Do not save during decode/load.

`PortableSettingsCandidate` stores `source_schema_version` privately, computes its digest from `encode_portable` current-v2 bytes, and exposes `pub(crate) const fn source_schema_version(&self) -> u16` for the package reader. `new()` always sets source version 2.

- [ ] **Step 5: Make reconstruction sites preserve presentation**

Change every existing `PortableSettings::new` call to pass the current presentation value when reconstructing, or `PresentationSettings::comfortable()` only for an intentional fixture/default. The production backup and reminder update paths must use:

```rust
PortableSettings::new(
    current.value().portable().reminders().clone(),
    policy,
    *current.value().portable().presentation(),
)
```

and the symmetrical reminder update. Do not add a two-argument compatibility constructor that can silently reset density.

- [ ] **Step 6: Run GREEN, state regression, and strict focused Clippy**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-state --test settings_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test restore_contract --test automatic_recovery_contract --locked
$env:RUSTFLAGS='-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-state --all-targets --locked
```

Expected: all named tests pass; warnings are errors; no production caller can construct portable settings without an explicit presentation value.

- [ ] **Step 7: Commit Task 1**

```powershell
git add -- crates/state/src/settings crates/state/src/lib.rs crates/state/tests/settings_contract.rs crates/state/tests/restore_contract.rs crates/state/tests/automatic_recovery_contract.rs crates/app/src crates/app/tests
git diff --cached --check
git commit -m "feat(state): migrate portable settings to schema v2"
```

Stage only files actually changed by this task; do not use `git add .`.

### Task 2: Bind package manifest compatibility to the settings entry

**Files:**
- Modify: `crates/state/src/package/manifest.rs`
- Modify: `crates/state/src/package/reader.rs`
- Modify: `crates/state/tests/package_support/mod.rs`
- Modify: `crates/state/tests/package_contract.rs`
- Modify: `crates/state/tests/package_adversarial_contract.rs`

**Interfaces:**
- `Manifest` retains `settings_schema_version: u16` after decode.
- `Manifest::new(...)` always uses current settings schema version 2.
- Decode accepts only inclusive range `1..=SETTINGS_SCHEMA_VERSION`.
- Package reader requires `candidate.source_schema_version() == manifest.settings_schema_version`.
- Test support produces independent legacy settings-v1 config and backup bytes without exposing a production generic archive writer.

- [ ] **Step 1: Add failing package compatibility and mismatch tests**

Add tests equivalent to:

```rust
#[test]
fn container_v1_reads_settings_v1_and_writes_settings_v2() {
    let legacy = legacy_config_bytes_v1();
    assert_eq!(u16::from_le_bytes(legacy[46..48].try_into().unwrap()), 1);
    let verified = read_config_bytes(&legacy).expect("legacy config");
    let canonical: serde_json::Value = serde_json::from_slice(
        &verified.settings().encode_json().expect("canonical settings")
    ).expect("canonical settings json");
    assert_eq!(canonical["schema_version"], 2);
    assert_eq!(canonical["portable"]["presentation"]["density"], "comfortable");

    let (current, _) = config_bytes_at(PACKAGE_TIME);
    assert_eq!(u16::from_le_bytes(current[46..48].try_into().unwrap()), 2);
    assert!(read_config_bytes(&current).is_ok());
}

#[test]
fn manifest_and_entry_settings_schema_must_match() {
    let mismatch = package_with_settings_source_schema(1, current_v2_portable_json());
    assert_eq!(
        read_config_bytes(&mismatch).expect_err("schema mismatch").code(),
        StateErrorCode::Integrity
    );
}
```

Use manifest bytes `14..16` relative to the 40-byte manifest, which are absolute package bytes `46..48`. Add version 0 and 3 mutations that reseal descriptor/package digests and still return `UnsupportedVersion`. Add a legacy backup test proving the database payload and metadata survive while settings migrate.

- [ ] **Step 2: Run RED**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-state --test package_contract container_v1_reads_settings_v1_and_writes_settings_v2 --locked
cargo +1.97.0 test -p tokenmaster-state --test package_adversarial_contract manifest_and_entry_settings_schema_must_match --locked
```

Expected: old settings-schema packages are rejected by the current manifest equality gate and source-schema mismatch is not detected.

- [ ] **Step 3: Build independent legacy package fixtures in test support**

In `package_support/mod.rs`, add a test-only typed builder that accepts exact `settings_schema_version: u16`, strict settings JSON bytes, optional database bytes, and the existing fixed profile. It must independently:

1. encode the fixed 32-byte header and 40-byte manifest;
2. compress one settings frame with checksum, content size, window log 23, and pledged size;
3. write the exact prefix/suffix lengths and SHA-256;
4. optionally append the database entry;
5. compute descriptor binding, `TMEND001`, preceding-byte SHA-256, and complete bytes.

The helper stays under integration-test support and cannot be called by production. Use exact current constants copied as test-oracle values and assert each structural offset. Do not expose a raw writer from `tokenmaster-state`.

- [ ] **Step 4: Preserve and validate manifest settings version**

Replace the manifest-local hard-coded constant with the settings module constants and add the field:

```rust
pub(crate) struct Manifest {
    pub(crate) kind: PackageKind,
    pub(crate) entry_count: u8,
    pub(crate) settings_schema_version: u16,
    pub(crate) database_schema_version: u16,
    // existing fields
}
```

`new()` assigns current version 2. `decode()` reads the field, rejects values below 1 or above 2, validates all existing reserved/kind/count/database/purpose rules, and preserves the decoded value rather than normalizing it. In `reader.rs`, after the exact frame and outer package checks but before constructing `ParsedPackage`, require:

```rust
let settings = PortableSettingsCandidate::decode(&settings_bytes)?;
if settings.source_schema_version() != manifest.settings_schema_version {
    return Err(StateError::integrity());
}
```

- [ ] **Step 5: Refresh the current deterministic golden vector intentionally**

Rename the misleading `v1_config_golden_vector...` test to identify container v1 plus settings v2. Keep container/header/manifest version assertions at 1, add the manifest settings-version assertion at 2, and update only the deterministic length/SHA expected from the freshly generated canonical v2 payload. The separate legacy fixture test is the compatibility proof; do not relabel the old SHA as current.

- [ ] **Step 6: Run GREEN and the complete package/recovery regression**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-state --test package_contract --test package_adversarial_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test restore_contract --test recovery_resource_contract --test backup_performance_contract --locked
$env:RUSTFLAGS='-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-state --all-targets --locked
```

Expected: current and legacy settings packages pass; mismatches and unsupported versions fail closed; all restore/resource/package regressions pass.

- [ ] **Step 7: Commit Task 2**

```powershell
git add -- crates/state/src/package crates/state/tests/package_support crates/state/tests/package_contract.rs crates/state/tests/package_adversarial_contract.rs crates/state/tests/restore_contract.rs crates/state/tests/recovery_resource_contract.rs crates/state/tests/backup_performance_contract.rs
git diff --cached --check
git commit -m "feat(state): preserve settings package compatibility"
```

### Task 3: Pure desktop persisted-style projection and reconciliation

**Files:**
- Modify: `crates/desktop/src/presentation_style.rs`
- Modify: `crates/desktop/src/reliable_state.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/tests/presentation_style_contract.rs`
- Modify: `crates/desktop/tests/reliable_state_projection_contract.rs`

**Interfaces:**
- Produces: `DesktopPresentationPersistence::{Saved,Saving,NotSaved}` with stable codes `saved`, `saving`, `not_saved`.
- Produces: `DesktopPresentationSettings::new(DesktopDensity)`, `comfortable()`, and `density()`.
- Produces: `DesktopReliableStateSummary::new_with_settings(...)`; existing constructors delegate to comfortable presentation for compatibility fixtures.
- Extends: `DesktopPresentationStyle::from_persisted`, `select_density_index_if_admitted`, `observe_persisted`, `mark_not_saved`, and `apply_persisted_override`.

- [ ] **Step 1: Write failing pure state-machine tests**

Add these behavior cases without UI mocks:

```rust
#[test]
fn persistence_reconciliation_never_overwrites_a_newer_unsaved_selection() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);
    assert_eq!(
        style.select_density_index_if_admitted(1, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);
    assert_eq!(
        style.select_density_index_if_admitted(2, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.observe_persisted(DesktopDensity::Compact),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.density(), DesktopDensity::UltraCompact);
    style.mark_not_saved();
    assert_eq!(style.persistence(), DesktopPresentationPersistence::NotSaved);
    assert_eq!(
        style.observe_persisted(DesktopDensity::UltraCompact),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saved);
}

#[test]
fn explicit_import_override_is_atomic_and_checked() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Compact);
    style.select_density_index_if_admitted(2, |_| true);
    assert_eq!(
        style.apply_persisted_override(DesktopDensity::Comfortable),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.density(), DesktopDensity::Comfortable);
    assert_eq!(style.persisted_density(), DesktopDensity::Comfortable);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saved);
}
```

Add rejection tests proving the admission closure is called once only after fixed-index and revision validation, returns false without mutation, and revision overflow does not call it. Add a reliable-state test that projects `UltraCompact` through `new_with_settings` while legacy constructors default to comfortable.

- [ ] **Step 2: Run RED**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_style_contract --test reliable_state_projection_contract --locked
```

Expected: compilation fails for the new persistence and projection APIs.

- [ ] **Step 3: Implement constant-state reconciliation**

Extend the style value, without heap allocation, to:

```rust
pub struct DesktopPresentationStyle {
    density: DesktopDensity,
    persisted_density: DesktopDensity,
    revision: DesktopPresentationRevision,
    persistence: DesktopPresentationPersistence,
}
```

`select_density_index_if_admitted` validates index/equality/checked successor first, calls the provided admission closure exactly once with the proposed density, and assigns density/revision/`Saving` only on `true`. `observe_persisted` always updates `persisted_density`; it clears to `Saved` on equality, applies and revision-advances only when current status was `Saved`, and otherwise retains the newer visible unsaved density. `mark_not_saved` changes `Saving` to `NotSaved` only while visible and persisted differ. `apply_persisted_override` validates the checked successor before changing a different visible density, then assigns visible/persisted/status together.

Keep the existing `select_density_index` as a pure local compatibility method that uses the same validation/assignment and marks a changed unpersisted selection `NotSaved`; production wiring will use the admitted method.

- [ ] **Step 4: Add the typed reliable-state presentation value**

Add `DesktopPresentationSettings` to `DesktopReliableStateSummary`. Introduce `new_with_settings` containing backup policy, reminder policy, and presentation settings. Make `new_with_reminder_policy` delegate with `DesktopPresentationSettings::comfortable()`, and `new` continue delegating through it. Add `DesktopReliableStateProjection::presentation()`; `unavailable()` remains comfortable.

- [ ] **Step 5: Run GREEN and strict focused Clippy**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_style_contract --test reliable_state_projection_contract --locked
$env:RUSTFLAGS='-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-desktop --all-targets --locked
```

Expected: pure state and projection contracts pass with no new allocation, worker, timer, or dependency.

- [ ] **Step 6: Commit Task 3**

```powershell
git add -- crates/desktop/src/presentation_style.rs crates/desktop/src/reliable_state.rs crates/desktop/src/lib.rs crates/desktop/tests/presentation_style_contract.rs crates/desktop/tests/reliable_state_projection_contract.rs
git diff --cached --check
git commit -m "feat(ui): model persisted density state"
```

### Task 4: Off-thread application persistence and live Desktop hydration

**Files:**
- Modify: `crates/app/src/command.rs`
- Modify: `crates/app/src/operation.rs`
- Modify: `crates/app/src/state.rs`
- Modify: `crates/app/src/application.rs`
- Modify: `crates/app/src/operation_tests.rs`
- Modify: `crates/app/src/state_tests.rs`
- Modify: `crates/app/src/application_tests.rs`
- Modify: `crates/desktop/src/reliable_state.rs`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/ui/views/settings-view.slint`
- Modify: `crates/desktop/tests/presentation_density_ui_contract.rs`
- Modify: `crates/desktop/tests/reliable_state_projection_contract.rs`

**Interfaces:**
- Adds: `DesktopIntent::UpdatePresentationDensity(DesktopDensity)`.
- Adds: `ApplicationCommand::UpdatePresentationDensity` and `ApplicationOperationPayload::PresentationDensity(ApplicationPresentationDensityUpdate)`.
- Adds: `ApplicationOperationRequest::update_presentation_density`.
- Adds: `ApplicationStateOwner::update_presentation_density` preserving every other settings field.
- Adds: `DesktopOperationKind::{ApplyConfig,RestoreWithPortableSettings,UpdatePresentation}` so confirmed config import, portable-settings restore, and failed density persistence are distinguishable from preview/cancel, data-only restore, and reminder/backup policy operations.
- Slint adds read-only `presentation-persistence-state` and one accessible status label; no Save button or timer.

- [ ] **Step 1: Write failing worker coalescing and state preservation tests**

In `operation_tests.rs`, mirror the existing reminder replaceable-payload test with densities `Compact`, `UltraCompact`, `Comfortable`. Hold the first active execution, submit two follow-ups, and assert:

```rust
assert_eq!(snapshot.active_count(), 1);
assert_eq!(snapshot.pending_count(), 1);
assert_eq!(executed, vec![DesktopDensity::Compact, DesktopDensity::Comfortable]);
```

Add a second stress case that holds the first operation, submits 10,000 alternating
presentation updates, proves `active_count() == 1` and `pending_count() == 1` throughout,
then releases and proves only the first and final payload execute.

In `state_tests.rs`, start from ultra-compact settings, update backup and reminder policy, and assert density remains ultra-compact after each save. Then update density to compact and assert reminder, backup, and device route are byte-for-byte/typed equal to the pre-update values.

- [ ] **Step 2: Write failing startup/UI persistence tests**

Extend `presentation_density_ui_contract.rs` with an accepting recording sink and reliable-state fixtures:

```rust
#[test]
fn persisted_density_hydrates_before_show_and_admitted_switch_is_immediate() {
    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state_with_density(DesktopDensity::UltraCompact, None),
        sink.clone(),
    ).expect("shell");
    let window = shell.window();
    assert_eq!(window.get_presentation_density_key(), "ultra_compact");
    assert_eq!(window.get_presentation_persistence_state(), "saved");
    window.invoke_select_presentation_density(1);
    assert_eq!(window.get_presentation_density_key(), "compact");
    assert_eq!(window.get_presentation_persistence_state(), "saving");
    assert_eq!(sink.last(), Some(DesktopIntent::UpdatePresentationDensity(DesktopDensity::Compact)));
}
```

Add cases for rejected admission retaining density/revision, an intermediate persisted old density not overwriting a newer saving selection, matching persistence becoming saved, failed `UpdatePresentation` becoming not-saved, successful `ApplyConfig`/`RestoreWithPortableSettings` applying an override, preview/cancel/data-only restore not applying one, and 10,000 accepted switches preserving window/route/model counts.

- [ ] **Step 3: Run RED**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-app operation_tests::presentation_density_follow_up_replaces_only_the_pending_payload --locked
cargo +1.97.0 test -p tokenmaster-app state_tests::presentation_density_update_preserves_every_other_settings_class --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_density_ui_contract persisted_density_hydrates_before_show_and_admitted_switch_is_immediate --locked
```

Expected: compilation fails because the command, payload, state operation, projection mapping, intent, and Slint persistence property do not exist.

- [ ] **Step 4: Add the typed replaceable application operation**

Define `ApplicationPresentationDensityUpdate` as a copyable redacted wrapper around `DesktopDensity`, and map it exhaustively to `tokenmaster_state::PresentationDensity` inside the app boundary. Add the command/payload/request constructors. Extend the replaceable match in `operation.rs` with only the exact matching command/payload pair.

Map `ApplicationCommand::UpdatePresentationDensity` to `DesktopOperationKind::UpdatePresentation`; map only `ConfirmConfigImport` to `ApplyConfig`; map only `RestoreDataAndPortableSettings` to `RestoreWithPortableSettings`. Preview/cancel remain `ImportConfig`, and data-only restore remains `Restore`. Presentation persistence is cancellable before irreversible publication. Extend every exhaustive application command/payload match and redacted `Debug` implementation. `DesktopIntent` remains path-free and its `Debug` prints only the enum density.

- [ ] **Step 5: Implement the typed state save**

Add:

```rust
pub(crate) fn update_presentation_density(
    &self,
    permit: &ApplicationCommandPermit,
    density: PresentationDensity,
    mut on_irreversible: impl FnMut(),
) -> Result<(), ApplicationError>
```

Require the exact command and live permit, load current settings, return idempotent success when equal, construct `PortableSettings` with cloned reminders/backup and the new `PresentationSettings`, preserve current device settings, recheck cancellation, cross `begin_irreversible`, publish the atomic operation marker, and call `SettingsStore::save`. No UI, archive, runtime, or SQLite call belongs in this method.

- [ ] **Step 6: Project state density and hydrate the sole presentation owner**

In `ApplicationStateOwner::reliable_state_projection_for_outcome`, map the loaded state density to `DesktopPresentationSettings` and call `DesktopReliableStateSummary::new_with_settings`.

Change the Desktop presentation owner from `Rc<RefCell<_>>` to one `Arc<Mutex<_>>` only so the existing `Send` latest-delivery closure can carry it to the UI event loop. Neither the command worker nor notifier publisher may lock or mutate it; only `deliver_latest`, direct `apply_reliable_state`, construction, and the Slint callback do so on the UI thread. Add the style owner to `ReliableStateNotifierInner` and preserve one lock order.

Construct the style with `DesktopPresentationStyle::from_persisted(reliable_state.presentation().density())` before initial property publication. Reconcile each later projection as follows:

```rust
match projection.operation() {
    Some(operation)
        if matches!(
            operation.kind(),
            DesktopOperationKind::ApplyConfig
                | DesktopOperationKind::RestoreWithPortableSettings
        ) && operation.phase() == DesktopOperationPhase::Succeeded => {
        style.apply_persisted_override(projected_density)
    }
    Some(operation)
        if operation.kind() == DesktopOperationKind::UpdatePresentation
            && matches!(
                operation.phase(),
                DesktopOperationPhase::Failed | DesktopOperationPhase::Cancelled
            ) => {
        let outcome = style.observe_persisted(projected_density);
        style.mark_not_saved();
        outcome
    }
    _ => style.observe_persisted(projected_density),
}
```

Apply Slint properties only after releasing the style lock. A successful config preview,
config-preview cancellation, or data-only restore must never override a local unsaved
density.

- [ ] **Step 7: Wire admission-before-apply and visible persistence truth**

The density callback must borrow/lock the style, call `select_density_index_if_admitted`, and inside its closure submit `DesktopIntent::UpdatePresentationDensity(density)`. Treat only `Started`, `Queued`, and `Coalesced` as admitted. Release the style lock before setting Slint properties.

Add root property:

```slint
in-out property <string> presentation-persistence-state: "saved";
```

Pass it to `SettingsView` and render one accessible non-color-only line whose stable English fallback is `Saved`, `Saving…`, or `Not saved — choose a density again to retry`. Do not add a timer, spinner animation, modal, or manual persistence queue.

- [ ] **Step 8: Make confirmed import/restore and policy updates preserve/apply density**

Run the application config-import and restore tests using compact/ultra-compact candidates. Assert preview contains `SettingsChangeCategory::Presentation`, commit preserves device route, post-operation reliable projection carries imported density, and failed/cancelled operations retain current persisted density. Confirm backup/reminder live synchronization behavior is unchanged.

- [ ] **Step 9: Run GREEN and cross-crate regression**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-app --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_style_contract --test reliable_state_projection_contract --test presentation_density_ui_contract --test ui_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test settings_contract --test package_contract --test restore_contract --locked
$env:RUSTFLAGS='-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-app -p tokenmaster-desktop -p tokenmaster-state --all-targets --locked
```

Expected: every named test passes, rapid selections retain one pending worker payload, and strict Clippy is clean.

- [ ] **Step 10: Commit Task 4**

```powershell
git add -- crates/app/src crates/app/tests crates/desktop/src crates/desktop/ui crates/desktop/tests crates/state/tests
git diff --cached --check
git commit -m "feat(ui): persist presentation density"
```

Stage only the files changed for this task.

### Task 5: Authority audits, stress evidence, and project truth

**Files:**
- Modify: `scripts/audit-reliable-state.ps1`
- Modify: `scripts/tests/audit-reliable-state.Tests.ps1`
- Modify: `scripts/audit-backup-package.ps1`
- Modify: `scripts/tests/audit-backup-package.Tests.ps1`
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`

**Interfaces:**
- Reliable-state audit proves exact v2 fields, strict v1/v2 dispatch, downgrade rejection, no future-axis placeholders, and no new path/secret authority.
- Package audit proves manifest/entry source-version binding and no generic raw writer/extractor.
- Desktop audit proves admission-before-apply, one style owner, latest-only worker reuse, no UI-thread settings I/O, and the 10,000-switch contract.

- [ ] **Step 1: Write failing source and mutation audit cases**

Add deterministic mutations that each must fail its audit:

1. remove one density enum/key;
2. accept schema version 0 or 3;
3. add `skin`, `layout`, `color_scheme`, or `locale` to v2;
4. make v1 migration choose compact;
5. remove manifest/entry version equality;
6. expose a public raw package writer or extractor;
7. remove presentation preservation from reminder/backup update;
8. apply density before command admission;
9. add a new worker/channel/timer/watcher or direct state/filesystem import to Desktop;
10. remove the 10,000-switch or latest-payload coalescing proof.

- [ ] **Step 2: Run audit RED**

```powershell
pwsh -NoProfile -File scripts/audit-reliable-state.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
pwsh -NoProfile -File scripts/audit-backup-package.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
Invoke-Pester -Path scripts/tests/audit-reliable-state.Tests.ps1 -Output Detailed
Invoke-Pester -Path scripts/tests/audit-backup-package.Tests.ps1 -Output Detailed
Invoke-Pester -Path scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed
```

Expected: the newly added assertions fail before their final production anchors/mutations are complete; existing audit cases remain green after each fixture reset.

- [ ] **Step 3: Complete audits and update authoritative documentation**

Record exact settings schema v2, in-memory v1 migration, package compatibility, async replaceable persistence, startup hydration, saved/saving/not-saved semantics, and developer-only evidence. Remove stale claims that the current settings/manifest schema is exactly 1 or that no presentation field exists. Preserve historical statements explicitly labeled as prior v1 history.

Update traceability/parity from P4-A runtime-only density to P4-B durable density, while retaining skins, layouts, color schemes, locales, typography/row sizing, accessibility/DPI/paint/resource acceptance, P5/P6, M0, package/signing/soak, and release as open. Do not put the current commit hash in tracked documents.

- [ ] **Step 4: Run focused stress and all deterministic audits**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 test -p tokenmaster-app operation_tests::presentation_density_follow_up_replaces_only_the_pending_payload --locked
cargo +1.97.0 test -p tokenmaster-app operation_tests::ten_thousand_presentation_updates_keep_one_latest_payload --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_density_ui_contract --locked
pwsh -NoProfile -File scripts/audit-reliable-state.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
pwsh -NoProfile -File scripts/audit-backup-package.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
Invoke-Pester -Path scripts/tests/audit-reliable-state.Tests.ps1 -Output Detailed
Invoke-Pester -Path scripts/tests/audit-backup-package.Tests.ps1 -Output Detailed
Invoke-Pester -Path scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed
```

Expected: all stress assertions and all Pester mutations pass; the UI test still proves one window, unchanged route/models, and 10,000 bounded switches.

- [ ] **Step 5: Run the baseline quality gate**

```powershell
pwsh -NoProfile -File scripts/audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4b'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Expected: every command exits zero; the full test/doctest output has zero failed groups. A third-party Cargo future-incompatibility notice is recorded separately and is not misreported as a TokenMaster Clippy warning.

- [ ] **Step 6: Commit Task 5**

```powershell
git add -- scripts/audit-reliable-state.ps1 scripts/tests/audit-reliable-state.Tests.ps1 scripts/audit-backup-package.ps1 scripts/tests/audit-backup-package.Tests.ps1 scripts/audit-desktop-shell.ps1 scripts/tests/audit-desktop-shell.Tests.ps1 spec docs
git diff --cached --check
git commit -m "docs: record durable density verification"
```

### Task 6: Independent reviews, final verification, and safe cleanup

**Files:**
- No product edits unless review finds a Critical or Important defect.
- Update: `.superpowers/sdd/progress.md` after each clean task review.
- Delete only after evidence: `C:\code\.tokenmaster-target\p4b` and task-owned review-package scratch files.

- [ ] **Step 1: Review each implementation task before advancing**

For Tasks 1-5, record the task base commit, generate one review package for the exact base-to-head range, and dispatch a fresh read-only reviewer against the task brief, design, report, and diff package. Require separate spec-compliance and code-quality verdicts plus Critical/Important/Minor counts. Resolve all Critical/Important findings through the same task implementer/fixer and repeat review. Record clean task ranges in `.superpowers/sdd/progress.md`.

- [ ] **Step 2: Request final whole-branch high-risk review**

Generate a review package from the P4-B design parent (`58e482f`) through P4-B HEAD. The final read-only Sol High reviewer must inspect:

- migration/downgrade semantics and mixed A/B generations;
- manifest/entry source-version binding and legacy packages;
- config/backup/restore preservation;
- optimistic UI versus durable truth races;
- worker coalescing, cancellation, shutdown, and retained resources;
- lock order, UI-thread ownership, panic/failure behavior;
- privacy/authority expansion and docs/test overclaims.

Require a final 0 Critical / 0 Important result before closure. Minor findings are either fixed or explicitly recorded as accepted follow-up with rationale.

- [ ] **Step 3: Rerun verification after the final fix commit**

Run the complete Task 5 baseline again from the final committed tree, not from an earlier commit or agent report. Capture exact pass/fail counts and `git status --short --branch`.

- [ ] **Step 4: Remove only task-owned build/review artifacts**

Before deletion, verify no task-owned `cargo`, `rustc`, `TokenMaster`, test, Pester, or audit process is running. Resolve `C:\code\.tokenmaster-target\p4b`, prove it is a direct child of `C:\code\.tokenmaster-target`, then remove only that directory. Remove only review-package files created for P4-B. Re-run clean-root and Git cleanliness checks.

- [ ] **Step 5: Record honest next state**

P4-B may be called developer-complete only with the exact final verification and review evidence. Do not claim full P4, M0 acceptance, packaging, signing, soak, or release. The next critical slice is a separately designed real production owner for built-in skin/color tokens, followed by its explicit settings migration; no backend foundation rewrite is authorized.
