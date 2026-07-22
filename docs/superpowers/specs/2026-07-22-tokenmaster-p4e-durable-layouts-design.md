# TokenMaster P4-E Durable Layouts Design

## Outcome

Add one durable, instant layout axis to the production presentation selection. The
three fixed layouts are `refined`, `control_center`, and `workbench`, matching the
already accepted M0 vocabulary while operating on the production Dashboard's existing
bounded models. Fresh and legacy settings select Refined.

This slice closes built-in layout switching only. It does not claim the separate
TM-FUNC-004 board customization requirement to reorder, hide, or collapse individual
sections, and it does not alter width-derived narrow layouts on data routes.

## Contract audit

- Production correctness: settings schema v5 stores one complete
  `{density, skin, color_scheme, layout}` value. Strict v1-v4 migration supplies
  `refined`; partial payloads and unknown keys fail closed.
- Presentation ownership: the existing admission-first owner and one-active plus
  one-latest-pending application worker carry the complete four-axis value. No new
  thread, queue, cache, timer, model, snapshot, or persistence authority is added.
- UI behavior: Refined preserves today's Dashboard composition. Control Center and
  Workbench visibly rearrange the same six ordered semantic sections. Narrow windows
  always use the safe single-column composition while retaining the selected layout.
- Boundedness: switching reuses the same `MainWindow`, route models, and Dashboard
  input models. Stress covers all 81 presentation combinations and 10,000 layout
  switches.
- Security: layout is a fixed enum. It carries no path, content, command, SQL, HTTP,
  filesystem, plugin, or dynamic UI authority.

## Alternatives considered

1. Treat density-scaled shell chrome as layout. Rejected because density and modular
   layout are distinct normative axes.
2. Add only `adaptive` and `stacked`. Rejected as a weak product distinction that
   discards the repository's established three-layout M0 vocabulary.
3. Add arbitrary per-section positions and visibility now. Rejected because it mixes
   the separate board-customization contract into the release-critical layout slice
   and would require a larger bounded configuration design.

## Layout semantics

- **Refined:** current production arrangement: Plan Usage full width; Code Output with
  Trend; Sessions with Activity; Model Usage full width.
- **Control Center:** on wide windows, Plan Usage and Code Output form the first
  operational row; Trend remains full width; Sessions, Activity, and Model Usage form
  the lower monitoring area.
- **Workbench:** on wide windows, Plan Usage remains full width; Code Output is paired
  with Sessions; Trend is paired with Model Usage; Activity remains full width.
- **Narrow safety:** below the existing Dashboard breakpoint, all three render the
  same ordered single column. Selection stays durable and becomes visible again when
  width permits.

## Evidence and stop conditions

Focused RED/GREEN tests must prove strict v5 serialization/migration, complete app
payload replacement, admission-before-apply, visible compiled Slint layout geometry,
same-window/model stability, and 10,000-switch boundedness. Run one implementation
review and at most one re-review, then the relevant receipts and baseline gates once.

Do not add new source-text audit rules unless an existing release receipt directly
fails because of the changed four-axis contract. Two consecutive audit/test/docs-only
correction rounds or 60 minutes of audit-only work triggers `AUDIT_HARDENING_LOOP` and
an immediate return to product delivery.
