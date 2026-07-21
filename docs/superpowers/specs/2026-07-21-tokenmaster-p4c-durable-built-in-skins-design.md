# TokenMaster P4-C Durable Built-in Skins Design

## 1. Decision

P4-C delivers a complete durable vertical slice for three production built-in skin
families. It extends the existing single presentation owner, persists the complete
presentation selection in settings schema v3, and applies one immutable color-palette
value to the existing Slint token global. It does not add an external skin loader,
layout switching, color-scheme switching, localization infrastructure, or another
runtime owner.

The selected families and stable keys are:

| Rust variant | Stable key | English fallback label | Purpose |
| --- | --- | --- | --- |
| `Refined` | `refined` | Refined | Existing TokenMaster identity and safe default |
| `Graphite` | `graphite` | Graphite | Low-chroma blue-grey alternative |
| `Ember` | `ember` | Ember | Warm amber/copper alternative |

Skin family and color scheme remain independent axes. P4-C supplies dark palettes for
all three families because production currently has one dark scheme. A later P4 slice
adds `system`, `light`, and `dark` scheme selection without changing the skin keys or
the owner boundary.

This design is approved by the user's standing instruction to select the best bounded
option autonomously and the explicit `go` after the P4 audit and next-slice proposal.

## 2. Options considered

### 2.1 Runtime-only skin selection

Rejected. It would either reset visibly on restart or report the existing presentation
state as saved while the skin was not durable. It would also require a second pass to
replace a skin-only intent with an atomic multi-axis presentation intent.

### 2.2 Durable built-in vertical slice

Selected. It fixes stable IDs, token ownership, UI switching, settings migration,
package compatibility, mixed-axis coalescing, failure reconciliation, and deterministic
evidence in one bounded contour.

### 2.3 External `.tmskin` loading now

Rejected for P4-C. Directory watching, inheritance, package validation, diagnostics,
reload coalescing, and filesystem security are independent work. Built-in behavior must
be accepted first. The future loader may only produce the same validated immutable
palette value consumed by this design.

## 3. Product behavior

Settings displays separate Density and Skin selectors. Selecting either value updates
the existing window immediately after command admission. The selected skin changes all
semantic colors without recreating the window, replacing product models, querying the
archive, scanning sources, or waiting for settings I/O.

The presentation persistence label applies to the complete density-plus-skin selection:

- `saved`: the visible complete selection equals durable truth;
- `saving`: the latest complete selection was admitted and is being persisted;
- `not_saved`: persistence failed or was cancelled; selecting either current value
  again retries the complete selection without incrementing its revision.

An invalid selector index is rejected before command admission and leaves every field
unchanged. Config import and portable-settings restore atomically replace both axes.
Data-only restore, cancelled previews, ordinary reliable-state refresh, and unrelated
policy updates cannot overwrite a newer local presentation selection.

## 4. Skin and color-token model

`tokenmaster-desktop` adds a fixed `DesktopSkin` enum and a `DesktopColorTokens` value.
Both are `Copy`, contain no heap allocation, and expose only stable keys, fixed Slint
indices, and the current immutable palette. `Refined` is index 0, `Graphite` is index 1,
and `Ember` is index 2. Other indices reject.

The palette contains exactly the current fifteen semantic color roles:

1. `background`
2. `surface`
3. `surface_raised`
4. `surface_subtle`
5. `border`
6. `text_primary`
7. `text_secondary`
8. `accent`
9. `accent_subtle`
10. `accent_secondary`
11. `accent_tertiary`
12. `ready`
13. `waiting`
14. `degraded`
15. `unavailable`

The exact RGB values are:

