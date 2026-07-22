# TokenMaster decisions

## ADR-001 — Single-root native workspace

Decision: TokenMaster has one root Rust workspace. Rationale: one build graph,
unambiguous ownership, no cross-project runtime dependency, and reliable verification.

## ADR-002 — Reference hierarchy

Decision: WhereMyTokens guides UI/product completeness and ccusage guides usage
analysis completeness. Rationale: requirements are taken from mature user-facing
behavior while TokenMaster keeps its own safe, bounded implementation.

## ADR-003 — Rust, Slint, and SQLite

Decision: Rust 1.97, Slint 1.17, and bundled SQLite are the product stack. Rationale:
native portable deployment, predictable ownership, declarative reactive UI, and
transactional local storage.

## ADR-004 — Presentation isolation

Decision: skins, layouts, and locales are declarative presentation state over immutable
snapshots. Rationale: instant switching without archive mutation, reparsing, or stale
asynchronous overwrite.

## ADR-005 — Incremental archive with staging

Decision: stream bounded source data into a strict SQLite archive; use invisible
staging generations for replacement/reconciliation. Rationale: fast append paths,
crash consistency, deterministic canonical selection, and safe rollback.

## ADR-006 — M0 gates remain hard

Decision: bounded M1 work may continue while M0 external evidence is open, but no M0
acceptance or package claim is permitted. Rationale: development can progress without
weakening real interactive and long-run validation.

## ADR-007 — Explicit replay lineage before analytics

Decision: canonical totals are selected from retained observations using explicit
session ancestry, versioned structural replay signatures, and fail-closed
pending/conflict states. Rationale: timestamp/fingerprint deduplication alone cannot
detect copied fork/subagent prefixes, while time or filename heuristics can suppress
legitimate equal-valued usage.

## ADR-008 — Codex-first provider-neutral source seam

Decision: local Codex discovery/reader/decoder is the only 1.0 ingestion adapter, but
engine and downstream crates consume provider-neutral bounded drafts/snapshots. Codex
is compiled in. Future third-party providers use versioned WebAssembly Components in
one isolated on-demand host process per package; native DLL/executable plugins are not
supported. Rationale: providers can be installed without rebuilding TokenMaster while
the default Codex path stays fast, the GUI carries no Wasmtime runtime, and untrusted
code receives only explicit bounded capabilities.

## ADR-009 — Core-owned canonical identities

Decision: providers emit observation drafts containing normalized facts and replay
basis; a provider-neutral TokenMaster canonicalizer computes fingerprints, replay
signatures/evidence, event IDs, and canonical-event values. Rationale: built-in and
external providers cannot diverge from or bypass accounting identity rules.

Implementation status: active. `tokenmaster-accounting` is the exclusive constructor;
Codex emits drafts/late session relations, and the store accepts opaque canonical
events only. Fingerprint v2 and replay signature v1 are versioned deterministic
framed hashes. The same crate owns the pure bounded replay transition so storage and
providers cannot introduce competing replay semantics.

## ADR-010 — Fail-closed replay promotion and recoverable staging

Decision: a replay revision becomes current only after exact fixed-manifest seal,
zero-pending promotion, and proof that the replacement accounts for every previously
visible event in its evidence overlay or immutable legacy snapshot. Promotion is
one immediate transaction with fault-tested rollback. Failed, obsolete, or
quality-only staging can be discarded only by exact revision/epoch CAS, without
touching current or legacy state. Rationale: rebuilds remain crash-safe and retryable
without allowing partial scans, stale workers, or incomplete replacements to erase
user-visible accounting.

## ADR-011 — SQLite-owned scalable replay manifests

Decision: the product begins a replay revision by snapshotting every registered
source with set-based SQL in one immediate transaction. Revision source counts are
stored and exposed as checked `u64` values within SQLite's signed-integer ceiling, but
never size an application collection. Exact seal and promotion validate deterministic
`file_key` keyset pages of at most 256 rows; continuation uses only a cheap
closed-source aggregate and cannot promote data. The explicit 256-key
`ReplayManifest` remains a bounded test/repair API and cannot seal a subset.

Exact schema v2 archives migrate to v3 by validate, foreign-keys-off outside a
transaction, create-new, copy, drop-old, rename-new, recreate indexes, foreign-key
check, commit, and guaranteed policy restoration. `writable_schema` and
rename-old-first are forbidden. Rationale: normal Codex histories may contain
thousands of JSONL files, so a 256-source product limit is invalid, while collecting
all source identities in Rust would violate the stable-memory target.

## ADR-012 — Adapter-prepared staging and non-destructive replacement

Decision: replay begin remains provider-neutral and creates empty invisible staging,
then the adapter prepares each untouched source through exact revision/epoch CAS using
a validated zero-offset checkpoint with its live path-private physical identity and
valid bounded resume payload. The store never manufactures provider state. A reader
truncate/replace classification does not authorize removal: promotion still requires
coverage of every previously visible event, and an omitted prior event leaves the old
projection current.

Rationale: copying an old physical identity while clearing offsets makes legitimate
atomic replacement unrecoverable, while an empty opaque resume cannot be decoded
after restart. Constrained preparation solves both without coupling SQLite to Codex.
Fail-closed prior coverage prevents a truncated, cancelled, incomplete, or parser-bug
rebuild from erasing real accounted usage; P1 must define explicit carry-forward and
retention authority before continuous reconciliation.

Implementation update: P1-A now supplies that authority through ADR-013. Truncation
and replacement still authorize no deletion; complete promotion uses explicit retained
projection state, while incomplete or cancelled rebuilds remain blocked.

## ADR-013 — Self-contained canonical projection and explicit retention

Decision: schema v4 removes the canonical projection's foreign key to deletable source
observations and records `projection_revision_id`, `origin_revision_id`, and
`retained`. Promotion atomically applies one fixed policy: eligible selection replaces,
replay-only suppresses, conflict-only retains, and absence retains. A retained row
keeps its original source key, generation, offset, event values, and older origin
revision without keeping the obsolete source generation alive or copying it into a
synthetic observation. The publishing revision remains a deferred foreign key and all
projection mutations share the generation/revision transaction.

Unrebuilt legacy rows are not carried into replay-verified totals because v1 identity
and quality cannot safely deduplicate against the new overlay; their immutable legacy
snapshot remains readable separately. Partial, cancelled, pending, stale, or invalid
rebuilds never reach the retention transaction.

Rationale: keeping old generations can retain entire obsolete histories and grow
without bound; attaching old events to a new generation fabricates provenance; copying
the full canonical page into every staging revision doubles large archives. A
self-contained indexed projection with explicit origin/retained state preserves
history in set-based bounded-memory SQL and supports atomic rollback without those
costs or false claims.

## ADR-014 — Provider-qualified scan-set authority

Decision: schema v5 groups one bounded, duplicate-free manifest of
`(provider_id, profile_id)` scopes under a `scan_set_id` and creates one typed child
scan per scope. The store owns observation membership through
`usage_source.last_seen_scan_id`. Only a complete child may derive `missing`; all
other outcomes preserve the prior value. A new source registered after any complete
scan for its scope starts missing until a later complete scan observes it. Ordinary
append has no scan authority. Parent/child creation and complete-only finalization are
immediate transactions with explicit fault rollback.

Historical v4 scans are migrated only when their provider can be derived from exact
referenced sources; otherwise they are isolated as `legacy-unverified`. Replay
revisions have nullable scan-set provenance for migration and bounded test/repair
compatibility, while production begin, continuation, seal, and promotion require and
revalidate one exact complete scan set. Zero-source sets publish retention-only truth
without replacing missing-source generations. Closed scan history is pruned as whole
sets only when unreferenced and older than the newest 32 closed sets for every child
scope. One transaction removes at most 64 sets; running sets and source/replay
references are retained, and backlog recovery repeats the same bounded operation.

Rationale: a profile ID is not globally unique across providers, one archive replay
can cover several scopes, and append activity is not proof of complete enumeration.
A scan set provides one archive-wide authority boundary without retaining a
scan-by-source history table or allowing partial enumeration to erase evidence.
The fixed 32/64 policy keeps steady-state refresh cost and database growth bounded
without a full usage-event foreign-key scan or a history-sized Rust allocation.

## ADR-015 — Constant-state synchronous refresh coordination

Decision: `tokenmaster-engine` owns one active refresh permit and at most one pending
aggregate containing only highest urgency and merged live deadline. Admission and
terminal outcomes are separate, IDs are checked and monotonic, cancellation is an
`Arc<AtomicBool>`, and deadlines use caller-supplied monotonic milliseconds. No async
runtime, path, provider descriptor, request history, or per-hint allocation is retained.
P1-C.2 provides object-safe adapter/archive/clock/writer-lease ports with sealed
provider-neutral identities, scope-exact canonical batches, 32-KiB checkpoints,
256-item observation/relation batch limits, and stable path-free errors. Adapter callbacks never
receive archive/store authority; archive calls never receive provider descriptors or
raw source bytes. P1-D supplies Codex and OS implementations.

Rationale: one synchronous coordinator plus cooperative boundaries gives deterministic
ownership, shutdown, and memory behavior while still coalescing bursts and allowing a
single follow-up. Keeping the OS lease and Codex reader behind later ports preserves
portability and prevents platform/UI concerns from entering the engine core.

P1-C.3 composes these ports in one synchronous `OneShotExecutor`. It streams discovery
without a source collection, stores only fixed counters and the latest replay handle,
canonicalizes each bounded adapter batch under core authority, and promotes only after
complete continuation and seal. Cross-scope discovery, non-progress, replay identity
change, epoch regression, stale state, cancellation, deadline, or port failure fail
closed. Cleanup targets only the last confirmed unpublished handle. `busy` is reserved
for writer-lease admission; the same code from an already-running port is a failure.
Rationale: this keeps memory independent of source/history size, prevents adapter or
archive boundary confusion from becoming canonical truth, and preserves exact recovery
evidence without adding async, Codex, OS, or UI dependencies.

P1-C.4 adds one optional `RefreshWorker` over the same coordinator: one dedicated
owned thread, capacity-one wake and latest-result channels, immediate execution of at
most one aggregate follow-up, and non-blocking checked result supersession. Explicit
shutdown and `Drop` both cancel/wake/join; no thread is detached. Ordinary `failed`
remains recoverable, while a callback panic publishes fixed `failed`/`panicked`,
abandons its allocated follow-up, faults the worker, and requires recreation after
archive recovery. A process-global hook wrapper is installed once because Rust invokes
the panic hook before `catch_unwind`; thread-local filtering suppresses only worker
payload output and delegates all other panics to the previous hook. Custom application
hooks must therefore be installed before the first worker and not replaced during its
lifetime. The engine rejects `panic=abort` at compile time rather than silently losing
fault containment. Rationale: this closes the last P1-C ownership/backpressure/privacy
gap without an async runtime, per-hint allocation, unbounded result queue, or
provider/UI coupling.

## ADR-016 — Separate banked reset inventory with capability-gated activation

Decision: provider-granted banked rate-limit resets are typed independently from quota
epochs, credits, and temporary usage. Inventory uses separate expiry lots, immutable
change points, one indexed reminder queue, and normalized activation intents/receipts.
The first-run reminder profile is 7 days, 24 hours, 12 hours, 6 hours, and 1 hour, but
each value is independently selectable and users may replace it with up to eight
unique bounded custom thresholds. Existing profiles are stable across upgrades and
change only through an explicit settings revision.
Assisted activation may open an official provider surface. Automatic activation is
disabled unless a connector exposes official idempotent mutation and status
capabilities; it additionally requires explicit versioned policy, fresh evidence,
CAS, durable intent, and reconciliation. Scraping, browser automation, session reuse,
and generic plugin/LLM mutation authority are rejected.

Rationale: the provider can grant several resets with different expirations, while a
normal weekly reset can occur without consuming any grant. Modeling these as one bar
would lose expiry safety and corrupt history. Capability separation permits useful
manual inventory and reminders now, preserves portability, and leaves a safe path to
future automation without binding TokenMaster to unstable private web behavior.

## ADR-017 — Descriptor-bound two-pass full rebuild

Decision: engine source identity includes one fixed 32-byte logical-file key in
addition to provider, profile, and provider source ID. A full rebuild uses two linear
adapter enumeration passes. The first writes discoveries directly into the exact scan
set. The second lends one temporary descriptor-bound `SourceBatchReader` through an
object-safe callback, allowing the engine to pull bounded batches without receiving or
retaining a path, file handle, descriptor, raw bytes, or source list. Every batch must
match the complete source identity. Exact archive preparation rejects sources outside
the completed set and duplicates; exact seal remains the disk-backed proof that no
source was omitted. Any incomplete second-pass quality discards the latest confirmed
unpublished replay handle and cannot seal or promote.

This supersedes P1-C's archive replay-page/cursor assumption. Real Codex session files
share one provider source ID, so that older identity could alias valid files, while
archive-driven paging could not recover a path-private live descriptor without an
unbounded path cache or repeated enumeration. Two O(N) streaming passes preserve
provider separation and memory stability. Full rebuild remains bootstrap/repair;
P1-D's steady-state path must be incremental tail-only rather than replaying every
JSONL file after each watcher hint.

## ADR-018 — Atomic replay facts per reader batch

Decision: `ReplayAppendBatch` owns both its bounded canonical events and at most 256
late `SessionRelationDraft` values. The store applies observation and replay overlays,
session relation reconciliation, selection invalidation, continuation work, chunk
proofs, checkpoint/source state, and evidence epoch in one immediate transaction.
The batch validates one expected revision/epoch and advances it exactly once,
independent of relation count. Fault boundaries after event-overlay work and after
relation work must restore every affected table and the prior checkpoint/epoch.

Rationale: the P0-E driver previously committed the event batch and then each late
relation separately. A failure after the first commit could leave SQLite at a newer
epoch while the engine retained an older exact replay handle, making cleanup stale and
recovery ambiguous. One bounded fact batch matches one reader pull, removes that
partial-commit state, and preserves deterministic restart without enlarging memory.

## ADR-019 — Separate bootstrap composition with a strict Codex checkpoint envelope

Decision: production bootstrap composition lives in a separate `tokenmaster-runtime`
crate. Its built-in Codex adapter owns only the bounded provider discovery snapshot,
enumerates JSONL descriptors synchronously, and lends one descriptor-bound reader at
a time to the provider-neutral engine. A fresh source checkpoint is created by safe
open/metadata probe without reading source content. It starts at offset zero with a
full-prefix proof over the empty covered prefix; store preparation receives a distinct
canonical zero-offset incremental checkpoint and promotes to full-prefix only through
the atomic replay append.

