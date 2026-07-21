# TokenMaster P3-D.2c Bounded Sessions Pagination Design

**Status:** Approved for implementation by autonomous product direction. Completion
requires focused tests, independent review, and the locked workspace gates.

## Goal

Add later-page navigation to Sessions without exposing query cursors, retaining a page
history, growing UI models, creating another worker, or allowing a late navigation
result to replace a newer snapshot.

## User contract

Sessions opens on the newest all-time page, capped at 64 rows. When the current page has
more data, `Next page` replaces the visible page with the next older page. A continuation
page offers `Back to newest`, which replaces it with the first page. Ordinary product
refresh is also newest-authoritative.

There is deliberately no `Load more`, arbitrary page number, or previous-page stack.
Those designs either grow retained state or require cursor history. The product reports
`Newest page` or `Older sessions` instead of inventing a stable page number for an opaque
cursor.

## Identity and authority boundary

Slint sends only a typed direction: `next` or `newest`. The public desktop intent carries
the current `DesktopSnapshotEpoch`, `ProductGeneration`, and a monotonically increasing
navigation generation. It never carries a cursor, provider/profile/session key, path, or
source identity.

The controller accepts only its current epoch/product generation and a newer navigation
generation. For `next`, the worker resolves and clones the opaque continuation cursor
from the controller-owned current product snapshot immediately before querying. For
`newest`, it creates the existing first-page request. The cursor stays inside the
query/controller boundary and is never projected, logged, serialized, or retained by
the UI.

`UsageSessionPage` records only whether it is the newest or a continuation page. This
path-free boolean is enough for presentation and recovery; it does not expose cursor
contents or create navigation authority outside the controller.

## Scheduling and stale-result rules

Pagination uses the existing capacity-one desktop worker. Controller state retains at
most one latest pending navigation intent; repeated clicks replace pending work. A page
navigation request invalidates pending session detail because the selected ordinal is
page-relative. A regular refresh is newest-authoritative and supersedes navigation work
that has not started.

The worker publishes a page only when all three axes still match:

1. controller snapshot epoch;
2. viewed product generation;
3. latest navigation generation.

A newer refresh, backend replacement, or navigation intent makes an older result
ineligible. Cancellation and query errors remain explicit failures; neither old detail
nor a partial page is retained as current truth.

## Product and presentation state

Successful Sessions replacement atomically clears session selection and exact detail.
This prevents detail from a prior page being shown under a new ordinal. A failed page
query places Sessions in its existing explicit unavailable state and also clears detail.
`Back to newest` remains an always-bounded recovery action whenever navigation is idle;
ordinary refresh provides the same recovery path.

`DesktopSessionsProjection` adds only identity-free navigation facts: newest versus
continuation page, `has_more`, and navigation pending. `DesktopState` retains one active
navigation correlation so accepted input can synchronously disable both buttons and show
loading. Publication or epoch replacement clears that correlation. The row model remains
a single replace-only model capped at 64.

## UI behavior

The Sessions footer contains focusable `Next page` and `Back to newest` buttons:

- `Next page` is enabled only when Sessions is ready, `has_more` is true, and no
  navigation is pending.
- `Back to newest` is enabled on continuation/unavailable recovery states when no
  navigation is pending.
- accepted navigation immediately shows a pending status and disables both controls;
- replacement keeps focus behavior explicit and never appends rows;
- route-only switching performs no query and reconstructs no page model.

Pointer, Enter, Space, and explicit Tab navigation receive the same typed action. A
rejected application handoff restores the non-pending projection without blocking the UI
thread.

## Bounds, privacy, and acceptance

- One Sessions page/model: at most 64 rows.
- One pending navigation intent and one navigation correlation; no queue or cursor stack.
- One existing controller worker; no new thread, timer, cache, database, or query owner.
- One replace-only model update per accepted page publication.
- No cursor, opaque key, absolute path, prompt, response, reasoning content, command,
  credential, or source identity in Slint/public desktop state.
- Tests cover newest/next requests, no-cursor rejection, refresh precedence, rapid input
  coalescing, stale epoch/product/navigation generations, cancellation/failure, detail
  invalidation, backend replacement, application contention/rejection, projection bounds,
  privacy, and real Slint pointer/keyboard behavior.
- Audits pin the one-worker/one-slot topology, 64-row bound, no cursor leakage, no append
  model path, and exact application wiring.

## Rejected alternatives

1. `Load more`: retained rows grow with navigation and violate the fixed-memory target.
2. Previous/next cursor history: memory grows with depth and duplicates query authority.
3. Cursor in Slint or public intents: breaks the opaque identity and privacy boundary.
4. Stable page numbers: opaque continuation does not guarantee random access or a stable
   count, so page numbers would be misleading.
5. A pagination worker or page cache: adds ordering, shutdown, and memory surfaces without
   improving the bounded user contract.
6. Retaining prior-page detail: the ordinal/key belongs to a different page and can show
   false detail as current truth.
