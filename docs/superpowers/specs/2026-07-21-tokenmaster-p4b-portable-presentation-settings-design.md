# TokenMaster P4-B Portable Presentation Settings Design

**Status:** approved for implementation under the user's delegated architecture authority

**Date:** 2026-07-21

## 1. Outcome

P4-B makes the already implemented production density axis durable and portable without
slowing its UI hot path. TokenMaster settings schema version 2 adds one strict
`presentation` object containing the supported density value. Existing settings records,
`.tmconfig` files, and `.tmbackup` files whose settings entry is schema version 1 migrate
in memory to the current typed value with `comfortable` density. New settings records and
packages are written only as schema version 2.

The visible density still changes synchronously in the existing `MainWindow`. Persistence
uses the existing bounded application operation worker and its replaceable latest-payload
slot; it never writes reliable state on the UI thread and adds no worker, timer, queue,
watcher, renderer, window, archive query, or source scan. The Settings route reports
whether the current visible density is saved, saving, or not saved.

This slice does not add placeholder skin, layout, color-scheme, or locale values. Each
remaining axis receives its own vertical implementation and explicit settings migration
when its production owner exists.

## 2. Context and binding constraints

The product architecture defines five independent presentation axes:

1. `skin`;
2. `layout`;
3. `density`;
4. `colorScheme`;
5. `locale`.

P4-A implemented only density with the exact stable keys `comfortable`, `compact`, and
`ultra_compact`, one checked presentation revision, one window, and constant retained
state. The reliable-state design separately requires settings to persist only fields
owned by working product features and explicitly rejects speculative placeholders.

P4-B therefore preserves these invariants:

- a supported density selection updates visible state without parsing, SQLite access,
  source work, window recreation, or product-model replacement;
- invalid selection and revision overflow leave the prior visible style unchanged;
- reliable-state I/O remains typed, bounded, redundant, and off the UI thread;
- one current visible style and one latest unsaved intent are retained, never selection
  history or an unbounded delivery queue;
- portable import cannot overwrite device-local route state;
- downgrade protection remains fail-closed when a version-1 binary encounters a valid
  version-2 record;
- a package manifest and its settings entry must claim the same settings schema version;
- no path, credential, prompt, response, command, source content, or provider payload is
  added to settings, previews, errors, `Debug`, or UI projections.

## 3. Options considered

### Option A — schema v2 adds the supported density axis only (selected)

Add a strict `PresentationSettings { density }` value now. Version-1 inputs receive the
safe `comfortable` default. Future production axes advance the settings schema with
explicit migrations.

This creates deliberate, testable schema evolution but never persists a value that the
running product cannot validate and apply. It follows the reliable-state contract and
keeps this slice independently releasable.

### Option B — schema v2 freezes all five axes now (rejected)

This reduces the number of future schema versions, but four fields would be promises
without production owners. Imports could select values that the application cannot apply,
and defaults chosen today could constrain later skin inheritance, layout manifests, locale
fallback, or system color-scheme behavior. This conflicts with the existing no-placeholder
settings decision.

### Option C — extensible string map or optional property bag (rejected)

This avoids migrations at the cost of strictness. Unknown keys, weak enum validation,
ambiguous preview counts, hidden compatibility behavior, and accidental authority growth
would enter the reliable-state boundary. It is incompatible with fail-closed typed config
and package contracts.

## 4. Settings schema version 2

### 4.1 Current typed value

`tokenmaster-state` owns the portable semantic types:

```rust
pub enum PresentationDensity {
    Comfortable,
    Compact,
    UltraCompact,
}

pub struct PresentationSettings {
    density: PresentationDensity,
}

pub struct PortableSettings {
    reminders: ReminderPolicy,
    backup: BackupPolicy,
    presentation: PresentationSettings,
}
```

The serialized record form is strict JSON:

```json
{
  "schema_version": 2,
  "portable": {
    "reminders": {
      "enabled": true,
      "lead_seconds": [604800, 86400, 43200, 21600, 3600]
    },
    "backup": {
      "periodic_enabled": true,
      "quiet_seconds": 300,
      "interval_seconds": 21600,
      "retention_budget_bytes": 2147483648
    },
    "presentation": {
      "density": "comfortable"
    }
  },
  "device": {
    "last_route": "dashboard"
  }
}
```

The portable candidate form contains the same `portable` object and no `device` object.
Unknown, duplicate, missing, misspelled, out-of-version, or wrong-type fields fail before
publication. Density has exactly the three P4-A snake-case wire values.

`PortableSettings::new` requires all three owned settings groups. This intentional source
break makes every reconstruction site preserve presentation rather than silently reset it
when changing reminder or backup policy.

### 4.2 Defaults

