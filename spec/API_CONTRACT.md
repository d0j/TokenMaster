# TokenMaster interface contract

## Reminder policy command

`UpdateReminderPolicy` is a sealed, redacted, bounded operation-worker payload. The
application publishes Pending, saves portable desired state, then synchronizes the
single global profile; a successful durable save with an unavailable archive remains
retryable Pending. Settings projection exposes enable/disable, five recommended leads,
and up to eight normalized custom leads only. Per-scope editing, snooze, quiet hours,
OS/tray delivery, usage alerts, activation, CLI/MCP, and release APIs remain absent.

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

`SourceBatchReader::take_repository_activity_hint` is a separate transient side
channel beside the preceding pull. It returns at most one latest sealed activity
association and defaults to `None` for providers without that capability. The caller
must take it before the next pull. The value is never a field of `AdapterBatch`,
`CanonicalBatch`, `AdapterCheckpoint`, `Archive`, or a public refresh result. Taking
or dropping the hint cannot alter usage accounting or archive publication.

`GitRepositoryHintIngress` accepts one sealed transient hint, replaces the same
candidate, evicts the oldest candidate above the fixed 32-repository cap, and returns
only stable runtime errors. `GitRuntime` owns asynchronous `refresh_now`, count-only
snapshot, pause, resume, power recovery, and shutdown operations. It completes all Git
I/O before a non-waiting writer lease and store open, rejects superseded sequences,
and never scans on a caller/query/UI thread. `LiveRuntime` routes a successful Codex
reader side channel into this independent runtime; Git failure cannot change usage
accounting or its publication outcome.

`UsageReadStore::capture_git_output` returns one owned immutable bounded store
projection with an independent publication revision, monotonic publication time,
freshness/quality, all-time and requested-range totals/categories, retained daily
points, warnings, unavailable reasons, omission counts, exact project-association
availability, daily-retention boundary, range-completeness flag, and repository
one-row-lookahead flag. Requests accept 1-32 repositories, at most 400 inclusive days,
and a nonzero hard deadline no longer than two seconds. SQLite interruption and a
completed-late read both return `deadline_exceeded`, and the progress handler is
cleared before reuse.

`QueryService::git_output` maps this store capture into an owned schema-v1 product
envelope with a checked process-local snapshot generation and independent Git
publication revision. Its range is an explicitly labelled UTC half-open calendar
range. It exposes all-time/range totals, eight categories, retained days, freshness,
quality, stable warnings/unavailable reasons, retention truth, and exact 32+1
lookahead without a transaction, connection, path, ref, email, commit, file, process,
SQL, or command value.

The optional usage/cost join reuses the same resolved UTC boundaries, materialized
usage/project/price aggregates, and one fixed store-owned salted project matcher.
The salt and opaque project key do not enter the product envelope. Only complete
range/association/Git quality, non-stale compatible evidence, exact complete or zero
cost, and nonzero product-code additions produce fixed-point cost per 100 added lines.
Missing/conflicting association, retention, partial/stale/corrupt/unavailable usage
evidence, unknown/conflicting cost, and zero lines remain typed unavailable. A usage
projection failure cannot hide independent Git metrics.

One `git_output` call shares one wall-clock read budget of at most two seconds across
Git capture, aggregate usage evidence, price evidence, and project matching. It never
scans raw usage events or a repository. Only a successful completed envelope consumes
the next process-local snapshot generation.

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
an existing exact schema-v13 archive read-only, applies query-only/defensive/no-checkpoint
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

Price-basis store calls use only the active generation of the price rollups introduced
in schema v9 and retained unchanged by the current schema.
Overview plus series is one batch of at most 401 targets; a breakdown kind or session
page/detail batch has at most 256 targets. Every batch returns at most 512 ordered
price keys globally plus exact included/omitted metrics per target. Scoped range
queries use bounded scope values and the composite scope/range index. Dynamic SQL is
host-built only from fixed fragments and numbered parameters, is not statement-cached,
and never contains caller identifiers or expressions. Exact dataset mismatch is
`stale_snapshot`; no cost call may fall back to raw events or issue one query per
visible point/session.

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

Every analytics overview, series point, breakdown item, session summary, and session
detail summary carries one immutable `CostResult`. It exposes requested mode, truthful
availability, optional USD-micro amount, source composition, pinned catalog ID,
override revision/use, total/priced/reported/assumed/unpriced/omitted/conflict event
counters, and bounded missing reasons. `QueryService::open` uses the embedded catalog
and `auto`; `open_with_pricing` and `replace_pricing` accept only an already validated
immutable engine and explicit mode. A switch retains no engine history and affects
only later snapshots. Cost and token captures must have identical dataset identity and
event counts or the whole facade call fails closed.

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

`QueryService::product_data_status` is the joined scalar entry point for product
composition. It maps one schema-v13 `UsageReadStore::capture_product_data_status`
transaction into `ProductDataStatusEnvelope`, allocates a public generation only after
capture and mapping succeed, and exposes separate usage, aggregate, quota, benefit,
and Git revisions/states plus at most 16 stable warnings. The capture uses a maximum
two-second deadline and fixed scalar/indexed statements; it never scans usage events,
rollups, quota samples, benefit changes, or Git days.

`tokenmaster-product::ProductReducer` is the only P2-F joined publication owner. Each
data submission carries a nonzero `ProductAttemptGeneration` independent from its
source envelope generation. Older attempts are rejected. A compatible failure retains
the last successful payload and records only its stable path-free code; a durable
identity change invalidates an incompatible payload. Runtime observations use an
independent nonzero `ProductRuntimeGeneration` and copy only bounded lifecycle,
scheduler, worker, retry, count, stable-code, and pending-state projections.

The reducer retains one current `Arc<ProductSnapshot>` and derives exactly 11 fixed
route statuses with `ready`, `degraded`, or `unavailable` state and a `u16` reason set.
Settings and Help/About require no archive. Data Health remains reachable whenever the
joined status is readable. During aggregate rebuild, Dashboard degrades section by
section, Activity and Data Health remain reachable, and History, Sessions, Models, and
Projects are unavailable. The reducer owns no SQLite connection, runtime, worker,
callback, path, provider identity, queue, or snapshot history. P3 may marshal only this
bounded snapshot onto the Slint event loop.

