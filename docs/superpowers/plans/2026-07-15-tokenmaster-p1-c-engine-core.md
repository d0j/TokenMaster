# TokenMaster P1-C Provider-Neutral Engine Core Plan

**Status:** In progress. Tasks 1 and 2 are implemented and verified; Task 3 is next. Execute
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

## Task 4 — deterministic worker shell

Add one optional dedicated worker abstraction using bounded standard-library channels:
one active operation, one coalesced follow-up, no worker per source, explicit shutdown,
and no detached thread. Prove burst, stale-result, shutdown, panic/error, and channel
backpressure behavior. Do not add an async runtime.

## Task 5 — documentation and acceptance

Update data/API/security contracts, decisions, traceability, current state, roadmap,
handoff, recovery, changelog, and history. Run focused tests, dependency/privacy
audits, then the root quality gate. P1-C completion does not claim Codex live
integration, filesystem watchers, a real OS lease, sleep/resume, M0 acceptance,
packaging, or a release.
