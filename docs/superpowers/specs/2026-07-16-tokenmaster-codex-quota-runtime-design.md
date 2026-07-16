# TokenMaster Codex Quota Runtime Design

**Status:** implemented and verified on 2026-07-16

**Scope:** deterministic native-executable discovery plus one dedicated, bounded
Codex quota refresh runtime. Benefit inventory/reminders/activation, UI, CLI, MCP,
and generic external provider plugins remain later contours.

## 1. Outcome

TokenMaster periodically refreshes the official local Codex quota snapshot without
coupling provider I/O to usage-history ingestion or holding SQLite state while the
Codex child is running.

The runtime sequence is fixed:

1. admit one bounded refresh request;
2. resolve one exact native Codex executable;
3. capture a positive wall-clock lower bound;
4. run the existing short-lived `codex app-server --stdio` transport;
5. discard the result if cancellation or a runtime deadline became effective;
6. acquire the existing cross-process writer lease without waiting;
7. open the archive and publish at most 32 normalized observations;
8. close the store, release the lease, and publish one redacted health snapshot.

Source failure never deletes or replaces prior quota truth. The last successful
samples continue to age through their stored freshness boundaries.

## 2. Chosen architecture

`tokenmaster-runtime` adds a separate `CodexQuotaRuntime`. It owns:

- one existing constant-state `RefreshScheduler` instance;
- one existing constant-state `RefreshWorker` instance;
- one Codex quota source composition;
- one store publisher using the shared archive writer lease;
- one latest-only, fixed-size health snapshot.

The generic engine worker and scheduler primitives are reused, but the existing
`LiveRuntime` usage worker is not extended and its execution object is not shared.
Quota transport latency, failure, cancellation, and health therefore cannot block,
fault, or mislabel usage ingestion.

The scheduler starts with one immediate recovery refresh, uses the existing
15-minute normal cadence, and may use the existing 60-second accelerated cadence
only for explicitly transient failures such as writer contention, process spawn,
deadline, early exit, or temporary unavailability. Schema, account, version,
configuration, and invalid-data failures remain on the normal cadence to avoid a
persistent child-process retry loop.

Manual refresh requests and resume events coalesce through the existing capacity-one
worker admission. No request payload, result history, per-window queue, or
unbounded retry state is retained.

## 3. Rejected alternatives

- **Extend `LiveRuntime` execution:** rejected because it acquires the archive writer
  lease before usage discovery/reads and would couple unrelated health and latency.
- **Attach quota I/O to the existing usage refresh request:** rejected because one
  slow or incompatible Codex app-server session would delay local usage ingestion.
- **Persistent app-server process:** rejected because a 15-minute read does not
  justify permanent process, pipe, memory, upgrade, suspend, and cleanup state.
- **New async runtime:** rejected because one bounded child session and two
  constant-state threads do not justify an executor or dependency surface.
- **One custom quota scheduler/worker implementation:** rejected because the existing
  coordinator already proves bounded coalescing, cancellation, pause/resume, panic
  containment, and latest-only completion publication.
- **Hold one long-lived writable SQLite connection:** rejected because the design
  explicitly forbids retaining SQLite/query state across provider I/O and must share
  the archive safely with other processes.
- **Make all windows one new store transaction:** deferred. The existing store
  contract already gives each independent quota window an exact idempotent
  transaction. The runtime holds the process writer lease across the bounded loop,
  records partial progress on failure, and never claims cross-window atomicity.

## 4. Executable discovery

Discovery is composition-owned; `tokenmaster-codex` continues to accept only an
already resolved `CodexAppServerCommand`.

Two modes exist:

### Explicit path

- An explicit path is authoritative.
- It is validated immediately through `CodexAppServerCommand::new`.
- Invalid explicit configuration fails startup; TokenMaster does not silently select
  another executable from `PATH`.
- The path is never serialized, logged, returned in an error, or shown by `Debug`.

### Automatic path

- Read the current process `PATH` at each poll so an install, upgrade, or path change
  can recover without restarting TokenMaster.
- Reject an oversized `PATH` or excessive directory count before search.
- Visit entries in platform order and test only the exact filename `codex.exe` on
  Windows or `codex` elsewhere.
- Do not resolve `PATHEXT`, shell aliases, PowerShell scripts, CMD shims, JavaScript
  wrappers, registry commands, browser state, or package-manager commands.
- Skip absent and invalid candidates; the first candidate accepted by
  `CodexAppServerCommand::new` wins.
- Return only stable path-private error codes: unavailable, invalid explicit
  configuration, or capacity exceeded.

Automatic discovery follows the launching environment's normal executable trust
boundary. Users or managed deployments that require a pinned binary use the explicit
path mode.

## 5. Source and publication boundary

The internal source interface has one operation:

```text
poll(observed_at_ms) -> bounded CodexQuotaSnapshot
```

The production source performs discovery, constructs the pinned transport with a
positive timeout no greater than the transport maximum, and returns owned normalized
observations. Tests replace the source behind a private runtime-only seam; no public
arbitrary command or plugin execution API is introduced.

The publisher owns:

- the canonical archive path;
- one `RuntimeWriterLease` factory;
- no open SQLite connection while idle or polling.

For one successful source snapshot it:

1. tries the writer lease once and returns `busy` without waiting;
2. opens `UsageStore`;
3. applies observations in their normalized deterministic order;
4. records started, advanced, duplicate, stale, allowance-change, and reset counts;
5. drops the store and guard before returning.