`tokenmaster-app` is the sole production application composition boundary.
`ApplicationEnvironment` captures only the current executable, `LOCALAPPDATA`,
`USERPROFILE`, and `CODEX_HOME`; production accepts no archive argument, CWD fallback,
or general environment-selected data path. `DataRoot::resolve` returns one canonical
local directory and archive filename under the exact installed/portable marker policy.
Its errors and `Debug` output are stable and path-free.

`RefreshWorker::spawn_notified` accepts one optional
`Arc<dyn WorkerCompletionNotifier>`. After publishing the capacity-one completion
receipt and outside worker locks, it sends one copied lossy completion hint. Notifier
panic is caught/redacted and cannot fault the worker or remove the receipt. Live,
nested Git, quota, and reminder runtimes expose additive `start_notified` constructors;
their existing `start` constructors retain identical no-notifier behavior.

`DesktopRuntimeObservation` carries one `ProductRuntimeGeneration` and exactly four
copied `Result<...Health, ProductRuntimeObservationError>` values. The controller
retains at most one pending observation, rejects equal/older generations, applies the
latest value only on its existing query worker, and publishes it only with a complete
non-cancelled `ProductSnapshot`. The desktop API receives no runtime owner or runtime
source type.

`QueryService::quota_overview` and `QueryService::benefit_overview` are the explicit
all-current read APIs used by the Dashboard. They are not aliases for empty exact
filters. The desktop query worker calls them once per accepted attempt and publishes
their immutable envelopes through `ProductReducer`; a failure is local to its product
section and may retain only compatible last-good truth as visibly degraded.

`DesktopDashboardProjection::from_snapshot` is the sole product-to-Dashboard mapping.
It produces exactly six ordered section projections and bounded owned row sets with
stable semantic and translation keys. `apply_projection` runs during initial window
construction and then only for an accepted newer product generation; each application
replaces the seven Slint list models once. Route
callbacks call the route-only projection path; they cannot rebuild Dashboard lists,
invoke the query facade, create a timer, or recreate `MainWindow`.

`UsageRange::recent_days(day_count)` is the bounded default-history request and rejects
zero or more than 400 days. `DesktopQueryPlan` owns one fixed 30-day daily History
request with exactly Model and Project breakdowns alongside the today-only Dashboard
request. Both execute sequentially on the existing capacity-one query worker; no third
Models or Projects analytics request exists. History success/failure publishes only the
independent product History section, and cancellation or deadline still discards the
entire unpublished attempt.

`DesktopHistoryProjection::from_snapshot` is the sole product-to-History mapping. It
copies one overview, exact resolved range/timezone/evidence, and at most 30 daily rows
newest-first. `apply_projection` replaces the single History Slint model only on
initial construction or an accepted newer product generation. The History route
callback remains route-only and cannot query, allocate a prior-range cache, create a
timer/worker, or recreate `MainWindow`.

`DesktopModelsProjection::from_snapshot` is the sole product-to-Models mapping and
reads that same recent-usage section. It copies the shared overview/range/timezone/
evidence and at most 64 canonical model rows with all typed token components, events,
and cost availability/mode/composition. Slint preserves token/cost availability and
renders actual cost composition as calculated, reported, or mixed in visible and
accessible labels. Backend or desktop truncation remains explicit. `apply_projection` replaces
the single Models Slint model only during initial construction or an accepted newer
product generation. Models route selection is presentation-only and cannot issue a
query, rebuild the model, add mutable range/filter/sort state, create a timer/worker/
cache, or recreate `MainWindow`.

`DesktopProjectsProjection::from_snapshot` is the sole product-to-Projects mapping.
It reads the same recent-usage section plus the existing UTC-today Git section, copies
at most 32 usage-centric Project rows, and performs at most 1,024 exact safe-alias
comparisons per accepted product generation. It never queries. `Unassociated` never
matches Git and Git-only aliases do not become zero-usage rows. Same-alias repository
commits/lines use checked sums; combined efficiency validates identical transient
usage identity/cost and counts project cost once.

The projection exposes separate recent-usage and UTC-today code ranges, timezones,
evidence, completeness, and truncation. `apply_projection` replaces the single
Projects Slint model only during initial construction or an accepted newer product
generation. Projects route selection is presentation-only and cannot issue analytics
or Git work, rebuild the model, add mutable range/filter/sort state, create a timer/
worker/cache/connection, or recreate `MainWindow`.

`DesktopActivityProjection::from_snapshot` is the sole product-to-Activity mapping.
It reads the already-published latest Activity page, copies at most 12 newest-first
rows containing UTC timestamp, canonical model, and typed input/cached/output/reasoning/
total tokens, and preserves optional `has_more`, freshness, quality, reasons, and
explicit truncation. It exposes no opaque row identity or provenance. The existing
`LatestActivityRequest::first(12)` remains the only Activity request and runs on the
same capacity-one refresh worker. One Activity Slint model is replaced only during
initial construction or an accepted newer product generation. Route selection cannot
query, rebuild the model, retain prior pages, create selection/filter/detail/export
state, add a timer/worker/cache/connection, or recreate `MainWindow`.

`DesktopNotificationsProjection::from_snapshot` is the sole product-to-Notifications
mapping. It reads the existing benefit overview once and copies at most 32 effective
profile rows, 256 separate current-lot rows, and eight leads per profile. Typed expiry
precision, evidence, policy source/coverage, warnings, freshness, quality, and explicit
frontend truncation remain intact. One scope model and one lot model are replaced only
during initial construction or an accepted newer product generation. Route selection
cannot query, rebuild either model, take/acknowledge/release a leased reminder batch,
schedule a notification, mutate settings, activate a benefit, create a timer/worker/
cache/connection, or recreate `MainWindow`. A future delivery bridge must remain
application-owned, acknowledge only after successful UI presentation, and release the
lease on any failed or cancelled presentation.

