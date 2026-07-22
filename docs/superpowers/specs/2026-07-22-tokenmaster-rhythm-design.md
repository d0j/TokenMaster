# TokenMaster Bounded Rhythm Design

Status: approved for implementation on 2026-07-22 after one root contract audit and
one independent architecture/security review.

## Goal

Close the remaining P3-D activity parity gap with truthful hourly and day-of-week
usage distributions for the selected 1/7/30-day History range. Keep the existing
newest-first Recent Activity page independent and usable when aggregate data is not.

## Product contract

The existing recent `UsageAnalyticsRequest` optionally requests one fixed
`hour_and_weekday` rhythm. Dashboard requests no rhythm. The rhythm request is valid
only for a resolved range of at most 30 civil days.

A successful `UsageAnalytics` carries either no rhythm or one `UsageRhythm` with:

- exactly 24 stable local-hour buckets ordered 00 through 23;
- exactly seven stable weekday buckets ordered Monday through Sunday;
- checked `UsageMetrics` in every bucket;
- `elapsed_minutes` and `occurrence_count` in every bucket;
- the parent analytics range, canonical IANA timezone, dataset identity, freshness,
  and quality as its sole evidence envelope.

The rhythm contains no cost, identity, cursor, path, content, raw event, provider
payload, or authority. Empty token facts remain unavailable. A skipped local hour has
zero exposure; a present hour with no events has positive exposure and unavailable
token facts. Repeated local hours increase exposure and occurrence count and fold into
the same stable hour label.

## Exact timezone plan

`tokenmaster-query` owns timezone interpretation. It walks the selected UTC range in
minute steps without retaining those steps. Consecutive minutes with the same local
date, local hour, and UTC offset form one resolved local-hour occurrence. Offset is
part of the private occurrence identity, so a fall-back fold produces two occurrences
even when both have the same visible date/hour.

Each occurrence is converted into at most three aligned rollup segments: optional
minute prefix, hourly middle, and optional minute suffix. This preserves half-hour and
quarter-hour transitions without assigning a whole UTC hour to the wrong local hour.
The plan is capped at 768 occurrences and 2,304 segments. Exceeding either cap fails
closed. The public request cap remains 30 civil days.

## Store contract

`tokenmaster-store` extends the existing analytics capture; it does not add another
connection or transaction. One internally generated bounded `VALUES` CTE joins the
tagged segments to `usage_time_rollup`, groups the same facts into 24 hour and seven
weekday rows, and returns exactly 31 rows in canonical order. It uses only the active
aggregate generation, exact dataset kind, `dimension_kind = 'all'`, the request's
bounded scopes, and the existing two-second deadline.

No query may read `usage_event` or `usage_legacy_event`, use `OFFSET`, accept caller
SQL, or fall back while aggregates are unavailable. The store verifies that the
checked sum of all hour buckets and independently all weekday buckets equals the
overview metrics. Missing, duplicate, overflowed, or inconsistent rows fail the whole
analytics capture.

## Product and Desktop integration

The existing History analytics envelope carries rhythm. Range replacement remains
atomic and generation-fenced through the existing capacity-one worker. There is no
new product section, worker, connection, queue, timer, cache, or route-time query.

`DesktopActivityProjection` keeps two independent substates:

- Recent Activity reads `snapshot.activity()` and retains at most 12 newest events;
- Rhythm reads `snapshot.history()` and retains exactly 24 hour plus seven weekday
  rows when available.

A History/rhythm failure cannot erase or relabel Recent Activity evidence. A compatible
retained History envelope retains its complete rhythm with the same degraded evidence;
partial bucket vectors are never published. Activity remains reachable while aggregate
rebuild makes rhythm unavailable.

The Activity view renders a bounded hourly distribution, weekday distribution,
canonical timezone/range, exposure, event counts, and token availability. Wide and
narrow layouts preserve the same accessible meaning. Route selection remains
presentation-only.

## Acceptance

- UTC totals and order are exact.
- America/New_York spring gap and fall fold are exact.
- Australia/Lord_Howe half-hour transition, Asia/Kathmandu fractional offset, and
  Pacific/Apia skipped date remain exact.
- Hour and weekday sums each equal the overview metrics.
- Empty exposed and skipped buckets are distinguishable; partial token algebra remains
  partial.
- 1/7/30 range replacement changes rhythm atomically through the existing worker.
- Aggregate unavailable/stale/deadline/corruption has no raw-history fallback and
  clears the progress handler.
- Desktop publishes 24+7 bounded rows with separate Recent Activity/rhythm evidence,
  constant replacement, accessible timezone/range/DST meaning, and privacy canaries.
- One focused implementation review and one final re-review are the complete audit
  budget for this slice.

## Non-goals

No 168-cell retained heatmap cube, per-model/scope rhythm filter, arbitrary range,
event export/detail, cost distribution, new activity query, CLI/MCP surface, skin or
locale work, OS interaction, package, signing, soak, M0, or release acceptance is part
of this slice.
