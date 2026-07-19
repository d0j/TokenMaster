# TokenMaster P3-D.5 Recent Activity Route Design

Status: approved for autonomous execution by the operator on 2026-07-19. This design
continues the product-first supporting-view rail after bounded History, Sessions,
Models, and Projects.

## 1. Goal

Replace the production Activity placeholder with a truthful, responsive, bounded
`Recent activity` view over the already-published `LatestActivityPage`. The route must
reuse the existing first-page request and immutable product section without adding a
query, worker, timer, cache, connection, archive owner, private identity, or route-time
work.

This is P3-D.5, not final WhereMyTokens rhythm parity. Hourly/day-of-week heatmaps,
timezone/DST aggregation, continuation, filters, event detail, export, skins, locales,
CLI/MCP, packaging, signing, and release acceptance remain separate slices.

## 2. Product intent retained and improved

The pinned WhereMyTokens reference presents activity rhythm and time distribution.
TokenMaster already has two different activity surfaces:

- a today-only Dashboard summary of eight fixed activity-category counters;
- an all-time newest-first `LatestActivityPage` of safe usage-event facts.

The Dashboard counters have no hourly/day-of-week distribution. The latest page has
timestamps and token usage but no activity categories and contains only the newest
bounded events. Neither can truthfully become a heatmap. P3-D.5 therefore exposes the
useful existing page as `Recent activity` and keeps full rhythm parity explicitly
open until a dedicated bounded aggregate query exists.

The route improves the raw query surface before presentation:

- only UTC timestamp, canonical model label, and explicit token components cross the
  Desktop boundary;
- provider/profile scope, event ID, cursor/fingerprint, and every source/session/path/
  content identity are omitted;
- incomplete first-page truth remains visible through `more activity available`;
- wide and narrow layouts share one bounded model and a complete accessible meaning;
- unavailable token values remain unavailable rather than display zeroes.

## 3. Options considered

### A. Pretend the latest 12 events are an activity rhythm

Rejected. A non-exhaustive newest-first page cannot prove hourly or weekday
distribution, and it contains no activity-category dimension.

### B. Combine Dashboard category counters with the latest event page

Rejected for P3-D.5. The Dashboard values cover today in the system timezone and depend
on aggregate readiness, while the latest page is all-time/newest-first and intentionally
remains available during aggregate rebuild. One combined route would hide different
evidence windows and readiness rules.

### C. Add a new hourly/day-of-week aggregate query now

Rejected for this slice. It requires an independently specified query/data contract,
timezone and DST composition tests, new product publication ownership, and performance
evidence. It is the correct future route to full rhythm parity, not a small frontend
change.

### D. Render the already-prefetched latest 12 events as Recent activity

Selected. It closes the placeholder truthfully, adds no backend work, preserves
aggregate-rebuild availability, and establishes the responsive/privacy boundary that
later pagination or rhythm exploration can replace deliberately.

## 4. Architecture and ownership

The existing capacity-one controller worker keeps exactly one request:

```text
LatestActivityRequest::first(PageSize(12))
```

The query facade already returns at most the requested 12 items plus internal
lookahead, newest-first, with explicit `has_more`. `ProductReducer` already publishes
one generation-compatible `ProductSection<QueryEnvelope<LatestActivityPage>>` and
retains a compatible last-good page on a later failure.

`DesktopActivityProjection::from_snapshot` becomes the sole frontend mapping site. It
selectively copies at most `MAX_ACTIVITY_ROWS = 12` safe facts, preserves query order,
maps header freshness/quality, and records page completeness. One accepted product
generation replaces one Slint model. Route selection only changes visibility.

Activity readiness continues to follow the existing product contract: data status,
the Activity section, and usage runtime health. It does not depend on aggregate
readiness, so Recent activity stays reachable during an aggregate rebuild exactly as
specified.

## 5. Public projection

`DesktopRecentActivityRow` contains only:

- UTC timestamp seconds and nanoseconds;
- canonical bounded model label;
- input, cached, output, reasoning, and total `DesktopTokenValue` facts.

`DesktopActivityProjection` contains only:

- section state and stable reason codes;
- optional freshness and quality;
- optional `has_more` page truth;
- one `Arc<[DesktopRecentActivityRow]>` capped at 12.

It never contains `QueryScope`, provider/profile, event ID, `ActivityCursor`, dataset
identity, publication identity, source/session/project/repository/account/workspace
identity, path, prompt, response, reasoning content, command, credential, or raw event.