`HelpAboutView` is a static presentation-only route and deliberately has no product
snapshot, archive, query, model, callback, or runtime API. `DesktopShell` passes only
`env!("CARGO_PKG_VERSION")` once immediately after `MainWindow` construction. The
view instantiates six fixed accessible sections and exactly one standard pinned
`AboutSlint`; wide/narrow reflow changes positions only and allocates no replacement
model. Route selection changes visibility in the existing window. TokenMaster exposes
no URL property, arbitrary-link callback, browser/session bridge, dynamic diagnostics,
release-channel lookup, or package/signing/SBOM assertion through this route. The
standard widget's fixed Slint attribution action is the sole library-provided license
surface and does not widen TokenMaster provider or automation authority.

`DesktopQueryPlan` also owns one all-time `usage_sessions` request with page size 64.
It executes sequentially on the same capacity-one query worker; Dashboard copies only
its first 12 summaries while the independent product Sessions section retains the
bounded page and `has_more`. Sessions failure remains section-local and complete-attempt
cancellation/deadline rules are unchanged.

`DesktopSessionsProjection::from_snapshot` is the sole product-to-Sessions mapping. It
copies at most 64 identity-free summary rows and explicit continuation availability.
`apply_projection` replaces the single Sessions Slint model only during initial window
construction or an accepted newer product generation. Route selection is presentation-
only and cannot rebuild the list, issue a detail query, create a timer/worker, or
recreate `MainWindow`.

P3-D.2b adds `DesktopSessionDetailIntent` with snapshot epoch, viewed product generation,
and identity-free selection generation/ordinal. The UI adapter changes highlight and
loading state synchronously, then submits that value to an installed typed sink. The
application sink admits it only through a nonblocking acquisition of the current live
bundle controller and rejects contention, safe-mode, missing, stale, deadline, or closed
ownership. The controller retains one
latest selection slot and multiplexes it with refresh work on the existing capacity-one
worker. Only inside that worker, and only while epoch/product generation still match, is
the ordinal resolved to `UsageSessionKey` and passed to `usage_session_detail`. Result
publication is latest-selection-only; cancellation, stale work, missing rows, failures,
and bundle replacement cannot surface another row's detail. No callback performs the
query, no click queue/cache/thread exists, and one bounded detail Slint model is replaced
in the existing `MainWindow`.

`CodexAppServerCommand` accepts one already resolved absolute native executable path.
The path must name a regular non-reparse file; Windows additionally requires an
`.exe`. It is canonicalized but has no public getter and its `Debug` output is
redacted. `CodexQuotaTransport` accepts that descriptor plus a positive timeout no
greater than 30 seconds and exposes one synchronous `poll(poll_started_at_ms)`
operation. The caller captures this wall-clock lower bound immediately before
admission. It is validated before process creation and becomes the conservative
observation time, so transport duration can age evidence slightly but can never
overstate freshness. The transport never accepts caller arguments, shell text,
environment overrides, endpoints, headers, credentials, or arbitrary protocol
methods.

One poll starts exactly `<executable> app-server --stdio`, with hidden/no-console
creation on Windows, and owns one child plus one helper I/O thread. It performs the
stable non-experimental `initialize`/`initialized`, `account/read` with
`refreshToken=false`, and `account/rateLimits/read` sequence. Initialization opts out
of `account/rateLimits/updated` and `remoteControl/status/changed`; any remaining
notification, wrong/duplicate/out-of-order ID, unknown field, malformed frame, RPC
error, unsupported user-agent version, early EOF, or deadline fails the complete
poll. The current supported app-server version is exactly `0.144.1`. One frame is
capped at 256 KiB, complete stdout at 1 MiB, and frame count at 64. Success and every
failure path terminate/reap the child and join the helper before return.

The response must identify one ChatGPT account with a bounded non-empty email. The
email is normalized only long enough to derive a domain-separated SHA-256
`QuotaAccountId`; it is never returned, persisted, logged, or included in `Debug`.
The current official response supplies no workspace identity, so workspace remains
explicitly unavailable. A non-empty multi-bucket map is authoritative over its legacy
duplicate. Primary/secondary windows expand to at most 32 normalized definitions and
samples with checked integer percentages, times, durations, deterministic observation
identity, provider-official evidence, and 20-minute/2-hour freshness boundaries.
Reset-credit rows are schema/count validated and normalized into one separate bounded
provider-neutral benefit observation. Detailed raw credit IDs are account-separated
SHA-256 inputs only; titles/descriptions are discarded. Detailed lots remain distinct,
and any unexplained positive available-count remainder becomes one aggregate lot with
unknown expiry. Quota observations and benefit observations remain separate values.

The transport performs no executable discovery, scheduling, SQLite access, writer
lease acquisition, query publication, UI callback, benefit persistence, reminder, or
activation. `CodexQuotaRuntime` is the separate composition boundary. Its config
accepts one archive path, automatic exact-native `PATH` discovery or one authoritative
explicit executable, and a positive transport timeout no greater than 30 seconds.
Automatic discovery is repeated for each poll, caps the captured `PATH` at 64 KiB and
128 entries, ignores relative entries, and tests only `codex.exe` on Windows or
`codex` elsewhere through `CodexAppServerCommand`; it never resolves shell aliases,
`PATHEXT`, `.cmd`, `.ps1`, JavaScript wrappers, or package-manager commands. An
invalid explicit executable fails configuration and never falls back.

`CodexQuotaRuntime` owns a scheduler and worker distinct from `LiveRuntime`. Startup
submits one recovery refresh. Manual requests coalesce into the existing one-active/
one-follow-up worker bound. Normal cadence is 15 minutes; only writer contention,
temporary spawn/unavailable, transport deadline, early exit, or cleanup failure select
the 60-second accelerated cadence. Version, schema, account, configuration, protocol,
RPC, and invalid-data failures retain the normal cadence to avoid persistent process
retry loops.

One execution captures the wall-clock lower bound, completes discovery and app-server
I/O, then rechecks cancellation/deadline before trying the shared process writer lease
once. Only after acquiring the guard does it open `UsageStore` and apply the at-most-32
owned quota observations in deterministic order followed by the optional separate
benefit observation. The guard spans the complete bounded publication, but every quota
window and the benefit inventory retain their own exact transaction/idempotency
contract. A quota failure may retain an exact committed quota prefix and does not
prevent an independently valid benefit transaction; a benefit failure never rolls
back committed quota. No cross-window or cross-domain atomicity is claimed. Store and
guard are dropped before health publication.

