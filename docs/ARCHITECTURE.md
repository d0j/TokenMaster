# TokenMaster architecture

```text
Codex JSONL sources
  -> bounded native watcher paths reduced immediately to one pathless hint aggregate
  -> capacity-one scheduler wake plus mandatory periodic reconciliation
  -> bounded discovery and streaming reader
  -> typed Codex decoder and provider-neutral ObservationDraft/SessionRelationDraft
  -> exclusive TokenMaster accounting canonicalizer
  -> replay classifier and revalidation/runtime engine
  -> transactional current/staging SQLite archive
  -> transactional generation-qualified UTC/session rollups
  -> immutable query snapshots
  -> one exact scalar product-status join
  -> independently replaceable immutable product sections and fixed route readiness
  -> Slint desktop UI, future CLI, future MCP

Exact installed Codex native executable
  -> short-lived version-gated `app-server --stdio` child
  -> strict account/rate-limit wire validation and pseudonymous account identity
  -> provider-neutral primary/secondary quota definitions and samples
  -> separate constant-state quota scheduler/worker
  -> shared non-waiting writer lease after provider I/O only
  -> bounded per-window transactional quota publication
  -> immutable quota epochs

Validated Codex reset-credit rows or future sandboxed read-only provider component
  -> typed banked-reset/credit/temporary-use lots
  -> bounded query snapshots, expiry queue, reminders, and pure policy evaluation
  -> the same Slint UI and read-only CLI/MCP projections
```

The reader handles append, truncation, rewrite, incomplete tails, and bounded
oversized-line discard without retaining file content. Provider code cannot supply
fingerprints, replay signatures/evidence, event IDs, dispositions, or canonical
events. The accounting crate is their only constructor. The store persists only
path-private identities and approved usage metadata. Current-generation batches are
one SQLite transaction; staging promotion is a separate atomic boundary.

The allocation-free accounting replay classifier is also store-independent. It
validates provider/profile/parent/ordinal scope and returns only typed disposition and
next-state values. Weak evidence and exhausted traversal budgets remain pending;
cycles and contradictory facts become conflict; proven divergence is irreversible.

Ancestry metadata may arrive after usage. The reader therefore emits a separate
bounded session-relation draft in addition to observation drafts; reconciliation can
apply it to earlier observations without retaining raw JSONL. Parser resume v2 stores
the next ordinal and bounded lineage state. Resume v1 fails closed because assigning
ordinal zero after prior emissions would create false identity collisions.

Current canonical events carry provider identity directly. When aggregate publication
is ready, SQLite triggers update dataset generation, event counts, UTC minute/hour
facts, and session facts in the same event transaction. Non-empty migration and repair
use persisted keyset pages capped at 2,048 events and disk-backed unpublished
generations; readers
never group the whole event archive as fallback.

Read-only analytics bind publication, dataset identity, active aggregate generation,
and owned payload in one short deferred transaction. Session timeline is explicitly
all-time: mixed-order 256+1 keyset pages use last UTC instant then provider/profile/
private-session identity. The raw session key remains inside a dataset-bound opaque
value with redacted Debug; exact detail reads only capped model/project session rows.
Period analytics use UTC time rollups and never relabel whole-session totals.

`tokenmaster-query` privately resolves validated calendar requests through pinned
Jiff rules. An explicit IANA or positively resolved system zone becomes exact UTC
minute/hour segments; gaps/folds use compatible civil-time semantics, skipped dates
remain zero-duration points, and historical sub-minute boundaries fail closed. The
public immutable facade exposes only dates, canonical zone identity, exact token
availability, bounded daily points/breakdowns, and opaque session keys. Session
continuations are bound to dataset plus scope filters, so changing a filter restarts
pagination instead of skipping rows.

The UI receives bounded view models rather than owning archive state. Skin, layout,
and locale selection alter presentation state only, so switching remains immediate and
does not reparse sources or rebuild the archive.

The production UI lives in `tokenmaster-desktop`; `tokenmaster-m0` remains a separate
probe/evidence package and is not a production dependency. P3-A maps one current
`ProductSnapshot` into exactly 11 fixed route rows, one selected route, and at most 11
stable reason codes per row. A complete candidate projection replaces the prior model
only when its product generation is newer. The initial shell uses the real waiting
product snapshot and contains no quota/session/chart fixtures.

