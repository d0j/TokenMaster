# TokenMaster P1-C Provider-Neutral Engine Core Plan

**Status:** Complete. Tasks 1 through 5 are implemented and verified. Execution was
root-only, test-first, one writer, on the current feature branch. The available
task-name-only child surface cannot prove requested model routing, so
`MODEL_ROUTING_DRIFT` remains explicit.

## Goal

Add a small synchronous `tokenmaster-engine` crate that owns refresh admission,
coalescing, monotonic deadlines, cancellation, one-shot provider/store orchestration,
and stable bounded results. It must remain independent of Codex paths/JSONL, Slint,
async runtimes, external plugins, and the desktop lifecycle.

## Resolved boundaries

- `tokenmaster-engine` may depend on provider-neutral domain/accounting/store
  contracts. It MUST NOT depend on `tokenmaster-codex`, `tokenmaster-platform`, Slint,
  Wasmtime, Tokio, filesystem paths, or UI state.
- P1-C defines `Adapter`, `Archive`, `WriterLease`, and monotonic `Clock` ports and
  proves them with deterministic fakes. P1-D supplies the compiled-in Codex adapter
  and the real platform writer lease. This preserves portability and keeps OS handles
  out of the pure engine.
- Admission and execution are distinct. Admission is `started`, `coalesced`, or
  `deadline_exceeded`. A started execution terminates as `completed`, `busy`,
  `cancelled`, `deadline_exceeded`, or `failed`. Thus cancellation and adapter/store
  faults cannot be mislabeled as the older four-state shorthand.
- Timeouts use caller-supplied monotonic ticks only. Wall clock, quota reset time,
  filesystem timestamps, and UTC never decide runtime deadlines.
- While active, submissions retain one aggregate only: dirty/not-dirty, highest
  urgency, and a deadline that remains live while any coalesced request remains live.
  They never retain paths, source IDs, request history, or one queue node per hint.
- Cancellation is cooperative and checked between adapter callbacks/batches and store
  transactions. It never interrupts a SQLite transaction.
- A writer lease failure returns `busy` without starting provider I/O or mutating the
  archive. P1-C tests a fake lease; P1-D owns the OS implementation.

## Task 1 — coordinator contracts RED/GREEN

Create the workspace crate and failing public contracts for typed monotonic request
IDs, urgency, deadlines, cancellation token, admission, terminal results, stale-ID
rejection, and ID exhaustion. Implement a constant-state coordinator with at most one
active request and one aggregate follow-up.

Contracts prove:

