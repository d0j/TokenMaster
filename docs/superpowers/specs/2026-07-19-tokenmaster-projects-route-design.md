# TokenMaster P3-D.4 Projects Route Design

Status: approved for autonomous execution by the operator on 2026-07-19. This design
continues the product-first supporting-view rail after bounded History, Sessions, and
Models.

## 1. Goal

Replace the production Projects placeholder with a truthful, responsive, bounded
project-usage view enriched by safe Git output facts. The route must consume the
already-published immutable recent-usage and Git envelopes without adding a query,
worker, timer, cache, database owner, path scan, private identity, or route-switch
delay.

This is P3-D.4, not final project exploration parity. Interactive ranges, project
aliases and associations, project detail, repository discovery/selection, filters,
sorting, export, skins, locales, CLI/MCP, packaging, signing, and release acceptance
remain separate slices.

## 2. Reference behavior retained and improved

The pinned ccusage reference offers project grouping, a named-project ordering with
an explicit unknown bucket, and configurable project aliases. The pinned
WhereMyTokens reference has session/project discovery and compact relative usage
patterns but no independent Projects screen in the reviewed revision.

TokenMaster retains the useful product semantics without copying source or inheriting
path-derived labels:

- named safe project aliases followed by one explicit `Unassociated` usage bucket;
- ranked recent usage with complete token mix, cost evidence, events, and a relative
  bar;
- optional matched Git commits, additions, deletions, net lines, and efficiency;
- explicit ready, partial, unavailable, empty, unmatched, and truncated truth;
- responsive wide/narrow layouts and full accessible row meaning;
- no repository path, basename guess, private ID, provider/profile/account identity,
  prompt, response, command, source content, or raw Git evidence.

## 3. The critical range decision

The two existing envelopes intentionally cover different periods:

- shared usage analytics: recent 30 local civil days in the system timezone;
- Git output: today as an exact UTC half-open day.

Changing Git to 30 days would make Dashboard's today card misleading. Adding a second
Git request would increase refresh work and publication state before interactive range
ownership exists. Silently joining recent usage and today's Git into one metric would
be mathematically false.

P3-D.4 therefore renders two explicitly labelled evidence windows in one Projects
route:

```text
Recent usage: <start> - <end> <system timezone>
Today code:   <start> - <end> UTC
```

Usage totals, token mix, cost, events, ordering, and relative bars belong only to the
recent window. Git commits/lines and cost per 100 added product-code lines belong only
to the UTC today window. The frontend never sums, rebases, interpolates, or relabels
one window as the other. Later interactive range ownership may replace both requests
with one generation-fenced range plan; this slice does not pre-implement it.

## 4. Options considered

### A. Usage-only Projects table

Rejected. It would reproduce ccusage grouping but ignore the already-available safe
Git/project association and make product readiness depend on Git without presenting
any Git value.

### B. Convert the existing Git request to recent 30 days

Rejected. Dashboard's code-output card is intentionally today-aligned. Reusing a
30-day Git result there would create a hidden cross-period comparison.

### C. Add a second recent-30-day Git request

Rejected for P3-D.4. It adds bounded but avoidable query/CPU/publication work and an
additional independently drifting range before the shared range scheduler exists.

### D. Present recent usage plus separately labelled today Git facts

Selected. It uses both immutable envelopes, preserves every boundary, introduces no
runtime owner, and gives Projects materially more value than a renamed Models table.

## 5. Architecture and ownership

The existing capacity-one controller worker keeps exactly the current query plan:

```text
analytics(today, Model + Project + Provider + Profile)
history(recent_days(30), Model + Project)
git(today UTC, max 32 repositories)
```

`DesktopProjectsProjection::from_snapshot` is the only join and mapping site. It
reads `ProductSnapshot.history` and `ProductSnapshot.git`, copies at most 32 recent
project rows, and scans at most 32 Git repository projections per row. The worst-case
join is therefore 1,024 exact alias comparisons per accepted product generation and
zero work on route-only selection.

The query/store boundary keeps at most 256 Project breakdown items with explicit
lookahead truncation. The desktop keeps the first 32 valid Project identities in
backend order. Git remains capped at 32 repositories. Only the current immutable
product and desktop projections survive; older `Arc` row models are replace-only.

Projects overall state combines the recent-usage and Git product sections:

- both waiting: waiting;
- both unavailable without retained data: unavailable;
- one usable and the other waiting/unavailable: degraded with usable facts retained;
- retained payload plus a newer failure: degraded with the exact failure reason;
- both ready and complete: ready;
- partial/stale/unknown/truncated evidence: degraded, never silently complete.

Product route readiness continues to require recent usage, aggregate health, and Git
health. No frontend state overrides that product contract.

## 6. Project identity and ordering

Only `UsageBreakdownIdentity::Project(ProjectAlias)` and
`UsageBreakdownIdentity::UnassociatedProject` can become rows. Provider, profile, and
model identities are ignored rather than coerced. A safe named alias is copied as a
bounded display string. The unassociated bucket is rendered as the literal
`Unassociated` and never matched to Git.