`CodexCheckpointV1` is a manual little-endian binary envelope capped at 32 KiB total.
It contains fixed schema/version flags, opaque physical/logical identities, checked
offsets and file observation metadata, a redacted boundary anchor, verification state,
and bounded parser resume bytes. It contains no path or source payload. Decode rejects
oversize input before payload allocation, unknown versions/flags, identity mismatch,
truncation, and trailing bytes. Runtime maps the store's zero-based IDs to the engine's
nonzero IDs by checked `+1`/`-1`; failures are stable path-free port codes.

Rationale: bootstrap must exercise the real Codex/store path without pulling provider,
filesystem, or SQLite dependencies into the engine and without mislabeling a full
history replay as the future live path. Distinguishing reader probe state from store
staging state preserves replacement detection and exact CAS. The fixed envelope makes
restart state portable and inspectable without serializing path-bearing descriptors.

## ADR-020 — Replay-aware current publication and tail-only refresh

Decision: schema v6 owns one strict singleton publication record containing a checked
archive generation, current replay revision, latest complete scan set, and explicit
`empty`, `complete`, `partial`, or `recovery_pending` quality. Steady-state refresh
first publishes exact complete-scan freshness, then preflights all present sources,
then reads only from persisted checkpoints. New sources are provisioned path-free by
that exact scan; non-empty sources remain pending until their bounded reads finish,
while missing historical sources are retained. Every current append compares both
revision epoch and archive generation and updates only affected fingerprints in the
same transaction as replay facts, chunks, checkpoint, source state, and both CAS
tokens. The replay-verified archive rejects the older canonical-only append path.

Replacement, rewrite, truncation, physical/logical identity mismatch, or anchor
mismatch changes only the CAS-checked publication to `recovery_pending`; prior visible
truth stays intact until `OneShotExecutor` completes a new exact rebuild. Watcher hints
are not source authority and are not part of this decision.

Rationale: re-running full history after every hint violates latency and memory goals,
while appending directly to the old canonical projection bypasses replay accounting.
The paired CAS prevents stale writers, exact scan authority admits new files without
path persistence, durable partial/recovery states make restart honest, and targeted
materialization avoids archive-sized work on the fast path.

## ADR-021 — Persistent empty sidecar with OS-owned writer lock

Decision: `tokenmaster-platform` derives one sidecar beside the archive after resolving
and validating a controlled local parent; Windows drive-type validation rejects mapped
remote and non-writable optical roots. The sidecar is opened read/write without
truncation, must remain a regular zero-byte file, and is never deleted during unlock.
Rust 1.97 `File::try_lock` supplies the non-blocking exclusive lock. Its typed
`WouldBlock` is the only engine `busy`; every other failure becomes a stable path-free
category. One guard owns one file handle, so drop, normal exit, and process death
release ownership without a PID, timestamp, heartbeat, polling thread, or stale-owner
repair protocol. Runtime implements the provider-neutral `WriterLease` port over this
platform type; the engine retains no path or OS handle.

Rationale: a SQLite row or owner timestamp can outlive a crash and create false
permanent ownership, while deleting a lock file can split Unix inode identity between
writers. A persistent empty sidecar plus an OS-owned handle preserves one lock identity,
recovers automatically after process death, consumes constant memory, and exposes no
private owner data.

## ADR-022 — Pathless atomic hints with mandatory periodic reconciliation

Decision: pin `notify = 8.2.0` inside `tokenmaster-runtime`, discard event/error paths
inside the callback, and retain only one atomic dirty/force/urgency/health/lifecycle
aggregate with one capacity-one wake. One owned scheduler thread applies a 250 ms quiet
window, 15 minute healthy poll, 60 second degraded poll, checked clock rollback, and
stable pause/resume/stop/fault transitions. Root generations contain at most 64
canonical existing configured directories; missing roots create no ancestor watch and
old callbacks are invalidated by generation. Watcher events never become source or
archive authority.

Rationale: poll-only scheduling either delays UI updates or repeatedly scans unchanged
history, while event/path queues grow with activity and expose private filesystem data.
Lossy pathless hints provide fast reaction at constant retained state; mandatory
periodic exact discovery repairs missed events. The pinned backend-owned internal
thread receives its stop signal when the watcher is dropped; resource contracts require
backend threads and handles to return to baseline after replacement/shutdown. Failure
of that gate blocks P1-D rather than weakening the bound.

## ADR-023 — Lease-first live composition with ordered joined shutdown

Decision: `LiveRuntime` owns the production Codex composition. Startup acquires the
persistent OS writer guard before SQLite open, migration, orphan-scan closure, or
staging recovery. Only exact current accounting versions and scan/revision/epoch
identity may resume or discard unpublished staging. The scheduler starts paused;
worker, watcher, and admission state are installed before its forced recovery submit.
The worker execution object owns the adapter, archive connection, and reusable lease.
Each refresh takes one guard, selects incremental only for replay-verified complete or
partial truth, and hands the already-held guard to full rebuild when required. Pause
closes admission before exact cancellation. Resume resets watcher assumptions and
forces recovery. Shutdown closes admission, drops the watcher, joins the scheduler,
then cancels and joins the worker; faulted state still attempts cleanup.

Rationale: independently started scheduler, watcher, recovery, and writer objects
leave startup races, double lease acquisition, and detached cleanup windows. One
composition root gives every mutable archive action one OS-owned guard, makes recovery
precede asynchronous work, and gives pause/shutdown a testable ownership order while
retaining fixed state and path-private public diagnostics.

## ADR-024 — Freeze the 1.0 delivery and native release boundary

Decision: keep Rust 1.97, Slint 1.17, bundled SQLite, the built-in Codex adapter, and
the provider-neutral architecture. After P2 query/data work, deliver the complete
desktop UI in P3, presentation/localization in P4, and the read-only CLI/MCP automation
surface in P5. P6 produces the canonical signed `x86_64-pc-windows-msvc` portable ZIP.
The current GNU lane remains development/M0 evidence until an explicit dual-lane P6
comparison passes; the workspace-global forced target is then replaced by explicit
build-script target selection. No automatic updater or installer ships in 1.0.

The Slint desktop distribution follows the Royalty-free License 2.0 attribution route
with Help/About and public-download attribution, dependency notices, license policy,
and SBOM. Pricing is a release-pinned embedded catalog plus bounded validated local
overrides. Release gates include advisory, dependency/source/license, secret,
immutable-CI-action, attestation, deterministic-content, clean-room-launch, signing,
interactive, and soak evidence.

The built-in Codex quota source is limited to a credential-free versioned local format
or a documented stable official machine interface. A dashboard, slash command,
browser/session state, or private endpoint is not a contract. Absence produces an
explicit unavailable/stale state and cannot authorize automatic reset activation.
`docs/FEATURE_PARITY.md` is the row-level behavioral ledger; a parity claim requires
every row to be implemented or explicitly rejected under a surviving normative
rationale and regression gate.

Rationale: leaving target, package, license, data-source, feature-parity, and phase
order implicit transfers product decisions into late implementation and allows unsafe
or unverifiable shortcuts. Freezing them now preserves the proven native core, makes
the user-visible product the next priority after its data contracts, and gives one
auditable definition of release readiness without pretending GNU developer evidence,
private web behavior, or a broad feature label proves the final product.

## ADR-025 — Static capacity-one Windows power callback boundary

Decision: use the Windows 8+ callback form of `RegisterSuspendResumeNotification` in
`tokenmaster-platform`. One process-wide static signal keeps only the latest suspend or
resume event plus checked counters. It has no heap callback context, helper thread,
hidden window, USER/GDI object, archive handle, or runtime reference. The product
controller removes the pending event and invokes `LiveRuntime::apply_power_event`;
suspend is idempotent pause, while every resume invalidates watcher assumptions and
forces authoritative reconciliation even when a suspend notification was missed.

Rationale: a message-only window adds a thread, window lifetime, USER handle, pump, and
UI coupling solely to receive two event classes. Calling runtime from the OS callback
would introduce lock order and SQLite lifetime hazards. A static last-event-wins signal
is callback-lifetime safe, constant-state, non-blocking, and preserves resume recovery
when suspend/resume notifications coalesce. Periodic exact reconciliation remains the
backstop when registration is unavailable.

## ADR-026 — Separate exact query-only archive reader

Decision: `tokenmaster-query` owns synchronous bounded frontend values, while
`tokenmaster-store::UsageReadStore` owns one separate SQLite `READ_ONLY|NO_MUTEX`
connection. It requires exact schema v13 and bundled SQLite, applies WAL/query-only/
defensive/QPSG/no-checkpoint policy with trusted schema and DQS disabled, a 250 ms busy
timeout, 4 MiB cache and zero mmap, and never migrates. One short deferred transaction
captures publication generation, independent dataset identity, exact scan truth and a
current or immutable-legacy activity page. Continuations require dataset identity and
use composite keyset seek with one lookahead row. A progress deadline is removed on
every result before connection reuse.

For a current replay, dataset identity is `(revision_id, dataset_generation)`. Revision
ID alone is insufficient because a bounded live tail append can mutate the canonical
row set inside the current revision. Replay evidence epoch is also insufficient because
an exact no-change scan can advance replay/CAS evidence without changing visible rows.
Schema v7 therefore advances a dedicated dataset generation transactionally after
every canonical event insert/delete/update, while a no-change scan advances publication
freshness without changing the pair.

Public envelope scopes mean explicitly applied filters, with empty meaning all; the
exact internal scan manifest may contain up to 256 scopes and is not copied into each
frontend result. P3 owns one bounded worker around the synchronous facade. UI, CLI and
MCP never receive a SQLite handle or permission for arbitrary SQL.

`QueryService` allocates process-local generations only after successful captures and
maps a current revision whose accounting versions differ from the compiled versions to
`unknown` plus `accounting_version_stale`. The facade owns the two-second duration
policy; `UsageReadStore` enforces it using a process-monotonic SQLite progress handler.
P2-A keeps cursors opaque in process; versioned CLI/MCP serialization remains a later
adapter contract and may not reveal raw fingerprint bytes.

Rationale: sharing the writer couples UI latency to mutation and exposes write/schema
authority; opening the writable store can migrate; long-lived transactions retain WAL
history; offset pages degrade and can mix revisions; copying a 256-scope authority set
into every header conflicts with the 32-filter API bound. Separate identity, ownership,
and exact short snapshots preserve responsiveness, paging continuity, privacy and
bounded memory without another daemon or async runtime.

## ADR-027 — Transactional generation-qualified usage aggregates

Decision: schema v8 materializes bounded UTC minute/hour and session rollups behind a
singleton publication state. Current canonical events store provider identity directly.
When aggregate state is `ready`, SQLite triggers maintain dataset generation, exact
event counts, missing-value algebra, time rows, and session rows in the same canonical
event transaction. Other states keep canonical ingestion authoritative, publish no
partial rollup, and require a rebuild.

Rebuild uses fixed fingerprint-keyset pages of at most 2,048 events, disk-backed rows in
an inactive aggregate generation, persisted progress, bounded cleanup, and one expected
dataset generation. Reopen resumes; mutation invalidates unpublished work; final
publication is one checked active-generation update. Aggregate readers must require
`ready` and the exact active generation and may never group raw history as fallback.

Rationale: view-time grouping fails million-row responsiveness, Rust maps grow with
history, long snapshots retain WAL, and call-site-only maintenance can miss replay or
promotion paths. Transactional triggers plus bounded resumable publication preserve
one accounting authority, bounded memory, crash safety, and a fast shared UI/CLI/MCP
query surface.

## ADR-028 — Opaque all-time session reads

Decision: session timeline and detail read only the active generation of
`usage_session_rollup`. Timeline pages are ordered by last UTC instant descending and
provider/profile/private-session identity ascending, use matching mixed-order keyset
continuation, retain 256 rows plus one lookahead, and bind cursor and opaque key to the
exact dataset identity. Raw session identity has no public getter and is redacted from
Debug. Detail returns `None` for a missing exact key or one all-time summary plus
independently capped model/project rollup collections. It never scans raw events.

Period selection remains a time-rollup concern. Returning a whole-session rollup for a
session that merely overlaps a period would falsely present all-time tokens as
period-clipped tokens, while exact clipping would require raw-event access or another
materialization. The explicit all-time boundary is therefore both truthful and fast;
a future period-clipped session product requires a separately specified indexed fact.

## ADR-029 — Exact private calendar and immutable aggregate facade

Decision: `tokenmaster-query` pins Jiff 0.2.32 and keeps every Jiff/timezone-rule type
private. Public requests select today/day/week/month or a custom range, an explicit
IANA or positively resolved system zone, one of seven week starts, optional daily
series, canonical scopes, and fixed breakdown kinds. Local half-open boundaries use
Jiff compatible gap/fold resolution and compose at most three UTC minute/hour
segments. Skipped civil dates remain zero-duration points; sub-minute historical
boundaries fail with `unsupported_time_boundary` rather than rounding.

Public token facts are `unavailable`, `known`, or `partial`; results are owned and
bounded to 400 points, four independently capped breakdowns, and 256 session rows.
Session keys/cursors keep raw identity opaque, and continuation additionally binds the
canonical scope filters so a filter change cannot silently skip keyset rows. Aggregate
rebuild is `unavailable` without a raw-history fallback and does not consume a snapshot
generation. The locked Windows dependency chain is Jiff 0.2.32,
`jiff-tzdb-platform` 0.1.3, and bundled `jiff-tzdb` 0.1.8 / IANA tzdb 2026c; changes
require an explicit dependency/provenance review.

An unavailable aggregate generation cannot produce a truthful analytics envelope, so
the analytics call returns stable `unavailable` and does not allocate a snapshot
generation. The joined P2-F status snapshot represents engine and aggregate health
without fabricating metrics and owns the visible
`aggregate_rebuilding` warning.

Rationale: storing local buckets or exposing timezone engines couples data to mutable
user settings; silent UTC/rounding gives plausible wrong totals; implicit series work
wastes CLI/MCP latency; and dataset-only session cursors can skip rows after a filter
change. Private exact composition plus validated immutable values preserves portable,
responsive UI/CLI/MCP parity without expanding SQL, memory, or privacy authority.

## ADR-030 — Measured 2,048-event aggregate rebuild pages

Decision: retain the persisted fingerprint cursor, one short immediate transaction per
page, inactive disk-backed aggregate generation, and expected-dataset-generation CAS,
but raise the aggregate rebuild hard cap from 256 to 2,048 events. A call can derive or
clean at most nine rollup rows per event (18,432 at the cap), owns no history-sized
Rust collection, and must meet a separate 500 ms page-p95 gate. Current and immutable-
legacy million-event rebuilds must sustain at least 5,000 events/s; query and process-
resource gates remain independent.

