# TokenMaster P2-F Joined Product Status Design

Status: approved by delegated product authority on 2026-07-17 after contract and
architecture self-review.

## Goal

P2-F provides the bounded Slint-independent product state that P3 can consume without
opening SQLite, calling a runtime, retaining result history, or waiting for every
dashboard section before painting. It must make aggregate rebuild and independent
usage, quota, benefit, Git, and runtime availability visible without fabricating a
globally atomic revision that does not exist.

P2-F does not implement visible P3 routes, settings, notification presentation, CLI,
MCP, packaging, or release acceptance.

## Context and invariants

- Usage, quota, benefit, and Git facts have deliberately independent publication
  identities. A single scalar must not pretend that they committed together.
- Durable status facts can be read exactly in one short deferred SQLite snapshot.
  Runtime health is process-local and must retain its own generation and lifecycle
  identity.
- Analytics may be unavailable while aggregates rebuild even though activity and
  archive health remain readable. The product status owns the visible
  `aggregate_rebuilding` state; analytics must not fall back to raw-history scans.
- A slow or failed section must not hide healthy sections or block first paint.
- Every retained collection and every public reason/warning set stays bounded. No
  status or view model contains a path, source identity, repository identity,
  provider account identity, SQL text, command, prompt, response, or raw provider
  value.
- One product-state owner retains at most one current value per section. Replaced
  values are dropped immediately; no revision history, task queue, callback, timer,
  database handle, runtime handle, or Slint object is retained.

## Options considered

### A. One atomic mega-query for the complete dashboard

Read usage analytics, quota, benefits, Git, sessions, prices, and health in one SQLite
transaction and publish only when all succeed. This gives a superficially simple
revision but couples independent data families, lengthens WAL retention, makes the
slowest section the first-paint latency, and lets one unavailable subsystem blank the
whole product. Rejected.

### B. Let Slint stitch individual query results

Expose every facade directly to UI callbacks and let each route decide ordering,
failure, and replacement behavior. This duplicates state rules, risks older async
overwrite, couples business truth to layouts/skins, and is not reusable by CLI/MCP.
Rejected.

### C. Exact durable status plus a constant-state product reducer

Read only durable scalar status/identity facts in one short transaction, then feed
that status and independently versioned bounded section envelopes into a pure
Slint-free reducer. Publish a new immutable product snapshot after each accepted
change, while preserving every section's native identity. Selected.

## Durable data-status capture

`UsageReadStore::capture_product_data_status` uses the existing defensive read-only
connection and a caller deadline no greater than two seconds. One deferred transaction
captures and validates:

- usage archive generation, dataset generation, replay identity, exact complete-scan
  data-through time, archive publication quality, accounting versions, and whether a
  staging replay exists;
- aggregate state, active/expected generations, current/legacy event counts, rebuild
  progress, and only a stable bounded failure code;
- quota revision, retained counts, and last publication time;
- benefit revision, bounded current/pending/retained counts, and last publication
  time;
- Git publication revision, bounded repository/association counts, and last
  publication time.

The capture contains no quota window values, benefit lots, Git repository rows,
usage events, opaque account/scope/repository keys, or installation salt. Corrupt,
missing, cross-inconsistent, post-open schema-drifted, interrupted, or completed-late
reads fail closed with existing stable store errors. The SQLite progress handler is
cleared on every return.

Aggregate status is one of `ready`, `rebuild_required`, `rebuilding`, or `failed`.
Progress is present only for `rebuilding`, checked as processed no greater than total,
and never converted into a fabricated percentage when total is zero. A non-ready
aggregate does not make archive/activity status unavailable.

## Public query status

`QueryService::product_data_status` maps the capture into one owned schema-v1
`ProductDataStatusEnvelope` with a checked process-local snapshot generation and one
clock sample. It exposes four independently identified durable components:

- usage/archive and aggregate status;
- quota status;
- benefit/reminder-data status;
- Git-output status.

Usage retains the existing publication/dataset identity, freshness, quality, and
stable warning semantics. Quota, benefit, and Git retain their native revisions and
last-publication availability. Revision zero is a valid empty/not-yet-published state,
not an error and not a zero allowance/inventory/output value.