The public quota-runtime snapshot contains only phase, normal/accelerated schedule
state, bounded worker state, latest attempt outcome/stage/stable code, count-only
quota and benefit processed/status/failure results, conservative observation/elapsed
time, overall last-success time, and separate last successful quota/benefit publication
times. Quota and benefit report arithmetic is validated before health publication;
an inconsistent internal report fails closed as domain `invalid_data`. Common
lease/open/control failure remains distinct from quota-transaction and benefit-
transaction failure. The snapshot contains no executable/archive path, account/window/
lot identity, label, quota or benefit value, provider payload, email, credential, or
inner OS/SQLite error. Pause closes admission and cancels the active permit; a source
result completing after cancellation is not published. Suspend maps to pause, resume
forces one recovery refresh, and shutdown/`Drop` join the scheduler and worker. The
current transport is not cancellation-aware mid-session, so pause/shutdown may wait up
to its bounded timeout while holding no writer guard or SQLite state.

`UsageStore::apply_quota_observation` accepts one validated window definition and one
same-window normalized sample. It returns only `Started`, `Duplicate`, `Stale`,
`Advanced`, `AllowanceChanged`, or `Reset`, the independent quota revision, the
per-window transition sequence, and an optional opaque transition ID. Duplicate and
stale results are exact no-ops. Visible results publish definition/sample/epoch/
transition/current state and advance the quota revision once inside one immediate
transaction; any failure rolls back the complete publication.

`UsageStore::maintain_quota_history_page` accepts one exact quota window and a page
size from 1 through 256. It returns examined, deleted, remaining-sample,
remaining-closed-epoch, and remaining-transition counts only. Maintenance never
returns observation IDs or rows, never changes quota revision, never scans another
window, and may delete only old unreferenced samples for which a newer equivalent
sample exists under the same definition revision. Zero or an oversized page fails
before writes.

`UsageStore::apply_benefit_observation` accepts one complete bounded provider-neutral
inventory observation. Duplicate/stale input is a no-op; each newly accepted
observation advances the independent benefit publication revision, while only
meaningful lot changes append immutable change points and material revisions. The
same transaction replaces one scope current projection, publishes exact freshness,
and rebuilds its durable due rows from the active inherited or override profile.

`UsageStore::set_benefit_reminder_override` atomically replaces or removes one scope
profile and rebuilds only that scope's due rows. `UsageStore::maintain_benefit_history_page`
accepts one exact scope and 1..=256 total deletions, protects current/latest terminal
evidence, compacts old changes/material revisions and noncurrent delivery receipts,
and returns counts only. None of these operations activates a benefit.

`UsageStore::process_due_in_app_benefit_reminders` is the only runtime-facing due
queue mutation. It accepts a positive wall-clock delivery time and a page from 1
through 256 and starts one immediate transaction. Existing unacknowledged outbox rows
are returned first. Otherwise it reads at most that many indexed due rows, drops
expired rows, collapses already-due thresholds to the smallest still-useful lead per
lot revision and channel, and inserts an immutable delivery/outbox row before deleting
the selected due rows and returning any provider-neutral value. One recorded threshold
suppresses equal or less-urgent already-missed thresholds when later profile/inventory
writes rebuild the queue; future more-urgent thresholds remain pending. The result
contains at most 256 owned deliveries, exact counts, and the next indexed in-app due
time. Runtime receives no SQL, scope/lot identity, archive connection, or activation
authority; the opaque acknowledgement key inside a delivery has no public accessor
and is omitted from `Debug`.

`UsageStore::acknowledge_benefit_reminders` accepts at most 256 store-issued delivery
values plus a positive acknowledgement time. One immediate transaction validates the
sealed keys and immutable public facts, then inserts missing immutable acknowledgement
rows idempotently. Acknowledgement never deletes the delivery receipt and never
weakens reminder deduplication. Unacknowledged rows are protected from retention.

`BenefitReminderRuntime` owns one dedicated scheduler thread, one existing bounded
`RefreshWorker`, one nearest wall-clock deadline, capacity-one coalesced urgency, one
latest count-only health snapshot, and one pending owned notification batch. Startup
submits one recovery pass. Inventory/profile/clock hints, manual reconciliation, and
resume coalesce; suspend pauses admission and resume forces recovery. Transient
writer/store unavailability uses one 60-second retry deadline. While an unconsumed
batch is pending, no later due page is committed, so public delivery values cannot be
silently overwritten. `take_notifications` leases a copy for presentation but does
not claim it was shown. `release_notifications` makes a failed presentation available
again. Only `acknowledge_notifications`, called after successful presentation,
commits the durable acknowledgement and reopens reconciliation. A crash before that
commit replays the outbox row after restart; a crash after it does not duplicate the
event. Shutdown and `Drop` join both owned threads.

Reminder health exposes only phase, normal/accelerated retry, pending flags, nearest
due/retry times, worker state, stable failure, bounded examined/expired/suppressed/
delivery counts, aggregate pending/retained counts, elapsed time, and last-success
time. It contains no archive path, provider/account/workspace/lot/delivery identity,
provider payload, credential, email, or inner SQLite/OS error. The returned delivery
batch contains only provider-neutral kind, quantity, localization key, lead time,
channel, due/expiry time, and committed delivery time.

`RuntimeReminderPresentationPort` is the sole app adapter for reminder presentation.
It maps only `InApp` batches into `DesktopInAppNotificationBatch`, releases a lease if
mapping fails, and exposes only stable failure classes. `DesktopInAppNotificationBridge`
accepts one batch and one one-shot receipt, uses an independent checked epoch plus weak
window, and reports `Presented` only after the event-loop callback has applied and
verified the complete visible model. `ReminderPresentationCoordinator` coalesces
10,000 pumps behind one local in-flight bit and owns one condition-variable worker.
That worker retries acknowledgement only for `Busy` or `StoreUnavailable` after the
fixed 60-second interval. Confirmed release after failed presentation schedules a re-pump
on the same worker and a newer receipt wakes that wait immediately. A terminal
acknowledgement error releases without automatic re-presentation. Every other presentation
failure and shutdown releases the lease before clearing local
backpressure; `Err` and `false` do not clear it. Runtime acknowledgement catches and
redacts panics, restores the batch to `Leased`, and the adapter may recover only outer-
mutex poison to execute fallback release. OS/tray scheduling, snooze,
quiet hours, per-scope settings editing, and activation remain unimplemented.