Rationale: the deterministic current-million red run at 256 reached only 912,128
events after 346.44 seconds, approximately 2,850 events/s, despite stable ~14 MiB
private memory. The 2,048 cap reduced the same rebuild to 75.528 seconds / 13,240
events/s with 246.558 ms page p95; legacy completed in 81.142 seconds / 12,324 events/s
with 268.305 ms page p95. It preserves bounded crash/resume semantics while removing
an avoidable transaction/set-up bottleneck. A larger unmeasured cap is rejected because
it would increase writer hold time without a demonstrated product benefit.

## ADR-031 — Fact-only price rollups and release-pinned fixed-point pricing

Decision: schema v9 stores source pricing facts, never calculated historical cost.
`usage_price_time_rollup` and `usage_price_session_rollup` retain model, bounded project,
tier, context, reported-state, checked token basis, and optional reported USD micros in
the same aggregate generation as token facts. A pure immutable `tokenmaster-pricing`
engine selects `auto`, `calculated`, or `reported` cost from an embedded reviewed
catalog plus an optional validated override snapshot. Arithmetic uses checked integer
microdollars and one final half-up rounding. Unknown and truncated inputs produce
partial/unavailable evidence, never a plausible zero.

Overview plus 400 series points is one 401-target/512-key batch. Breakdown and session
surfaces use bounded target batches over indexed price rollups; no raw-history fallback
or per-visible-row query is permitted. Scoped range batches materialize at most 32
parameterized scope keys and force composite-index seeks. Current and immutable-legacy
million-event gates require at most 3.0x database amplification, full/scoped analytics
below one second, cached overview below 250 ms, and session page/detail below 100 ms.
The production pricing/query dependency closure and release libraries must contain no
runtime pricing network path.

Rationale: persisting calculated money couples immutable history to mutable rates;
floating point and fuzzy aliases can silently drift; runtime catalogs expand privacy
and supply-chain authority; one query per chart/session grows latency with UI size.
Fact/rate separation, exact aliases, immutable overrides, batched indexed reads, and
explicit provenance preserve reproducibility, responsiveness, and honest unknowns.

## ADR-032 — Quota-owned strict schema and exact migration boundary

Decision: schema v10 adds a quota-owned revision and seven `STRICT` tables for
definition revisions, immutable samples, current and closed epochs, reset/allowance
transitions, and the exact current window projection. Quota identity remains separate
from usage dataset identity. Same-scope/window composite foreign keys bind every
current sample/epoch and retained evidence reference; allowance-change kind must agree
with complete old/new units and capacity direction. Published history rejects
`UPDATE`, while later bounded maintenance may delete only unreferenced whole rows.

An exact v9 archive is validated before one immediate migration transaction creates
and seeds only quota objects, advances `user_version`, validates v10, and commits.
Injected failure after quota creation leaves exact v9 and no quota residue. No usage
or price row is rewritten or reclassified.

Rationale: loose global-ID foreign keys permit cross-window projections, SQL `NULL`
semantics can weaken relationship checks, and mixing quota revision with usage
generation would invalidate independent consumers. Exact composite ownership,
semantic checks, and an isolated rollback-safe migration preserve restart truth,
privacy, and future bounded retention without coupling quota history to local usage.

## ADR-033 — One-transaction quota publication and fail-closed current projection

Decision: `UsageStore::apply_quota_observation` owns one `BEGIN IMMEDIATE` transaction
per normalized definition/sample pair. It loads one window, calls the pure evaluator,
and treats duplicate/stale results as exact no-ops. A visible result inserts one
immutable sample, updates the current epoch/window, optionally closes one epoch and
inserts one transition, and advances the independent quota revision exactly once.
Global observation identity is content-stable, definition revisions are immutable,
and every generated revision/count/sequence is checked against SQLite capacity.

The current epoch, current window projection, and exact last sample must agree on
revision, observation/epoch identity, timestamps, quality/source/confidence, and
transition sequence. Live use and reopen reject missing or mismatched projection state;
the writer never silently repairs it. Injected failures after sample, epoch,
transition, current projection, and revision must restore the exact prior state.

Rationale: separate transactions can expose partial resets or consume revision without
history, while silent projection reconstruction can turn corruption into plausible UI
truth. One bounded transaction, pure classification, strict identity reuse, exact
projection validation, and deterministic retry preserve idempotency, restart truth,
and responsive constant-state writes without retaining history in memory.

## ADR-034 — Evidence-preserving quota retention and fixed hard caps

Decision: quota history uses per-window soft defaults of 512 samples and 256 closed
epochs/transitions, hard caps of 2,048 samples and 1,024 closed epochs/transitions,
and maintenance pages of at most 256 candidates. The write path may replace only the
immediately previous unprotected same-definition sample when every normalized quota
fact is equivalent. Explicit maintenance may delete only an older unprotected sample
that has a newer equivalent inside the same scope/window and definition revision.
First, current/last, ratio/unit maximum, closed-epoch, and transition pre/post/max
evidence are always protected. Task 5 never merges or deletes transitions or closed
epochs.

Maintenance owns one immediate transaction, updates only the retained sample count,
does not advance semantic quota revision, and returns counts rather than identities or
rows. Applying a sample that would cross a hard cap fails before publication and rolls
back completely. Writable reopen validates every stored per-window hard cap in
addition to global count/projection integrity, so an externally altered over-cap
archive fails closed.

Rationale: retaining every poll causes unbounded SQLite growth, while age-only
deletion can erase the exact evidence needed to explain resets, maximum use, and
allowance changes. Definition-bound equivalence plus reference-aware deletion keeps
steady polling near constant size without changing visible truth. Fixed pages and
hard caps bound work and storage; preserving semantic revision avoids invalidating
current/transition consumers for deletion of interchangeable internal detail.

## ADR-035 — Defensive quota snapshots with revision-bound keyset history

Decision: quota reads stay on the existing separate `UsageReadStore` and expose two
fixed operations only. Current capture accepts at most 32 unique exact window keys.
Transition capture accepts one exact window, optional expected quota revision, an
opaque revision/filter-bound cursor, and a page of at most 256 rows plus one lookahead.
Both operations use one deferred transaction, fixed quota-only parameterized SQL,
indexed exact/keyset predicates, no `OFFSET`, no caller-defined SQL/sort/projection,
and a total deadline of at most two seconds with guaranteed progress-handler cleanup.
Missing current windows are absent, not zero.

Stored rows are not trusted merely because schema v10 accepted them earlier. Reads
restore domain/quota authority objects, recompute deterministic transition identity,
and validate current epoch/current-row and transition pre/post projections against
their joined samples. A stale expected revision, changed cursor filter, malformed row,
missing last transition, or post-open projection drift fails closed without returning
partial values.

Rationale: UI, CLI, and MCP need one immutable quota truth without blocking on full
history scans or accepting corrupted duplicated columns as plausible state. Separate
read authority, fixed bounds, revision-bound keyset continuation, owned values, and
repeated relational validation keep latency/memory bounded while preserving restart,
privacy, and automation semantics.

## ADR-036 — Independent immutable quota facade and offline acceptance

Decision: `tokenmaster-query` exposes quota through `QuotaQueryHeader` and
`QuotaEnvelope<T>`, never through usage `DatasetIdentity`. The header owns one checked
process-local snapshot generation, exact quota revision, generated/data-through time,
provider-defined aggregate freshness, worst truthful quality, exact bounded window
filters, and stable warnings. Current requests preserve caller order and return one
explicit unavailable result for every missing requested window. Transition
continuation retains an opaque store cursor plus the exact public filter and revision.
Snapshot generation is committed only after store capture, mapping, and header
validation all succeed.

Public quota values are query-owned immutable projections. Their `Debug` surfaces
redact filters, provider epochs, labels, and opaque cursor identities. The core
acceptance gate covers an adversarial no-inference matrix, 32 windows, 1,000
transitions, 10,000 duplicate polls, restart, 256-row paging, bounded maintenance,
current and legacy usage coexistence, Windows resource return, and a release
dependency/source/library audit that rejects network, browser, cookie, shell, socket,
and async-client authority.

Rationale: reusing usage identity or TTL would invalidate independent quota updates
and misstate provider freshness. Omitting missing windows would make UI ordering
ambiguous, while allocating generations before a failed stale cursor would create
false consumer progress. An owned redacted facade plus measured offline acceptance
gives UI, CLI, and MCP one bounded truth without authorizing the still-separate Codex
transport or benefit mutation.

## ADR-037 — Short-lived version-gated official Codex quota transport

Decision: the built-in Codex quota source uses one already resolved native Codex
executable and one short-lived `app-server --stdio` child per bounded poll. The
connector performs only the stable non-experimental initialize, account-read, and
rate-limit-read sequence supported by app-server `0.144.1`; it opts out of the two
observed unsolicited notification methods and rejects every unknown field, method,
ID, schema, version, size, time, or process outcome. The child has one helper thread,
one monotonic deadline, fixed frame/output/count caps, discarded stderr, hidden
Windows creation, and mandatory terminate/reap/join cleanup.

Account email exists only as transient official response input to a domain-separated
pseudonym. Multi-bucket data is authoritative over the legacy duplicate; provider
primary/secondary windows map to exact fixed-point quota observations. The same
response's reset-credit rows are validated but discarded until the independently
authorized benefit-inventory contour. Executable discovery, polling, writer
coordination, SQLite publication, UI, reminders, and activation remain separate.

Persistent app-server ownership, shared sockets, session JSONL quota inference,
dashboard/slash-command scraping, browser cookies, private endpoint replay, and local
token-derived allowance are rejected. A persistent child saves less than one second
on an infrequent refresh but permanently adds process/memory/lifecycle authority.
Private or presentation-derived sources are brittle and violate the security
contract. A replaceable short-lived official boundary keeps TokenMaster
credential-blind, bounds retained resources, preserves truthful stale data on failure,
and allows a future connector implementation without changing the quota domain,
store, query, or UI contracts.

## ADR-038 — Separate bounded Codex quota runtime and I/O-before-lease publication

Decision: executable selection and quota polling are composed in a dedicated
`CodexQuotaRuntime`, not in the usage `LiveRuntime`. Explicit executable configuration
is authoritative. Automatic selection captures at most 64 KiB/128 process-`PATH`
entries on every poll, visits only absolute entries, and validates the exact native
`codex.exe`/`codex` filename through `CodexAppServerCommand`; shell aliases,
`PATHEXT`, script/package-manager shims, browser state, and credential files are never
resolved or executed.

The runtime reuses independent instances of the existing constant-state scheduler and
worker. It starts with one recovery refresh, coalesces manual/resume requests into at
most one follow-up, uses a 15-minute normal cadence, and selects the 60-second cadence
only for transient writer/process unavailability. Version, schema, account, protocol,
RPC, configuration, and invalid-data failures remain on the normal cadence. Quota
phase/schedule/worker/latest-attempt health is separate from usage-engine publication
and contains only stable codes, counts, times, and retry mode.

One execution completes discovery and the short-lived app-server session before
trying the shared writer lease. It rechecks cancellation/deadline, acquires without
waiting, opens SQLite only under the guard, and applies at most 32 deterministic
observations while holding that process guard. Existing per-window transactions remain
the atomic unit: a later failure may leave an exact committed prefix and reports its
counts, but no other TokenMaster writer can interleave and no cross-window rollback is
claimed. Store/guard are dropped before health publication. Pause, suspend, resume,
shutdown, and `Drop` close admission, cancel exact permits, and join owned threads;
the bounded transport remains responsible for child cleanup.

Rationale: extending the usage execution would acquire the writer lease before remote
provider I/O and couple unrelated latency/health. A second custom orchestrator or
async runtime would duplicate already verified coalescing/lifecycle state. A
persistent child or aggressive retry on permanent incompatibility would increase
idle memory/process authority. Exact-native discovery, separate constant-state
composition, I/O-before-lease ordering, non-waiting publication, and count-only health
preserve responsiveness, bounded memory, cross-process safety, and truthful stale
quota history without importing UI or benefit-mutation authority.

## ADR-039 — Provider-neutral benefit inventory and strict schema v11

Decision: banked resets, usage credits, temporary usage, and unknown benefits are
separate bounded lots with typed expiry precision and opaque identities. The built-in
Codex normalizer hashes raw reset-credit IDs with the pseudonymous account before the
domain boundary, discards provider titles/descriptions, preserves detailed rows, and
represents only an unexplained available-count remainder as one aggregate unknown-
expiry lot. `tokenmaster-benefits` owns deterministic pure reconciliation and reminder
keys without I/O authority.

Schema v11 adds an independent benefit publication revision, strict current/material-
revision/change/profile/threshold/due/delivery objects, and exact rollback-safe v10
migration. One scope observation commits current/history/freshness/due state in one
immediate transaction. Duplicate polls append no history; freshness-only observations
advance publication without changing lot revisions. Retention uses 512 changes and
256 deliveries as soft defaults, 2,048/1,024 hard limits, and a total 256-row
maintenance page. The newest change per lot is protected as its revision cursor so a
terminal lot can reappear after restart without revision reuse; only an actually
observed retired ID is hydrated for reconciliation.

Rationale: merging reset credits by count or expiry loses independently expiring
value, while retaining raw provider IDs or every poll creates privacy and growth
hazards. A pure core plus strict bounded storage preserves restart truth, deduplicated
future reminders, constant memory, and provider-neutral extension points. Inventory
read and in-app reminder planning do not grant activation, browser, credential,
network, shell, arbitrary SQL, or plugin mutation authority.

## ADR-040 — Immutable benefit snapshots use one revision-bound read model

Decision: `UsageReadStore` owns separate schema-v12 current and change-page captures
for benefit inventory. Each capture starts by reading the independent global benefit
revision in one deferred transaction. Current rows are restored from immutable
material revisions, checked against every redundant projection column, and ordered by
known conservative expiry, unknown expiry, explicit kind rank, and opaque lot ID.
History is descending keyset pagination with 256+1 lookahead; its opaque cursor binds
the exact scope hash and global benefit revision.

`tokenmaster-query` maps those captures into owned benefit envelopes with a separate
header schema, checked process-local snapshot generation, explicit absent/freshness/
completeness/unknown warnings, nearest expiry/due facts, and inherited/override
profile metadata. Delivery coverage is `in_app_only` only when the configured profile
includes the currently implemented in-app channel; configured OS scheduling is
reported unavailable rather than implied. Generation advances only after store
capture and public mapping succeed.

Rationale: grouping benefits in UI code, reusing usage dataset identity, or permitting
unbound history cursors would make restart, partial inventory, and concurrent updates
ambiguous. A narrow benefit-owned read model keeps queries bounded and immutable,
prevents usage-event scans, fails closed on SQLite drift, and remains reusable by the
future UI/CLI/MCP without granting notification or activation authority.

## ADR-041 — One Codex poll publishes quota and benefits with separate truth

