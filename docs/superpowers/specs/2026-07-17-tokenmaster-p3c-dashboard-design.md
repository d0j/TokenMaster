# TokenMaster P3-C Quota-First Dashboard Design

Status: approved for autonomous execution after the P3-B.3 gate.
Date: 2026-07-17.

## 1. Decision

P3-C delivers the first data-bearing production route: one quota-first Dashboard
composed from the existing immutable `ProductSnapshot`. It does not add a UI query,
runtime, timer, scanner, database handle, provider handle, or second snapshot slot.

The implementation order is deliberately:

1. add bounded all-current discovery reads for provider quota windows and benefit
   scopes;
2. replace the dashboard controller's empty exact filters with those discovery reads;
3. map one immutable product snapshot into one bounded dashboard presentation value;
4. apply six semantic Dashboard sections to the existing Slint window;
5. prove bounded mapping, stale-generation rejection, section-local degradation,
   privacy, source authority, and resource return.

This order is mandatory. `QuotaCurrentRequest::new(Vec::new())` currently means an
empty exact request, not “all windows”, and `BenefitCurrentRequest` requires a known
exact scope. Treating either as discovery would render plausible but false empty UI.

## 2. Product contour

The persistent header exposes today's owned usage truth and data state. The Dashboard
contains exactly six semantic sections in the approved order:

1. Plan Usage;
2. Code Output;
3. Usage and Cost Trend;
4. Sessions;
5. Activity;
6. Model Usage.

P3-C does not yet persist reorder/hide/collapse settings. It gives every section a
stable key, translation key, semantic color role, state, and bounded payload so P4 can
switch skin, layout, density, scheme, and locale without replacing its data contract.
The initial UI is English through translation-key fallbacks; English/Russian/pseudo
locale switching remains P4 and must not require a query or archive mutation.

## 3. Discovery before presentation

### 3.1 Quota overview

Add a separate `QuotaOverviewQuery` and `QuotaOverviewRequest`. They are not aliases
for an empty exact filter.

- one deferred read transaction captures the independent quota revision;
- one indexed ordered query discovers at most 32 current window keys plus one internal
  lookahead;
- a 33rd current window returns `capacity_exceeded`, never silent truncation;
- discovered keys are restored and validated from canonical provider/account/
  workspace/window columns, ordered by opaque scope identity and window ID;
- the existing exact current-window loader maps every key under the same transaction;
- the public result reuses `QuotaCurrentSnapshot` and explicitly reports its discovered
  filter list, freshness, quality, warnings, and revision;
- exact filtered `quota_windows` behavior remains unchanged.

The UI receives definition label keys and values, never account/workspace/window raw
identities. Duplicate labels remain separate rows and receive a presentation-only
ordinal such as `Account 1`; no stable private identifier is displayed.

### 3.2 Benefit overview

Add a separate `BenefitOverviewQuery` and `BenefitOverviewRequest`.

- at most 32 current scopes and 256 total current lots are captured in one deferred
  transaction plus one lookahead at each global bound;
- every scope and lot is restored through the same validators as the exact-scope
  query;
- scopes are ordered by opaque `BenefitScopeId`; lots remain FEFO ordered within a
  scope;
- the overview exposes owned immutable scope snapshots and reminder coverage but no
  provider account/workspace text;
- presentation aggregates only available banked-reset lots for the reset-credit badge,
  while credits, temporary usage, unknown benefits, and unavailable states stay
  distinct;
- a scope/lot overflow fails the benefit section closed and leaves quota/usage/Git
  siblings readable;
- exact-scope inventory/history APIs remain available for the later drawer and P3-D.

The overview is read-only. Discovery grants no activation, acknowledgement, reminder
profile mutation, provider action, or browser capability.

## 4. Dashboard presentation model

`DesktopDashboardProjection` is owned, immutable, and constructed only from a
`ProductSnapshot`. It contains no query/store/runtime types with authority and no
opaque private keys. It is retained only as part of the current `DesktopProjection`.

Hard presentation bounds are:

| Payload | Maximum retained rows |
| --- | ---: |
| quota windows | 32 |
| reset-credit summary scopes | 32 |
| trend points | 240 |
| recent sessions | 12 |
| activity categories | 8 fixed |
| model rows | 12 |
| code-output repository contribution | aggregated from at most 32; no row list |

The existing overview query plan requests only 12 dashboard session/activity rows.
P3-D creates separate keyset-page intents up to 256; it does not enlarge the Dashboard
projection.

Every section has `waiting`, `ready`, `degraded`, or `unavailable` state, a stable
bounded reason set, and `has_data`. A retained compatible payload with a new failure is
shown as stale/degraded, not silently ready. No missing scalar becomes zero:

- tokens are `unavailable`, `known`, or `partial` with coverage counts;
- cost is unavailable/partial/complete/legitimate-zero with provenance state;
- quota ratio is unavailable unless provider evidence supplies used or remaining;
- reset time and transition are optional;
- Git lines and efficiency are separate, with explicit completeness;
- an empty authoritative result is distinct from an unavailable result.