`UsageReadStore::capture_quota_windows` accepts zero through 32 unique exact window
keys and a deadline no greater than two seconds. It returns the independent quota
revision plus owned available definitions, current samples, current epoch state and
first samples, and optional exact last transitions. Missing requested windows are
omitted rather than converted to zero. `UsageReadStore::capture_quota_transitions`
accepts one exact window, an optional expected quota revision, an optional opaque
revision-and-filter-bound cursor, a page size from 1 through 256, and the same deadline
bound. It returns newest-first immutable transitions using a 256+1 maximum lookahead,
owned pre/post samples, `has_more`, and a continuation only when another row exists.
Changed revision/filter cursors fail closed. Both captures use fixed quota-only indexed
SQL in one deferred snapshot, accept no caller SQL/sort/column expression, return no
usage/price rows, and clear deadline interruption state before every return.

`QueryService::quota_windows` maps zero through 32 exact requested windows into one
owned `QuotaEnvelope<QuotaCurrentSnapshot>` while preserving request order and
explicit missing-window results. `QueryService::quota_transitions` maps one newest-
first page into `QuotaEnvelope<QuotaTransitionPage>`. Their `QuotaQueryHeader` uses
the independent quota revision rather than usage `DatasetIdentity`, applies each
sample's exact fresh/stale boundaries, reports the worst truthful selected quality,
and allocates snapshot generation only after store capture and public mapping both
succeed. Failed/stale calls therefore consume no public generation.

Quota snapshots expose current window epochs and a bounded transition page. Full
weekly resets include before/after values, maximum pre-reset use, old/new reset times,
transition kind, evidence source, confidence, and an exact or bounded detection time.
CLI and MCP use the same fields and stable transition sequence so automation can react
idempotently. Unavailable provider capacity remains `null`/unavailable, not zero or an
estimate derived from local token usage.

`QueryService::benefit_inventory` returns one owned
`BenefitEnvelope<BenefitCurrentSnapshot>` for an exact provider/account/workspace
scope. It uses the independent benefit publication revision, allocates public
snapshot generation only after capture and mapping succeed, and exposes explicit
absent/fresh/aging/stale, complete/quantity-partial/partial, and unknown-expiry/
unknown-evidence facts. Present inventory contains at most 64 typed lots in
conservative FEFO order: known conservative expiry, unknown expiry, kind, then opaque
lot ID. The payload also exposes the nearest conservative available-lot expiry, the
nearest durable due time, and the active profile revision, normalized lead times,
configured channels, inherited/override source, and truthful `in_app_only` or
disabled delivery coverage.

`QueryService::benefit_changes` returns newest-first immutable change points in an
owned `BenefitEnvelope<BenefitChangePage>`. A page uses 256+1 lookahead and returns at
most 256 rows. Its opaque continuation binds the exact scope and global benefit
revision; a change in either fails closed without consuming snapshot generation.
Before/after material values retain their actual material revisions, including
terminal retirement and later reappearance. Current/history reads share one short
deferred transaction, never scan usage-event or rollup tables, reject redundant
projection drift after reader open, clear deadline handlers on every outcome, and
redact scope/lot/change identities from `Debug`.

Future activation-receipt pages use stable sequences. The same immutable schema will
serve UI, CLI, and MCP reads; manual facts remain explicitly marked and never become
official evidence.

The 1.0 CLI/MCP boundary is read-only for benefit inventory and pure policy evaluation.
Future activation is a separate host-owned mutation capability, not arbitrary HTTP or
browser control. It requires a strict provider/account/window scope, local consent,
expected inventory/policy revisions, deterministic idempotency key, durable intent,
and a reconciled receipt. No plugin or LLM may infer mutation authority from inventory
read access.

## Reliable state boundary

`tokenmaster-state` exposes typed settings load/save/import-preview, package create/
verify, catalog, retention, maintenance, bootstrap, and restore operations. It accepts
only controlled data-root capabilities and sealed selected-file descriptors. It does
not accept arbitrary SQL, a caller-defined archive entry, extraction path, shell
command, URL, credential, or provider payload.

The implemented Task 4 subset is `SettingsStore::{load,save,preview_import,
commit_import,full_backup_candidate,verify_target}` over one validated reliable-state
directory and fixed slot names. Load returns `Current`, `Fallback`, or `Defaults` plus
a stable path-private health code and optional generation. Preview owns one bounded
portable candidate and exposes only ordered changed categories and a field count;
commit requires the same base generation/record digest, preserves device-local state,
and does not publish a new generation when the portable value is already current.
Every successful publication is reread and returns a `PortableSettingsTarget` with a
nonzero generation and portable SHA-256 digest. Reconstruction rejects generation
zero; verification compares both generation and a freshly recomputed typed digest.
Task 4 alone does not claim catalog, retention, maintenance, bootstrap, or restore.
Tasks 8-10 implement the catalog/retention, maintenance, and sealed restore subsets
below; bootstrap remains a future fixed API.

The implemented Task 5/9/10 store subset exposes only `BackupSource::new`,
`BackupStaging::new`, `BackupControl::{new,is_cancelled}`, `create_online_snapshot`,
`inspect_archive_version`, `verify_backup_candidate`, and
`create_compact_snapshot`, `verify_recovery_archive`, plus explicit candidate discard and fixed-name abandoned-
candidate recovery. The source always names the implemented archive; staging chooses
only fixed create-new children. A verified candidate is an owning capability bound to
schema version, defensive runtime policy, physical file identity, exact length, and
SHA-256. Task 9 adds only its bounded path-free reader through
`VerifiedBackupCandidate::open_reader`; every consumer revalidates identity before and
after use. None of these APIs accepts caller SQL, a filename, an output path, or a
SQLite connection.