Decision: `CodexQuotaRuntime` consumes one owned normalized Codex snapshot, completes
provider I/O before writer admission, tries the shared process lease once, and opens
`UsageStore` once. While the same non-interleaving guard is held it publishes each
quota window and the optional benefit observation through their existing independent
transactions and revisions. A quota failure stops the remaining quota prefix but does
not prevent an independently valid benefit attempt; a benefit failure never rolls
back committed quota.

The retained health snapshot keeps common discovery/clock/transport/lease/open/control
failure distinct from quota-transaction and benefit-transaction failure. It reports
bounded per-domain observed, processed, exact status, failure, lot-change, pending-due,
and last-success facts. Overall success requires every represented domain to succeed,
but a sibling domain success remains visible after partial failure. Internal report
counts and status arithmetic are validated before publication and inconsistency fails
closed as domain `invalid_data`.

Rationale: a second provider poll would duplicate child-process latency and could
observe a different account moment; separate store opens or writer acquisitions would
allow unrelated writers to interleave. Conversely, one cross-domain SQLite transaction
would couple independent revisions and roll back useful quota facts when benefit
storage fails. One poll/guard/open with separate exact transactions preserves
responsiveness, restart idempotency, fault isolation, and truthful automation health
without adding a thread, timer, network path, notification, or activation authority.

## ADR-042 — Store-owned durable reminder delivery with one runtime timer

Decision: due-queue mutation remains inside `tokenmaster-store`. One immediate
transaction reads at most 256 indexed in-app due rows, collapses overdue thresholds
per lot revision/channel, records the selected immutable receipt before removing the
examined rows, updates exact global counts, and returns only bounded provider-neutral
delivery values plus the next due time. A selected urgent receipt suppresses equal and
less-urgent missed thresholds during future queue rebuilds while preserving
not-yet-due more-urgent thresholds.

`BenefitReminderRuntime` owns exactly one scheduler thread and one bounded worker,
one nearest wall-clock deadline, one coalesced urgency, one latest count-only health
snapshot, and one pending delivery batch. Startup/resume force recovery; inventory,
profile, and clock hints coalesce; transient writer/store failure gets one 60-second
retry. An unacknowledged batch backpressures later queue commits. Delivery/outbox
commit therefore precedes event publication. Durable acknowledgement follows
successful presentation, so restart replays a pre-acknowledgement crash without
duplicating a post-acknowledgement event. Shutdown and `Drop` join all owned threads;
scheduler panic output is thread-locally redacted.

Rationale: direct runtime SQL would duplicate archive invariants and permit partial
receipt/queue updates. Per-lot timers or callbacks would make memory and handle use
grow with inventory. An unbounded notification channel could leak memory, while
overwriting a capacity-one slot after receipt commit would lose user-visible value.
One store transaction plus one-timer runtime preserves bounded crash-safe replay and
post-acknowledgement deduplication, hibernation collapse, and provider neutrality
without granting UI, OS-notification, network, browser, credential, plugin, or
activation authority.

## ADR-043 — Schema-v12 durable reminder outbox acknowledgement

Decision: the schema-v11 delivery receipt alone is insufficient presentation truth
because process failure after receipt commit but before in-memory handoff can lose an
unseen reminder. Schema v12 therefore adds an immutable acknowledgement relation
separate from the immutable delivery/outbox row. Existing unacknowledged outbox rows
are replayed before new due work. `take_notifications` leases but does not acknowledge
the bounded batch; explicit acknowledgement occurs only after successful presentation,
while release makes a failed presentation retryable. Retention may remove only
acknowledged noncurrent delivery rows.

Exact v11 migration inserts acknowledgements for all legacy delivery rows because the
old contract already considered those rows consumed. Migration is one immediate
fault-tested transaction and changes no usage, price, quota, inventory, or history
fact.

Rationale: acknowledgement before presentation creates false delivered state;
acknowledgement after an in-memory-only handoff creates a crash gap unless the outbox
is replayable. A separate immutable acknowledgement preserves both no-loss and
no-duplicate behavior with one bounded batch, one additional fixed store operation,
no new thread, and no provider or activation authority.

## ADR-044 — Repository paths use a transient reader side channel

Decision: a built-in or future provider may declare `RepositoryActivity` and produce
one latest `RepositoryActivityHint` beside a source read. The hint binds exact
provider/profile/source/session/time and optional safe project alias to a sealed
canonical local-directory candidate. It is taken synchronously through
`SourceBatchReader` and is deliberately absent from adapter batches, canonical
events, checkpoints, archive ports, SQLite, and public snapshots.

Candidate construction shares the platform local-directory policy with the writer
lease: absolute existing local directories only, bounded platform path bytes, no
traversal, network/device/mapped-remote namespace, symlink, or reparse ancestry.
Repeated metadata and turn-context lines replace one in-memory slot. Untimed context
may be paired with the next valid timed usage line in the same bounded reader state;
an explicit invalid `cwd` clears the prior transient candidate. Parser resume does
not carry the candidate across a batch or restart.

Rationale: adding a path to `ObservationDraft`, `AdapterBatch`, or checkpoint state
would let private filesystem identity approach durable accounting and plugin-facing
surfaces. Reconstructing repository identity from `ProjectAlias` would be ambiguous.
The separate capacity-one side channel preserves exact association for Git discovery,
keeps old providers source-compatible through a default `None`, and gives the runtime
one narrow value it can consume or drop without changing usage truth.

## ADR-045 — Git aggregates use private immutable schema-v13 generations

Decision: schema v13 adds an independent Git installation salt/publication revision,
at most 32 opaque repositories, at most 4,096 opaque activity associations, immutable
daily/day-category/category/warning generations, and no path or raw Git identity.
Authoritative rebuild replaces one generation; a same-process append is accepted only
with exact scan-revision/ref-fingerprint CAS plus compatible object/mailmap/author/
category/shallow identity. An unchanged refresh mutates no aggregate. Any restart with
changed refs or incompatible identity marks the prior projection rebuild-required
until a complete replacement publishes.

Only the latest 400 daily rows are retained while all-time totals and categories stay
exact. Loss of an older day forces `daily_history_truncated`, partial quality, an
oldest-retained boundary, and a range-completeness result. Project attribution is
available only when every durable association agrees on one non-null opaque project
key; absence clears an earlier key and disagreement produces
`association_incomplete`. Fixed read capture owns its values, uses 32+1 repository
lookahead, accepts at most 400 inclusive days, and enforces a total deadline of at most
two seconds.

Rationale: mutable aggregate rows create torn snapshots and difficult rollback;
persisted paths/commit IDs violate the privacy boundary; timestamp-only incremental
authority is unsafe after rewritten history; and silently retained project keys or
truncated daily series would manufacture exactness. Immutable generations plus
bounded salted metadata keep restart cost acceptable while making failure, staleness,
retention, association ambiguity, and omission truth explicit.

## ADR-046 — Git query uses explicit UTC days and a store-owned exact project join

Decision: `QueryService::git_output` publishes one independent schema-v1 immutable
envelope. Git days are the UTC buckets already proven by the parser/projection and are
labelled UTC in the public half-open range; they are never presented as local civil
days. One successful call advances one checked process-local snapshot generation and
binds the payload to the independent Git publication revision.

The transient hint's exact safe `ProjectAlias` becomes a domain-separated SHA-256
fingerprint under the private installation salt. Query code obtains bounded
materialized project and price aggregates, then asks the store to match at most 32
opaque keys against at most 256 safe candidates. Only matched aliases enter the
product snapshot; neither salt nor project key does. Cost per 100 product-code added
lines uses round-half-up fixed-point arithmetic only for exact compatible UTC range,
complete association/range/Git evidence, non-stale usage evidence, complete or exact
zero non-conflicting cost, and a nonzero denominator.

The Git capture and all optional join reads share one two-second wall-clock budget.
Usage rebuild/unavailability/deadline/corruption becomes a typed efficiency absence
without hiding independent Git facts. Internal construction or invariant errors still
fail the call. No raw usage event, Git process, repository traversal, filesystem
lookup, per-repository SQL, or long-lived transaction exists on this path.

Rationale: hashing in the query layer would disclose salt authority; storing a label
would weaken the opaque archive boundary; basename matching would silently attribute
cost to the wrong repository; and treating local dates as UTC would manufacture
calendar precision. Failing the whole Git card when only usage pricing is unavailable
would also couple independent evidence streams and reduce UI resilience. The bounded
store matcher plus explicit UTC contract preserves privacy, exactness, responsiveness,
and graceful degradation.

## ADR-047 — Git runtime keeps bounded transient locators and publishes after I/O

Decision: one independent constant-state scheduler/worker owns at most 32 latest
in-memory repository candidates, one active native Git scan, and one aggregate
follow-up. Native discovery, scanning, parsing, and exact child cleanup finish before
one non-waiting shared writer-lease attempt and one SQLite open. Publications compare
the candidate sequence after the scan. Same-process compatible frontiers allow
unchanged or ancestry-proven append; recovery, pause/resume, rewrite, identity change,
or lost frontier forces an authoritative rebuild. A known repository scan failure
publishes explicit unavailable truth or marks the last trustworthy projection
rebuild-required instead of replacing it with zero.

Pause closes hint and worker admission, invalidates all object-ID frontiers, cancels
and waits for the exact child, but keeps only the latest bounded canonical candidates
so resume can force rediscovery without persisting a path. Shutdown and `Drop` clear
all candidates and join the scheduler, worker, and child. Health is count-only and
`LiveRuntime` routes Codex's side channel without coupling Git success to usage
accounting.

Rationale: rescanning on UI/query threads or while holding SQLite would harm response
time and contention; persisting commit IDs or paths would violate privacy; clearing
every candidate on suspend would make the required resume rediscovery impossible;
and overwriting trustworthy aggregates after a failed scan would fabricate absence.
The bounded in-process locator/frontier split preserves exact recovery, minimal
retained memory, and durable failure truth without adding an async runtime or Git
library dependency.

## ADR-048 — Exact joined status with independent immutable product sections

Decision: schema-v13 exposes one scalar joined product-status capture over usage
publication and aggregate progress plus independent quota, benefit, and Git revisions.
The capture is one short defensive deferred transaction with a two-second maximum
deadline and fixed statements; it never scans historical event, rollup, sample,
change, or day rows. `QueryService` maps it into one bounded schema-v1 status envelope
and consumes a generation only after successful capture and mapping.

`tokenmaster-product` is a leaf composition crate. One reducer retains only the
current `Arc<ProductSnapshot>`. Data refresh order uses a checked nonzero attempt
generation independent from each source envelope generation, and runtime observation
uses another independent generation. Compatible failures preserve the last successful
payload plus a stable code; an incompatible durable identity invalidates the payload;
older asynchronous work cannot publish. Runtime owners are projected into bounded
count-only health and are never retained.

Exactly 11 fixed routes derive `ready`, `degraded`, or `unavailable` state from one
`u16` reason set. Aggregate rebuilding disables only aggregate-dependent History,
Sessions, Models, and Projects, while Activity and Data Health remain reachable and
Dashboard degrades section by section. Settings and Help/About require no archive.

Rationale: stitching independent queries in Slint would create mixed-time truth,
couple UI callbacks to SQLite, and make stale async replacement difficult to prove. A
single mega-payload would couple independent fault/revision domains and force healthy
cards to disappear. The exact scalar join plus independently replaceable immutable
sections preserves responsiveness, truthful degradation, bounded retained memory,
and a reusable UI/CLI/MCP projection boundary without inheriting runtime authority.

## ADR-049 — Production desktop is separate from the M0 probe

Decision: P3 uses a new frontend leaf package, `tokenmaster-desktop`. The historical
`tokenmaster-m0` package remains an architecture/resource probe and is neither renamed,
promoted, nor added as a production dependency. The production Slint package selects
only `winit-software`; the probe opts into FemtoVG explicitly for its diagnostic
fallback. One `DesktopState` maps the current `ProductSnapshot` into exactly 11 fixed
route rows and rejects equal or older product generations. Slint receives only copied
bounded strings/state and emits validated presentation intents; it receives no store,
query service, runtime owner, path, provider input, or mock data.

Rationale: promoting or depending on the probe would mix seeded M0 models, stress
entry points, renderer diagnostics, and receipt-bound behavior into product truth.
Keeping a separate production frontend preserves earlier evidence identity and lets
P3 evolve without a legacy runtime dependency. A fixed snapshot projection makes
route truth and retained memory testable before the P3-B query worker is introduced.

## ADR-050 — Desktop refresh uses one worker, one reducer, and one latest slot

Decision: P3-B uses the existing `tokenmaster-engine::RefreshWorker` as the sole
desktop query coordinator. One worker-confined typed `QueryService` source runs data
status first, then bounded analytics, quota, optional exact-scope benefit, Git,
activity, and first-session-page reads. One worker-confined `ProductReducer` applies
typed success/failure values. Only after a complete non-cancelled attempt does the
controller replace one optional latest `Arc<ProductSnapshot>`.

At most one attempt runs and one follow-up is retained. Each coalesced intent returns
a receipt, not an attempt generation; the coordinator allocates the real follow-up
attempt after the active attempt finishes. Cancellation/deadline checks between reads
prevent partial visible publication. Query failure is section-local and path-free;
controller fault is reserved for lifecycle/invariant failure. Slint callbacks own no
query handle; P3-B.2 delivers through one coalesced event-loop operation.

The controller accepts an already selected archive path but does not choose an
installed or portable data root. That policy and sole-live-runtime composition remain
P3-B.3. Benefit querying stays unavailable unless an exact `BenefitScope` is supplied;
safe scope discovery/all-current support requires a separate public query contract and
must not be inferred from identity-free status values.

Rationale: direct or per-card UI queries would block callbacks, multiply workers and
connections, and permit unbounded pending results. Reusing the proven coordinator and
retaining one reducer/result gives deterministic coalescing, stale-generation
authority, constant result memory, sibling-fault isolation, and shutdown without
inventing data-root or benefit identity policy.

## ADR-051 — Desktop delivery shares one mailbox and queues at most one Slint event

Decision: P3-B.2 keeps the P3-B.1 latest-snapshot mailbox as the sole retained result
slot. The controller accepts one notifier only while running and idle; attachment to
an already populated idle mailbox triggers one wakeup. The notifier holds only a weak
bridge reference. One atomic scheduled flag coalesces all publications into at most
one `slint::invoke_from_event_loop` closure, which takes the newest snapshot, upgrades
one weak `MainWindow`, applies only a newer product generation, clears the flag, and
performs one post-drain mailbox recheck.

