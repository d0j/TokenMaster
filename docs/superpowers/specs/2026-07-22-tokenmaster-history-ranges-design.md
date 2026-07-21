# TokenMaster P3-D Interactive History Ranges Design

**Status:** Approved for implementation by autonomous product direction. Completion
requires focused RED/green tests, independent review, and the locked workspace gates.

## Goal

Replace the fixed recent-30-day History presentation with three truthful rolling range
controls without adding a query owner, worker, cache, retained range history, or a larger
frontend row bound.

## User contract

History offers one shared usage-range control with `1 day`, `7 days`, and `30 days`.
The initial and restart default is
`30 days`. Selecting another value starts one interactive request and keeps the last
published range visible while the replacement is pending. A successful result atomically
replaces the shared recent-usage snapshot and selects the requested control. Rejection,
cancellation, deadline, abandonment, or query failure clears pending state and restores
the control to the last published range.

These labels are deliberately rolling day counts. TokenMaster does not call them
`Today`, `This week`, or `This month`, because those labels imply different calendar
semantics. Arbitrary dates, custom ranges, aggregation-granularity switching, filters,
and more than 30 displayed points remain separate work.

## Shared analytics ownership

History, Models, and Projects consume one recent-usage envelope with Model and Project
breakdowns. The range control replaces that same envelope; it does not introduce a third
analytics request or a History-only shadow payload. Therefore a successful range selection
changes the exact range shown by all three routes. Their headers render the envelope's
range and timezone, and the History control is labelled `Usage range`, so the period and
shared ownership remain explicit rather than silently relabelled.

This intentionally evolves the current fixed-30-day MUST/ADR contract. The normative
`SPECIFICATION`, `API_CONTRACT`, `DATA_CONTRACT`, `SECURITY`, and `DECISIONS` documents
must be updated and committed before behavior changes. The new contract keeps 30 days as
the default and maximum displayed range while allowing one shared exact 1/7/30 selection.

The Dashboard today request, Projects UTC-today Git evidence, Sessions, and all other
sections remain unchanged. Route selection remains presentation-only. Only the dedicated
History range callbacks may submit range work.

## Typed intent and authority boundary

Slint exposes three fixed callbacks and receives only path-free selection/pending facts.
The desktop public boundary uses a closed `DesktopHistoryRangePreset` enum and a
`DesktopHistoryRangeIntent` containing:

- the current `DesktopSnapshotEpoch`;
- the currently viewed `ProductGeneration`;
- a checked monotonically increasing range-selection generation;
- one of the three fixed rolling presets.

No calendar date, free-form count, scope, provider/profile identity, query object, archive
handle, or path crosses the UI boundary. The controller alone maps a preset to validated
`UsageRange::recent_days`, system timezone, daily series, and the existing Model/Project
breakdowns.

## Scheduling and stale-result rules

Range work uses the existing capacity-one desktop worker. Controller state owns only the
published preset, one persistent range-generation high-water mark, one current correlation,
and one latest pending intent. The high-water mark survives success, failure, and terminal
rollback so an old intent cannot be replayed. There is no queue, selection history, result
cache, or additional thread.

Admission requires the current snapshot epoch, published product generation, and a newer
selection generation. A regular product refresh supersedes admitted range work and uses
the last successfully published preset. If a range intent follows active refresh work,
the worker runs only the latest eligible follow-up. The UI rejects repeated input while
pending; the controller's single replaceable pending slot is defense for racy or direct
programmatic submissions that reach it before presentation state converges.

History range work and Sessions detail/page work are mutually exclusive at admission. A
range request returns `Busy` while either Sessions interaction is active or pending;
Sessions detail/page admission returns `Busy` while range work is active or pending. This
keeps page-relative selection, cursor navigation, and shared analytics replacement from
invalidating each other's product-generation fence inside one worker batch.
The controller adds one optional active Session-detail attempt scalar because the current
pending-detail slot is consumed before query execution; worker completion clears that
scalar on every terminal path. This is correlation state only, not retained detail or a
second work queue.

| Existing work | Incoming work | Result |
| --- | --- | --- |
| none | History range | admit; run section-local range work |
| full refresh active | History range | retain only latest range follow-up |
| History range active/pending | full refresh | refresh supersedes range and rolls it back |
| History range active/pending | History range | UI rejects; controller direct ingress retains only newest valid generation |
| Sessions detail/page active/pending | History range | reject `Busy` without changing either interaction |
| History range active/pending | Sessions detail/page | reject `Busy` without changing either interaction |
| any interaction | backend epoch replacement | old epoch becomes stale; exact pending UI correlation rolls back |

Before query and again before publication, the worker validates epoch, viewed product
generation, worker-attempt correlation, and range-selection generation. A stale result is
discarded. Successful range publication advances the product generation, records the new
published preset, and replaces the sole immutable snapshot. A full refresh reads that
published preset, so later refreshes preserve the user's successful selection.