`SettingsValue::safe_defaults()` uses `PresentationSettings::comfortable()`. A version-1
record or portable candidate migrates to the same density. There is no platform-derived
density heuristic and no DPI-dependent persistence value.

### 4.3 Import preview

`SettingsChangeCategory` gains `Presentation`. The category is present when density
differs, and its changed-field contribution is exactly one. The bounded preview therefore
has at most four ordered categories:

1. reminder profile;
2. backup schedule;
3. backup retention;
4. presentation.

The preview continues to retain only one typed candidate, base settings identity, category
count/list, and changed-field count. It contains no raw package bytes, source path, file
name, or device-local candidate.

## 5. Exact v1-to-v2 migration

### 5.1 Dispatch

Migration code probes only `schema_version`, then decodes the complete input through the
strict wire type for that exact version:

- `1` decodes the old reminders/backup portable shape and current device route, then adds
  comfortable presentation;
- `2` decodes the complete current shape;
- any other version returns `UnsupportedVersion` without partial interpretation.

Separate strict wire types exist for complete records and portable candidates. The
version probe is not accepted as validation. The existing 1 MiB payload bound applies
before decoding.

### 5.2 Load and publication semantics

Loading a valid version-1 A/B record produces the current in-memory semantic value but
does not rewrite either slot during startup. This avoids a hidden startup write and
preserves forensic fallback. The next explicit save writes schema version 2 through the
existing inactive-slot, seal, publish, and reread path while retaining reminder, backup,
and device-route values exactly.

Mixed version-1/version-2 slots remain ordered by checked record generation. Equal
generations with different raw payload digests remain an integrity failure even if their
migrated semantic values would be equal. A valid unsupported newer record remains
`UnsupportedVersion`, not corruption or absence, so a downgraded binary cannot load
defaults or overwrite it.

### 5.3 Portable digest

After decoding either supported source version, `PortableSettingsCandidate` computes its
public target digest from the canonical current version-2 encoding. Package entry hashes
still validate the exact original compressed/expanded bytes before migration. This keeps
restore targets stable for the semantic value that will actually be saved.

## 6. Config and backup compatibility

The package manifest retains container version 1 but its settings-schema field becomes a
validated value instead of a hard-coded equality check.

- writers always put settings schema version 2 in the manifest and encode a version-2
  settings entry;
- readers accept settings schema versions 1 and 2 only;
- the decoded settings entry retains its source schema version internally until the reader
  proves it equals the manifest field;
- manifest/entry disagreement returns an integrity or unsupported-version failure before
  a verified package is constructed;
- config remains exactly one settings entry;
- backup remains exactly settings then database;
- all existing Zstandard window, size, hash, order, reserved-byte, and whole-package gates
  remain unchanged.

A version-1 config/backup can therefore be previewed or restored by the current binary.
Any subsequent export is canonical version 2. Database schema compatibility is independent
from settings schema compatibility.

## 7. Application and desktop ownership

### 7.1 Mapping boundary

`tokenmaster-state` owns durable `PresentationDensity`. `tokenmaster-desktop` keeps its
presentation-only `DesktopDensity`. `tokenmaster-app` performs one exhaustive mapping in
each direction; neither crate gains a dependency that collapses state and presentation
ownership.

`DesktopReliableStateProjection` carries a small `DesktopPresentationSettings` value with
the persisted density. It is populated from the same typed settings load that already
projects backup and reminder policies. Unavailable/safe fallback is comfortable.

The initial `DesktopShell` constructs `DesktopPresentationStyle` from that projected
density before the window is shown. Later reliable-state projections reconcile successful
config import, restore, or density persistence on the UI event loop.

### 7.2 Hot selection and asynchronous save

The Settings combo callback performs this bounded sequence:

1. validate the fixed Slint index and compute the checked next presentation revision;
2. submit `DesktopIntent::UpdatePresentationDensity` to the installed app sink;
3. if admission is `Started`, `Queued`, or `Coalesced`, atomically apply the new density and
   revision to the existing window and mark it `saving`;
4. if admission is rejected, leave the previous visible density and revision unchanged.

The existing application operation worker treats presentation updates as replaceable.
There is at most one active update and one latest queued payload. Repeated switches replace
only the queued payload; they do not accumulate settings values or writes.

The worker loads the latest typed settings, preserves reminders, backup policy, and device
state, and saves the requested density through `SettingsStore`. Equality is an idempotent
success. Reliable-state projection is republished through the existing newest-only event-
loop delivery.

### 7.3 Saved/saving/not-saved state

`DesktopPresentationStyle` retains only:

- current visible density;
- current checked presentation revision;
- last projected persisted density;
- one three-value persistence status.

It retains no request history, settings bytes, path, queue, or timer.