## 6. State, ordering, and completeness

- Waiting without a payload: waiting, no evidence, no rows, page state unavailable.
- Unavailable without a payload: unavailable, stable failure reason, no fabricated
  empty page.
- Compatible retained payload after failure: degraded, retained rows/evidence plus the
  current stable failure reason.
- Ready authoritative empty page: ready, zero loaded rows, complete first page.
- Aging/stale/estimated/partial/conflict/unknown evidence: degraded with exact stable
  reason and usable rows retained.
- `has_more = true`: ready or evidence-degraded data with explicit `more activity
  available`; it is normal first-page incompleteness, not a fabricated complete archive.
- A payload exceeding the Desktop cap: first 12 rows retained, `activity_truncated`
  degradation, and `has_more = true`.

The query-provided newest-first order is retained. There is no frontend sort, cursor,
selection, or cached previous page.

## 7. Information design

The route title is `Recent activity`, with a fixed `UTC timestamps` context label.
The header shows state/reasons, freshness/quality, loaded count, and either `First page
complete`, `More activity available`, or `Page status unavailable`.

Wide rows show UTC time, model, input, cached, output, reasoning, and total. Narrow rows
show UTC time/model/total first and the complete input/cache/output/reasoning mix on a
second line. Both render from the same model. Full timestamp, canonical model, every
token component, and page semantics remain accessible; color is supplementary only.

The UI never calls this a rhythm, heatmap, hourly distribution, day-of-week view, or
complete archive.

## 8. Data, privacy, and correctness rules

- Preserve each `TokenCount` independently. Available zero is a legitimate zero;
  unavailable remains unavailable.
- Do not derive total tokens from partial components or replace an unavailable total.
- Timestamp formatting is UTC and fail-closed to `Unavailable` for an invalid value.
- Do not copy query-private fields and then hide them in Slint; they must be absent from
  the public Desktop projection itself.
- `has_more` is the only continuation fact crossing Desktop. The cursor stays behind
  the query/controller boundary.
- Empty, unavailable, retained failure, partial evidence, and incomplete page remain
  distinct states.

## 9. Performance and memory budget

- Zero new query calls, workers, threads, timers, queues, watchers, caches, connections,
  callbacks with authority, polling loops, animations, dependencies, crates, or schema.
- At most 12 retained Desktop/Slint rows and one internal query lookahead.
- One model replacement during initial construction or an accepted newer product
  generation; zero replacements on route-only switching.
- One `MainWindow`; wide/narrow changes presentation only.
- No retained page history, selection, detail, cursor, archive handle, or raw event.
- 10,000 projection replacements must release the previous row list.

## 10. Verification and acceptance

P3-D.5 is complete only when tests and audits prove:

- the controller still performs exactly one first-page Activity query of 12 and adds no
  query/runtime owner;
- waiting, unavailable, retained failure, empty, ready, evidence degradation,
  `has_more`, backend lookahead, and Desktop cap behavior are exact;
- row order and all five token components survive query-to-product-to-Desktop-to-Slint;
- provider/profile/event/cursor/fingerprint/source/session/project/path/content fields
  cannot cross the projection or compiled UI boundary;
- Activity remains reachable during aggregate rebuild;
- compiled Slint renders wide and narrow Recent activity from one model with complete
  accessible UTC/token meaning and no route-only rebuild;
- deterministic source and mutation audits, clean-root, formatting,
  warnings-as-errors Clippy, locked workspace tests, release composition, and one
  independent read-only review pass;
- specification, data/API/security contracts, traceability, decisions, parity, current
  state, handoff, roadmap, changelog, and history remain synchronized without claiming
  rhythm parity or release acceptance.

## 11. Future rhythm slice

Full WhereMyTokens activity parity requires a separate bounded aggregate contract with
explicit timezone ownership, hourly and day-of-week buckets, DST skipped/repeated-hour
semantics, exact ranges, evidence state, fixed point caps, and no raw-event export.
That slice may replace or augment Recent activity only after its query/product contract
and performance budget are independently approved.

## 12. Non-goals

P3-D.5 does not implement rhythm/heatmaps, category timelines, pagination, event detail,
scope/model filters, search, sorting, local-time selection, raw event export, prompts,
responses, commands, source/session/project links, aliases, skins, localization, tray,
notifications, CLI/MCP, plugins, packaging, signing, M0 acceptance, or release evidence.