The outer generation means only "newer product-data status result in this process".
It never claims that independent component revisions were published atomically. A
failed capture or mapping consumes no generation. Public `Debug` is count/code-only.

## Product projection crate

A new leaf crate, `tokenmaster-product`, depends on immutable query/runtime/domain
values and has no Slint, SQLite, filesystem, provider, network, or platform authority.
It owns the product projection types and pure reducer used by P3.

The reducer has fixed slots for:

- product data status;
- usage analytics/overview;
- current quota windows;
- banked-reset inventory/reminder coverage;
- Git output/efficiency;
- latest activity;
- session page and optional exact session detail;
- runtime health and pending in-app reminder delivery metadata.

Each slot is `waiting`, `ready`, or `unavailable(stable_reason)`. A ready slot preserves
its native immutable envelope and identity. An unavailable slot contains only a stable
bounded reason and the last attempt generation; it never manufactures an empty
payload. Status updates invalidate a ready slot only when its durable identity is
proved incompatible. Freshness-only publication retains dataset-bound pages. A
missing or temporarily unavailable status does not destroy the last truthful payload;
the snapshot marks it retained/stale until incompatibility is proved.

The reducer increments one checked product generation only after an accepted change.
Equal/older section generations coalesce or reject without cloning the current
snapshot. Generation overflow fails closed. Updates are pure synchronous operations;
P3 owns one bounded query worker and sends no SQLite call through a Slint callback.

## Route readiness

The product snapshot derives data readiness for the fixed 1.0 routes: Dashboard,
History, Sessions, Models, Projects, Activity, Data Health, Notifications, Settings,
Help/About, and Compact Widget. This is data readiness, not a claim that P3 has rendered
the route.

Each route is `ready`, `degraded`, or `unavailable` with at most eight stable reasons.
Static Settings and Help/About remain data-ready without an archive. Data Health is
ready when product status itself is readable. Dashboard and Compact Widget degrade
section-by-section; missing quota or Git never hides usage. Aggregate rebuild makes
aggregate-backed routes unavailable while activity/data-health remain available.
Notification readiness follows benefit/reminder data and runtime delivery coverage,
not activation authority.

## Reactivity and ownership

P3 will paint the shell from the first product status snapshot, request only visible
route payloads, and publish each completed bounded result independently. One
capacity-one wake/latest-request query worker is the later UI execution boundary. A
route, range, filter, locale, skin, density, or layout change increments presentation
intent; an older async result cannot overwrite a newer intent. Skin/layout/locale
updates never mutate product data or trigger ingestion.

No product snapshot retains previous snapshots. Charts use existing 400-point bounds,
pages use existing 256+1 lookahead, quota uses 32 windows, benefits use 64 lots, Git
uses 32 repositories by 400 days, warnings use their existing caps, and route reasons
are additionally capped at eight.

## Performance and acceptance

P2-F is complete only when focused tests prove:

- one-transaction exact status under a concurrent writer commit;
- deadline interruption and progress-handler cleanup;
- aggregate ready/rebuild-required/rebuilding/failed truth and bounded progress;
- zero-revision empty semantics for quota, benefit, and Git;
- corruption/schema-drift/privacy failure closure;
- strictly newer product/section ordering and stale-dataset invalidation;
- freshness-only cursor/page retention;
- independent section failure and route degradation;
- 10,000 status/section replacements retain one value per slot;
- repeated open/capture/drop returns memory, handles, and threads;
- a deterministic large-archive status p95 below 25 ms, with no event/rollup scan in
  the query plan;
- clean-root, format, warnings-as-errors Clippy, complete workspace tests, and a
  focused dependency/source/privacy audit.

These developer gates prepare P3 but do not replace interactive paint, DPI,
accessibility, hibernation, or release soak receipts.

## Self-review

The selected design does not create a false global revision, add a long transaction,
move SQLite to the UI thread, duplicate presentation logic, discard healthy sections,
or introduce a new runtime queue. It gives aggregate rebuild a truthful visible home,
keeps UI first-paint independent from expensive route data, preserves exact native
identities, and gives later CLI/MCP the same bounded status truth. No known blocking
ambiguity remains for P2-F implementation.