The production binary is owned only by `tokenmaster-app` and selects only the Slint
software renderer. `tokenmaster-desktop` is library-only. Slint callbacks may
validate and emit presentation intents but cannot open SQLite, read provider input,
own a runtime/worker, or block. P3-B.1 adds one bounded worker outside the callback
boundary. It owns one typed `QueryService` source and one `ProductReducer`, reduces
status first, continues independent sections after a local query failure, and replaces
one latest immutable snapshot only after a complete non-cancelled attempt. Repeated
intents retain one pending follow-up; intent receipts are distinct from executed
product-attempt generations. P3-B.2 marshals that latest snapshot through one
capacity-one event-loop delivery. It shares the controller mailbox instead of
retaining a second result, holds one weak window, and uses one atomic scheduled flag
to coalesce publications into at most one `invoke_from_event_loop` closure. The event
takes only the newest snapshot, applies only a newer generation, clears scheduling
state even after window loss, and rechecks once for a racing publication. There is no
timer, polling thread, event queue owned by TokenMaster, or strong ownership cycle.
P3-B.3 adds the separate application composition root. An exact empty
`tokenmaster.portable` marker selects `<exe-dir>\data`; absence selects
`%LOCALAPPDATA%\TokenMaster`. Both paths are canonical local non-reparse directories,
and invalid portable intent fails without fallback.

`tokenmaster-product` is the leaf composition layer between query/runtime truth and
P3 presentation. `QueryService::product_data_status` captures usage publication,
aggregate progress, quota, benefit, and Git scalar state in one defensive schema-v13
transaction; fixed statements never scan history. The reducer retains one current
`Arc<ProductSnapshot>` and no history. Checked attempt generation is independent from
source revisions, so old async work cannot win and a failed compatible refresh keeps
the last payload with a stable failure code. A durable identity mismatch invalidates
only the affected payload.

Usage, quota/benefit, reminder, and Git runtimes remain owners of their workers,
schedulers, leases, processes, and cleanup. The product layer copies only bounded
count-only lifecycle/retry/failure projections under a separate runtime generation.
Eleven fixed route statuses use a `u16` reason set. Aggregate rebuild keeps Activity
and Data Health reachable, degrades Dashboard section by section, and disables only
aggregate-dependent History, Sessions, Models, and Projects. The P3-B.1 worker and
P3-B.2 newest-only event bridge now marshal complete snapshots to Slint; Slint
callbacks still cannot open SQLite or own a runtime. The app owns the sole usage/
nested-Git, quota, and reminder runtimes, copies four fixed health observations into
one controller slot, and refreshes through lossy worker-completion hints without a new
timer, polling thread, queue, ingestion path, or strong ownership cycle.

P3-C adds explicit quota and benefit overview reads on that existing worker. Empty
exact filters retain their original empty meaning. The current product snapshot maps
purely into six ordered Dashboard sections with hard caps of 32 quota rows, 32 benefit
summaries, 240 trend points, 12 sessions, eight activity categories, 12 models, and a
checked aggregate over at most 32 repositories. The projection contains no opaque
account/workspace/window/lot/repository/session/event/source identities. Slint applies
seven bounded list replacements at initial construction and for each accepted newer
generation; route selection uses
a smaller route-only path and does not reconstruct the Dashboard or window. Semantic
tokens and label keys make future skin/locale switching a presentation concern, while
the current Dashboard remains timer-, animation-, polling-, query-, and SQL-free.

The active P3-D.0 contour adds reliable state without changing the current live archive
identity. Task 1 establishes library-only `tokenmaster-state` with stable path-private
errors, checked byte/item limits, exact dependencies, and a deterministic authority
audit. `tokenmaster-store` will create consistent Online Backup candidates and verify
integrity, foreign keys, schema, and semantic invariants. Later state tasks will add
redundant settings/run/recovery records, fixed streaming `.tmconfig`/`.tmbackup`
packages, bounded retention, and one capacity-one maintenance worker.
`tokenmaster-platform` will own same-volume durable replacement and sealed file dialogs.
`tokenmaster-app` will stop every archive user, hold the existing writer
lease, quarantine main/WAL/SHM, resume a redundant six-state restore journal before
SQLite open, commit the selected data-only or data-plus-portable-settings mode, and
reconstruct one application bundle or safe mode. Automatic recovery remains data only.
Product/Desktop receive bounded health and intents only. The contour is in progress;
Task 1 adds no persistent settings, runtime, backup, or recovery claim.

