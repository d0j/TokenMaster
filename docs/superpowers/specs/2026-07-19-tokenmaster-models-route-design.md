# TokenMaster P3-D.3 Models Route Design

Status: approved for autonomous execution by the operator on 2026-07-19. This design
follows the product-first supporting-view rail after bounded History and Sessions.

## 1. Goal

Replace the production Models placeholder with a truthful, responsive, bounded view
of model usage for the exact same recent 30-day range already shown by History. The
view must expose the complete aggregate token mix and cost evidence without adding a
query, worker, timer, cache, database owner, private identity, or route-switch delay.

This is P3-D.3, not final exploration parity. Interactive ranges, model aliases and
filters, per-model trends, drill-down, export, Projects, Activity, skins, locales,
CLI/MCP, packaging, signing, and release acceptance remain separate slices.

## 2. Reference behavior retained and improved

The pinned WhereMyTokens reference renders a compact top-model card with canonical
model label, token/cost values, and a relative usage bar. The pinned ccusage reference
adds per-model input, output, cache, total, and cost evidence inside bounded period
reports and adapts wide versus compact presentation.

TokenMaster combines those useful behaviors but improves their truth boundary:

- one exact recent 30-day range, canonical timezone, freshness, and quality header;
- canonical normalized model keys, not provider/profile/account/source identities;
- input, cached, output, reasoning, total, event, and typed cost evidence;
- explicit known, partial, unavailable, empty, and truncated states;
- text and geometry carry meaning; color is never the sole distinction;
- one immutable snapshot shared with History and the future Projects projection.

No source implementation is copied and neither reference becomes a dependency.

## 3. Options considered

### A. Reuse Dashboard's today-only model preview

Rejected. It would make the Models route a larger copy of the existing 12-row card,
would not match the recent History period, and would be too weak for comparison work.

### B. Add an independent 30-day Models query and product section

Rejected. History already resolves the exact required range. A second equivalent
query would duplicate aggregate and price work on every refresh, add publication
state, and make future Models and Projects range controls drift independently.

### C. Enrich the existing bounded History analytics envelope

Selected. The existing `recent_days(30)` request gains Model and Project breakdowns.
History continues to consume its daily series; Models consumes the Model breakdown;
Projects will later consume the prefetched Project breakdown. One query therefore
captures one coherent usage-exploration dataset and one dataset identity.

The current product field remains named `history` because its range, overview, and
daily series semantics are unchanged. Renaming the public section now would create
mechanical churn without changing ownership or behavior. The normative contract calls
it the shared recent-usage analytics envelope where cross-view meaning matters.

### D. Add the final mutable range/sort/filter scheduler now

Rejected for P3-D.3. It would add UI generation, persistence, cancellation, sorting,
and filter state before the fixed Models view proves the full vertical route. Later
range selection will replace this one envelope, so the selected design preserves the
upgrade path without speculative state.

## 4. Architecture and ownership

The existing capacity-one controller worker keeps the same number and order of query
calls. Only the fixed History request changes:

```text
recent_days(30), system timezone, daily series, no scopes,
breakdowns = [Model, Project]
```

The query/store boundary returns at most 256 Model and 256 Project items plus explicit
lookahead-derived truncation. Those arrays live only in the one current immutable
product snapshot. `DesktopHistoryProjection` still copies at most 30 daily rows.
`DesktopModelsProjection` copies at most 64 Model rows and no prior range, query key,
filter, sort state, provider/profile identity, or runtime owner.

Models route readiness follows the shared recent-usage section, aggregate state, and
usage runtime health. It no longer follows the today-only Dashboard analytics section.
A failed recent-usage query degrades both History and Models consistently while the
last compatible payload may remain visible with its failure reason. Dashboard remains
independent.

`DesktopProjection::from_snapshot` is the only Models mapping site. `apply_projection`
replaces one Slint model only on initial construction or an accepted newer product
generation. Selecting Models changes route presentation only; it cannot query,
rebuild the model, recreate `MainWindow`, or schedule background work.