The query contract already orders named projects by total tokens, event count, and
stable alias and places the unassociated bucket explicitly. The desktop retains that
order and does not add a frontend sort. Git-only aliases are not appended in P3-D.4:
the route is usage-centric, and adding them would require a second ordering rule plus
zero-usage semantics. A named usage row matches Git only by exact safe `ProjectAlias`;
there is no basename, substring, path, fuzzy, or case-folded guess.

## 7. Git aggregation and efficiency

For each named usage row, all loaded Git repositories with the same exact alias are
aggregated with checked sums:

- repository count;
- commits;
- added and removed lines;
- signed net lines;
- worst freshness and quality;
- completeness across range completeness, quality, unavailable/rebuild state, and
  global repository truncation.

Zero matching repositories is `not linked`, not zero code output. Unavailable Git
retains recent usage and renders code facts as unavailable. If the repository query
has more results, every matched row and the code header remain incomplete because an
omitted repository could share the alias.

Repository efficiency values already prove an exact UTC range, exact project
association, complete Git/usage quality, compatible dataset identity, and complete
cost evidence. For one repository, that value is retained. For multiple repositories
with the same alias, the desktop exposes a project-level value only when every value
is available, all usage dataset identities and usage costs are identical, and product
code lines sum without overflow. It then computes once:

`round_half_up(project_usage_cost_micros * 100 / summed_product_code_added_lines)`.

The project cost is never multiplied by repository count. Any mismatch, unavailable
member, zero divisor, or overflow disables only efficiency and preserves independent
Git facts with an explicit stable reason.

## 8. Projects information design

The route has four semantic regions:

1. Header: overall state and exact recent-usage range/timezone/evidence.
2. Overview: recent total tokens, cost, events, loaded projects, and usage
   completeness.
3. Code evidence strip: exact UTC-today range, loaded repository coverage,
   freshness/quality, and truncation/completeness.
4. Ranked responsive project list: recent usage plus optional separately labelled
   today code facts.

Wide rows show project, relative usage, total, cost, events, commits, added, removed,
net, and efficiency, with the complete input/cached/output/reasoning mix on the row's
secondary line. Narrow rows keep project, total, cost, relative bar, full token mix,
events, and one `Today code` line. Both layouts use the same bounded model.

Long aliases are visually ellipsized while the complete bounded alias and all recent
usage/today-code semantics remain in the accessible label. Color is supplementary;
text and geometry carry state and quantity.

## 9. Data, privacy, and correctness rules

- Preserve `AggregateTokenValue` and `CostResult` availability/provenance component by
  component; unknown and partial values never become zero.
- Range/evidence labels derive only from accepted envelopes.
- Never copy Git repository ID, association ID, path, author, ref, commit/file
  identity, warning content, scope, provider/profile/account, dataset identity, source
  identity, key/cursor, SQL, prompt, response, reasoning content, command, credential,
  or pricing internals into the desktop or Slint model.
- Dataset identity may be compared transiently to validate multi-repository
  efficiency but cannot cross the projection boundary.
- Every count/sum/ratio uses checked arithmetic. Overflow makes the affected code
  evidence unavailable and degrades the section; it cannot wrap or crash.
- Missing breakdown, mismatched identity, no association, Git failure, and truncation
  are distinct states.

## 10. Performance and memory budget

- Zero additional query calls, threads, timers, queues, watchers, caches, connections,
  polling loops, idle animation, or route callbacks with external authority.
- At most 256 backend Project items, 32 desktop rows, 32 Git repositories, and 1,024
  bounded alias comparisons per accepted product generation.
- One Projects Slint model replacement on initial construction or an accepted newer
  product generation; none on route-only selection.
- One `MainWindow` and one model; wide/narrow layout changes presentation only.
- All labels derive from bounded domain values or fixed text. No unbounded formatting
  history or retained raw evidence exists.

## 11. Verification and acceptance

P3-D.4 is complete only when tests and audits prove:

- the controller still performs exactly two usage analytics calls and one existing
  today Git call, with no extra request or runtime owner;
- exact recent usage and exact UTC today code windows are both visible and never
  presented as one range;
- waiting, unavailable, degraded-retained, empty, partial, unmatched, unassociated,
  ready, 32-row cap, backend truncation, Git truncation, and mismatched identity are
  exact;
- multiple same-alias repositories use checked sums and never multiply project cost;
- every token/cost/Git availability state survives product-to-desktop-to-Slint;
- compiled Slint mounts Projects in wide and narrow layouts with a complete accessible
  label and no route-only reconstruction;
- frontend/public types and release strings contain no forbidden identity/authority;
- focused tests, deterministic desktop audit, clean-root, formatting,
  warnings-as-errors Clippy, locked workspace tests, and independent read-only review
  pass;
- specification, traceability, decisions, parity, current state, handoff, roadmap,
  changelog, and project history are synchronized without claiming full parity or
  release acceptance.

## 12. Non-goals

P3-D.4 does not implement project/repository detail, Git-only rows, repository lists,
path discovery, alias/association editing, arbitrary ranges, local sorting/filtering,
project trends, session drill-down, exports, CLI/MCP, skins, localization, tray,
notifications, plugins, packaging, signing, M0 acceptance, or release evidence.
