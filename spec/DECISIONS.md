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
