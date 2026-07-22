# TokenMaster P4-D Independent Color Scheme Design

Date: 2026-07-22
Status: accepted for autonomous implementation under the standing product direction

## 1. Outcome

P4-D adds one durable, independent presentation axis:

- `system`
- `light`
- `dark`

The selected scheme combines with every existing density and built-in skin. Switching is
instant and atomic, persistence remains latest-only and bounded, and a live system-theme
change updates a `system` selection through Slint's existing UI event loop.

This is a product slice. It does not add layouts, locale, typography, external skin
packages, a second settings worker, polling, filesystem/registry/network authority, or a
release claim.

## 2. Contract audit

The current production contract has three independent densities and three independent
skins in one complete schema-v3 record. Each skin owns one immutable dark 15-role palette.
The accepted P4-C design explicitly requires color scheme to be the next independent axis.

The narrow compatibility problem is that existing installations have only dark palettes.
Interpreting an old record as `system` could silently change an upgraded installation to
light. Therefore:

- fresh defaults select `system`;
- exact v1, v2, and v3 migrations select `dark`;
- schema v4 persists the complete `{density, skin, color_scheme}` record;
- unknown versions, fields, values, duplicates, and non-canonical shapes remain rejected.

Slint 1.17 already exposes the platform-observed `Palette.color-scheme` as a reactive
property. TokenMaster must consume that property instead of adding a Windows registry
reader, platform watcher, timer, or polling loop.

## 3. Considered approaches

### A. Independent requested and effective scheme (selected)

Persist `system|light|dark`. Resolve `system` against Slint's observed platform scheme,
with a deterministic dark fallback when Slint reports unknown. Rust remains the sole owner
of all exact palette values and publishes one complete palette to Slint.

This preserves the five-axis architecture, supports live system changes, and adds no new
authority or long-lived resource.

### B. Six combined skin variants

Encode `refined_light`, `refined_dark`, and so on. Rejected because it couples skin and
scheme, creates invalid cross-axis persistence, and scales poorly when layout and locale
arrive.

### C. Derive light colors inside Slint

Keep dark Rust palettes and transform colors in UI expressions. Rejected because exact
semantic palette ownership would be split across Rust and Slint, contrast would be hard to
validate, and application could expose mixed frames during a switch.

## 4. State and migration

`PresentationColorScheme` is a closed state enum with stable wire values `system`, `light`,
and `dark`. `PresentationSettings` becomes the complete three-axis value and cannot be
partially constructed.

Settings schema v4 has exactly:

```json
{
  "schema_version": 4,
  "portable": {
    "presentation": {
      "density": "comfortable",
      "skin": "refined",
      "color_scheme": "system"
    }
  }
}
```

Migration is memory-only and deterministic:

| Source | Result |
| --- | --- |
| v1 `{}` | comfortable + refined + dark |
| v2 `{density}` | retained density + refined + dark |
| v3 `{density,skin}` | retained density + retained skin + dark |
| fresh/default | comfortable + refined + system |
| v4 | exact decoded triple |

Export emits only v4. Import preview and application compare the complete triple and never
split the axes.

## 5. Desktop presentation owner

`DesktopPresentationSelection` becomes the complete `{density, skin, color_scheme}` value.
The existing `DesktopPresentationStyle` remains the only owner of current selection,
persisted selection, revision, and saved/saving/not-saved state.

Selector admission remains admission-first:

1. decode the bounded Slint index;
2. construct one complete candidate selection;
3. admit the complete candidate to the capacity-one persistence path;
4. on admission, publish one complete palette and metadata revision;
5. on rejection, retain the prior complete selection and palette.

There is still one active payload and at most one latest pending payload. No history,
per-axis queue, retained palette list, timer, or additional worker is introduced.

## 6. Requested versus effective scheme

Desktop uses two closed concepts:

- requested scheme: `System|Light|Dark`, persisted and shown in Settings;
- effective scheme: `Light|Dark`, used to select exact color tokens.

Resolution is total:

| Requested | Slint observation | Effective |
| --- | --- | --- |
| Light | any | Light |
| Dark | any | Dark |
| System | Light | Light |
| System | Dark | Dark |
| System | Unknown | Dark |

The unknown fallback is dark because it preserves the pre-P4-D visual contract and is
deterministic in headless/compiled UI tests.

A system observation change alters only the effective scheme. It does not persist a new
requested value, change the presentation revision, mark settings dirty, or enqueue work.
If the requested scheme is not `system`, the observation is ignored.

## 7. Palette ownership

Rust owns six immutable 15-role palettes: light and dark for each Refined, Graphite, and
Ember skin. Every palette supplies the same semantic roles and is returned by the total
function `(skin, effective_scheme) -> DesktopColorTokens`.

The UI receives one complete `UiPalette`; it never derives semantic colors and never sees a
partially updated role set. Existing contrast/distinctness validation extends across all
six palettes.

## 8. Slint bridge and Settings UI

`MainWindow` imports the standard `Palette` global and exposes a bounded observed-system
scheme id derived from `Palette.color-scheme`. A change callback reports only
unknown/light/dark to Rust. Rust recomputes and republishes the palette only when the
effective scheme actually changes.

Settings adds one fixed combo box with labels `System`, `Light`, and `Dark`. Its index maps
to the closed desktop enum. Root diagnostic properties expose requested and effective
stable keys for compiled UI tests without exposing paths or user data.

The bridge must apply palette roles before requested/effective scheme metadata so a UI
observer never sees a new scheme key with the old palette.

## 9. Application mapping

The app maps each state enum to its desktop counterpart independently, then constructs one
complete selection. It must not enumerate the 27 density/skin/scheme combinations.

Reliable state, intents, snapshots, import preview, and persistence completion carry the
complete triple. A stale completion can confirm persisted state but cannot replace a newer
visible selection, matching the existing latest-only contract.

## 10. Security and resource boundary

P4-D adds fixed enum values and fixed Rust RGB data only. Slint already owns platform theme
observation. TokenMaster adds no registry, path, file, shell, HTTP, provider, credential,
prompt, response, command, source-content, plugin, or arbitrary theme authority.

Retained memory remains constant: one current selection, one persisted selection, one
active palette, and one latest pending complete settings payload. System changes use the
existing reactive property and event loop, with no polling or growing history.

## 11. Verification and stop condition

Focused red/green tests must prove:

- exact v4 round trip and exact v1/v2/v3 dark-preserving migration;
- fresh default is system;
- all invalid v4 shapes/values are rejected;
- complete application and reliable-state mapping;
- admission-first color-scheme selection and stale-completion behavior;
- total six-palette selection, contrast/distinctness, and dark compatibility;
- compiled Settings selector and system observation wiring;
- 10,000 mixed-axis switches retain one active and at most one latest pending payload;
- system observation changes cause no persistence or revision churn.

Then run the affected crate/workspace tests and the repository baseline once. Allow one
implementation review and one final re-review only. Any further audit-only hardening is
out of scope unless a new Critical production/security/data-loss defect has a focused
reproducer.

P4-D is complete when the final committed tree has direct evidence for the above, docs are
reconciled, the worktree is clean, and task-owned processes/artifacts are gone. It advances
only the color-scheme portion of P4 and makes no M0, package, release-candidate, or stable
release claim.