The implemented Task 6/8 state subset exposes typed `ConfigPackage::{write,read}` and
`BackupPackage::{write,write_to_backup_stage,verify_backup_stage,read}`,
`BackupMetadata`, the three fixed compression profiles,
the six fixed backup purposes, verified package values, and path-private receipts.
Public codec methods accept only a platform-owned `DurableFileReader`,
`DurableStagedFile`, or sealed exact-slot `BackupStagedFile`; raw generic `Read`/`Write`
helpers remain private and the authority audit permits exactly the named typed writer
and verifier. Config write emits portable settings only. Backup write requires a
declared length/SHA-256 from an already verified standalone database source and
independently recounts/rehashes it.
Config read returns owned verified portable settings. Backup read streams the database
into an unpublished stage and seals it only after the complete outer footer, every
descriptor/entry digest, and settings decode pass. Any codec or seal error poisons and
discards the stage; subsequent write, seal, and publication return `InvalidState`.
No API accepts an extraction name/path or returns a package entry iterator.

The implemented Task 7 subset exposes `BackupPassphrase`, the explicit
`BackupEncryptionContext`, `EncryptedBackupPackage::{encrypt,decrypt}`, and one
path-private sealed-output receipt. New and existing passphrase constructors take
caller-owned `String` buffers, move them immediately into zeroizing secret storage,
and clear every supplied buffer on success or failure. New passphrases additionally
require exact confirmation. `encrypt` accepts only `ManualExport`, a controlled
reader, and an opaque `VerifiedBackupPackage`; it recounts and rehashes the source
against that proof while streaming the standard age v1 envelope. `AutomaticBackup`
is rejected before source I/O and automatic packages keep no secret. `decrypt` accepts
only the controlled age reader and unpublished database stage, authenticates the outer
stream, and runs the private typed backup parser before sealing that database.
Excessive scrypt work,
wrong password, malformed/non-scrypt header, authentication failure, truncation,
trailing bytes, source substitution, output failure, or final seal failure discards
and poisons the output before return. Neither operation accepts a path, generic
stream, command, environment variable, recovery password, or arbitrary recipient.

The implemented Task 8 platform subset exposes `BackupDirectory::{open_or_create,
scan,create_staged,publish,open_reader,delete}` over one fixed `backups` child and 32
fixed private slots. `BackupStagedFile` exposes only bounded write length, chunk write,
seal, sealed path-free read, and discard; publication remains solely on its owning
`BackupDirectory`. Directory snapshots and entries are opaque path-free generation/
ordinal capabilities.

The Task 8 state subset exposes `BackupCatalog::{rebuild,bind_verified}`,
`RetentionPolicy`, `RetentionAdmission::{preflight,confirm_published}`, and
`RetentionCycle::{next_deletion,delete_next}`. Catalog selection is only checked
catalog generation plus bounded ordinal. Cold rebuild never returns verified health;
`BackupPackage::verify_backup_stage` must fully parse the same sealed unpublished
stage before admission, and exact proof is rebound after publication. Confirmation
requires exactly one added verified candidate and every prior file preserved. A cycle
returns or deletes at most one exact verified unprotected point, after full current-
verified-set and target content revalidation plus directory-generation confirmation;
another deletion requires rebuild and replan. No public state value returns a path,
filename, physical identity, or digest.

The implemented Task 9 subset exposes `MaintenanceCoordinator`, `MaintenanceSchedule`,
`MaintenanceWorker`, and `BackupMaintenanceRuntime`. Admission is started, coalesced,
an explicit empty-install/corrupt-quarantine mandatory bypass, or a stable rejection.
The coordinator owns one active and one merged follow-up; a second distinct mandatory
guard is busy rather than silently replaced. `MaintenancePermit` exposes typed purpose,
urgency, attempt/root request IDs, cooperative cancel, the final-publication boundary,
and a linked store-owned `BackupControl`; it exposes no raw atomic, path, stream, or
database handle. `MaintenancePurpose` has no source-retry variant: source retry is an
internally constructed urgency that preserves the original purpose. A published result
is accepted only after the permit entered the non-cancellable boundary.

`BackupMaintenanceRuntime` owns and joins exactly one worker and one scheduler. It
accepts typed manual/mandatory intents, a bounded automatic dirty hint, settings-policy
updates, and pause/resume/shutdown. Snapshots contain fixed phase/source/purpose/latest-
completion/counter facts only. The last mandatory-guard completion is retained
separately from lossy general health so a caller can match the root request and block
its mutation until exact final success. Empty-install and already quarantined corrupt
sources are the only explicit no-prior-backup bypasses. `Healthy` startup seeds the
first ordinary interval at runtime creation; `HealthyUnpublished` stays closed. Turning
periodic work off removes a merged periodic-origin follow-up without removing an
internal retry or any mandatory guard.

`VerifiedBackupCandidate::open_reader` returns only a store-owned bounded
`VerifiedBackupCandidateReader`; `BackupPackage::write_verified_candidate_to_backup_stage`
is the sole state/store codec bridge. The bridge accepts no path or generic `Read`,
streams without a database-sized copy, revalidates source identity/length/SHA-256
before and after consumption, and poisons the package stage on every failure.

The SQLite-specific snapshot and candidate verifier are store-owned fixed APIs.
The platform package owns durable same-volume replacement and native file selection.
Application composition alone may sequence runtime shutdown, writer-lease admission,
restore, and bundle restart. Product/Desktop receive only bounded copied health,
phase/progress, settings-preview, and at most 15 catalog-generation-bound ordinal
choices; they receive no path, file handle, SQLite connection, package digest,
recovery journal, or mutation capability.

Implemented Task 14 exposes only `FileDialogSelector::select_input` and
`select_output` over fixed `Config`/`Backup`/`EncryptedBackup` types. A result is exactly
`Selected`, `Cancelled`, or `Failed(FileDialogError)` with a fixed path-private code.
Native selection uses the Windows Common Item Dialog directly; the controlled selector
accepts one already validated local directory plus one bounded child for deterministic
tests and unsupported hosts. Selected input owns an already open, size-capped, regular,
single-link `DurableFileReader`. Selected output exposes only one successful bounded
stage creation over its complete lifetime, selection-current publication, and bounded
reread. It records absent or opaque physical
identity at selection; create-new/replace publication rechecks that state and never
opens the selected target for truncating writes. Existing replacement captures and
identity-checks the displaced target, restores a raced target before returning
`selection_changed`, and retains recovery evidence if rollback becomes ambiguous.
`NativeFileDialog` is thread-affine and requires a current active owner; it is not a
`Send`/`Sync` worker service. Neither capability exposes a path, filename, generic file/
stream, arbitrary child constructor, shell, or process.

