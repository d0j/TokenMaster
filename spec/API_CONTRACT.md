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

`RuntimeWriterLease` bridges the engine port to `tokenmaster-platform` without exposing
a path or handle. Construction resolves one controlled local archive parent and one
persistent sidecar identity. Acquisition is non-blocking; only OS lock contention maps
to `busy`, while invalid sidecar/location and I/O failures remain stable path-free
codes. The returned erased guard owns exactly one locked file handle.

A replay append accepts at most 256 canonical events and at most 256 late session
relations. Observation/overlay work, relation reconciliation, selection invalidation,
continuation work, chunks, checkpoint, source state, and evidence epoch commit in one
immediate transaction. One accepted batch advances the epoch exactly once regardless
of relation count; any validation, database, or injected boundary failure rolls every
component back together.

The production composition is `tokenmaster-runtime`, not an engine or UI API.
`CodexAdapter` refreshes one bounded discovery snapshot, emits configured scopes,
and enumerates files twice without retaining JSONL descriptors. It supplies an opaque
manual `CodexCheckpointV1` envelope whose entire encoded size is at most 32 KiB and
whose decode is bound to the expected logical-file identity. `StoreArchive` bridges
only normalized identities, canonical batches, exact scan/replay handles, and stable
codes. Its checked ID translation is internal. `OneShotExecutor` remains the
bootstrap/full-rebuild path. `refresh_incremental` is the separate tail path and
returns `complete`, `partial`, or `rebuild_required` plus checked file/byte/event/
batch/diagnostic counters and archive generation. It performs an exact complete scan,
preflights every present source before tail writes, reads only after persisted
checkpoints, reports profile-scope drift as `rebuild_required`, and never exposes paths
or checkpoint bytes. Full rebuild may replace only an exact unadmitted provisional
generation left by interrupted admission.

`LiveRuntime::start` acquires the process-owned writer lease before SQLite open and
startup recovery, then creates the worker, a paused scheduler, and watcher before
opening admission and forcing the first reconciliation. Its fixed snapshot exposes
only lifecycle, scheduler, worker, watcher, latest stable refresh kind/outcome/error
code, and one immutable `EngineSnapshot`. The engine snapshot is seeded from current
archive truth before worker start and contains one checked in-process generation,
persisted archive generation, optional revision and exact scan-set IDs, exact scan-set
completion time as `data_through_ms`, publication quality, and fixed checked diagnostic
counters. It contains no query rows, history, path, source, connection, transaction,
watcher, or UI handle. Consumers replace a local snapshot only when
`candidate.is_newer_than(current)`.

Incremental work is selected only for replay-verified complete or partial publication;
typed rebuild-required falls through to full rebuild under the same permit and
pre-acquired guard. A newer archive generation is copied only after the archive write
and guard release. Equal/older candidates and busy/cancelled/deadline results cannot
advance the engine generation. `pause` closes admission and cancels the exact active
request, `resume` invalidates watcher assumptions and forces recovery, and `shutdown`
drops the watcher, joins the scheduler, then cancels/joins the worker. Debug contains
no archive or source path.

On Windows 8+, `SuspendResumeMonitor` owns the single process registration for
`RegisterSuspendResumeNotification`. The OS callback maps suspend and every supported
resume form into one capacity-one last-event-wins atomic signal; it never calls runtime,
opens SQLite, acquires a mutex, allocates, or creates a helper window/thread. A shell or
controller removes the pending event and passes it to `LiveRuntime::apply_power_event`.
Suspend is idempotent pause. Resume invalidates watcher assumptions and forces recovery
even if runtime is already logically running, covering a missed or coalesced suspend.
Registration and unregistration errors are stable and contain no OS handle or message.
Failed explicit shutdown keeps the registration guard active so cleanup can be retried.

Malformed, incomplete, or oversized relevant provider input is a blocking adapter
diagnostic. The live reader returns fixed `invalid_data` before checkpoint or batch
commit; a rebuild therefore remains failed/`recovery_pending` and preserves the prior
canonical publication until a later authoritative read completes. Non-blocking quality
diagnostics may still accompany an otherwise valid bounded batch.

`RefreshScheduler` owns one thread and one capacity-one wake. Its clonable
`RefreshHintSink` accepts only pathless filesystem/force/health signals and exposes no
event, root, source, request, or backend error. The fixed 250 ms quiet window, 15 minute
healthy poll, 60 second degraded poll, monotonic rollback handling, pause/resume, and
shutdown produce only `RefreshUrgency`. `BoundedFilesystemWatcher` owns at most one
current `notify` backend generation for at most 64 canonical roots. Replacement
invalidates old callbacks before dropping the old backend; snapshots expose only
generation and root count. Missing roots create no watch. Scheduler/watcher errors are
stable path-free codes.

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

