# TokenMaster P3-B Desktop Controller Design

Status: approved for execution from the approved product architecture and the
operator's explicit autonomous `go` instruction.
Date: 2026-07-17.

## 1. Decision

P3-B introduces one bounded desktop controller between `tokenmaster-query` and the
existing production desktop projection. It reuses `tokenmaster-engine::RefreshWorker`
instead of creating another scheduler, owns one `ProductReducer` on that worker, and
publishes at most one latest immutable `Arc<ProductSnapshot>` for the UI to consume.

```text
typed refresh intent
        |
        v
RefreshWorker (one active + one coalesced follow-up)
        |
        v
QueryService -> ProductReducer -> latest snapshot slot -> DesktopShell
                   (worker-owned)      (capacity one)
```

P3-B is deliberately split into three reviewable contours:

1. **P3-B.1 — controller core:** typed query plan, real QueryService adapter, one
   worker, coalesced refresh admission, one reducer, one latest-snapshot slot, stable
   redacted errors, and deterministic shutdown.
2. **P3-B.2 — UI event-loop bridge:** capacity-one Slint event scheduling that drains
   the controller snapshot without polling or blocking a callback.
3. **P3-B.3 — application composition:** resolve the configured archive root, compose
   live runtime plus query controller, and publish runtime health without duplicating
   ingestion ownership.

This change implements P3-B.1. It leaves the executable on its truthful initial
snapshot until P3-B.2 and P3-B.3 establish the remaining ownership contracts.

## 2. Options considered

### A. Query directly from Slint callbacks

Rejected. SQLite deadlines, recovery, and multiple section queries can block the UI
thread. Callback re-entry could also create unbounded pending work and make shutdown
nondeterministic.

### B. Create a worker per route or dashboard card

Rejected. It multiplies connections, threads, timers, result queues, cancellation
paths, and retained payloads. It also permits cards from different attempts to race
without a single product-generation authority.

### C. Reuse one engine refresh worker and one product reducer

Selected. The existing worker already proves capacity-one admission, coalescing,
cancellation, deadline checks, panic redaction, and deterministic shutdown. A single
reducer provides the existing identity and stale-generation rules. A capacity-one
latest slot keeps retained desktop state constant even when the UI is temporarily
busy.

## 3. Controller contract

### 3.1 Ownership

`DesktopController` owns exactly:

- one `RefreshWorker` and its one worker thread;
- one query source, normally a `QueryService`;
- one worker-confined `ProductReducer`;
- one immutable `DesktopQueryPlan`;
- one synchronized optional latest `Arc<ProductSnapshot>`.

The controller owns no Slint component, store writer, live ingestion runtime, file
watcher, notification scheduler, plugin runtime, network client, shell, browser, or
payload history. Dropping or explicitly shutting down the controller joins its
worker. The query source is never shared with the UI thread.

### 3.2 Typed query plan

One refresh attempt has fixed, bounded requests for:

- product data status, always first;
- usage analytics;
- current quota windows;
- optional current benefit inventory for one explicit `BenefitScope`;
- Git/output analytics;
- latest activity;
- first usage-session page.

The default overview plan uses bounded product limits: at most 240 chart points, 256
activity/session rows, and 32 repositories. Request types remain those of
`tokenmaster-query`; there is no arbitrary SQL, range expression, filesystem path,
or provider text in a desktop intent.

Benefit inventory remains optional because the current public query contract requires
an exact account/workspace scope while product data status intentionally does not
expose identity. P3-B.1 must not guess an identity, leak it into the UI, or broaden
the API silently. A future safe scope-discovery/all-current contract belongs to a
separate query change before the benefit card becomes production-ready.

### 3.3 Refresh admission

The public desktop API exposes typed urgency and stable admission/outcome enums; it
does not leak engine internals. At most one attempt is running and one follow-up is
remembered. Repeated hints update the single pending request rather than allocating a
queue. Every attempt number is monotonic and maps to one
`ProductAttemptGeneration`.

An attempt checks cancellation and its monotonic deadline between section queries.
The underlying QueryService retains its own bounded per-query deadlines. Cancellation
or deadline expiration before final publication discards the attempt's partial
visible result; no half-built snapshot replaces the last accepted snapshot.

