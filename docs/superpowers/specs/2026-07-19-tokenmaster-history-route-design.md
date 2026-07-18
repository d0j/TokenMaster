# TokenMaster P3-D.1 History Route Design

Status: approved for autonomous execution by the operator on 2026-07-19 after the
product-direction audit selected a product-first History then Sessions rail.

## 1. Goal

Replace the production History placeholder with one truthful, responsive, bounded
30-day usage-history route. The route reads a separate immutable query section rather
than reusing the Dashboard's today-only analytics or issuing work from Slint.

This is the first P3-D exploration slice. Sessions/detail, interactive arbitrary date
ranges, Models, Projects, Activity, notifications, skins, locales, automation, and
release work remain later independently reviewable slices.

## 2. Options considered

### A. Render History from the existing today-only Dashboard analytics

This is the smallest diff, but it would label one day as history and would make the
new route visibly incomplete at birth. Rejected.

### B. Add one fixed bounded recent-history query to the existing refresh attempt

Selected. One additional `UsageAnalyticsRequest` captures the latest 30 local civil
days as at most 30 daily points, without breakdown queries. The result is published as
an independent product section and projected into one bounded Slint model. Dashboard
truth remains today-only and existing worker/lifecycle ownership remains unchanged.

### C. Implement the final interactive range/query scheduler now

This would add mutable range intents, query-plan replacement, cancellation races,
filter persistence, and range-specific caches before the first History screen exists.
It is eventually required but is rejected for P3-D.1 because it recreates the recent
infrastructure-first failure mode.

## 3. Architecture

The existing single query worker executes this fixed order in one refresh attempt:

```text
product status
  -> Dashboard analytics (today)
  -> History analytics (recent 30 local civil days)
  -> quota / benefits / Git / activity / sessions
  -> one immutable ProductSnapshot publication
```

`tokenmaster-query` adds `UsageRange::recent_days(30)`. Resolution occurs from the
query clock and requested timezone, so the controller does not calculate wall-clock
dates. The range is half-open `[today - 29 days, tomorrow)` and remains bounded from 1
through 400 days. DST or timezone transitions use the existing calendar resolver and
daily bucket semantics.

`ProductSnapshot` adds one `history: ProductSection<QueryEnvelope<UsageAnalytics>>`.
It has the same dataset-identity compatibility and stale-result rules as the current
analytics section, but failures remain local: a failed recent-history query must not
remove a healthy Dashboard payload.

`DesktopHistoryProjection` maps exactly one history section into owned presentation
values. It retains at most 30 daily rows and no query key, cursor, SQL handle, path,
runtime owner, or prior snapshot. The Slint event loop receives one complete model
replacement only after the product generation is accepted.

## 4. History information design

The route has four semantic regions:

1. A header showing the exact local date range, canonical timezone, freshness and
   quality evidence.
2. Overview metrics for events, total tokens, and cost. Unknown and partial values
   remain visibly distinct from legitimate zero.
3. A 30-point token/cost trend using the same semantic colors as Dashboard.
4. A descending daily table with date, events, total tokens, input, cached, output,
   reasoning, and cost.

The route shows explicit waiting, unavailable, degraded, empty, and ready states.
An empty exact range is legitimate zero data and is not presented as unavailable.
Partial token or cost evidence uses the stable partial label; it is never formatted as
a complete total.

P3-D.1 does not show non-functional range buttons. Interactive Today/7d/30d/90d,
calendar selection, filters, comparisons, and CSV/JSON export are added only with the
later bounded range-intent contour.

## 5. Data and error rules

- The request contains no scopes and no breakdowns; it therefore performs one bounded
  overview plus 30 daily series reads and their exact cost batch.
- `recent_days` rejects zero and values above `MAX_QUERY_SERIES_POINTS`.
- History route readiness depends on the new history section, aggregate readiness, and
  usage-runtime health. Dashboard readiness continues to depend on its own analytics.
- A history failure retains the last compatible payload with its stable error code.
- A dataset-identity change invalidates both Dashboard analytics and History
  independently.
- Date labels are derived only from public `CalendarDate`; no locale-sensitive parsing
  or absolute user path crosses the presentation boundary.
- Formatting uses checked/saturating presentation conversion and bounded strings.

## 6. Responsiveness and memory

- No new thread, timer, watcher, database connection, or Slint callback authority is
  introduced.
- The query worker remains capacity-one with one aggregate follow-up.
- The product reducer retains one current History payload inside the one current
  snapshot; the desktop retains one projection and Slint retains one 30-row model.
- A route switch performs no query, scan, or window reconstruction.
- History paint uses only fixed-size models and software-rendered Slint components.

## 7. Verification

P3-D.1 is complete only when:

- query tests prove exact 1/30/400-day resolution, zero/401 rejection, leap-day and DST
  behavior, and a maximum of 400 series points;
- product tests prove independent publication/failure retention, dataset invalidation,
  stale-attempt rejection, and History route truth;
- desktop projection tests prove waiting/unavailable/empty/ready/partial mappings,
  exact 30-row cap, newest-first table order, and no fabricated zero;
- compiled Slint tests prove the History route replaces the generic placeholder,
  displays real fixture values, switches without window recreation, and keeps all
  callbacks non-blocking;
- focused tests, clean-root, formatting, warnings-as-errors Clippy, and locked workspace
  tests pass;
- traceability, feature parity, current state, handoff, roadmap, changelog, and project
  history are updated without claiming full P3-D or 1.0 parity.

## 8. Non-goals

P3-D.1 does not implement Sessions/detail, arbitrary range controls, provider filters,
export, skins, localization, tray, notifications, CLI/MCP, provider plugins, packaging,
signing, or soak evidence. It does not refactor the 19-crate workspace or expand the
Reliable State contour.