| Role | Refined | Graphite | Ember |
| --- | --- | --- | --- |
| background | `#0b0f17` | `#101216` | `#140d0a` |
| surface | `#111827` | `#181b20` | `#201511` |
| surface raised | `#182234` | `#22262d` | `#2e1f19` |
| surface subtle | `#0e1624` | `#14171c` | `#190f0c` |
| border | `#293548` | `#343a44` | `#4b3026` |
| text primary | `#f4f7fb` | `#f5f7fa` | `#fff7ed` |
| text secondary | `#9eabc0` | `#aab2bd` | `#cdb09d` |
| accent | `#7cd4fd` | `#78a9ff` | `#fb923c` |
| accent subtle | `#173044` | `#1f2d45` | `#472417` |
| accent secondary | `#a78bfa` | `#a5b4fc` | `#fbbf24` |
| accent tertiary | `#f0abfc` | `#d8b4fe` | `#f472b6` |
| ready | `#70d6a5` | `#73d7ad` | `#86d49d` |
| waiting | `#8fa3bf` | `#9aa7b8` | `#bda99e` |
| degraded | `#f2c66d` | `#eac574` | `#f4c86f` |
| unavailable | `#f08b8b` | `#ee8d93` | `#f58f86` |

The minimum measured WCAG contrast among primary text, secondary text, accent, and
semantic-state foreground tokens against their corresponding surface is greater than
6.8:1 for all three palettes. P4-C tests exact values and the contrast floor. Full
Windows high-contrast and screen-reader acceptance remains a later interactive P4/P6
gate and is not inferred from this calculation.

## 5. Slint ownership and atomic application

`tokens.slint` exports one `UiPalette` struct with the fifteen roles and keeps the
existing `UiTokens.background`, `UiTokens.surface`, and other role properties as
derived aliases. All existing views therefore remain unchanged.

`UiTokens` receives one complete `UiPalette` property from Rust. Slint contains no
second named family table and no conditional palette selection. The constructor-only
default is replaced synchronously by the Rust presentation owner before the production
window can be shown. `apply_presentation_style` performs no event-loop yield between
the complete palette assignment and the root skin/density/revision properties, so one
paint observes one complete presentation snapshot.

`DesktopPresentationStyle` remains the sole mutable presentation owner. It stores:

- current `DesktopPresentationSelection { density, skin }`;
- persisted `DesktopPresentationSelection`;
- one checked revision;
- one persistence state.

It retains no product snapshot, path, callback, database handle, runtime owner, queue,
timer, cache, history, or prior palette.

## 6. Durable settings schema v3

Current settings schema becomes version 3. The nested portable shape is exactly:

```json
{
  "presentation": {
    "density": "comfortable",
    "skin": "refined"
  }
}
```

`PresentationSkin` admits exactly `refined`, `graphite`, and `ember`.
`PresentationSettings` contains exactly `density` and `skin`. The top-level portable
and device field sets do not change.

Migration is decode-only and performs no startup write:

- v1 retains reminders/backup and defaults to Comfortable plus Refined;
- v2 retains density and defaults skin to Refined;
- v3 decodes both fields exactly;
- version 0 and version 4 or later fail as unsupported;
- unknown, missing, duplicate, or malformed fields fail closed.

The next successful settings mutation writes canonical v3. Typed `.tmconfig` and
`.tmbackup` packages accept source settings versions 1 through 3, preserve the source
version in previews, and require manifest/entry source-version equality. Import or
restore reseals only through existing typed APIs; no raw writer or extractor is added.

## 7. Application command and coalescing

The density-only command and payload become one presentation command carrying the full
selection:

```rust
ApplicationCommand::UpdatePresentation
ApplicationOperationPayload::Presentation(ApplicationPresentationUpdate)
```

`ApplicationPresentationUpdate` contains both desktop density and desktop skin and
converts them to the state equivalents. Both UI selectors capture the complete current
selection and submit that value. The application reads durable settings, returns early
only when the complete selection already matches, reconstructs settings while
preserving reminders, backup, and device values, crosses the existing irreversible
boundary, and saves once.