### 3.4 Reducer publication

Product data status is queried and reduced first so it establishes snapshot identity.
Each sibling query then publishes or fails through the existing typed reducer method.
A section failure is a truthful product result, not a controller crash: other
sections continue and the final snapshot contains stable section-local failure codes.

Reducer outcomes still reject stale or identity-incompatible data. The controller
does not reinterpret product readiness. On completion it publishes one final reducer
snapshot by replacing the latest slot atomically. A slower consumer observes the
newest snapshot only; older unpublished snapshots are released.

Worker failure is reserved for controller invariants, panic, cancellation, or
deadline termination. Display errors are stable bounded codes and never wrap a path,
SQLite message, provider text, or panic payload.

## 4. Application and event-loop boundary

P3-B.1 exposes an `open(path, plan)` composition API for an already selected archive
path and a testable `spawn(source, plan)` API for typed sources. It does not invent a
Windows installed/portable archive location. The repository has no approved
production data-root policy yet, and choosing one implicitly would make upgrades and
portable mode unsafe.

P3-B.2 will add a Slint weak-handle bridge with one scheduled event and one replaceable
snapshot. The event-loop closure will call the existing `DesktopShell::apply_snapshot`
only on the UI thread. No timer, busy polling, per-card channel, or blocking callback
is allowed.

P3-B.3 will compose that bridge with the existing live runtime. `LiveRuntime` remains
the sole ingestion/watcher owner; the desktop query controller never duplicates tail
or archive writes. Startup must open/validate dependencies before showing live data,
and shutdown must stop admission, join the query worker, then close runtime ownership.

## 5. Boundedness and responsiveness

- Refresh admission is capacity one active plus one coalesced follow-up.
- Result retention is one optional `Arc<ProductSnapshot>`.
- The reducer retains one current product state, not attempt history.
- Analytics, repositories, activity, and sessions preserve query contract caps.
- No query or lock wait occurs on the Slint event thread in P3-B.1.
- Mutex critical sections only replace or take one `Arc`; query work runs outside.
- Repeated refreshes cannot accumulate callbacks, threads, snapshots, or channels.
- Shutdown is deterministic even after cancellation, query failure, or worker panic.

## 6. Security and privacy

- Paths are accepted only by the composition API and never enter snapshots, display
  errors, logs, or completion payloads.
- Prompts, responses, reasoning, commands, source contents, credentials, raw partial
  lines, and raw provider/SQLite/OS errors remain forbidden.
- The desktop controller adds no HTTP, shell, browser, arbitrary filesystem, plugin,
  or write authority.
- Query failures are reduced from stable `QueryErrorCode` values only.
- Refresh attempts cannot alter settings, activate reset benefits, acknowledge
  notifications, or perform any other action authority.

## 7. Verification

P3-B.1 is complete only when focused tests prove:

- a real empty schema-v13 archive produces a truthful final product snapshot;
- data status is reduced before sibling sections and one attempt maps to one product
  generation;
- a section failure remains section-local while independent sections publish;
- cancellation or deadline termination does not publish a partial snapshot;
- repeated hints coalesce to one follow-up and latest-result retention stays one;
- shutdown joins the worker and further admission fails with a stable code;
- controller errors contain no supplied absolute path or wrapped raw error;
- the deterministic desktop audit permits only the newly approved query/engine
  dependencies and rejects new runtime/store/provider/network/shell authority;
- clean-root, format, warnings-as-errors Clippy, and full locked workspace tests pass.

P3-B.1 does not claim a live-wired GUI, production archive-root selection, benefit
scope discovery, visible-paint latency, long-running resource soak, packaging,
signing, or release acceptance.

## 8. Closure review

The design was checked against the specification, data/API/security contracts,
current state, handoff, roadmap, the P3 desktop design, and the public query, product,
engine, and runtime APIs. The critical ambiguities are now explicit rather than
hidden: benefit queries need a safe scope contract, and application composition needs
an approved data-root policy. Neither blocks the bounded controller core.

P3-B.1 therefore has no remaining implementation ambiguity: reuse one proven worker,
keep one reducer and one latest result, publish only complete attempts, and keep all
blocking work off the GUI thread.