The bridge adds no timer, polling thread, queue, data source, or strong window cycle.
Scheduler-unavailable failure clears the scheduled flag and retains the one current
snapshot for a later explicit notification; terminated event loop, dropped window,
or poisoned presentation state closes or faults with a stable code. Fixed saturating
counters expose delivery health without retaining events or history.

Rationale: timer polling trades idle CPU for latency, while one closure per
publication turns the Slint event queue into unbounded hidden history. Sharing the
capacity-one mailbox preserves newest-only truth and constant retention. Explicit
weak-window upgrade inside the queued closure guarantees scheduled-flag cleanup even
when the component has been destroyed. P3-B.3 still owns data-root and live-runtime
composition; P3-B.2 does not grant the UI filesystem, query, or ingestion authority.

## ADR-052 — A separate app owns deterministic storage and live composition

Decision: `tokenmaster-app` is the sole owner of the production `TokenMaster.exe`.
`tokenmaster-desktop` is library-only and retains its audited frontend/query boundary.
An exact empty `tokenmaster.portable` marker beside the validated current executable
selects `<exe-dir>\data\tokenmaster.sqlite3`; marker absence selects
`%LOCALAPPDATA%\TokenMaster\tokenmaster.sqlite3`. Invalid marker/location fails closed
without fallback, CWD, general override, or path-bearing errors.

Existing usage, nested Git, quota, and reminder workers accept one optional lossy
completion notifier. Notification occurs after capacity-one receipt publication and
outside worker locks; notifier panic is isolated from ingestion. The app notifier owns
only weak shared state, copies four fixed product-health observations under a checked
generation, and requests the existing capacity-one desktop refresh. The controller
retains one newest observation, not a runtime or second product snapshot.

Live usage/Git startup is mandatory. Quota and reminder startup failures publish
independent unavailable health so healthy usage remains usable. On event-loop exit the
app removes shared state, pauses all runtime admission, joins the controller, and then
shuts down reminder, quota, and live/nested-Git ownership without a lock across joins.

Rationale: putting runtimes in the UI package would erase authority isolation, while
polling adds idle work and another lifecycle. A marker makes ZIP portability explicit
and installed behavior deterministic. Lossy hints plus authoritative snapshots keep
latency low and memory constant without turning the UI into an ingestion owner.

## ADR-053 — Dashboard uses explicit discovery and bounded snapshot replacement

Decision: P3-C introduces separate all-current quota and benefit overview APIs rather
than changing empty exact-filter semantics. One immutable `ProductSnapshot` maps into
exactly six ordered Dashboard sections. Presentation retains at most 32 quota rows,
32 benefit summaries, 240 trend points, 12 sessions, eight fixed activity rows, 12
models, and one checked Git aggregate over at most 32 repositories. Private opaque
identities stop before the Dashboard projection.

After initial construction, each accepted newer product generation replaces the seven
bounded Slint list models once.
Route selection updates only the fixed route projection, preserves the window and
Dashboard models, and performs no query or background work. P3-C adds no timer,
animation, polling thread, secondary worker, snapshot history, or renderer fallback.
Semantic values and stable label keys stay separate from English fallback strings so
P4 skins and locales can hot-switch without changing archive/query contracts.

Rationale: overloading an empty filter would silently turn exact-empty truth into
discovery and could display false empty limits. Per-card queries or route-triggered
model rebuilding would add latency, hidden queues, and allocation churn. Explicit
overview reads plus capped immutable replacement keep truth, responsiveness, privacy,
and retained memory independently testable.

## ADR-054 — Keep one fixed live archive and add verified staged recovery

Decision: P3-D.0 keeps the implemented `tokenmaster.sqlite3` identity and its
`.tokenmaster-writer.lock` sidecar. Live backups use SQLite Online Backup into an
isolated candidate; compact manual export vacuums only that candidate. The fixed
`.tmconfig`/`.tmbackup` container uses bounded streaming Zstandard, exact lengths, and
entry plus whole-package SHA-256. Optional manual passphrase protection wraps the
package in a bounded standard age v1 stream; automatic backups store no secret.

Settings, run state, and restore intent use alternating checked A/B records. At the
prior Task 4 boundary, settings schema v1 stored only product-owned portable reminder/
backup policy plus the device-local route. The historical schema-v2 boundary added one
fixed `presentation.density` value. Current strict schema v3 owns the complete fixed
`presentation.{density,skin}` pair. v1 migrates in memory to Comfortable+Refined and v2
retains density while defaulting skin to Refined, both without a startup write; schema
0 and schema 4 or newer are unsupported. This valid-envelope version distinction prevents a downgraded binary
from loading defaults and overwriting newer state. Ordinary schedule settings cannot
lower the five-minute quiet or six-hour interval gates. Portable preview/commit is
base-generation/digest bound, preserves device state, and returns a reconstructible
target for idempotent journal resume. Generic records and directory paths remain
private.

The implemented snapshot layer uses 64-page Online Backup steps, bounded busy retry,
cooperative cancellation/deadline checks, fixed staging children, and a defensive
standalone verifier with explicit SQLite allocation limits. Accepted candidates bind
physical identity, length, and SHA-256 across verification and compaction. Cleanup
failure is observable and a later single-owner recovery pass scans only the fixed
candidate namespace. These constraints were added by independent high-risk review;
they do not change the fixed live archive identity or grant package/restore authority.

The implemented package layer is an exact typed v1 container rather than ZIP/TAR or
a generic extractor. It uses a fixed header/manifest/ordered-entry/footer grammar,
one checksummed content-sized Zstd frame per entry, levels 6/12/19, an 8 MiB decoder
window, expanded-entry and descriptor binding, and both preceding-byte and complete-
file SHA-256 receipts. Public methods accept only platform-owned bounded readers and
staged files. A failed encode, decode, outer verification, or final seal irreversibly
poisons/discards its output stage before returning; cleanup uncertainty becomes
`RecoveryRequired`. This closes the partial-output publication gap found by independent
review without introducing a second archive library or runtime thread.

The implemented encryption layer uses `age = 0.12.1` with default features disabled
and the standard binary age v1 scrypt recipient only. Manual export fixes
`log_n = 16`; import accepts at most 16 before derivation. Encryption requires an
opaque `VerifiedBackupPackage` and rechecks its exact length/complete-file SHA-256 in
the encryption pass, so a typed name alone cannot grant authority. Passphrases are
non-cloneable redacted zeroizing values created by taking and clearing caller UI
buffers; new values require exact 12-through-128-scalar confirmation without trim or
normalization. Automatic encryption is explicitly rejected. Every failed encrypt or
decrypt poisons/removes its output stage, and cleanup uncertainty is
`RecoveryRequired`.

Rationale: a generic age file wrapper could encrypt unverified input, leave plaintext
or ciphertext fragments publishable, or let attacker-controlled scrypt parameters
consume unbounded resources. Binding the standard interoperable envelope to the
already verified package proof preserves cryptographic interoperability without
adding custom crypto, filesystem authority, password persistence, or unattended
recovery credentials.

The implemented catalog/retention layer uses one sealed platform-owned `backups`
directory with 32 fixed private slots. State receives opaque physical entry tokens,
bounded readers/stages, and checked generation/ordinal selections, never paths or
filenames. A candidate is written, fully parsed while still sealed and unpublished,
admitted without deletion, published by the owning directory, rebound to the exact
catalog proof, and only then may retention act. Cold header validity is distinct from
full verification.

Retention protects the candidate, newest two verified points, and the latest
pre-migration point until verified post-migration evidence exists, then applies shared
four-newest/seven-UTC-day/four-ISO-week tiers under a 15-point and checked byte cap.
Before each one-file deletion it fully revalidates every current verified fact and the
exact target. Deletion is a write-through rename to an exact private tombstone followed
by removal; interruption is explicit recovery state and every successful deletion
requires catalog rebuild/replan.

Rationale: deriving selection from filenames, trusting a disposable cache, publishing
before verification, or batch-deleting from stale facts would make corrupt/stale
evidence authoritative. The fixed namespace, sealed prepublication proof, complete
current-fact revalidation, and deterministic one-delete prefix keep memory bounded and
make crash/data-loss behavior reviewable without introducing generic filesystem
authority.

Restore stops every archive owner, holds the stable writer lease, journals each idempotent
phase, quarantines WAL/SHM, atomically replaces the main file while preserving the old
main, reverifies the new active database, and reconstructs one application bundle.
Interrupted work resumes before SQLite open. Definitive corruption may select only a
newest-first fully reverified backup. Busy, access, capacity, transient-I/O, and
schema-too-new failures do not authorize replacement. No valid backup produces an
explicit quarantine plus authoritative-source rebuild, never silent zero truth.

The journal has six exact states ending in `settings_published` then `complete`.
Manual restore chooses data only or data plus portable settings; automatic recovery is
always data only, and device-local settings are never restored. A settings-publish
failure rolls the database back while keeping the prior settings generation. An
existing main uses atomic replacement; a missing main with prior durable artifacts
uses separately journaled same-volume promotion; no prior artifacts means normal first
install. Disabling periodic backups never disables mandatory healthy-source safety
points.

Rationale: copying the main file can omit committed WAL state; a hot mirror propagates
logical errors; moving the active archive through generation directories would change
the proved lease identity and let older binaries split truth. A fixed archive plus
Online Backup, independent historical points, Windows atomic replacement, and a
redundant recovery journal has the smallest auditable crash state space while keeping
foreground writes and Slint independent.

## ADR-055 — Use a capacity-one native maintenance runtime with typed store interop

Decision: backup maintenance uses one standard-library worker thread and one scheduler
thread, capacity-one wake channels, one active request, and one urgency-merged follow-
up. It adds no async runtime. Mandatory safety points outrank manual work, manual work
outranks source retry, and source retry outranks periodic work. A second unresolved
mandatory guard is rejected busy rather than queued or replaced.

The automatic schedule is scalar constant state. Exact `Healthy` startup truth seeds
the first interval at the current monotonic tick; `HealthyUnpublished` remains closed.
It otherwise opens only after first healthy publication, requires the configured quiet
and ordinary minimum intervals, and emits one catch-up after resume or clock rollback.
Disabling periodic work removes a merged periodic-origin follow-up but does not remove
an internal retry or disable pre-migration, pre-restore, or pre-destructive-maintenance
guards. The runtime retains
one latest general completion plus one latest mandatory-guard completion; it never
retains a request or progress history.

Each permit creates a store-owned `BackupControl` linked to the same cancellation
state. Cancellation is cooperative through snapshot, verification, and package work;
a compare-exchange enters a short non-cancellable final-publication section. A source
retry receives a fresh attempt ID and lower scheduling urgency but preserves the root
request and exact backup purpose, so a mandatory point cannot be satisfied by a
periodic-labeled package. Source retry is therefore not a caller-submit purpose, and
the state machine rejects `Published` before or `Cancelled` after the boundary.

The store owns the only path-free reader over a verified SQLite candidate. State owns
the only explicit bridge from that reader into a sealed unpublished backup stage. It
streams with fixed buffers and revalidates physical identity, exact length, and full
SHA-256 before and after consumption; any changed source or output error poisons the
stage. Implemented Task 12A supplies the owned snapshot -> verify -> package -> verify
-> publish -> retain operation through this fixed runtime boundary.

Rationale: an unbounded executor queue, timer per request, async runtime, or generic
path/`Read` bridge would increase retained memory and authority while making shutdown
and data-loss behavior harder to prove. Treating retry as a new periodic request would
also break mandatory migration/restore semantics. The capacity-one native design keeps
latency, memory, cancellation, and guarded mutation receipts deterministic without
coupling state to Slint or application lifetime.

## ADR-056 — Restore crosses sealed platform, store, and state capabilities

Decision: durable restore is not a state-only filesystem feature. Platform owns one
fixed `ArchiveRecoveryScope` bound to the exact data root, active
`tokenmaster.sqlite3`, staging child, quarantine child, and writer-lease identity.
The lease guard carries a private scope proof, so a guard for another archive cannot
authorize mutation. Platform alone derives opaque operation IDs, reserves at most
three create-new quarantine sets, observes the fixed main/WAL/SHM set, performs
write-through moves, `ReplaceFileW`/same-volume promotion, and rollback, and rejects
links, reparse points, unexpected entries, or mixed artifact identities. It accepts no
caller-provided path or filename and never automatically removes quarantine evidence.
The recovery staging namespace is separately capped at three exact operation-derived
reservation/candidate/stage artifacts; only an exact absent or completed journal authorizes their bounded
cleanup, while unexpected evidence is preserved and blocks.
The matching physical guard is checked before either store or platform cleanup. Both
allocators enforce the same ceiling. Admission observes active length `A` and selected
database length `B`, requires `max(2B, B+A) + 8 MiB` actual free space, releases the
candidate-verifier proof before corruption verification, and rejects active-fact drift
before the journal exists.

Package expansion produces a platform-owned `RecoveryStagedArchive`, not a generic
path. Store validates a path-free bounded reader by copying it into the existing
store-owned candidate namespace, then applies the complete SQLite/schema/foreign-key/
semantic verifier. The resulting proof contains only schema, length, and digest.
Platform promotion rechecks that proof against the still-sealed original stage. The
new active archive is reopened through the same path-free reader-to-store verifier,
so neither state nor a future UI/plugin receives a path, SQL connection, or generic
filesystem authority.

State owns the six-state redundant journal and orchestration only. Its payload also
records the fixed-set presence/digest facts needed to distinguish absent, moved,
replaced, rolled-back, and ambiguous artifacts. Settings restore first prepares an
exact next-generation/digest target, then publishes or verifies that target
idempotently. A conflict, invalid dual journal, wrong lease, stale catalog/candidate/
active identity, or artifact state that cannot prove exactly one forward or rollback
step enters safe mode with every artifact preserved.

Resume treats sidecar movement, main promotion, and settings publication as explicit
mutation-before-journal crash windows. In particular, if native promotion already
consumed the staged candidate while the journal still records
`sidecars_quarantined`, platform/state must fully verify the exact active candidate and
complete the same promotion step; absence of the staging name alone is never proof.

Rationale: the previous state-only Task 10 file plan could neither pass a verified
SQLite candidate into platform replacement without exposing its path nor prove that
an arbitrary lease guard protected the active archive. A platform-to-store dependency
would create a cycle, while a raw-path bridge would erase the reliable-state authority
boundary. A bounded reader copy costs one recovery-only sequential pass but keeps the
dependency graph acyclic, retained memory constant, and validation/promotion identity
explicit and independently testable.

## ADR-057 — Split startup diagnosis from application-owned reconstruction

Decision: startup reliability has two explicit owners. State owns fixed A/B run
markers, pre-open read-only diagnosis, journal resume, newest-first fully reverified
backup selection, and corruption-only automatic data-only restore. Runtime accepts the
already-held platform writer guard, retains it through archive open and startup
recovery, then releases it; later writes reacquire the same fixed lease per operation.
This eliminates the pre-open unlock/relock race without claiming a lifetime lock.
The application owns migration safety backups, provider-backed reconstruction when no
backup is usable, creation and teardown of every live/query/controller/maintenance
owner, and final clean publication.

