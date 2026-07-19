# TokenMaster P3-D.2b Exact Session Detail Design

**Status:** Complete in the production product/controller/Desktop/application path with
focused/release audits, independent review, and final locked workspace baseline passing.
M0, packaging, signing, and product-release acceptance remain separate gates.

## Goal

Add click-to-detail to the bounded Sessions route without exposing an opaque query key,
querying eagerly, creating another worker, or allowing a late result/controller restart
to display detail for the wrong row.

## Required identity axes

Three independent values are required:

1. `DesktopSnapshotEpoch` identifies one controller/bridge lifetime. It increases each
   time the application replaces the live backend bundle.
2. `ProductGeneration` identifies one immutable snapshot inside that epoch.
3. `ProductSessionDetailSelectionGeneration` identifies one UI selection and increases
   on every accepted row click.

Product generation alone is insufficient because a replacement controller restarts its
counter. The bridge therefore accepts a higher epoch regardless of the inner product
generation, rejects an older epoch, and applies the existing newer-only rule inside one
epoch. A higher epoch clears any active session selection before rendering its snapshot.

## Intent and query boundary

Slint sends only a zero-based visible ordinal. `DesktopState` validates the ordinal
against its current 64-row model, allocates a nonzero selection generation, records the
current snapshot epoch/product generation, switches the detail card to `loading`, and
submits one typed `DesktopSessionSelectionIntent`.

The application routes that intent to the currently installed controller through a weak
current-bundle reference. Bundle ownership uses `try_lock`; contention rejects immediately
rather than waiting on the UI thread. The controller rejects a mismatched snapshot epoch
or non-newer selection generation. It stores one
latest pending selection and one `refresh_pending` bit, then submits the existing
capacity-one worker. Repeated clicks replace the pending selection; refresh and detail
requests can coexist in the next coalesced execution. No per-click queue or thread exists.

On the worker, the controller resolves the ordinal from its current product Sessions
page only when both the viewed product generation and controller epoch still match. The
opaque dataset-bound `UsageSessionKey` is cloned only inside that execution and passed
directly to `QueryService::usage_session_detail`. It is never stored in Slint, a public
intent, logs, serialization, or the desktop projection.

After query completion, the result publishes only if its selection generation is still
the latest. A newer click suppresses the old result. Stale epoch/product generation,
missing row, query failure, cancellation, and missing detail remain explicit path-free
states; a new selection never retains the previous selection's detail as degraded truth.

## Product and presentation state

`ProductSnapshot` retains one optional identity-free selection correlation
(`selection generation + row ordinal`) beside its existing detail section. The reducer
publishes success/failure atomically for that correlation, rejects older attempts, never
retains another selection's payload, and still invalidates detail on dataset drift.

The desktop owns one `DesktopSessionDetailProjection` with states `idle`, `loading`,
`ready`, `missing`, or `unavailable`. Ready detail contains:

- the exact summary timestamps, duration, events, token buckets, total, and cost;
- freshness and quality from the exact query envelope;
- at most 32 model and 32 approved path-free project-alias breakdown rows;
- explicit truncation when the query or desktop cap omits additional breakdown rows.

The combined breakdown model is capped at 64. It contains display kind/label plus
aggregate event/token/cost values only. Provider/profile/session/source keys, cursors,
paths, prompts, responses, reasoning content, commands, and credentials remain absent.

## UI behavior

Rows are keyboard/focus accessible buttons with selected and hover states. They expose
explicit Tab navigation and Enter/Space activation. Clicking a
row updates selection/loading presentation synchronously; the query remains off the UI
thread and also transfers focus to the row. A detail card below the list shows exact
summary and bounded breakdowns in both wide and narrow layouts; every token component,
including reasoning, remains visible. Duration formatting uses exact nanosecond borrowing.
A rapid
second click immediately moves the highlight/loading state and makes the first result
ineligible. Route-only navigation still performs no query or model reconstruction.

## Bounds and acceptance

- Sessions page/model: 64 rows, unchanged.
- Detail breakdown model: 64 rows total, 32 per supported kind.
- One controller worker, one pending selection, one refresh bit, one product snapshot
  slot, one detail model, no detail cache/history.
- One monotonically allocated bridge epoch per backend bundle; overflow fails closed.
- Tests: epoch replacement/rejection, valid selection, stale generation, missing row/
  detail, query failure, rapid selection, refresh/detail coalescing, cancellation,
  application routing/restart/contention, projection bounds/privacy, duration borrowing,
  and real Slint pointer/Enter/Space interaction plus a Tab-binding mutation.
- Audits: one worker/slot, exact model/bounds/application sites, no UI query authority,
  no opaque identity, clean root, strict Clippy, full locked workspace tests.

## Rejected alternatives

1. Product generation without epoch: aliases after controller replacement.
2. Opaque keys in Slint: breaks the privacy/authority boundary.
3. Eager detail for 64 rows: multiplies indexed reads and retained memory.
4. A second detail worker: adds ordering/shutdown/resource surfaces.
5. Retaining old detail on failure: displays the wrong selection as degraded truth.
6. A click queue: stale work grows with input and increases latency.
