# TokenMaster interface contract

Future interfaces are versioned, local-only, and bounded. Until implementation,
unlisted API, CLI, and MCP behaviors do not exist.

## CLI

The future CLI returns strict JSON for data commands and uses non-zero exits for
failure. Inputs use enumerated ranges, capped limits, and schema validation. Errors
use stable codes and bounded path-free descriptions.

## MCP

MCP uses stdio JSON-RPC. Every schema sets `additionalProperties: false`. It may expose
only bounded TokenMaster queries and an idempotent non-destructive refresh operation.
It MUST NOT expose arbitrary SQL, shell, HTTP, filesystem, credential, prompt,
response, or transcript operations.

A refresh result reports explicit scan-set and per-scope outcomes. Only an all-complete
set may advance archive freshness or start the production replay path; partial,
cancelled, failed, and timed-out results remain visible quality states and never
silently become success. Public surfaces expose bounded IDs, counts, timestamps, and
stable codes, not source keys or paths.

Refresh admission is `started`, `coalesced`, or `deadline_exceeded`. A started refresh
terminates as `completed`, `busy`, `cancelled`, `deadline_exceeded`, or `failed`.
Request IDs are checked and monotonic within one engine lifetime. Coalesced admission
does not imply a second queued operation; it contributes only to one bounded follow-up
aggregate.

The synchronous engine boundary is provider-neutral and object-safe. `Adapter`
streams owned validated scopes and discovered sources through callbacks and returns
at most 256 observations, 256 relations, 18 chunk-proof updates, and a 32-KiB opaque
checkpoint per pull batch. Every source identity includes a fixed logical-file key,
so files under one provider source root remain distinct. During full rebuild the
adapter lends one temporary `SourceBatchReader` while it still owns the path-private
descriptor; that reader cannot escape the callback. `Archive` receives only
discovered normalized identity, opaque checkpoint state, completion summaries, and
canonical accounting batches; it never receives a provider descriptor, path, store
handle, or raw source bytes. Stable port errors contain only enumerated codes.
Cancellation/deadline checks use the operation's caller-supplied monotonic clock and
occur between callbacks, pulls, and archive calls, never by interrupting a transaction.

`OneShotExecutor` acquires `WriterLease` before adapter or archive work. It retains
only the bounded scope manifest, one temporary reader/batch, opaque checkpoints,
fixed counters, and the latest exact replay handle. Discovery and rebuild sources are
written through and never collected in an engine list. Exact archive preparation and
seal prove second-pass membership; extra, duplicate, omitted, cross-scope, or
cross-logical-file input cannot publish. Unchanged non-terminal checkpoints, changed
replay revision identity, regressed epochs, and exhausted continuation work fail closed.
Only lease acquisition may produce terminal `busy`; a `busy` code from any later port
is an execution failure. Failure after replay begin attempts exact discard and reports
whether cleanup succeeded without masking the original stable error code.

`RefreshWorker` owns exactly one dedicated thread, one capacity-one wake channel, and
one capacity-one latest-only completion channel. Admission mutates the shared
constant-state coordinator directly; a coalesced hint allocates no command node and
wakes no additional worker. If the completion slot is occupied, publication removes
only that older completion, increments a checked fixed supersession counter, and
publishes the newer fixed result without blocking. Completion and snapshot values
contain only request identity, phase/outcome/kind, aggregate flags, and counters.

Worker phases are `running`, `shutting_down`, `stopped`, or `faulted`. Callback and
worker-boundary panics are contained, expose no panic payload through worker results,
and close admission; callback panic is reported as `failed`/`panicked` and abandons
the one allocated follow-up. The first worker spawn installs one process panic-hook
wrapper that delegates every non-worker panic to the prior hook and suppresses output
only for the thread-local marked TokenMaster worker. Product lifecycle code MUST
install any custom process hook before the first worker and MUST NOT replace it while
a worker exists. `shutdown` and `Drop` cancel cooperatively, wake idle work, and join
the owned thread; there is no detach or forced transaction interruption. The engine
rejects `panic=abort` builds at compile time because they cannot satisfy this API.

Automatic scan-history retention is an internal maintenance detail. Public refresh
results remain bound to their returned scan-set identity even if an older unreferenced
set is later pruned; no CLI or MCP surface exposes arbitrary pruning or row deletion.

Published freshness identifies the exact complete scan set that authorized its replay
revision. A zero-present-source publication is explicitly retention-only and reports
zero scanned sources without implying zero historical usage.

## UI data boundary

The UI consumes immutable bounded snapshots. It receives stable data-quality and
freshness states and never directly receives source paths or raw source content.

Quota snapshots expose current window epochs and a bounded transition page. Full
weekly resets include before/after values, maximum pre-reset use, old/new reset times,
transition kind, evidence source, confidence, and an exact or bounded detection time.
CLI and MCP use the same fields and stable transition sequence so automation can react
idempotently. Unavailable provider capacity remains `null`/unavailable, not zero or an
estimate derived from local token usage.

Benefit inventory snapshots expose bounded typed lots separately from quota windows:
benefit kind, quantity, target window, expiration value and precision, state, source,
freshness, confidence, activation capability, active bounded reminder profile and its
revision, reminder coverage, and nearest due time.
Bounded transition and activation-receipt pages use stable sequences. An identical
schema serves UI, CLI, and MCP reads; manual facts are explicitly marked and never
become official evidence.

The 1.0 CLI/MCP boundary is read-only for benefit inventory and pure policy evaluation.
Future activation is a separate host-owned mutation capability, not arbitrary HTTP or
browser control. It requires a strict provider/account/window scope, local consent,
expected inventory/policy revisions, deterministic idempotency key, durable intent,
and a reconciled receipt. No plugin or LLM may infer mutation authority from inventory
read access.

## Provider plugin ABI

The future external-provider ABI is `tokenmaster:provider@1` expressed in WIT and
executed only by an isolated `tokenmaster-plugin-host`. A provider component may expose
bounded metadata, health, discovery, scan-page, and quota-page operations. It returns
provider-neutral observation drafts, read-only benefit lots, and opaque checkpoints,
never canonical events,
fingerprints, replay dispositions, SQL, UI components, commands, or MCP tools.

Plugins receive no ambient WASI filesystem, network, environment, subprocess, or
stdio authority. Optional host capability imports provide scoped read-only filesystem,
allowlisted HTTPS, host-injected credential, and clock operations. All values and the
engine-to-host framed protocol use strict versioned schemas and hard byte/count/time
limits. The full package/runtime contract is recorded in
`docs/superpowers/specs/2026-07-14-tokenmaster-provider-plugin-system-design.md`.
