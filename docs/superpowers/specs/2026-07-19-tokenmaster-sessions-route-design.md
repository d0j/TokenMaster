# TokenMaster P3-D.2 Sessions Route Design

**Status:** P3-D.2a list and P3-D.2b exact detail are implemented; later-page navigation
remains a separate future slice over the same bounded section.

## Goal

Replace the Sessions placeholder with the next real product route while preserving
TokenMaster's constant-state desktop boundary, query privacy, and instant route changes.
The first accepted slice is P3-D.2a: a bounded first page. P3-D.2b adds exact detail
only after the selection-generation contract is explicit and tested.

## Current facts

- `QueryService::usage_sessions` already returns dataset-bound, newest-first,
  keyset-paged all-time summaries with at most 256 rows and one lookahead.
- Each summary owns an opaque `UsageSessionKey`. Its raw identity has no getter and
  `Debug` is redacted. The product snapshot already retains this query page.
- `QueryService::usage_session_detail` accepts only a previously returned opaque key.
- The Dashboard currently requests and renders 12 identity-free session summaries.
- The Sessions route is still a generic placeholder. The desktop has one query worker,
  one latest snapshot, and route callbacks deliberately perform no query work.

## Decision

### P3-D.2a — bounded Sessions list

The controller requests one first page of at most 64 sessions on every accepted product
refresh. Activity remains capped independently at 12. Dashboard continues to copy only
its existing 12 summaries; the new Sessions projection copies at most 64 summaries.

The Sessions projection contains only:

- first and last UTC timestamp facts;
- event count;
- input, cached, output, reasoning, and total token availability/value facts;
- cost availability/value;
- page `has_more`, freshness, quality, state, and stable reasons.

It contains no opaque key, cursor, provider/profile/session identity, path, query
service, database handle, prior page, callback, or runtime owner. One Slint model renders
newest-first rows. Wide layout shows all token buckets; narrow layout shows last activity,
events, total tokens, and cost. Empty/unavailable values remain explicit. `has_more`
is visible truth, not an implicit claim that the first page is complete.

Route selection remains an in-place presentation update and performs no query, model
replacement, timer, animation, worker creation, or window reconstruction.

### P3-D.2b — exact detail selection

Detail is a separate immediately following slice, not an eager per-row query. It will add:

- a nonzero `DesktopSessionSelectionGeneration`;
- a UI-to-controller intent carrying only the selected bounded row ordinal plus the
  product generation that the user saw;
- controller-side ordinal-to-opaque-key resolution from the current product page;
- one latest selected key and one coalesced detail request on the existing worker;
- publication that binds result, selection generation, and opaque key;
- projection that displays detail only when the current selection still matches;
- stale-click, changed-dataset, missing-detail, cancellation, and rapid-selection tests.

The opaque key never becomes a Slint value or serialized field. No session detail is
queried eagerly, and no per-row query loop is allowed.

## Rejected alternatives

1. **Reuse the Dashboard's 12 rows as the complete route.** Too shallow and falsely
   suggests completeness when more sessions exist.
2. **Request detail for every visible row.** Violates bounded indexed-read intent,
   increases latency, and retains unnecessary detail.
3. **Send the opaque session key through Slint.** Breaks the privacy/wire boundary.
4. **Add a second detail worker.** Creates avoidable ordering, shutdown, and memory
   surfaces; the existing worker can serialize the selected request.
5. **Implement click-to-detail without a selection generation.** A slower old response
   could be rendered for a newer row after rapid selection.

## Bounds and acceptance

- session request/projection/Slint rows: 64 maximum;
- Dashboard session rows: unchanged 12 maximum;
- one query worker, one product snapshot slot, one Slint Sessions model;
- no prior page or detail cache in P3-D.2a;
- no raw/private identity in projection, UI, `Debug`, logs, or docs;
- focused controller/projection/UI RED/GREEN contracts;
- expanded deterministic desktop audit with mutation tests for the new bound, model,
  route-only application path, and forbidden authority;
- clean-root, format, strict Clippy, and complete locked workspace verification before
  claiming P3-D.2a complete.

## Follow-on order

1. P3-D.2a bounded Sessions list.
2. P3-D.2b generation-bound exact detail.
3. Models, Projects, and Activity routes.
4. Interactive History range replacement once these exploration intent patterns are
   stable and shared rather than independently improvised.
