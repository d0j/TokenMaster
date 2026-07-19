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

P3-D.1 extends the same path with an independent History section. The query plan
resolves one fixed recent-30-day daily request and executes it sequentially on the
existing capacity-one worker. Product failure/retention and dataset invalidation stay
section-local. `DesktopHistoryProjection` copies at most 30 newest-first daily rows
plus overview/range/timezone/evidence into one Slint model. Route selection remains a
pure in-place presentation update; there is no History timer, worker, cache, prior
range, database handle, or private row identity. Future bounded range controls replace
this section rather than adding query ownership to Slint.

P3-D.3 enriches that one request with capped Model and Project breakdowns rather than
adding per-route analytics work. `DesktopModelsProjection` consumes the shared envelope
and copies at most 64 canonical model rows with complete typed token/cost/event evidence.
Cost availability, selection mode, and actual composition remain typed, with partial
and calculated/reported/mixed evidence explicit at the Slint boundary.
History keeps only its 30 daily rows and future Projects will consume the prefetched
Project breakdown. Backend and frontend truncation remain explicit. One Slint model is
replaced only on accepted publication, so Models navigation is instant and adds no
worker, query, timer, cache, connection, prior dataset, or private identity.

P3-D.2a adds an independent Sessions page without widening frontend authority. The same
query plan requests at most 64 all-time newest-first session summaries and publishes
`has_more`; Dashboard still copies only its first 12 rows. `DesktopSessionsProjection`
removes the opaque dataset-bound keys and copies aggregate timestamps, event/token/cost
facts, evidence, and page completeness into one responsive Slint model. Route selection
does not query or rebuild that model.

P3-D.2b adds a three-axis exact-detail path without widening that authority. Every
controller/bridge lifetime has a checked `DesktopSnapshotEpoch`; the typed selection also
binds the viewed product generation and a nonzero click generation plus visible ordinal.
The application admits it only against the current bundle. One latest-only work slot
multiplexes detail with refresh on the existing controller worker, where the opaque key is
resolved and used transiently. Product and Desktop retain one replace-only correlated
detail, never another row's payload or key. Slint changes highlight/loading synchronously
and replaces one capped 32-model+32-project detail model with explicit missing,
unavailable, evidence, and truncation truth. No query callback, queue, thread, timer,
cache, window reconstruction, or additional snapshot slot exists.

The active P3-D.0 contour adds reliable state without changing the current live archive
identity. Task 1 establishes library-only `tokenmaster-state` with stable path-private
errors, checked byte/item limits, exact dependencies, and a deterministic authority
audit. Task 2 establishes the platform publication boundary: validated exact children,
bounded create-new staging, streaming length/SHA verification, Windows write-through
same-volume move and exact-backup replacement, Unix no-overwrite publication, and an
explicit `RecoveryRequired` outcome for every uncertain post-publication state.
Task 3 adds the crate-private A/B record layer over that boundary: six literal slot
names, fixed versioned envelope, 1 MiB strict-JSON cap, checked generation and dual
SHA-256 validation, highest-valid selection, conflict-aware equal generations, and a
two-pass writer that does not retain encoded JSON. The platform reads only a caller-
bounded exact child and replaces only the inactive slot without creating a third
backup. A post-publication reread is mandatory and any uncertainty becomes
`RecoveryRequired`.
Task 4 adds the only public settings surface over that private core. Schema v1 stores
the implemented provider-neutral reminder default and automatic-backup schedule/
retention policy separately from the device-local route. It rejects unknown/newer/
invalid/unbounded input, loads safe defaults without rewriting two invalid slots,
previews only portable category/count changes, preserves device state on import, and
binds a confirmed publication to a reread-verifiable generation plus portable digest.
Task 5 makes `tokenmaster-store` create consistent page-stepped Online Backup
candidates and independently verify integrity, foreign keys, exact schema/indexes,
stored counts/generations, and semantic invariants under bounded defensive SQLite
policy. Verified candidates bind physical identity, length, and SHA-256 before and
after every consumer; cleanup health and fixed-name recovery are bounded. Task 6 adds
the fixed typed streaming `.tmconfig`/`.tmbackup` package codec over platform-owned
bounded readers and stages. One checksummed/content-sized Zstd frame per typed entry,
exact length/hash/descriptor/footer binding, an 8 MiB decoder window, and independent
expanded counters fail closed; every failed output is irreversibly poisoned before it
can be sealed or published. Config additionally has a 2 MiB encoded fail-fast ceiling.
Task 7 adds binary age v1 only for manual exports. It
requires an opaque verified backup proof, rechecks exact source length/SHA-256 during
encryption, fixes scrypt work at 16 and import maximum at 16, owns zeroizing redacted
passphrases, parses authenticated plaintext through the private typed backup reader,
and poisons failed ciphertext/database stages. Automatic backups remain
unencrypted. Task 8 adds one sealed platform-owned `backups` directory with 32 fixed
private slots, a disposable self-describing catalog, and deterministic protected
retention. Candidate bytes are fully parsed while sealed and unpublished before
no-delete admission; after exact publication/bind, each deletion revalidates the full
current verified set plus exact target and removes only one write-through tombstoned
file before rebuild/replan. Later state tasks add typed run/recovery stores and one
capacity-one maintenance worker.
`tokenmaster-platform` owns durable replacement and now owns sealed file dialogs.
`tokenmaster-app` now owns bootstrap, migration safety, selected journaled restore, and
one joined bounded operation worker in addition to the replaceable backend bundle. The
worker embeds the sole fixed command coordinator, one capacity-one wake, one latest-only
completion, and no async runtime or generic task queue. Its first production binding
submits and waits for manual backup outside the Slint thread while holding the current
bundle stable. Shutdown cancels/wakes/joins this worker before bundle shutdown and clean
run publication.