Manual restore requires a typed selected candidate, current catalog/preview identity,
an explicit data-only or data-plus-portable-settings mode, and a second explicit
confirmation. Device-local settings are never an input. Cancellation is valid only
before atomic publication/replacement. Automatic recovery accepts no UI/CLI/MCP
request, always uses data-only mode, and is limited to definitive corruption plus a
newest-first fully reverified candidate.
Safe mode exposes retry, verified restore, fresh rebuild, and quarantine export only;
it never exposes arbitrary filesystem or corrupt-row salvage.

Restore records the selected mode and optional staged portable-settings target in the
redundant journal. Application composition may complete only after the active database
and selected settings target are reread and verified. A settings-publish failure must
roll the database back while retaining the prior settings generation; a crash after
durable settings publication resumes by exact generation/digest rather than publishing
a second generation.

Implemented Task 10 exposes `ArchiveRecoveryScope` only over the exact active archive,
matching `ExclusiveFileLeaseGuard`, fixed reliable-state staging, and fixed quarantine.
It accepts no path or name. Opaque operation IDs reserve their namespace with a
create-new marker and derive all children; staging retains at most three exact recovery
artifacts and may be discarded only after journal absence or completion,
while quarantine retains at most three complete sets and is never auto-deleted.
Existing-main promotion uses `ReplaceFileW`; missing damaged main uses a write-through
same-volume move. Before the first new sidecar move, quarantine rechecks main plus the
active/quarantine WAL and SHM locations as one coherent layout. Pre-existing drift or a
conflicting target fails without moving a different member; an exact already-moved
member is accepted only for the same opaque resumed operation. Every individual move,
promotion, and rollback repeats the relevant fixed main/WAL/SHM identity check so a
later namespace race remains fail-closed and an exact interrupted move can resume.

`RecoveryCoordinator::{restore_selected,restore_definitively_corrupt_selected,resume}`
accepts only a generation-bound catalog selection, typed mode/proof, exact archive/store
capabilities, and the matching lease guard. `restore_selected` requires an opaque
published pre-restore maintenance completion; the corruption-only entry point derives
its own authority by running the complete verifier and accepts no caller assertion.
Package expansion ends in a sealed `RecoveryStagedArchive`; store performs
the complete SQLite verifier on both candidate and reopened active main. State persists
only the six exact phases and path-free package/candidate/prior-artifact/settings facts.
Resume proves both normal phase boundaries and the cases where sidecar movement, main
promotion, or settings commit completed before journal advance. The hidden test observer
reports only typed recovery boundaries and grants no file, SQL, or mutation authority.
Before staging, recovery observes active-main length `A` and checks actual available
bytes for `max(2B, B+A) + 8 MiB`, where `B` is the selected database length. Platform
and store both enforce the shared three-artifact staging cap, and the physical guard
is authorized before either cleanup path. The journal persists the fixed physical backup slot,
package length, and package digest; process-local catalog generations are never durable
resume authority.

Implemented Task 11A adds `StateBootstrap::prepare` over one validated data root, the
same-root reliable-state capabilities, a held matching `ExclusiveFileLeaseGuard`, and
bounded backup control. Construction and every prepare call reauthorize the exact
data/reliable roots before mutation. It returns only a `PreparedBootstrap` containing
a path-free `BootstrapReport` and retained `RunSession`; outcomes are `Healthy`,
`FirstInstall`, `MigrationRequired`, `UpgradeRequired`, `RecoveryRequired`,
`Unavailable`, or `SafeMode`.

Bootstrap resumes a pending recovery before ordinary archive inspection. It opens the
fixed SQLite archive read-only, uses normal validation only after an exactly clean run,
adds bounded quick-check validation otherwise, and automatically restores only from
definitive corruption or missing-main damage with prior backup evidence. Candidate
selection is newest-first with complete package revalidation and automatic data-only
mode. Non-corruption failures, newer schema, no usable backup, or ambiguous artifacts
do not mutate the active archive.

`LiveRuntime::{start_guarded,start_notified_guarded}` consumes the already-held platform
guard and keeps it continuously through `UsageStore::open` and startup recovery; only
then is that startup guard released. There is no unlock/relock window before those
operations, while later runtime mutations acquire the same fixed lease per operation.
Existing start APIs remain wrappers that acquire their own startup guard. The application must retain the
`RunSession`, authorize a healthy launch only after the owned bundle is viable, and
publish clean only after every archive, controller, maintenance, and runtime owner has
joined.

Implemented Task 12A composes that startup boundary in `tokenmaster-app`. One
`ApplicationStateOwner` prepares state before any live/query/controller owner, and a
safe-mode shell owns no archive user. A healthy bundle owns exactly one capacity-one
backup maintenance runtime. Its application-owned operation performs SQLite Online
Backup, complete candidate verification, typed package staging and verification,
sealed publication, catalog proof binding, and one-at-a-time retention. On its worker
thread, the first operation fully verifies the bounded cold catalog; later operations
carry proofs only for unchanged package identities and retain one current projection.
Terminal maintenance receipts are submitted and awaited atomically through one bounded
condition-variable wait. While that exact root is awaited, a later request is rejected
busy and cannot overwrite its receipt; no polling thread or UI timer is added.
The older split submit/read-wait API remains suitable only for advisory observation;
mandatory application operations must use the atomic submit-and-wait form.

A supported old schema remains read-only until a verified `PreMigration` point is
published. Before writable open, run-state schema v2 durably records the exact bounded
source/target schema pair as a pending post-migration obligation. The same held startup
guard then enters writable open/migration, and the bundle is not exposed until a
verified `PostMigration` point exists and clears that exact obligation. A restart with
an already-current archive and a pending obligation repeats the post point before live
publication. Periodic backup disablement does not suppress either point. Any startup or
migration ambiguity retains the unclean run and the safe-mode shell. Clean publication
occurs only after the
maintenance runtime, controller, quota/reminder runtimes, and live runtime join.
Implemented Task 12B.1 adds one application-owned, path-free command admission core.
It retains one active command plus at most one distinct follow-up, coalesces identical
hints, rejects a third distinct request, cancels an exact active or queued request, and
forbids cancellation after an explicit irreversible boundary. Admission can pause for
a controlled restart without losing the active receipt and can close permanently for
shutdown. Config, backup, verification, generation/ordinal restore, and rebuild intents
carry no path, bytes, digest, provider identity, or arbitrary command payload.