## 5. Models information design

The route has four semantic regions:

1. Header: exact range, canonical timezone, freshness, and quality.
2. Overview: total tokens, cost, events, and loaded/completeness status for the shared
   range. Unknown values remain unavailable rather than zero.
3. Ranked distribution: a relative total-token bar for each row, ordered by the query
   contract (total tokens descending, then events and stable key).
4. Responsive table: model, input, cached, output, reasoning, total, cost, and events.

The desktop keeps the first 64 valid Model identities. `truncated` is true when the
backend breakdown has more items or the desktop cap discards any rows. The UI renders
`64 loaded - more available` rather than claiming an exact total. A ready breakdown
with zero items is an explicit empty range, not an error.

Wide layout shows all columns. Narrow layout keeps model, total, and cost prominent
and presents input/cached/output/reasoning/events in the row's second line; it does not
discard aggregate meaning or allocate a second model. Long model keys are ellipsized
visually while the full bounded key remains the accessible label.

## 6. Data, privacy, and correctness rules

- Accept only `UsageBreakdownIdentity::Model`; ignore any mismatched identity instead
  of coercing it into a model row.
- Preserve `AggregateTokenValue` known/partial/unavailable semantics component by
  component and preserve `CostResult` availability/provenance semantics.
- Never synthesize component sums or zero cost in the frontend.
- Do not expose provider, profile, source, account, workspace, project, session,
  opaque key/cursor, path, SQL, prompt, response, reasoning content, command,
  credential, or pricing internals.
- Model labels are canonical bounded `ModelKey` values. User display aliases belong
  to the later typed configuration contour; P3-D.3 does not add hard-coded aliases.
- Range and evidence labels derive only from the accepted immutable envelope.
- Backend ordering is retained; no unbounded or unstable frontend sort is introduced.

## 7. Performance and memory budget

- Zero additional analytics calls per refresh compared with P3-D.2b.
- Zero new threads, timers, queues, watchers, database connections, or callbacks with
  external authority.
- At most 30 History rows, 64 Models rows, 256 backend Model items, and 256 prefetched
  Project items in one current dataset; all prior datasets are replace-only.
- One complete Models Slint model replacement per accepted product generation and no
  replacement on route-only selection.
- Responsive layout uses the same bounded model and contains no idle animation.

The Project prefetch is intentional rather than speculative: it prevents the next
route from adding a third equivalent 30-day query. The fixed 256-item query cap and
single-snapshot replacement keep its cost explicit and constant.

## 8. Verification and acceptance

P3-D.3 is complete only when tests and audits prove:

- the controller still performs exactly two analytics calls (`today`, `recent_days`)
  and the recent request contains exactly Model and Project breakdowns;
- Models readiness follows recent usage, not today-only Dashboard analytics;
- waiting, unavailable, degraded-retained, empty, partial, ready, ordering, 64-row
  cap, query/desktop truncation, and mismatched-identity behavior are exact;
- every token component and cost availability survives product-to-desktop-to-Slint;
- real compiled Slint mounts Models instead of the generic placeholder in wide and
  narrow layouts without window/model reconstruction on route selection;
- public/frontend types and release strings contain no forbidden identity/authority;
- focused tests, clean-root, formatting, warnings-as-errors Clippy, locked workspace
  tests, and an independent read-only review pass;
- traceability, decisions, parity, current state, handoff, roadmap, changelog, and
  project history are updated without claiming complete P3-D, parity, or release.

## 9. Non-goals

P3-D.3 does not implement arbitrary dates, weekly/monthly aggregation, local sorting,
provider/profile/project filters, model aliases, per-model time series, session/project
drill-down, pricing mode controls, export, CLI/MCP, skins, localization, tray, alerts,
plugins, packaging, signing, M0 acceptance, or release evidence.