The lease spans the complete at-most-32-observation loop, so another TokenMaster
writer cannot interleave. Each observation remains independently transactional and
idempotent. If observation N fails, observations before N remain valid committed
provider facts; the health snapshot reports the processed count and the complete
refresh fails.

## 6. Lifecycle and cancellation

Public lifecycle mirrors the stable runtime model:

- `start`;
- `refresh_now`;
- `snapshot`;
- `try_completion`;
- `pause`;
- `resume`;
- `apply_power_event`;
- `shutdown`;
- idempotent `Drop` cleanup.

Pause closes admission, pauses scheduling, and cancels the active worker permit. The
current transport cannot interrupt its child session through the engine cancellation
token, so pause/shutdown may wait up to the already bounded transport timeout. The
runtime checks cancellation again immediately after transport I/O and publishes
nothing if cancellation won. No writer lease is held during that wait.

Resume forces one coalesced recovery refresh. Suspend maps to pause; resume maps to
resume or a recovery refresh when already running.

A panic in the worker faults only `CodexQuotaRuntime`. Scheduler and worker threads
are joined on shutdown. The transport remains responsible for its task-owned child
and helper-thread cleanup.

## 7. Health contract

Quota health is separate from usage-engine health. One copyable snapshot contains
only bounded non-sensitive data:

- runtime phase;
- scheduler phase and normal/accelerated retry mode;
- worker phase, active/pending state, and coalescing counters;
- latest attempt sequence and outcome;
- latest failure stage plus stable error code;
- observation, processed, changed, duplicate, stale, allowance-change, and reset
  counts;
- conservative observation time and bounded elapsed milliseconds;
- last successful observation time.

It never contains executable/archive paths, account identity, workspace identity,
window IDs, display labels, quota values, raw frames, provider messages, email,
credentials, reset-credit IDs, or inner OS/SQLite errors.

Failure is tagged by stage:

- discovery;
- clock;
- transport;
- publication;
- runtime control.

Transport retains the existing precise `CodexQuotaErrorCode`. Publication uses
stable busy/store/invalid/capacity categories. Debug formatting is path- and
payload-private.

## 8. Resource and performance bounds

- Threads while running: one quota scheduler plus one quota worker; one transport I/O
  helper exists only during a poll.
- Child processes: zero while idle, at most one during a poll.
- Pending refreshes: one active plus one coalesced follow-up through the existing
  worker coordinator.
- Latest health: one fixed-size snapshot.
- Source output: at most 32 observations under the existing transport cap.
- Writable SQLite lifetime: publication only, after transport completion.
- Writer wait: none; contention returns `busy`.
- Normal poll: 15 minutes.
- Accelerated retry: 60 seconds only for bounded transient classes.
- Transport deadline: default 15 seconds, never above 30 seconds.
- No unbounded vectors, channels, retry histories, logs, or retained raw JSON.

## 9. Security and privacy invariants

- No shell construction or caller-controlled arguments.
- No browser, cookie, dashboard scraping, private HTTP endpoint, auth-file read, or
  credential handling.
- Explicit and discovered paths remain process-local and non-serializable.
- Discovery never runs a `.cmd`, `.ps1`, JavaScript wrapper, or package manager.
- App-server stderr and raw stdout remain discarded/non-persistent under the existing
  transport contract.
- Account pseudonymization remains inside `tokenmaster-codex`; runtime health does not
  receive the account identifier.
- Store errors are reduced to stable codes.
- No source or store failure can mutate usage-history publication state.

## 10. Acceptance

The contour is accepted only when tests prove:

- explicit path authority and no fallback;
- deterministic exact-name `PATH` discovery, shim rejection, path bounds, and
  redaction;
- source I/O completes before any writer acquisition or store open;
- writer contention performs no store write and maps to `busy`;
- at-most-32 publication accounting, duplicate/stale handling, partial-failure
  accounting, and idempotent retry;
- cancellation after source I/O performs no publication;
- startup refresh, coalesced manual refresh, normal/accelerated cadence,
  pause/resume/suspend/shutdown, and panic containment;
- quota failure does not change usage runtime health;
- snapshots and errors contain no configured path, archive path, account data, raw
  response, or fixture-private values;
- no task-owned child/thread remains after focused tests;
- clean-root audit, formatting, strict clippy, and full workspace tests pass.

Benefit inventory, reset-credit expiration reminders, opt-in activation, quota UI,
skins, localization, CLI/MCP projections, and generic provider packages begin only
after this runtime contour is verified.

## 11. Implementation evidence

- Exact-native discovery, configuration redaction, and fail-closed public contracts
  pass.
- Execution, lifecycle, cadence, cancellation, partial-publication, and usage-runtime
  isolation contracts pass under strict runtime Clippy.
- The isolated Windows harness passed 16 warm-up plus 48 measured rounds covering
  success, RPC failure, forced timeout, writer contention, and pause/resume. It
  retained a 3,149,824-byte private floor with a 5,615,616-byte sampled high,
  131 handles, four threads, USER=1, GDI=0, and no task-owned fixture child.
- The release audit covered 114 production dependency packages, the production
  portions of six quota-runtime source files, and one release library with zero
  forbidden network/browser/cookie/private-endpoint/credential-file/shell/socket/
  direct-SQL or foreign-runtime matches.
- Clean-root, formatting, strict locked workspace Clippy, the complete locked
  workspace test/doctest baseline, and a concurrent usage-runtime/quota-worker fault
  isolation regression pass. Existing opt-in authenticated-live and explicit
  million-row tests remain intentionally ignored by the normal workspace command.