`ProductReducer::publish_history` or `fail_history` is authoritative about whether a
section-local reduction was accepted. Only `ProductPublishOutcome::Accepted` may publish
a snapshot; only an accepted successful query may update the published preset. Coalesced,
older, or incompatible reducer outcomes publish nothing, leave the preset unchanged, and
remain current until worker completion drives the exact no-snapshot rollback. An accepted
query failure may publish the shared degraded retained envelope but never changes preset.

Terminal completion without a snapshot uses one dedicated optional History-range notifier
beside the existing Sessions-navigation notifier. Mutual-exclusion admission proves both
cannot own current work for the same worker attempt, so one cannot displace the other.
The worker completion adapter reconciles both fixed slots and notifies only the exact
still-current intent. Each bridge route is idempotent and matches the whole intent before
clearing pending UI state. Snapshot publication happens before terminal completion for the
same attempt, and successful commit consumes current work first, preventing rollback from
erasing a committed selection.

## Failure semantics

The existing product History failure behavior retains compatible last-good data as
degraded. Because Models and Projects derive from the same section, a failed range
replacement degrades all three routes with the same reason and the same retained exact
prior range. The visible range and selected control remain the last successfully published
preset, preventing an old payload from being labelled as the failed target. Terminal
cancellation/deadline paths without publication only clear pending state and do not invent
a product failure snapshot.

If the application cannot hand an admitted UI intent to the current controller, it rejects
the exact intent immediately and restores controls without blocking the UI thread. Backend
epoch replacement resets the range to the default 30 days and clears old correlations.

## Presentation and UI

`DesktopState` retains only the published preset and one active intent. Accepted input
synchronously sets `pending=true` and disables all three controls. Snapshot replacement
commits the active preset only when the accepted publication is newer and its exact daily
series length matches that preset. Because recent-day resolution includes zero-usage and
skipped civil dates as explicit points, 1/7/30 points are an exact query-owned discriminator.
A degraded failure retaining the prior payload therefore cannot confirm a different
preset. Rejection, mismatched publication, or terminal rollback restores the prior
selection.

The History header renders three focusable controls with explicit accessible labels,
selected state, and pointer/Enter/Space behavior. The existing History rows and trend are
replaced once only for a newer product snapshot. Models and Projects receive their normal
single replacement from the same snapshot. Route-only switching does not rebuild any
model or submit work.

## Bounds, privacy, and performance

- Exactly three fixed presets: 1, 7, and 30 rolling civil days.
- At most 30 daily History rows and trend bars; existing Models and Projects caps remain.
- One published preset, one scalar high-water generation, one active correlation, and one
  latest pending intent; no history or collection.
- One existing capacity-one worker; no timer, cache, connection, thread, or query owner.
- One shared recent-usage analytics call per full refresh, or one section-local call per
  accepted range replacement; no duplicate Models/Projects query.
- System timezone remains query-owned and sampled through the existing exact calendar
  resolver. Missing/partial/cost evidence semantics remain unchanged.
- No free-form dates/counts, identities, paths, prompts, responses, reasoning, commands,
  credentials, or source contents enter the new intent/control state. Existing exact
  civil-date/range presentation remains intentionally visible in Slint.
- Stress tests cover 10,000 replacements without model/state growth and audit the exact
  fixed-capacity topology.

## Alternatives considered

1. **Recommended and selected: fixed 1/7/30 rolling presets.** It provides useful
   interaction while preserving the existing 30-row memory/render bound and exact query
   semantics.
2. **Expose 90/365/custom ranges.** The query facade supports up to 400 dates, but the
   current non-virtualized trend/table would materially increase UI objects and memory.
   This waits for a separately measured virtualized/downsampled design.
3. **Calendar day/week/month controls.** The controller would need a new query-clock-owned
   current-calendar request value or would risk reconstructing local dates outside the
   query boundary. Rolling labels are honest and require no new calendar authority.
4. **Separate History from Models/Projects.** This adds a third analytics request or a
   second retained envelope and violates the present shared-owner contract.
5. **Run a full refresh for every selection.** This repeats unrelated quota, benefit, Git,
   activity, and Sessions work and makes the interactive path slower without improving
   correctness.

## Acceptance

Focused contracts must prove preset validation, same-worker latest-wins admission,
refresh precedence, epoch/product/generation fences, successful shared-range replacement,
failure rollback, terminal no-snapshot recovery, exact UI enablement/keyboard behavior,
30-row/model bounds, and absence of new identity/query/thread/cache surfaces. Source audits
must reject widened presets, retained range collections, route-time queries, a third
analytics owner, missing stale fences, and missing application/terminal wiring.