`tokenmaster-query` owns the shared schema-v1 read values for UI, CLI, and MCP. Every
envelope carries a checked process-local snapshot generation, persisted publication
generation, separate dataset identity (`empty`, immutable legacy, or replay revision
plus dataset generation), exact generated/data-through time, freshness,
quality, at most 32 explicitly applied scope filters and 16 stable warnings, and one
owned payload. An empty scope-filter list means all scopes; the internal exact scan
manifest remains separately bounded and is not copied into every frontend snapshot.
Activity pages contain at most 256 items and expose only a fingerprint-redacted opaque
cursor.
Invalid bounds, capacity, stale identity, deadline, archive, version, overflow, and
internal failures use stable path-free codes. The facade clock supplies one exact wall/
monotonic sample; frontends do not supply publication time.

`UsageReadStore` is the only archive-read implementation behind that facade. It opens
an existing exact schema-v8 archive read-only, applies query-only/defensive/no-checkpoint
policy, performs no migration, and returns owned data after one short deferred read
transaction. Continuation requires its dataset identity. Current and immutable legacy
pages use composite keyset seek and at most one lookahead row; progress interruption is
cleared before the connection can serve another query.

`UsageStore::rebuild_aggregates_page` accepts from 1 through 2,048 canonical events.
The upper bound is a measured SQL work cap, not an allocation allowance: the method
retains only persisted cursor/progress state, writes generation-qualified staging rows,
and derives or cleans at most 18,432 rollup rows in one transaction. Zero or a value
above the exported hard cap fails before writes.

Aggregate requests use only fixed generation-qualified rollup queries. A state other
than `ready`, a generation mismatch, a stale dataset identity, or a bound/deadline
failure returns a stable unavailable/error code; it never triggers a raw-event
`GROUP BY`. Store requests use exact UTC half-open boundaries. Calendar/timezone
selection remains a private query-facade responsibility and cannot inject SQL or a
timezone file.

One store overview range is one to three non-empty adjacent aligned UTC segments. Each
segment selects a fixed minute or hour rollup and segments compose as one half-open
range in request order. The store rejects gaps, overlaps, misalignment, more than three
segments, more than 32 unique provider/profile scopes, or a deadline above two seconds
before reading. Header, aggregate state, and every segment are captured in one deferred
transaction; checked result addition cannot wrap.

The combined analytics store call returns overview, zero to 400 ordered series points,
and zero to four unique breakdown kinds from the same deferred transaction and active
aggregate generation. Non-empty series points must form an adjacent exact partition of
the overview range; a minute-aligned zero-duration point represents a skipped civil
date. Model, project, provider, and provider-qualified profile breakdowns are fixed
independent queries, each retains at most 256 items plus one internal lookahead and
reports truncation. Project absence is typed, not an empty user-facing string.

The session store API is deliberately all-time. First and continuation requests accept
at most 32 unique provider/profile scopes, retain at most 256 summaries plus one
lookahead, and use a two-second maximum deadline. Ordering is last UTC instant
descending followed by provider/profile/private-session identity ascending; the
continuation predicate mirrors that mixed direction and is bound to the exact dataset.
Session keys and cursors are cloneable opaque in-process values with redacted Debug and
no raw session getter. A continuation without the matching dataset is invalid, and a
changed dataset is stale. Exact detail requires the key's dataset identity and returns
either no matching detail or one summary plus model/project rollup collections, each
capped independently at 256 plus one lookahead. Page and detail capture publication,
ready aggregate generation, and payload in one deferred transaction, clear progress
cancellation before connection reuse, and never query `usage_event` or use `OFFSET`.

`QueryService::usage_analytics` accepts a validated today/day/week/month/custom range,
explicit IANA or resolved-system timezone, one of seven week starts, optional daily
series, at most 32 canonical unique scopes, and any unique subset of the four fixed
breakdowns. Custom and returned daily series are capped at 400 dates. It returns a
canonical zone identity, exact local-date/UTC boundaries, known/partial/unavailable
token facts, owned activity counters, series, and independently bounded breakdowns.
Jiff values never cross this facade.

`QueryService::usage_sessions` returns an owned 256-item maximum all-time page. Its
public cursor binds the exact dataset and canonical scope-filter set; a filter change
is `invalid_value` and a dataset change is `stale_snapshot` rather than a mixed page.
`QueryService::usage_session_detail` accepts only a previously returned opaque key and
returns typed absence or one owned summary with model/project breakdowns. Failed
calendar, rebuild, stale, or store captures do not consume process-local snapshot
generation.

`QueryService` is the only public archive facade in this contour. It allocates a
strictly increasing process-local snapshot generation only after a successful capture,
maps complete/partial/recovery/legacy truth explicitly, applies the 20-minute/2-hour
usage freshness policy, and downgrades a readable current revision with obsolete
accounting versions to `unknown` plus `accounting_version_stale`. A no-change
publication may advance publication/freshness while retaining dataset identity and a
valid cursor. Every canonical event mutation advances dataset generation in the same
transaction, so a cursor cannot cross a row-set mutation inside the same replay
revision. Replay evidence epoch remains separate CAS/provenance state and may advance
during a no-change scan without invalidating the cursor.
`QuerySnapshotSlot` retains one candidate and rejects older generations;
P3 wraps the synchronous facade with one bounded worker rather than calling SQLite from
a Slint callback.

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