State publishes and rereads `unclean` before catalog, package, or SQLite access. An
exactly clean prior run uses bounded normal inspection; unclean, missing, or invalid
state adds `quick_check(100)`. Startup never uses full integrity scan and never migrates
through the read-only inspector. A pending journal is resumed first. Definitive
corruption may select the newest completely revalidated backup and skip a corrupt newer
point; non-corruption failures preserve the active set. Two unclean launches of the
same recovered candidate are allowed, and the third enters safe mode.

Rationale: state has neither provider authority nor ownership of application threads,
so letting it create a replacement archive or mark a run clean would make data-loss
and shutdown claims unverifiable. Conversely, deferring run publication and diagnosis
to the UI/application would permit writable SQLite access before durable crash truth.
The split keeps the pre-open boundary small and auditable while making Task 12 prove
the larger lifecycle end to end.

## ADR-058 — Compose backup and migration safety under one application owner

Decision: `tokenmaster-app` owns one `ApplicationStateOwner` before every live owner
and one capacity-one backup maintenance runtime inside a healthy bundle. The concrete
backup operation follows only sealed store/state/platform capabilities through online
snapshot, full verification, typed package creation and re-verification, publication,
verified-package catalog binding, and bounded retention. The operation publishes one
current immutable bounded catalog projection: its first worker execution fully verifies
the bounded cold directory, and later rebuilds carry proofs only for unchanged
identities. This keeps retention active across restart without putting package
decompression on the UI/startup thread. Heavy snapshot, verification, and recovery I/O
uses an `Arc` snapshot outside the short-held projection mutex. A condition variable exposes
one deadline-bounded terminal receipt without adding polling, a UI timer, or another
thread. Mandatory application waits atomically reserve one exact maintenance root
before the worker is woken. While reserved, later submissions are rejected busy, so a
newer completion cannot overwrite the receipt before its waiter observes it.

An exact supported legacy archive remains read-only until a verified pre-migration
point exists. Run-state schema v2 then durably records the exact source/target schema
pair before writable open. Writable open and migration consume the same held startup
guard, and the bundle remains unpublished until a verified post-migration point exists
and clears that pair. If migration committed but post publication did not, the next
startup recognizes the already-current archive plus pending pair and completes the post
point before live publication. Periodic disablement cannot suppress either point. A
failure at any boundary discards partial owners, preserves the unclean launch and
durable old-or-migrated evidence, and leaves the existing safe-mode window as the sole
application surface. Clean is published only after every
bundle owner, including maintenance, has joined.

Rationale: placing migration backup policy in store/state would grant those layers
application lifetime authority, while constructing maintenance after live publication
would expose an unprotected migration window. A generic executor or polling waiter
would also enlarge retained state and latency variance. One application owner preserves
dependency direction, deterministic shutdown, constant-state scheduling, and the exact
pre/post safety invariant. Typed restore/restart commands and provider-backed no-backup
reconstruction remain a separate Task 12B milestone.

## ADR-059 — Bound application commands and generation-bind service restart

Decision: the application owns one constant-state typed command coordinator. It retains
one active permit and at most one distinct pending value; identical active or pending
hints coalesce, while a third distinct request is rejected busy. Request IDs are
checked, cancellation targets the exact active or queued ID, and an atomic
running/cancelled/irreversible state prevents late cancellation. Retry is an explicit
resubmission of only the last failed typed command. Pause removes the follow-up but
preserves the active receipt; final close additionally prevents admission permanently.

Current-bundle restart retains the Slint window, joins all old backend owners, acquires
a fresh fixed archive guard, and invokes the same guarded live construction path with a
strictly increasing bundle generation. Every runtime notifier holds only a weak bundle
slot plus its exact generation. It compares generation under the slot mutex before
publishing, so an old notifier cannot race from a successful precheck into a new bundle.

Rationale: an unbounded executor, generic command payload, per-command thread, or
notifier keyed only by a shared slot would permit memory growth, authority expansion,
or stale publication after restore/restart. The bounded coordinator and same-lock
generation check keep admission, cancellation, and observation deterministic. Task
12B.2 must bind these intents to one operation worker and sealed state/platform
capabilities; this decision alone does not claim import, restore, or reconstruction.

## ADR-060 — Identity-pin selected restore through retention and recovery launch

Decision: one restore choice is generation/ordinal only until `tokenmaster-app` seals
it against the current fully verified backup directory. Application state returns an
RAII process-local pin containing a private exact package binding. The pin gate is
shared with every post-publication retention deletion. A cycle admitted before the
restore command must consult a late pin immediately before each deletion; it replans
with the selected identity protected or fails closed. The pin remains active while the
old bundle joins and the protected `PreRestore` maintenance root publishes, then clears
before journaled replacement when no backup writer remains.

After recovery reaches its durable complete phase, the application binds the exact
operation generation/candidate receipt to the retained run session before inspecting
or starting restored bytes. A current candidate enters the normal guarded bundle path.
A supported legacy candidate instead repeats verified `PreMigration`, durable pending
source/target publication, guarded migration, verified `PostMigration`, and exact
pending completion. Only then may the replacement bundle publish. Clean-after-join
accepts the recovery generation, preventing a completed manual restore from becoming a
new recovery launch on the next start.

Rationale: a snapshot-only binding can race an already irreversible retention cycle,
while binding only after joining maintenance can turn a valid UI choice stale and force
an avoidable safe-mode transition. A generic global operation mutex would block
unrelated catalog reads and enlarge latency. One fixed late-pin gate serializes only
binding versus deletion, preserves constant memory, and keeps heavy I/O outside the
mutex. Receipt-before-restored-lifecycle ordering closes the independent crash window
between a complete journal and run-state acceptance. Task 12B.2b.1 now supplies the
worker/manual-backup/config core in ADR-061; the remaining Task 12B.2b owns native-file/
UI bindings, cancellation propagation, verification, restore/rebuild execution, and
no-backup reconstruction.

## ADR-061 — Execute application operations on one joined bounded worker

Decision: the production application replaces its bare command coordinator with one
`ApplicationOperationWorker`. The worker owns the sole coordinator, one
standard-library thread named `tokenmaster-operation-worker`, one capacity-one wake,
and one latest-only completion. It executes the fixed typed permit outside its mutex,
normalizes exact cancellation before coordinator completion, converts a caught callback
panic to a fixed internal failure and faulted/closed state, and joins on explicit
shutdown and `Drop`. Clean run state requires this join before bundle shutdown can be
accepted. It adds no async runtime, generic closure queue, unbounded channel, operation
history, per-command thread, or detached owner.

The first production executor binding is manual backup. The command crosses its
irreversible boundary immediately before handing mutation authority to the existing
maintenance runtime, holds the bundle slot stable through one atomic exact-root wait,
and maps only the path-private maintenance receipt to a fixed command outcome. This is
conservative cancellation: once maintenance submission can publish, late cancel is
rejected and shutdown joins the result.

The same milestone establishes config operations over sealed capabilities without
claiming native dialogs. `.tmconfig` has a separate 2 MiB encoded ceiling enforced by
writer and reader. Export accepts a controlled create-new target, stages typed portable
settings, crosses irreversible state immediately before publication, and rereads the
published package. Import fully verifies an already open bounded reader and retains one
typed candidate/base-identity preview with at most three categories; confirm consumes
that preview through the existing atomic settings commit and preserves device-local
state. UI never receives a target, reader, path, filename, raw bytes, or digest.

Rationale: executing backup or package work in a Slint callback would make visible input
latency depend on disk, compression, SQLite, and antivirus behavior. A general executor
or command-payload channel would expand authority and retained memory. Reusing the
constant-state coordinator inside one joined worker keeps memory independent of command
count, provides a single shutdown barrier, and lets later sealed native-file, verify,
restore, and rebuild capabilities attach without changing the UI intent contract.
Task 12B.2b still owns those remaining bindings and no-backup reconstruction.

## ADR-062 — Keep native file selection sealed inside platform capabilities

Decision: Task 14 uses the existing pinned `windows = 0.62.2` bindings for the Windows
Common Item Dialog instead of adding `rfd`, a webview, a shell command, or an Explorer
child. `NativeFileDialog` initializes the calling thread as STA, balances every
successful COM initialization, installs one exact file filter/default extension, keeps
the working directory unchanged, requests filesystem/non-link results, distinguishes
user cancellation from failure, copies the returned path into transient platform-owned
memory, and frees the COM allocation. It is deliberately `!Send`/`!Sync`, requires an
active current-thread owner, and fails unavailable instead of moving COM to an arbitrary
worker or showing an unowned dialog. Unsupported native hosts return one stable
unavailable result; `ControlledFileDialog` supplies a deterministic capability-only
selector without a path callback or queue.

The public boundary has three fixed file kinds and one generic result with only
selected/cancelled/stable-failure states. Selected input is already opened, regular,
single-link, and caller-size-bounded. Selected output records target absence or opaque
physical identity, creates only one bounded adjacent stage, rechecks selection state
before stage/publication, and publishes only a sealed candidate through atomic
create-new or replace. Input uses a final-component no-follow open and selected-parent
physical binding. Windows stage cleanup remains bound to a retained delete-capable
handle.
Existing replace captures the displaced target, validates its physical identity after
the syscall boundary, rolls back concurrent identity drift, revalidates the published
identity/bytes, and only then deletes the old target; ambiguity preserves recovery
evidence. Existing target bytes are never opened for truncating writes.
Unicode child names are accepted under fixed UTF-16/path bounds; invalid suffix,
remote/device/mapped-remote parent, linked/reparse/hard-linked target, directory,
capacity excess, and observed identity drift fail closed. Every capability/error/
`Debug` is path-private and nothing is persisted.

Rationale: a raw `PathBuf`, generic `File`, or callback returning a path would move
filesystem authority into app/Desktop/CLI and make later plugin confinement impossible.
Writing directly to the Save-dialog result would let cancellation, codec failure, or
process death truncate the user's prior export. A new cross-platform dialog dependency
would expand supply chain and renderer/event-loop behavior without improving the
Windows-first contract. The synchronous platform primitive is intentionally not a UI
binding: Task 15 must invoke selection on the owning Slint/STA thread, then dispatch
only its sealed capability to bounded operation work; interactive acceptance must be
recorded separately.

This decision does not claim equivalent hostile-race cleanup on Unix. Native selection
is unavailable there; the controlled selector remains deterministic and fail-closed on
observed identity drift, while conditional handle/directory-relative deletion is a
portability gate before enabling a future Unix native selector.

## ADR-063 — Reconstruct into fresh truth and publish loss explicitly

Decision: when the active archive is definitively corrupt and no fully reverified
backup is usable, the recovery coordinator may create only one fresh normal-schema
archive through the ordinary store constructor. It fully verifies that archive, stages
and reverifies it, proves active corruption again, publishes a reconstruction journal
with no backup identity, quarantines the exact main/WAL/SHM set, atomically promotes,
and fully verifies the new active archive. Journal backup absence is accepted only for
this explicit reconstruction mode; normal restore keeps its exact backup identity.
There is no corrupt-row salvage, partial copy, main-only copy, or plausible zero fill.

Application composition does not treat the empty verified archive as healthy. It starts
one guarded live runtime, forces `RefreshUrgency::Recovery`, and waits through the
worker's bounded event-driven completion path until one successful authoritative local
source reconciliation is complete and no refresh is active or pending. Only then does
it start backup maintenance from `Healthy`. Failure or timeout returns to safe mode.
The durable path-free recovery projection distinguishes verified-backup restoration
from authoritative-source reconstruction and explicitly marks quota, reset-credit,
reminder, and Git history as non-reconstructible and unavailable.

A complete reconstruction journal is also durable evidence of an unfinished source
reconciliation whenever bootstrap starts that candidate. Application preflight maps
that state to an effective recovery-required outcome, retries the bounded source refresh
on cold start, and clears the obligation only after successful reconciliation. A failed
same-process refresh reuses the promoted archive on explicit retry; it does not require
the archive to remain corrupt and does not repeat quarantine or promotion.

The same application/UI milestone binds native dialogs on their owning Slint/STA thread
and sends only sealed capabilities to the single joined operation worker. One latest-
only projection carries bounded state, preview, operation phase, and recovery receipt.
Each mutating operation publishes `AtomicPromotion` and disables cancel exactly after
its irreversible boundary. Manual backup remains cancellable until that boundary.
Queued follow-ups publish `Running` when execution actually starts. Restore confirmation
consumes the exact reviewed generation/ordinal identity, and unknown reliable-state
metrics remain unavailable rather than zero. Desktop owns no path, file, SQLite, state/store/runtime,
provider, or recovery capability and uses no polling timer or progress queue.

Rationale: immediately exposing a structurally valid empty archive would turn temporary
reconstruction state into false healthy zero truth and could publish a backup before
authoritative input was restored. Reusing the normal store schema and live refresh path
avoids a second ingestion implementation. An optional journal backup identity preserves
old restore compatibility while making source reconstruction explicit and resumable.
Latest-only UI state and exact irreversible phases keep retained memory constant and
prevent cancellation receipts from contradicting durable mutation.

## ADR-064 — Accept Reliable State through a separate bounded evidence rail

Decision: P3-D.0 uses three release-mode developer contracts and one strict receipt,
separate from M0 and product release. Backup throughput runs automatic, normal, and
compact pipelines against deterministic 8 MiB and 96 MiB schema-13 freelist fixtures.
The fixture's intentionally random Git installation salt is normalized only in test so
its byte length and SHA-256 are reproducible. Streaming evidence uses an absolute 64 MiB
private-growth ceiling plus 16 MiB large-database headroom rather than comparing two
single sampled peaks, because allocator/Zstd high water can be constant yet shorter than
the Windows sampling interval on the small fixture.

Resource evidence warms backup, acquired-candidate cancellation, and a complete restore
before establishing one immutable baseline. It then runs 256 backup/package/verify/
import-inspect-cancel/retention cycles, 16 cancellations after source-reader/candidate
acquisition, and 16 complete isolated data-only restores. Every measured retention pass
must return to the exact filled-tier bytes; final encrypted compact resources are
compared with the original baseline, never a newly relaxed one. UI evidence waits for
one completed warm-up backup and pins the next started cycle; that same cycle must remain
in progress across all loaded Dashboard query and software-paint samples and complete
only during joined shutdown.