Application config export/import is already sequenced over platform-owned controlled
targets/readers and state-owned typed packages/settings: create-new export is reread-
verified, import retains one bounded category/count preview, and confirm preserves
device-local settings. Task 14 adds a synchronous lower-level Windows Common Item Dialog
backend plus a deterministic controlled selector. Exact typed filters produce only an
already open no-follow bounded input or an identity-bound staged output. The selected
parent is bound by physical identity; on Windows a retained delete-capable stage handle
pins cleanup.
Existing-target publication captures the displaced file, validates its selection-time
identity, rolls back a raced replacement, and deletes old bytes only after the new file
is reverified. The thread-affine native selector requires an active owner and cannot be
sent to a worker. Paths remain private to platform and every result is selected/
cancelled/stable-error. Task 15 invokes it on the owning Slint/STA thread and dispatches
only the sealed capability to the worker. Config, backup, verify, confirmed restore,
rebuild, retry/cancel, and policy changes are path-free typed intents. Desktop receives
one latest-only bounded reliable-state projection rather than adding reliable state to
the archive-backed product snapshot; safe mode can therefore render Data Health and
Settings with no query/controller/runtime owner. No UI filesystem authority or
interactive acceptance is claimed.

No-backup reconstruction remains split by authority. Store creates the ordinary fresh
schema and owns complete verification; state owns the explicit no-backup journal,
bounded staging/quarantine, and atomic recovery sequence; app owns definitive command
composition, live startup, and the mandatory recovery-urgency source refresh barrier.
The fresh archive is not exposed as healthy and maintenance does not seed until that
bounded refresh completes. The durable path-free receipt marks non-reconstructible
quota, reset-credit, reminder, and Git history unavailable. Desktop receives the receipt
and exact operation phase only; it has no recovery capability, polling timer, or progress
queue. A complete reconstruction journal plus a started recovery candidate becomes a
preflight reconciliation obligation: cold start runs the same barrier before healthy
publication, while a failed in-process attempt remains retryable without repeating
quarantine/promotion. Desktop retains the exact restore identity reviewed by the user,
publishes follow-up `Running` at actual worker start, and represents unknown counts and
bytes as unavailable rather than zero.

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

## P3-D.0 evidence boundary

Reliable State closes through a separate release-mode developer rail rather than the
M0 or product-release pipeline. State owns deterministic 8/96 MiB schema-13 fixtures,
real automatic/normal/compact backup/package verification, the 10,000-trigger and
resume coalescing model, and one Windows process sampler. The sampler is the only added
measurement thread and records private bytes, process handles, threads, USER/GDI
objects, and child processes after both ToolHelp snapshots close.

The lifecycle contract establishes one baseline only after backup, acquired-candidate
cancellation, and a complete restore have initialized their process-global state. It
then runs 256 backup/import-cancel/retention cycles plus 16 forced cancel/recovery and
16 isolated restore cycles. Disk truth is not a trend estimate: every cycle must return
to the exact filled 15-point retention bytes and verification staging to zero. Manual
compact age encryption is included before final settlement against that original
baseline.

Desktop evidence uses the software renderer but performs a real Slint snapshot paint.
It waits for one background backup to complete, pins the identity of the next in-progress
96 MiB cycle, and requires that same cycle to span all loaded cached-query and
route-input-to-paint samples. The UI remains capability-free; the workload uses the
production typed state/store surfaces on its own joined test worker. The resulting
strict JSON receipt binds a clean commit and application SHA-256 but is ignored local
developer output. Physical-display/OS-input, DPI/accessibility, soak, MSVC packaging,
signing, and release acceptance stay on their separate later rails.