- a locally admitted different density is `saving`;
- a projected persisted density equal to the visible density is `saved`;
- a failed presentation-settings operation while the values differ is `not_saved` and
  keeps the valid visible density rather than causing a jarring rollback;
- a later admitted selection retries persistence;
- a successful confirmed config import or restore explicitly including portable settings
  applies its persisted density, clears the unsaved state, and advances the presentation
  revision only if the visible value changes; config preview/cancel and data-only restore
  do not override a local unsaved density.

Keeping a valid unsaved style visible is intentional. A disk failure is not invalid style
input, and silently rolling back can overwrite a newer local selection after coalesced
operations. The explicit status makes restart behavior truthful: only `saved` survives.

## 8. Failure and race behavior

- Invalid density index, invalid wire value, missing field, revision overflow, command
  admission failure, and mutex/borrow failure leave the current visible style unchanged.
- A settings save failure leaves the previous durable A/B truth untouched and reports
  `not_saved`; it does not block the UI thread.
- An intermediate reliable projection with an older persisted density cannot overwrite a
  newer locally visible unsaved density.
- A matching later projection clears the unsaved state even if an intermediate operation
  status delivery was coalesced.
- Explicit successful confirmed config import or restore including portable settings is
  authoritative and may replace a local unsaved density because the user confirmed that
  portable-settings operation. Config preview/cancel and data-only restore are not such
  authority.
- Failed/cancelled import or restore does not change density.
- A stale or displaced reliable-state delivery remains bounded by the existing latest-only
  notifier and never creates a presentation history.
- Safe mode and no-valid-settings defaults remain comfortable.

## 9. Performance and memory

The visible hot path performs fixed enum checks, one bounded command submission, one small
state update, and a fixed set of Slint property writes. It performs no serialization or
filesystem work. Existing density switch targets remain binding: p95 at or below 16.7 ms,
no window recreation, and no product-model replacement.

Persistence reuses the existing single application operation worker and replaceable slot.
The slice adds no thread, channel, runtime, timer, watcher, database table, cache, or
dependency. A 10,000-switch stress contract must prove bounded submissions, one current
style, latest-value convergence, unchanged route/models/window, and retained-resource
return.

## 10. Test and acceptance matrix

P4-B is developer-complete only when focused RED/GREEN tests prove:

1. exact schema-v2 serialization and all three stable density values;
2. strict rejection of unknown, duplicate, missing, wrong-type, version 0, and version 3
   record/candidate inputs;
3. version-1 record migration preserves reminder, backup, and device route while adding
   comfortable density;
4. explicit save of a migrated value emits version 2 and remains reread-verifiable;
5. version-1 portable candidate migration produces the canonical version-2 digest;
6. import preview reports the ordered Presentation category and one changed field;
7. version-1 `.tmconfig` and `.tmbackup` read successfully, version-2 packages round-trip,
   and manifest/entry schema mismatch fails closed;
8. backup/reminder updates preserve density, and density update preserves every other
   settings class;
9. startup hydrates compact and ultra-compact before show;
10. admitted UI selection applies synchronously while persistence runs off-thread;
11. rejected/failed persistence is truthful and does not corrupt or unexpectedly roll back
    visible state;
12. coalesced rapid updates converge to the latest value without an unbounded queue;
13. successful config import/restore applies density through the reliable-state projection;
14. 10,000 hot switches preserve the window, selected route, bounded models, and constant
    retained presentation state;
15. privacy, authority, source-mutation, clean-root, format, warnings-as-errors Clippy, and
    complete locked workspace tests pass.

Acceptance remains developer evidence only. P4 skins/layouts/color schemes/locales,
remaining density typography and row sizing, accessibility/DPI/visible-paint/resource
evidence, P5 automation, P6 packaging/signing, M0 acceptance, soak, and release remain
open.

## 11. File boundary

Expected implementation ownership is limited to:

- `crates/state/src/settings/{value,migration,preview,store}.rs` and exports;
- `crates/state/src/package/manifest.rs` plus the typed reader compatibility check;
- focused settings/package/restore tests that prove migration compatibility;
- `crates/desktop/src/{presentation_style,reliable_state,ui}.rs`, Slint settings/root
  bindings, and focused desktop contracts;
- `crates/app/src/{command,operation,state,application}.rs` and focused application/worker
  contracts;
- deterministic state/Desktop authority audits and the affected project-truth documents.

No archive schema, query facade, ingestion/runtime engine, provider/plugin boundary, CLI,
MCP, new dependency, or upstream source is part of P4-B.

## 12. Next slice

After P4-B, continue vertical P4 implementation rather than starting another backend
foundation. The next design should implement a real production owner for the next axis
(built-in skin/color tokens is the likely critical path), then add its explicit settings
migration. External `.tmskin.json`, layout manifests, locale catalogs, and final combined
switch/resource acceptance remain separate bounded slices.