The acceptance receipt binds a full clean commit, `dirty=false`, the tested application
SHA-256, exact format/tool versions, fixture identities, command arrays, durations,
metrics, limits, and each gate. It is generated only under ignored `reports/`, contains
no private data, and fails closed on missing or duplicate fields. A software-rendered
paint is real Slint paint work but not physical-display or OS-input evidence. Therefore
this receipt can close P3-D.0 only after Task 18 documentation and full workspace gates;
it can never accept M0, packaging, signing, soak, or a product release.

## ADR-065 — Ship History as an independent bounded product slice

Decision: P3-D.1 introduces a separate product History analytics section rather than
reusing the today-only Dashboard payload. Its initial request is the exact latest 30
civil days in the positively identified system timezone, daily series, initially no breakdowns,
with the existing 400-day query maximum as the future range ceiling. The request runs
sequentially on the existing capacity-one desktop query worker and publishes/fails
independently. Cancellation and deadline preserve complete-attempt publication.

The desktop copies at most 30 rows newest-first into one identity-free projection and
one Slint model. The view presents overview tokens/cost/events, exact display range,
timezone, freshness/quality, trend, and responsive daily detail. Route selection does
not query, create a worker/timer/cache, reconstruct the window, or retain prior ranges.
Interactive ranges and pagination remain later work over the same replace-only section.

Rationale: reusing Dashboard analytics would make History depend on a today-only
request and risk misleading totals. A final mutable range scheduler now would widen
the state machine before one real supporting route proves the complete vertical path.
One fixed bounded request validates query, reducer, controller, projection, and UI
contracts while preserving constant frontend memory and a direct upgrade path to
bounded range controls.

Evolution: ADR-068 later adds bounded Model and Project breakdowns to this same request
without changing its History range/series semantics or adding an analytics call.

## ADR-066 — Split the Sessions list from exact detail selection

Decision: P3-D.2a publishes one all-time newest-first page of at most 64 session
summaries through the existing product snapshot and capacity-one desktop query worker.
Dashboard copies its existing first 12 summaries; the Sessions route owns a separate
identity-free projection, one Slint model, explicit `has_more`, responsive aggregate
columns, and no query behavior on route selection. Opaque session keys and cursors do
not cross the controller boundary.

Exact detail is P3-D.2b rather than a UI callback over the current page. It will add a
checked `DesktopSessionSelectionGeneration`; Slint will submit only a visible ordinal
plus the viewed product generation, the controller will resolve that pair to its opaque
dataset-bound key, and only the latest matching selection/result may publish. It will
reuse the existing worker with one latest/coalesced request and one replace-only detail
section, not create a per-selection thread, queue, cache, or database owner.

Rationale: the bounded page is independently useful and proves the complete list path.
Bolting detail onto row callbacks would either expose private query identity to Slint or
allow rapid selection and dataset refresh to publish stale detail for the wrong row.
The split delivers visible value now while making the concurrency and privacy boundary
explicit before detail authority exists.

## ADR-067 — Bind exact session detail to backend, snapshot, and selection generations

Decision: P3-D.2b identifies an exact selection with three axes: a checked monotonic
`DesktopSnapshotEpoch` for the live controller/bridge lifetime, the viewed immutable
`ProductGeneration`, and a nonzero selection generation plus visible row ordinal. Slint
submits no opaque key. The application forwards the intent only to the current live
bundle; the existing controller worker resolves the ordinal to `UsageSessionKey` only
after epoch and product-generation validation. A higher backend epoch accepts a restarted
inner generation, rejects the old backend, and clears selection.

The controller shares one latest-only selection slot with ordinary refresh admission and
creates no worker, queue, or cache. Product state correlates one replace-only detail
section by selection generation/ordinal and never retains a different selection's payload
on failure. Desktop renders one explicit idle/loading/ready/missing/unavailable projection
with exact summary/evidence and a combined maximum of 32 model plus 32 approved path-free
project-alias rows. Route-only navigation remains query-free and does not rebuild models.
The weak application bundle handoff uses `try_lock` and rejects contention so maintenance
ownership can never turn a row selection into a UI-thread wait.

Rationale: product generation alone can alias after controller replacement, while an
opaque UI key would widen the privacy/authority boundary. A per-click worker or queue
would add shutdown/order/memory growth. The three-axis value plus latest-only slot makes
rapid clicks, concurrent refresh, cancellation, failure, dataset drift, and backend
replacement fail closed with constant frontend memory and no stale-row disclosure.

## ADR-068 — Share one bounded recent-usage envelope across History, Models, and Projects

Decision: P3-D.3 adds Model and Project breakdowns to the existing shared recent-usage
History analytics request. The request is selected only by exact rolling 1/7/30 presets,
with 30 as default and maximum. History continues to consume its daily series; Models reads
the Model breakdown; P3-D.4 Projects reads the already captured Project
breakdown. There remains exactly one today Dashboard query and one recent-usage query
per refresh on the existing capacity-one worker. No Models product section, third
query, cache, timer, connection, thread, or route-time work is added.

The query boundary caps each breakdown at 256 items plus lookahead and explicit
truncation. Desktop copies at most 64 canonical model rows with complete typed input,
cached, output, reasoning, total, event, and cost evidence. Cost availability, selection
mode, and actual calculated/reported/mixed composition remain typed in Desktop; Slint
renders availability and actual composition visibly and accessibly, so partial cost
cannot resemble complete cost. One Slint model is replaced
only for an accepted product generation. Backend and desktop truncation are visible;
wide and narrow layouts retain the same row meaning. Provider/profile/source/account/
workspace/project/session identities and every opaque authority remain outside Models.

Rationale: reusing Dashboard would mislabel today as recent exploration, while a separate
Models query would duplicate aggregate/price work and let view ranges drift.
Renaming the current product `history` field would add mechanical API churn without
changing ownership. The selected shared immutable envelope gives History, Models, and
Projects coherent range/timezone/evidence, constant frontend memory, instant route
switching, and one future replacement point for bounded interactive range controls.

## ADR-069 — Render Projects as bounded recent usage plus separately labelled UTC-today code

Decision: P3-D.4 consumes the prefetched Project breakdown from ADR-068 and the
existing today Git envelope. It adds no query, product section, worker, timer, cache,
connection, dependency, or route-time work. Desktop keeps at most 32 usage-centric
rows, matches named aliases to at most 32 loaded Git repositories by exact safe
`ProjectAlias`, and never matches `Unassociated` or appends Git-only aliases.

The evidence windows remain independent: recent usage is the selected shared local civil
range (1/7/30, default 30), while code output is the existing exact UTC-today range. Both ranges,
timezones, quality, freshness, and completeness render explicitly. Usage controls
ordering, tokens, cost, events, and relative distribution; Git controls commits/
added/removed/net and efficiency. The frontend never combines or relabels the periods.

Same-alias repository facts use checked sums. Project-level efficiency is recomputed
only when every repository carries compatible available evidence with one identical
transient usage dataset identity and cost; product-code additions are summed and the
project cost is counted once. Any mismatch/absence/overflow disables efficiency or
the affected code facts without hiding independent usage. Private IDs, paths, content,
and authority cannot cross Desktop or Slint.

Rationale: a usage-only Projects table would make Git-dependent route readiness
incoherent, changing the Dashboard Git request to 30 days would mislabel Dashboard,
and a second Git query would add work/state before interactive range ownership exists.
The selected two-window presentation provides material project value now while
preserving truthful boundaries and a clean future upgrade to generation-fenced shared
range controls.

## ADR-070 — Ship Recent activity from the existing bounded latest page

Decision: P3-D.5 renders the already-prefetched `LatestActivityPage` as one
newest-first Recent activity route. Desktop retains at most 12 rows with UTC timestamp,
canonical model key, typed input/cached/output/reasoning/total tokens, evidence, and
optional page incompleteness. The existing `LatestActivityRequest::first(12)` remains
the only Activity query and uses the current capacity-one refresh worker. Activity
stays available during aggregate rebuild because its product section is independent.

One accepted product generation replaces one `Arc` row list and one Slint model.
Route selection changes only visibility and cannot query, rebuild, select a row,
paginate, filter, export, retain prior pages, or create a timer, worker, queue, cache,
connection, callback, or raw-event surface. Scope, provider/profile/account,
event/dataset/source/session/project identity, cursor/fingerprint/key, paths, content,
prompts, responses, commands, credentials, and authority never cross Desktop/Slint.

The route is deliberately named Recent activity, not rhythm or heatmap. A truthful WMT
time-distribution counterpart requires a separate bounded hourly/day-of-week aggregate
with explicit timezone and DST semantics; deriving it from a 12-item page would be
incomplete and misleading.

Rationale: the latest page is already captured for Dashboard and is immediately useful,
so another query or owner would add latency and memory without new truth. Treating a
small event sample as a rhythm chart would create false parity. This slice delivers
responsive drill-out value now while preserving the exact future aggregate boundary.

## ADR-071 — Separate the read-only expiry center from notification delivery receipts

Decision: P3-D.6 renders the existing all-current benefit overview as one bounded
Notifications route. Desktop retains at most 32 identity-free effective-profile rows,
256 separate current-lot rows, and eight leads per profile. It preserves every expiry
precision and evidence/policy distinction. One accepted product generation replaces
one scope model and one lot model; route selection changes visibility only.

The projection and navigation do not call reminder `take`, `acknowledge`, or `release`.
A future visible-delivery bridge remains application-owned: it leases a bounded batch,
applies every row on the UI event loop, acknowledges only after successful presentation,
and releases on failed, cancelled, or closed-window presentation. Settings editing,
snooze, quiet hours, OS/tray delivery, usage alerts, and activation remain independent
capability slices.

Rationale: acknowledging during query, projection, or navigation would claim delivery
before the user could see it and could permanently suppress an expiry warning. Exposing
the reminder runtime to Slint would also widen frontend authority. The split delivers
instant inventory/profile visibility now while preserving crash-safe retry semantics,
constant frontend memory, and a narrow future command boundary.

## ADR-072 — Keep Help/About static and use the standard Slint attribution surface

Decision: P3-D.7 replaces the Help/About placeholder with one archive-independent
static Slint view containing six fixed accessible sections. `DesktopShell` applies the
compile-time Cargo package version exactly once during window construction. The view
owns no product projection, list model, query, diagnostic probe, worker, timer,
callback, cache, queue, connection, polling loop, or mutable capability.

The view mounts exactly one pinned standard `AboutSlint` widget. Its library-owned
fixed Slint attribution action is accepted as the selected Royalty-free License 2.0
route. TokenMaster adds no `Platform.open-url`, URL property, arbitrary-link callback,
browser/session reuse, or private endpoint surface. Generated third-party notices,
SBOM, canonical MSVC package/signature identity, public-download attribution, and
release acceptance remain P6 receipt-owned. P4 migrates the complete UI together to
en/ru/pseudo locales; P5 adds bounded read-only CLI/MCP without changing this view.

Rationale: a static route is immediately useful in normal, unavailable, recovery, and
safe-mode states while retaining constant memory and zero refresh latency. A dynamic
diagnostics or release-status screen would duplicate authoritative owners and invite
false claims. The standard widget satisfies the chosen attribution surface without
inventing a TokenMaster-controlled browser or URL API.

## ADR-073 — Acknowledge expiry reminders only after app-owned visible presentation

Decision: Desktop owns a present-only batch capped at 256 rows, one independent checked
epoch, one weak-window event callback, and one transient Slint model. It emits the
one-shot `Presented` receipt only after replacing the complete model and count label,
setting the panel visible, and verifying the applied row count. Desktop never receives
the reminder runtime, store, delivery IDs, paths, provider payload, or acknowledgement
authority.

The application owns the runtime adapter and one condition-variable receipt worker.
The worker retains no batch, acknowledges outside the UI thread, retries only `Busy`
and `StoreUnavailable` acknowledgement failures after exactly 60 seconds. A confirmed
release after failed presentation schedules re-pump on the same worker; a newer receipt
interrupts that wait. A terminal acknowledgement error releases without automatic
re-presentation. Runtime acknowledgement catches and redacts panic, restores `Acknowledging`
to `Leased`, and the app's narrow fallback release may recover outer-mutex poison.
`Err` or `false` release retains local backpressure. Desktop clears bridge-busy state
before receipt invocation, and the worker joins before reminder pause/shutdown. Settings,
snooze, quiet hours, OS/tray delivery, usage alerts, and activation stay separate.

Rationale: event-loop scheduling is not evidence that the user-visible model exists.
Separating the present-only bridge from durable authority prevents false delivery,
keeps UI latency and memory constant, preserves crash replay, and avoids exposing
runtime locks or private delivery identity to Slint.

## ADR-074 — Synchronize one global reminder profile from portable desired state

Decision: portable settings remain the desired-state authority for reminder policy.
The existing single operation worker publishes Pending, durably saves a changed policy,
and then uses one synchronizer to map generation `N` to global profile revision `N +
1`. Startup, restored/reconstructed/migrated paths, explicit Save, and confirmed
config import reuse that synchronizer. A failed archive projection never rolls back a
durable desired state and remains visibly retryable as Pending. At startup, optional
reminder runtime health may independently report StoreUnavailable, but it cannot
replace the exact enabled/leads desired-state projection with Unavailable.

The store replaces only the global profile in one immediate transaction, rebuilding
inherited due rows while preserving scope overrides, deliveries, acknowledgements, and
provider evidence. It admits 32 inheriting scopes, 64 current lots per scope, and 256
aggregate lots, with at most eight leads. Desktop owns a fixed five-preset/eight-custom-row editor with
checked conversion, dirty-draft retention, accessibility labels, and no runtime,
store, timer, polling, worker, queue, or delivery authority.

Rationale: deriving the global profile from durable portable settings gives import and
recovery one authoritative policy without allowing a UI draft or archive availability
to become semantic authority. Per-scope editing, snooze, quiet hours, OS/tray delivery,
usage alerts, activation, P4/P5/P6, M0, packaging, signing, soak, and release remain
separate work.

## ADR-075 — Acknowledge Pending before mutation and retain the latest desired policy

Decision: payload-free commands retain ordinary coalescing, but reminder and backup
policy changes use one immutable active operation plus one replaceable pending payload.
The first later policy occupies that pending slot and subsequent matching policies
replace only its payload, so the newest admitted desired state executes without queue
growth. Other payload-bearing commands never report a coalesced admission when their
payload would be discarded.

Explicit reminder Save and confirmed config import publish a Pending projection with
the exact atomic-promotion operation and wait at most five seconds for the Slint event
loop to apply it before changing settings bytes. Failure restores the prior sync state,
aborts the settings mutation, and preserves a config-import preview for retry. The
one-shot acknowledgement adds no persistent worker, timer, or queue and transfers no
store, path, provider, delivery, or activation authority to Desktop.