The controlled current-bundle restart joins the old owners, acquires a fresh fixed
archive guard, and reuses the one guarded construction path without replacing the Slint
window. Each bundle/notifier pair has one checked generation. The notifier compares it
under the same mutex that protects slot replacement, so an obsolete completion returns
before allocating a product-runtime generation or touching a new controller.
Implemented Task 12B.2a adds one internal selected-restore lifecycle primitive. The
caller supplies only a generation/ordinal choice and fixed restore mode. Application
state converts it to an opaque, path/digest-private verified package binding and one
RAII pin before closing admission. Every retention deletion shares its narrow gate and
replans around a pin that arrived after the cycle's admission. After a protected
mandatory `PreRestore` receipt and joined old
bundle, one fresh fixed guard authorizes the existing journaled recovery coordinator.
Its exact `RecoveryReceipt` is bound to the retained `RunSession` before archive
inspection or replacement startup. Current archives enter the ordinary guarded bundle
path; supported legacy archives repeat the mandatory pre/post-migration protocol and
durable pending pair before publication. Any ambiguity leaves no bundle and no second
owner. The catalog is an immutable bounded `Arc` snapshot, so its mutex is held only
while copying or replacing the projection, never across backup or recovery I/O.

Implemented Task 12B.2b.1 replaces the production root's bare coordinator with one
joined `ApplicationOperationWorker`. It uses one standard-library thread, one
capacity-one wake, the existing active-plus-one-follow-up coordinator, and one
latest-only completion slot. Execution occurs outside the worker mutex; exact
cancellation is normalized before completion, callback panic becomes only fixed
`internal` failure and closes admission, and shutdown/`Drop` cancel, wake, and join.
The first production binding is manual backup: it crosses the command irreversible
boundary before atomically submitting/waiting on the existing maintenance root while
holding the bundle generation stable, then returns only a fixed command outcome.

The same slice adds sealed config operations below the worker/UI boundary. Export
accepts only an already controlled `DurableFileTarget`, writes and seals portable
settings into a create-new stage, crosses the irreversible boundary immediately before
publication, then reopens and fully verifies the published package. Import accepts only
an already open bounded `DurableFileReader`, fully verifies one `.tmconfig`, retains one
bounded typed candidate plus base settings identity, and exposes only category/field
counts, creation time, and package bytes. Confirm consumes that exact preview and uses
the existing atomic settings commit, preserving device-local settings. The codec rejects
encoded config input above 2 MiB before parsing.

Implemented Task 12B.2b/Task 15 binds the sealed selector to the owning Slint thread and
dispatches only `SelectedInputFile`/`SelectedOutputFile` capabilities to the single
operation worker. Desktop submits fixed path-free intents for config preview/confirm/
cancel, normal/compact/encrypted backup, verification, confirmed restore, rebuild,
retry, cancel, and backup-policy changes. The UI receives admission immediately and
observes only newest immutable operation/reliable-state projections; it never waits for
file, compression, SQLite, recovery, or provider work.

Application composition publishes `AtomicPromotion` and disables cancellation exactly
when each mutation crosses its one-way boundary. Config export, compact/encrypted
export, settings commit, policy commit, selected restore, and rebuild cannot report a
late cancellation. Dialog cancellation performs no command admission or write. Restore
keeps its generation/ordinal confirmation and explicit data-only or data-plus-portable-
settings mode; the confirmation consumes the exact identity retained at preview even
if the catalog row changes meanwhile. Device-local settings remain excluded. Operation
`Running` publication occurs at actual worker execution start for both an admitted
permit and a promoted follow-up. Manual backup is cancellable before
`begin_irreversible`, then publishes `AtomicPromotion` and disables cancellation before
its exact maintenance wait.

`RecoveryCoordinator::reconstruct_definitively_corrupt` accepts only the fixed archive
scope, matching lease, proven definitive corruption, and absence of a usable verified
backup. It creates a fresh archive with the ordinary store schema, fully verifies it,
publishes a reconstruction journal without backup identity, quarantines the exact
corrupt main/WAL/SHM set, atomically promotes, and fully verifies the active result.
Application then starts one guarded live runtime, requests `RefreshUrgency::Recovery`,
and waits on a bounded event-driven worker completion until the source reconciliation
is complete and no refresh remains active or pending. Maintenance starts as `Healthy`
only after that barrier. The durable projection exposes only verified-backup or
authoritative-source kind plus an explicit non-reconstructible-loss boolean. A complete
source-reconstruction journal is also durable preflight evidence: after process death,
startup must run this same barrier before healthy publication. A failed same-process
barrier leaves a retryable obligation that reopens the promoted archive without
repeating reconstruction.

P3-D.0 performance and resource evidence does not widen this API. The three release
contracts call the same typed backup, package, retention, recovery, query, and Desktop
surfaces used by production composition. Their JSON lines and the ignored
`reports/p3d0-developer-evidence.json` receipt are developer-only evidence formats, not
CLI, MCP, plugin, UI, recovery, or mutation authority. The receipt may contain only
version/commit/executable/fixture identities, non-private machine class, exact command
arrays, durations, bounded scalar measurements, limits, and individual gate results.
It contains no raw path, filename, settings value, database row, provider payload,
prompt, response, command observed from a user session, or general extraction handle.

### P3-E.1 route command palette API

Desktop exposes only typed presentation callbacks to open/dismiss the palette, replace
its bounded filter, move the current ordinal, and activate one existing stable route
key. Ctrl+K and the visible header control call the same open callback; Escape,
Up/Down, Enter, pointer click, and accessibility default action call the same bounded
dismiss/move/route-selection callbacks. Successful activation uses the existing
`DesktopState::select_stable_key` path and then dismisses the transient model.

This API cannot execute arbitrary commands, SQL, shell, HTTP, filesystem, provider,
backup, restore, reminder, activation, or native lifecycle work. Snapshot application
clones the bounded projection while holding Desktop state, releases the mutex, and only
then updates Slint properties and refreshes an open palette.

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
