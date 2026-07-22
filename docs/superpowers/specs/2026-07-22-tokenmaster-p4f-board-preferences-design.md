# TokenMaster P4-F Board Preferences Design

## Outcome

Deliver the remaining TM-FUNC-004 board behavior: users can reorder, hide, collapse,
and restore the six fixed Dashboard sections without losing any product data. The
preference is portable, durable, bounded, and applied instantly through the existing
atomic presentation update path.

This slice does not add arbitrary geometry, drag-and-drop, width classes, section
variants, locale, typography, new data queries, or runtime authorities.

## Contract audit

- Product correctness: the six semantic sections remain
  `plan_usage`, `code_output`, `trend`, `sessions`, `activity`, and `models`. A stored
  board is exactly one permutation of those keys, contains no duplicates, and keeps
  at least one section visible. Collapse and visibility affect presentation only.
- No data loss: the Dashboard projection always retains and refreshes all six section
  payloads. Hidden and collapsed sections do not remove archive data, query inputs,
  snapshot fields, or automation state.
- Persistence: settings schema v6 stores one complete presentation value containing
  the four existing axes plus one board manifest. v1-v5 migrate in memory to canonical
  order, all visible, none collapsed, without a startup write.
- Atomicity: edits replace the complete presentation selection through the existing
  `UpdatePresentation` admission and one-active/one-latest-pending worker. No second
  persistence authority or partial per-row command is introduced.
- Boundedness: the manifest is a fixed six-element array. UI models contain at most
  six board rows; no retained history, unbounded queue, timer, worker, or cache is
  added.
- Security: keys are a closed enum and all rendering branches are compiled Slint
  components. Preferences cannot inject UI code, paths, commands, SQL, HTTP,
  filesystem access, content, prompts, or source data.

## Alternatives considered

1. Add a separate board store and per-section commands. Rejected: it permits partial
   revisions, expands failure states, and duplicates the already accepted atomic
   presentation owner.
2. Keep board state device-local or in memory only. Rejected: it violates durable
   layout parity and makes restore/import behavior surprising.
3. Persist arbitrary positions or Slint geometry. Rejected: it expands the authority
   and validation surface beyond the six-section product contract.
4. Drag-and-drop as the primary editor. Rejected for this release slice: Up/Down,
   Visible, Collapse, and Reset controls are deterministic, keyboard accessible, and
   cheaper to verify. Drag-and-drop may be added later without changing the manifest.

## State model

`BoardSectionKey` is a closed six-value enum. `BoardSectionPreference` contains the
key plus `visible` and `collapsed` booleans. `BoardPreferences` owns exactly
`[BoardSectionPreference; 6]` and validates a complete permutation plus at least one
visible row during construction and deserialization.

`PresentationSettings` remains a copyable complete value and gains `board`. Selecting
any presentation axis preserves the board; editing a board row preserves density,
skin, color scheme, and layout. Reset restores only the canonical board manifest.

## UI behavior

- Settings shows six bounded editor rows in current order. Up/Down swaps adjacent
  rows. Visible cannot turn off the last visible row. Collapse is retained even while
  a row is hidden. Reset restores canonical order, visibility, and expansion.
- Dashboard derives a visible ordered model from the manifest. Every slot selects one
  of the six compiled section cards by key; no dynamic component or data lookup is
  admitted.
- A collapsed slot renders a compact labelled card with the section state, while its
  complete backing projection remains present.
- Narrow mode renders visible slots in one column. Wide mode places visible slots in
  the selected P4-E template by ordinal, compacting missing slots without gaps. The
  canonical manifest preserves the current P4-E compositions exactly.
- UI changes only after the complete selection is admitted. Persistence status and
  retry behavior remain the existing presentation contract.

## Evidence and stop conditions

Focused RED/GREEN tests must prove strict v6 JSON validation, v1-v5 migration,
complete app payload replacement, last-visible rejection, order/collapse/reset
semantics, compiled Slint bindings, unchanged six-section projection ownership, and
bounded repeated edits. Then run one implementation review, at most one re-review,
the directly affected receipts, and the baseline once.

This cycle changes product behavior. Source-text audit additions are out of scope
unless an existing required receipt fails on the v6 contract. Two consecutive
audit/test/docs-only correction rounds or 60 minutes of audit-only work triggers
`AUDIT_HARDENING_LOOP` and an immediate return to the next release-critical product
slice.