- monotonically increasing checked IDs with no wrap;
- immediate deadline rejection without active state;
- 10,000 active-time hints collapse to one pending aggregate;
- highest urgency and live deadline merge deterministically;
- exactly one follow-up begins after completion;
- stale completion/cancellation cannot affect a newer request;
- explicit cancellation dominates a nominal success;
- expired pending work does not start;
- `Debug` and errors contain no path or provider data.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test coordinator_contract --locked
```

## Task 2 — bounded provider-neutral ports

Add sealed value types for scope/source identity, opaque adapter checkpoints, batches,
chunk proofs, counters, stable diagnostics, and completion quality. Define synchronous
callback/pull ports; the adapter never receives a store handle and the archive never
receives a provider descriptor or raw source bytes. Add compile-fail dependency and
privacy checks plus exact count/byte bounds.

Completed: sealed scope/source/checkpoint/proof/counter/diagnostic values, scope-exact
draft and canonical batches, object-safe callback/pull adapter plus archive/clock/lease
ports, bounded replay pages, stable coded errors, and compile-fail privacy/dependency
contracts. The normal engine graph contains domain/accounting only and excludes
Codex, platform, Slint, Tokio, Wasmtime, filesystem, and UI dependencies.

## Task 3 — one-shot executor TDD

Compose the coordinator, fake lease, fake adapter, accounting authority, and archive
port. Acquire the lease before provider I/O; begin one exact scan set; stream discovery
without retaining a source list; close truthful per-scope outcomes; begin replay only
from an all-complete set; process bounded batches; continue, seal, and promote; publish
one small result. On cancellation/deadline/fault, close the scan truthfully and discard
only the exact unpublished revision/epoch.

Contracts cover complete, zero-source retention, partial discovery, adapter failure,
busy lease, cancellation at every phase, deadline at every phase, store fault, stale
epoch, continuation bound, and prior-canonical readability.

Completed: the synchronous executor acquires the writer lease before provider work,
streams scope-exact discovery directly into one scan set, starts replay only from an
all-complete set, canonicalizes bounded batches, validates replay revision/epoch
continuity, and promotes one small result. Eighteen public contracts cover the listed
success/failure cases plus cross-scope rejection, non-progressing cursors/checkpoints,
cleanup failure, and lease-only `busy` semantics. Existing store contracts remain the
evidence that staging and failed exact discard leave prior canonical truth readable.

## Task 4 — deterministic worker shell

Add one optional dedicated worker abstraction using bounded standard-library channels:
one active operation, one coalesced follow-up, no worker per source, explicit shutdown,
and no detached thread. Prove burst, stale-result, shutdown, panic/error, and channel
backpressure behavior. Do not add an async runtime.

### Resolved worker design

Use one `RefreshWorker` thread with a shared mutex-protected `RefreshCoordinator`, a
capacity-one `sync_channel` carrying only a wake token, and a capacity-one result
channel. `submit` updates coordinator state directly; only `Started` stores one permit
and wakes the thread, while every `Coalesced` request changes the existing fixed
aggregate without allocating a command node. The worker executes a supplied
`FnMut(&RefreshPermit) -> RefreshOutcome`, calls coordinator `finish`, and immediately
runs at most the returned single follow-up. The callback returns status only; P1-E
later owns immutable publication of the full `OneShotResult` snapshot.

The result channel is latest-only. Publishing never blocks: if its one slot is full,
the worker removes the older completion, increments a checked fixed supersession
counter, and publishes the newer request ID. Public completion state contains only
request ID, outcome, execution kind, follow-up/deadline/capacity flags, and the
supersession count. It contains no generic payload, provider/source identity, path, or
history. A caught callback panic publishes a stable `Panicked` failure, marks the
worker `Faulted`, abandons any newly allocated follow-up, and exits; callers recreate
the worker after archive recovery. An ordinary `Failed` outcome remains recoverable
and may run the one coalesced follow-up.

Because Rust invokes the process panic hook before `catch_unwind`, first worker spawn
installs one wrapper that delegates non-worker panics to the prior hook and suppresses
output only for the thread-local marked worker. Application hooks must be installed
first and not replaced while workers exist. An outer boundary faults and clears the
fixed coordinator state for a non-callback worker-port panic. The engine rejects
`panic=abort` builds because they cannot implement this contract.

`shutdown` stops admission, cancels the exact active permit, wakes an idle worker, and
joins the owned `JoinHandle`. `Drop` performs the same cancel/wake/join fallback, so a
thread is never detached. Shutdown relies on the existing cooperative cancellation
contract and never force-terminates a task or transaction.

### Task 4.1 — burst, backpressure, and latest-only RED/GREEN

**Files:**

- Create: `crates/engine/tests/worker_contract.rs`
- Create: `crates/engine/src/worker.rs`
- Modify: `crates/engine/src/lib.rs`

**Interfaces:**

- `RefreshWorker::spawn(Arc<dyn Clock>, F) -> Result<RefreshWorker, WorkerError>` where
  `F: FnMut(&RefreshPermit) -> RefreshOutcome + Send + 'static`.
- `submit(RefreshUrgency, Option<RefreshDeadline>) -> Result<RefreshAdmission, WorkerError>`.
- `cancel(RefreshRequestId)`, `try_completion()`, and `snapshot()` expose only fixed
  state and stable errors.

- [x] Write contracts showing a blocked first task plus 10,000 hints retain one
  follow-up and execute exactly twice; a normal failed first task still permits that
  follow-up; and an unread result slot is replaced by the newest completion.
- [x] Run
  `cargo +1.97.0 test -p tokenmaster-engine --test worker_contract --locked` and verify
  RED because the worker API is absent.
- [x] Implement the capacity-one wake/result topology, fixed public values, stable
  error mapping, coordinator submission/finish loop, and non-blocking latest-only
  publication.
- [x] Re-run the focused contract and verify the burst/backpressure cases are GREEN.

### Task 4.2 — shutdown, stale IDs, panic, and ownership RED/GREEN

- [x] Add contracts proving cancellation before execution, stale cancellation cannot
  affect a newer active request, explicit shutdown cancels cooperatively and joins,
  `Drop` also joins, callback panic becomes bounded `Panicked`/`Faulted` state, and
  submissions after shutdown/fault fail with stable codes.
- [x] Run the focused contract and verify the new cases fail for missing behavior.
- [x] Implement pre-execution cancellation/deadline checks, phase transitions,
  panic containment without panic-payload exposure, idempotent shutdown, and the
  no-detach `Drop` fallback.
- [x] Re-run the focused contract, then
  `cargo +1.97.0 test -p tokenmaster-engine --locked` and strict engine Clippy.

### Task 4.3 — review and project truth

- [x] Review fixed memory/channel/thread ownership, race boundaries, stable Debug/error
  output, follow-up behavior, and worker recreation after panic against the approved
  P1 design.
- [x] Update API/data/security/decisions/traceability plus current state, roadmap,
  handoff, recovery, changelog, history, and this plan without adding a commit hash.
- [x] Run dependency/privacy audits and the full root quality gate before committing.

## Task 5 — documentation and acceptance

Update data/API/security contracts, decisions, traceability, current state, roadmap,
handoff, recovery, changelog, and history. Run focused tests, dependency/privacy
audits, then the root quality gate. P1-C completion does not claim Codex live
integration, filesystem watchers, a real OS lease, sleep/resume, M0 acceptance,
packaging, or a release.