The existing replaceable worker slot is reused. It retains at most one active payload
and one latest pending complete payload. A mixed burst cannot persist density from one
request with skin from another because no partial presentation payload exists.
Cancellation, shutdown, panic, retry, and terminal publication keep the existing
bounded lifecycle.

## 8. Reconciliation and failure behavior

UI selection is admitted before style mutation. Rejected admission leaves the complete
style unchanged. Successful admission makes the complete optimistic selection visible
and marks it Saving.

On terminal projection:

- successful presentation update confirms the exact durable complete selection;
- failed or cancelled update retains a newer optimistic selection and marks it NotSaved;
- successful config import or portable-settings restore atomically overrides both axes;
- unconfirmed/equal/older projections cannot confirm or overwrite a newer local
  density-to-skin-to-density sequence;
- revision exhaustion retains every field and avoids command admission.

No fallback silently changes a requested family. Refined is used only for defaults and
legacy migration, not as a recovery from an otherwise valid admitted selection.

## 9. Performance, memory, and security

P4-C adds no dependency, process, thread, timer, polling loop, async runtime, channel,
queue, cache, filesystem access, network access, SQL, unsafe code, palette-specific
heap allocation, or additional window. The three fixed palettes occupy constant static
memory. Existing bounded Slint string/property updates remain unchanged. The
presentation owner holds one current and one persisted fixed selection.

Immediate UI work is bounded to one enum validation, one command admission, one fixed
style update, one fixed palette assignment, and fixed property assignments. Durable I/O
remains on the existing application operation worker. Skin selection cannot mutate or
query the archive and cannot access external skin files.

External skin data remains declarative-only future work under TM-SEC-005. It cannot be
represented as a native plugin, Slint source, script, DLL, process, URL, or arbitrary
path by this slice.

## 10. Verification and acceptance

P4-C is developer-complete only when all of the following pass from the final committed
tree:

1. State tests prove exact v3 serialization, v1/v2 migration, 0/4+ rejection, malformed
   candidates, downgrade behavior, A/B records, config/backup/restore preservation, and
   package source-version binding.
2. Desktop unit tests prove fixed keys/indices, exact palette values, contrast floor,
   invalid-index rejection, checked revision behavior, persistence reconciliation, and
   atomic import/restore override.
3. Compiled Slint tests prove both selectors, all fifteen token changes, one window,
   stable route/model counts, and 10,000 mixed density/skin switches.
4. Application tests prove one full payload, one active plus one latest pending payload,
   10,000 mixed updates, cancellation/shutdown return, and no cross-axis lost update.
5. Reliable-state, package, Desktop, and application source/mutation audits pin the v3
   schema, exact public surfaces, sole owner, one palette assignment, admission order,
   fixed bounds, and zero new authority.
6. Clean-root, formatting, strict warnings-as-errors workspace Clippy, and complete
   locked workspace tests pass.
7. Independent review returns zero Critical and zero Important findings. Minor findings
   are fixed or recorded with an explicit accepted rationale.

This is P4 developer evidence only. It does not claim layout or color-scheme switching,
localization, external skins, interactive accessibility/DPI/paint/resource acceptance,
P5, P6, M0 acceptance, packaging, signing, soak, or product release.

## 11. Documentation updates

The closing task updates `SPECIFICATION.md`, `DATA_CONTRACT.md`, `API_CONTRACT.md`,
`SECURITY.md`, `TRACEABILITY.md`, `DECISIONS.md`, `CURRENT_STATE.md`, `HANDOFF.md`,
`ROADMAP.md`, `FEATURE_PARITY.md`, `PROJECT_HISTORY.md`, and `CHANGELOG.md`. Tracked
documents describe the accepted behavior and remaining gates without embedding the
current commit hash.

## 12. Next slice

After P4-C, add the independent color-scheme axis over the same immutable palette owner,
then layout manifests, unified en/ru/pseudo locale ownership, typography/row sizing, and
interactive accessibility/DPI/paint/resource closure. External skin packages remain
after built-in family, scheme, and validation contracts are stable.