The store represents aggregate due counts as `u64` because valid overridden scopes are
outside synchronized rebuild caps. Every fallible conversion occurs before the final
SQLite commit; result construction after commit is infallible. The 32-scope, 64-lot-
per-scope, 256-synchronized-lot, and eight-lead work caps remain unchanged.

Rationale: accepting a coalesced payload that is never executed loses user intent, and
scheduling a UI callback is not proof that Pending became visible. A bounded latest-
wins slot and visible acknowledgement close both races while preserving constant memory
and single-worker ordering. Pre-commit validation keeps an error return from
contradicting already committed archive state.

## ADR-076 — Keep the command palette route-only and inside the sole window

Decision: P3-E.1 adds one final full-window overlay to the existing production
`MainWindow`. It projects only the fixed 11 `RouteRow` values, owns one query capped at
64 Unicode scalar values and one replace-only result model, and routes every activation
through the existing `DesktopState` stable-key selection. It adds no second window,
snapshot, controller, query, timer, worker, cache, general command registry, or native
authority. Accepted snapshots refresh the open overlay after releasing the Desktop
state mutex.

Rationale: a generic action palette would become a new mutation/security boundary and
could duplicate application authority. Reusing the immutable route projection keeps
latency and retained memory bounded while giving keyboard, pointer, and accessibility
users one consistent navigation surface. Compact content and native lifecycle remain
separate P3-E decisions.

## ADR-077 — Keep compact quota content in the existing snapshot and window

Decision: P3-E.2 mounts one always-present `CompactWidgetView` inside the sole
production `MainWindow`. Selecting `compact_widget` hides the normal shell, shows all
current `DashboardQuotaRow` values up to the existing 32-row cap, and changes the
window to a 420 by 560 logical-pixel presentation. Returning to Dashboard restores one
captured valid normal physical size. The compact surface renders freshness/reasons,
explicitly distinguishes an unknown usage ratio, and exposes one keyboard, pointer,
and accessibility return action through the existing stable route selection.

The mode owns no query, second snapshot, provider-specific fixed row, worker, timer,
queue, cache, controller, window, or native lifecycle authority. Its one bounded
geometry slot is device-local and is not portable configuration or data authority.
Close-to-tray, always-on-top policy, Explorer recovery, activation, hotkey, and startup
remain later typed app/platform capabilities.

Rationale: a separate compact window or data owner would duplicate renderer, snapshot,
refresh, focus, and shutdown state. Reusing the current quota model makes route entry
immediate and constant-bounded while preserving provider-defined windows and missing-
data truth. Exact restoration keeps compact presentation reversible without letting
window geometry enter the product snapshot.

## ADR-078 — Isolate one checked Windows tray owner and keep lifecycle authority in the application

Decision: P3-E.3 adds one TokenMaster-owned Windows tray adapter and one fixed icon
asset. It emits only Show, Hide, OpenCompact, OpenDashboard, and Quit through the
single-install `Rc` router with no event queue. One hidden top-level tool window owns
the icon and fixed menu on the Slint/winit UI thread. An atomic reservation rejects a
second owner. The production Desktop dependency no longer enables Slint
`system-tray`; that feature is scoped only to the separate M0 probe.

The application shows the main window before deferred tray installation and runs the
event loop until explicit Quit. Explorer `TaskbarCreated` triggers one checked re-add.
If re-add fails, availability becomes Unavailable and Show is submitted immediately;
close-to-tray is then disabled and a close request quits instead of hiding. Show and
route actions unminimize, show, raise, and request foreground focus for the same raw
native window. Quit remains only an event-loop return request, so the established
joined shutdown is still the sole clean-run authority.

Rationale: independent review proved that Slint 1.17.1 creates its callback window as
`HWND_MESSAGE`, while Explorer broadcasts only to top-level windows, and that the
backend discards the re-add result. Keeping it could strand a permanently invisible
process. A pinned upstream upgrade does not fix the current implementation. Replacing,
rather than duplicating, that owner gives TokenMaster explicit availability and
failure semantics without a thread, timer, polling retry, or queue. The confined
Win32 adapter is the only `unsafe` boundary: it boxes, installs, and exact-readback
verifies its raw callback pointer before registering the icon or publishing Available,
then clears that pointer before destroying handles. Interactive Explorer restart, Windows foreground policy,
sleep/resume, and resource return still require acceptance. Current-session hotkey/
single-instance status is governed by ADR-079; current-user startup, M0, package,
signing, soak, and release are not claimed.

## ADR-079 — Use one current-session event for single-instance arbitration and activation

Status: implemented as P3-E.4 developer evidence. Interactive multi-process, focus,
hotkey-conflict, cross-token ACL, sleep/resume, and real hotkey resource acceptance
remain open.

Decision: reserve the non-inheritable auto-reset event
`Local\TokenMaster.CurrentSession.Activation.v1` before renderer/data construction. A
new event is the primary reservation; an existing event is a secondary process that
only calls `SetEvent` and exits. The primary later starts one joined message-driven
thread with one unnamed shutdown event and fixed `Ctrl+Alt+T` / ID `0x544D` hotkey.
Both secondary and hotkey input normalize to the existing Show/restore/focus action and
coalesce through one pending bit and one event-loop task.

Rationale: one auto-reset event provides both ownership lifetime and a capacity-one
cross-process wake without a path, payload, server, socket, pipe, hidden window, queue,
or polling loop. `Local\` gives explicit session isolation; the creator-token default
DACL gives current-user access without embedding identity in an object name. Starting
the native thread only after the weak-window sink exists retains an activation that
arrives during startup while removing a mutable callback slot from Platform.

The fixed chord deliberately avoids WhereMyTokens' ordinary `Ctrl+Shift+D`, which can
override a common in-application shortcut. Registration conflict and other failure are
distinct bounded health states and do not make the visible product unusable. A claim or
secondary-signal failure fails closed as `current_session_unavailable`; unregister or
join failure prevents clean-run publication. Configurable hotkeys, current-user startup,
interactive conflict/secondary/focus/ACL/sleep/resource acceptance, M0, package,
signing, soak, and release remain later work.

## ADR-080 — Keep current-user Run registration as the sole startup truth

Status: implemented as P3-E.5 developer evidence. Packaged sign-in launch, relocation,
denied ACL, and resource-return behavior remain interactive acceptance.

Decision: use only the fixed `TokenMaster` `REG_SZ` below
`HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`. Do not add a
second startup flag to reliable settings. Read-only inspection yields Disabled,
EnabledVerified, StaleRelocation, Conflict, AccessDenied, or Unavailable. Enable,
explicit stale repair, and disable/removal are the only mutations. A conflict is never
overwritten; enabled and disabled success require immediate exact readback. The stored
command is the quoted current executable path without arguments, capped at 260 UTF-16
code units excluding its terminating NUL, and enabled readback
must reopen the same physical file identity.
Only ordinary drive-letter paths on fixed, removable, or RAM local volumes are
eligible. UNC/device/remote paths fail before file I/O, and a same-basename alternate
local path is stale without being opened.

Rationale: duplicating the preference between A/B settings and the Windows Run value
would create an impossible cross-store atomic commit and ambiguous recovery. Treating
the OS value as the sole device-local truth makes observed state honest and excludes it
from config/backup by construction. Explicit stale repair prevents a moved copy from
silently replacing a same-named entry, while fixed HKCU-only authority preserves
portable use without elevation, installer, process, shell, task, service, polling, or
retained-path machinery. Bounded synchronous calls keep the UI path immediate and add
no long-lived memory or thread owner.

## ADR-081 — P4-B durable density remained one bounded axis (historical)

Settings schema v2 adds only `presentation.density`, fixed to Comfortable, Compact,
and Ultra compact. v1 is read and migrated in memory to Comfortable; startup does not
rewrite it. Package manifest and decoded settings source versions bind exactly. The
Desktop admits the typed command before style mutation and reuses the existing
one-active/one-pending worker; no new desktop worker, queue, timer, cache, or I/O owner
is justified. Skins, layouts, schemes, locales, and interactive acceptance remain open.

## ADR-082 — P4-C adds three built-in skins without new authority

Adopt schema v3 and one complete presentation selection. `Refined`/`refined`/0,
`Graphite`/`graphite`/1, and `Ember`/`ember`/2 are stable. Rust owns the immutable
15-role palettes; Slint contains no family table or selection branch. This remains a
partial P4 decision: layout, scheme, locale, typography, accessibility/DPI, paint and
resource acceptance are explicitly deferred.

## ADR-083 — Validate P3-E interaction only against the packaged production executable

Status: acceptance contract and fail-closed schema/local-identity preflight implemented;
authenticated P6 package provenance and external evidence remain absent.

Decision: P3-E developer implementation closes before P6, while its final interactive
receipt is produced only after P6 supplies the exact packaged production executable.
The read-only preflight is defined early and checks an operator-attested receipt against
a clean Git commit, executable name/SHA-256, disposable-host and rollback claims, eleven
fixed scenario claims, and a post-warm-up resource envelope. It never authenticates
those external claims or P6 provenance, launches the app, mutates HKCU/Explorer/power
state, creates a package, or treats `tokenmaster-m0` as product evidence. P6 must add
the authenticated producer/package-manifest binding before acceptance.

Rationale: requiring a packaged executable inside P3-E implementation order creates a
circular dependency on P6; accepting an unpackaged or isolated probe binary weakens the
startup identity obligation. Separating developer closure from packaged interactive
evidence preserves both sequencing and exact executable truth. Real mutation remains an
external disposable-host operation, so repository automation cannot damage the
operator's current profile or fabricate manual Windows behavior.

## ADR-084 — Navigate Sessions with one private forward cursor and no page history

Status: implemented as P3-D.2c developer evidence.

Decision: Sessions exposes only `Next page` and `Back to newest`. The current immutable
page carries an identity-free newest/continuation kind and `has_more`; Slint emits only
the typed direction plus checked snapshot/product/navigation generations. The opaque
dataset-bound continuation remains inside the existing capacity-one controller worker.
The controller retains one latest pending navigation intent, no queue and no cursor
history. `Back to newest` jumps directly to the fixed first request. Ordinary refresh
also resets to newest and supersedes navigation.

Every accepted result replaces the sole at-most-64-row Sessions model and clears exact
detail. Pending navigation clears detail and disables row selection without rebuilding
the model. Snapshot epoch, viewed product generation, navigation generation, cancellation,
deadline, refresh precedence, and current publication authority all fail stale work
closed. The application handoff retains only a weak current bundle and uses nonblocking
admission. No worker, channel, timer, cache, archive owner, public cursor, page number,
or unbounded retained collection is added.

Rationale: cumulative load-more or a browser-like cursor stack grows frontend memory and
complicates stale-detail ownership. Direction-only replacement preserves access to older
sessions with constant memory, while direct recovery to newest makes refresh and failure
semantics unambiguous. Exact keyboard, model-identity, sink-lifetime, mutation, and
generation-race contracts pin the boundary without exposing private query identity.

## ADR-085 — Recover terminal Sessions navigation through one exact event-loop rollback

Status: implemented as corrective P3-D.2c developer evidence.

Decision: use the existing worker completion notifier and snapshot bridge lifecycle to
release an admitted Sessions navigation when cancellation, deadline, abandonment, or
stale execution publishes no snapshot. Callback and receipt polling share one
idempotent reconciliation function; the first consumer atomically takes the exact
intent, then notifies after all controller locks are released. The bridge retains only
the highest navigation generation in one slot, schedules one existing event-loop task,
applies a pending snapshot first, and rejects only an exactly equal UI intent.

Refresh supersession sends the same exact rollback immediately after releasing the work
lock. Failed `Next` with a retained page permits direct newest recovery, while initial
unavailable state without a page remains closed. Synchronous completion observation
continues to report reconciliation failure; the worker callback is intentionally
best-effort. No polling loop, new worker/thread/channel, completion history, cursor
history, or second page/model is introduced.

Rationale: publishing a synthetic product snapshot for cancellation would weaken
snapshot authority, while leaving UI correlation dependent on polling can freeze the
controls forever. One identity-safe rollback preserves constant memory and keeps
successful and query-error snapshots authoritative.

## ADR-086 — Bound interactive shared History ranges and arbitration

Decision: one shared recent-usage envelope is controlled by exact rolling 1/7/30-day
presets, defaulting and capping at 30. History, Models, and Projects replace and degrade
together; Dashboard today and Projects UTC-today Git evidence remain separate. The
controller retains one published preset, one persistent scalar range-generation
high-water mark, one active correlation, and one latest pending intent. Range work and
Sessions detail/page work are mutually exclusive at admission (`Busy` on conflict).
Refresh supersedes range work; only the latest eligible follow-up survives. Epoch,
product-generation, worker-correlation, and selection-generation fences apply before
query and publication, and stale output is discarded. Two optional non-displacing
terminal notifier slots reconcile only exact whole-intent matches. The intent boundary
contains no free-form dates/counts, identity, paths, query objects, or archive handles.
Only `ProductPublishOutcome::Accepted` publishes a section-local snapshot; only an
accepted successful query updates the preset. `Coalesced`, `RejectedOlder`, and
`RejectedIncompatible` publish nothing and leave the preset unchanged, then complete
through exact no-snapshot rollback. Backend epoch replacement resets the published
preset to default 30 days and clears old correlations. Snapshot publication precedes
terminal completion observation, successful commit consumes current work first, and
exact idempotent reconciliation cannot roll back a committed selection.

Rationale: fixed presets preserve the 30-row bound and truthful rolling semantics while
avoiding a second analytics owner, retained range history, stale Sessions page-relative
state, or unbounded privacy surface.

## ADR-087 — Aggregate full rhythm through the shared recent History envelope

Decision: an opted-in recent History request produces two fixed distributions: 24
local-hour buckets and seven Monday-Sunday buckets. The request is limited to 30 civil
days; query planning walks time without retaining minute rows, compresses local
occurrences, and rejects more than 768 occurrences or 2,304 aligned rollup segments.
Each bucket carries the existing aggregate token/event/activity algebra plus elapsed
minutes and occurrence count, so skipped hours, repeated folds, fractional offsets,
and skipped dates remain distinguishable from exposed zero activity.

Store evaluation stays inside the existing deferred analytics transaction and reads
only generation-fenced `usage_time_rollup` rows with the existing dataset and scope
filters. Activity projects the rhythm from the accepted History envelope using two
replace-only fixed models, visibly labels range/timezone/evidence/events/exposure, and
scales hour and weekday distributions independently. The separate newest-first Recent
activity page remains readable and truthful even when History is unavailable.

Rationale: folding a 12-event page would fabricate time-distribution parity, while a
minute-series payload or new query owner would grow memory and state. Fixed rollup
distributions deliver the useful WhereMyTokens rhythm surface with constant frontend
memory and ccusage-style aggregate truth without raw events, costs, or new authority.