### 4.1 Header

The header shows today tokens, today cost, event count, data freshness/quality, and
refresh state. It never invents plan/account names. Provider plan text is shown only
after a future bounded provider value exists.

### 4.2 Plan Usage

Each row contains a bounded label key/fallback, presentation direction, used and
remaining parts-per-million when available, optional units, advertised reset time,
freshness, quality, confidence, and last transition kind/sequence. The visual bar is
derived from a checked 0..1,000,000 value. Fixed five-hour/weekly rows are forbidden.

Banked resets are a separate badge and summary under the card, not another quota bar.
It shows available quantity, nearest conservative expiry, reminder coverage, and
inventory freshness. No activation button is enabled in P3-C.

### 4.3 Code Output

The card checked-sums range commits, added lines, removed lines, and signed net lines
across the bounded repository set. It exposes range completeness, freshness/quality,
more-repositories state, and cost-per-100-added-lines only when the query marks
efficiency available. Repository IDs and paths never enter the model.

### 4.4 Trend

The card receives at most 240 ordered points with calendar boundaries, token
availability/value, cost availability/value, and precomputed checked maxima. P3-C
shows the plan's current range. Range switching remains a later bounded intent and
must use the same controller rather than a callback query.

### 4.5 Sessions

The Dashboard retains the newest 12 summaries. Each row exposes presentation ordinal,
start/end time, token/cost state, and bounded model summary when the query facade can
provide it without N+1 reads. Raw session IDs, event IDs, paths, prompts, and commands
are forbidden. The full virtualized page and detail belong to P3-D.

### 4.6 Activity and models

Activity uses the eight fixed aggregate activity categories; it does not render raw
recent event IDs or tool arguments. Model Usage maps at most 12 model breakdown rows,
preserves unknown aliases and pricing state, and uses accessible semantic series keys
rather than hard-coded colors.

## 5. Threading and responsiveness

Queries and product reduction remain on the one existing desktop worker. The event
loop performs only a bounded immutable projection conversion and one model replacement
per accepted product generation. Dashboard list caps keep this conversion independent
of archive size. There is:

- one controller worker;
- one runtime-observation slot;
- one product snapshot slot;
- one scheduled Slint event gate;
- no per-card timer, task, subscription, or history;
- no animation in P3-C that can run while idle.

P4 must measure visible-paint latency and may move pure formatting earlier if needed,
but it may not add a second worker or snapshot history. Numeric facts remain separate
from fallback display strings so localization does not require new queries.

## 6. Slint composition

The Dashboard is split into small code-native components and models:

- `dashboard-view.slint` owns responsive section layout and scrolling;
- one component per semantic card family;
- shared `section-state`, `metric`, `bar`, and empty/degraded-state components;
- `models.slint` contains fixed structs with stable semantic fields;
- `tokens.slint` supplies semantic roles only, not final skin packages.

The shell switches the content body by `active-route-key`. Non-Dashboard routes keep
their truthful route-state placeholder until P3-D/P3-E. At narrow widths sections stack
in one column; at wider widths they use a deterministic two-column arrangement. No
window is recreated during navigation or refresh.

## 7. Failure and privacy rules

- Discovery overflow, corrupt identity, deadline, or revision mismatch fails only its
  product section with a stable path-free code.
- A newer compatible failure may retain the last payload but must visibly degrade it.
- A durable revision/identity mismatch invalidates incompatible payload.
- The UI never receives account/workspace/window/lot/repository/session/event opaque
  IDs, even if their `Debug` output is redacted.
- Display text is bounded and contains no provider raw payload or wrapped error.
- Dashboard code adds no filesystem, SQL, process, HTTP, browser, shell, credential,
  activation, or arbitrary environment surface.

## 8. Verification

P3-C is complete only when focused tests and audits prove:

- empty exact quota filters remain empty, while the explicit overview discovers all
  current windows under one revision;
- 32 windows/32 benefit scopes/256 lots pass and each plus-one case fails closed;
- discovery ordering, same-transaction revision binding, corruption/deadline handling,
  and path/account redaction;
- six exact section keys/order and every waiting/ready/degraded/unavailable mapping;
- missing/partial values never display as zero or full/empty quota;
- 240 trend, 12 session, eight activity, 12 model, and 10,000 snapshot replacement
  bounds retain only one current model;
- the compiled headless Slint Dashboard shows real fixture values, dynamic quota rows,
  reset-credit separation, and in-place navigation without seeded data;
- source/release audits preserve one worker/slot/event owner, no polling/authority,
  no private identities/old project strings, and software renderer only;
- clean-root, format, warnings-as-errors workspace Clippy, all workspace tests, and
  doctests pass.

P3-C does not claim P4 skin/locale/accessibility/paint/resource completion, P5
automation, P6 packaging/signing, activation, M0 acceptance, or release.