The built-in live quota source is separate from the JSONL usage reader. Composition
supplies one already resolved absolute native Codex executable to
`CodexQuotaTransport`. `CodexQuotaRuntime` resolves it either from authoritative
explicit configuration or by scanning a bounded current process `PATH` for the exact
native filename only; shell aliases, `PATHEXT`, scripts, package-manager wrappers, and
relative entries are ignored. One poll creates exactly `app-server --stdio`, performs the
stable non-experimental protocol supported by app-server `0.144.1`, reads account and
multi-bucket rate limits, and then terminates/reaps the child and joins its one helper
thread. The connector owns no endpoint, credential, browser, socket, SQLite
transaction, writer lease, scheduler, or UI callback. Frame/output/count/time bounds
and strict unknown-field/version checks make an incompatible response unavailable
rather than guessed.

Account email is transient input to a domain-separated pseudonym and never enters a
snapshot, store, log, error, or `Debug`. Multi-bucket results supersede the legacy
duplicate; primary/secondary provider windows map to exact fixed-point quota samples.
The official response's reset-credit rows are normalized into separate typed benefit
lots in the same owned Codex snapshot. Raw IDs are account-separated hash input only;
titles/descriptions are discarded. Quota and benefit publication remain independent
transactions and neither inventory read nor reminder delivery inherits activation
authority.

Quota runtime scheduling, worker state, and health are independent from `LiveRuntime`.
The normal period is 15 minutes; only bounded transient process/lease failures select
the 60-second period. Discovery and the complete app-server poll finish before the
shared writer lease is tried. The runtime then opens a writable store only under that
guard and applies at most 32 observations. The guard covers the complete loop while
each window keeps its existing independent idempotent transaction; a failure may
report an exact committed prefix but no other TokenMaster writer can interleave. The
store and guard are dropped before one latest count-only health snapshot is published.
Pause/suspend cancellation after source I/O prevents publication, resume coalesces one
recovery refresh, and shutdown joins both quota-owned host threads. No executable/
archive path, account/window identity, label, quota value, provider payload, or inner
OS/store error enters quota health.

Benefit reminder delivery is a third isolated runtime composition. It owns no Codex
transport and receives no SQL. After one non-waiting writer lease succeeds, the
schema-v12 store first replays unacknowledged immutable outbox rows or atomically
examines at most 256 indexed in-app due rows, records new outbox rows, collapses
already-missed thresholds, and returns a bounded provider-neutral batch plus the next
due time. One scheduler thread retains a single wall-clock deadline; one bounded
worker performs the store call. Capacity-one hints and a single ready/leased batch
prevent per-lot timers, callback retention, overwrite, and unbounded growth.
`take_notifications` leases without claiming display; release retries a failed
presentation, and an explicit post-presentation acknowledgement inserts a separate
immutable row. Startup, resume/hibernation recovery, profile/inventory changes, and
clock-change hints reconcile through the same path. P3 still owns actual rendering;
OS/tray delivery and activation are separate future capabilities.

The watcher is never source authority. Its callback discards `notify` event/error paths
before touching shared state; one atomic aggregate retains only dirty/force/urgency,
latest monotonic tick, health, lifecycle, and fixed counters. A 250 ms quiet window and
15 minute healthy or 60 second degraded poll trigger authoritative discovery. Missing
roots are not replaced by broad ancestor watches.

`LiveRuntime` is the production composition boundary. Startup acquires the persistent
OS writer lease before opening, migrating, or recovering SQLite; it closes a bounded
orphan scan and resumes or exact-discards only validated staging. The worker owns the
Codex adapter, store connection, archive bridge, and reusable lease object. Each write
acquires one guard, selects incremental only from replay-verified complete/partial
truth, and otherwise runs the exact full rebuild. Pause closes admissions before
cancelling the active permit. Resume invalidates watcher assumptions and forces one
authoritative reconciliation. Shutdown drops watcher ownership, joins the scheduler,
then cancels and joins the worker, so no task-owned thread or lease survives.

Provider-benefit inventory read does not imply activation authority. A future banked
reset mutation is a separate host-owned official capability with explicit local
policy, compare-and-swap admission, durable intent, provider idempotency/status, and
post-action inventory/quota reconciliation. Browser/session automation and generic
plugin/LLM mutation are outside the product boundary.
