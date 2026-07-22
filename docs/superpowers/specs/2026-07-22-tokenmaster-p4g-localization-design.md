# P4-G Unified Localization Design

## Outcome

TokenMaster switches the complete current production window between English,
Russian, and pseudo locale without restart, archive mutation, unbounded work, or a
second presentation owner. The selected locale is portable presentation state.

## Release boundary

This slice closes the locale/language part of P4 presentation. It does not include
OS-locale inference, RTL, a plugin translation API, dynamic catalog loading,
typography, DPI, or general accessibility remediation. Stable identifiers, evidence
codes, availability codes, paths, timestamps, numbers, and source data are never
translated.

## Considered approaches

1. **Bundled Slint catalogs plus a bounded desktop display-label resolver (chosen).**
   Compile fixed `ru` and `pseudo` catalogs into the executable, use Slint's native
   invalidation for markup strings, and resolve Rust-generated display labels at the
   existing desktop projection boundary. This reuses the repository's probe-proven
   Slint 1.17 mechanism and adds no runtime I/O or dependency.
2. One large Rust struct containing every UI string. This is closed and testable but
   duplicates Slint's translation machinery, creates a very wide generated bridge,
   and increases per-update allocation and maintenance.
3. Per-view dictionaries or runtime catalog files. Rejected because they introduce
   multiple owners, partial updates, file-system failure modes, and an unnecessary
   extension surface.

## State and migration

Settings schema v7 adds required `portable.presentation.locale` with the closed
values `en`, `ru`, and `pseudo`. New/default and every v1-v6 migration select `en`.
Current v7 input rejects missing, duplicate, wrong-type, or unknown locale values.
The existing complete presentation update carries locale together with density,
skin, scheme, layout, and board preferences, so persistence and optimistic/latest
generation reconciliation remain atomic.

## Runtime flow

`PresentationSettings` maps to a closed desktop `DesktopLocale`, contained in
`DesktopPresentationSelection`. The Settings locale selector submits the existing
`UpdatePresentation` operation. On an admitted selection,
`apply_presentation_style` selects the bundled translation and updates the locale id
in the same UI-turn as the other presentation properties. A rejected catalog
selection cannot partially update the window.

All fixed Slint-visible and accessibility strings use `@tr`. English remains the
source text; `ru` and `pseudo` are compile-time catalogs. Rust projection code keeps
machine fields invariant and localizes only user-facing labels through a closed
locale match. Pseudo strings expand visibly while preserving placeholders and
format arguments, giving a deterministic overflow/completeness probe.

## Boundedness and failure behavior

Catalogs are immutable executable resources. Locale switching performs no scans,
threads, timers, network, SQL, shell, or filesystem work. The number of locales and
keys is build-time bounded. Unsupported indexes are rejected before persistence;
unsupported settings fail strict decode. Failure to select an expected bundled
locale is an explicit presentation rejection, not a silent mixed-language state.

## Verification

- State red/green tests cover v7 round-trip, v1-v6 migration to English, and strict
  missing/duplicate/unknown locale rejection.
- Desktop tests cover locale admission/rejection, bundled catalog selection, hot
  callback propagation, all-key Russian/pseudo completeness, placeholder retention,
  and unchanged stable fields.
- Application tests prove complete locale persistence and latest-wins reconciliation.
- A source contract rejects new unwrapped user-visible Slint literals in production
  UI, while allowing stable non-user-visible identifiers.
- Finish with the repository baseline: clean-root audit, fmt, warnings-as-errors
  clippy, and locked workspace tests.

## Stop condition

Stop after one implementation review and one final re-review when the production
window is hot-switchable and the required gates pass. Further wording polish,
catalog tooling, typography, DPI, or speculative parser hardening belongs to later
product slices and must not extend this cycle.
