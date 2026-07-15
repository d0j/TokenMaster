# TokenMaster data contract

## TM-DATA-001 — Private content prohibition

The archive, settings, diagnostics, UI snapshots, CLI, MCP, backups, logs, reports,
and release artifacts MUST NOT persist or expose prompts, responses, reasoning,
commands, command output, file contents, OAuth material, API keys, or raw incomplete
JSONL tails.

## TM-DATA-002 — Canonical usage event

A canonical event contains only bounded profile/session/source identities, UTC time,
bounded model metadata, explicit token availability and values, service tier, bounded
project/activity metadata, and a full deterministic fingerprint. The shortened public
event ID is not a uniqueness key.

A provider observation draft carries a zero-based session ordinal, optional bounded
parent session identity, explicit lineage-conflict marker, delta usage, and optional
cumulative usage. The TokenMaster canonicalizer, not the provider, derives the
fixed-size replay signature, evidence level, event fingerprint, and public event ID.
Replay identity is distinct from the event fingerprint and excludes timestamp, source
identity, display metadata, and activity.

## TM-DATA-003 — Source state and checkpoints

Source keys, physical/logical identity, fingerprints, anchors, and chunk hashes are
fixed-size path-private values. Checkpoints record complete-line committed and numeric
scan offsets. A partial replacement requires exact prior length and digest proof.

Adapter checkpoints are versioned, opaque outside their provider adapter, and capped
by the common checkpoint bound. They MUST NOT contain source content, credentials, or
unbounded transport state.

Codex resume v2 carries bounded lineage state and the next zero-based usage ordinal.
Resume v1 MUST fail closed and be rebuilt through a new non-destructive generation;
the ordinal MUST NOT be guessed. Late ancestry is a separate bounded session-relation
draft so it can reconcile prior observations without retaining source content.

## TM-DATA-004 — Current and staging generations

Current-generation append writes observations, canonical selections, chunk coverage,
checkpoint, and source metadata in one transaction. A stale generation, identity,
offset, scan position, or partial proof MUST write nothing.

Staging generations MUST remain invisible to canonical reads. Compatibility and
test/repair replay begin may still snapshot all registered sources, but the production
path binds to one exact complete scan set in an immediate transaction. A scan set
contains a bounded duplicate-free manifest of
provider/profile scopes and exactly one child scan per scope. Only a complete child
may set unseen sources missing or restore observed sources present. Registration and
ordinary append never manufacture presence: after a scope has complete-scan
authority, a newly registered source starts missing until a later complete child
observes it. Partial, cancelled, failed, timed-out, pending, stale, or foreign-scope
scans preserve prior missing state. Scan-set creation and source finalization are
single immediate transactions with fault-tested rollback. Scan-bound replay stages
exactly the present sources whose last-seen child belongs to that set and stores
`scan_set_id` on the revision. Begin, continuation, seal, and promotion revalidate
completion, exact membership, staging counts, and foreign keys. A complete set with
zero present sources creates no staging generations and may publish a retention-only
revision while missing sources keep their prior current generations. The compatibility
replay path still snapshots every registered source into SQLite;
the stored checked 64-bit source count is never an application allocation authority.
Closing a scan set atomically prunes only whole, closed, unreferenced sets beyond the
newest 32 closed sets for every scope they contain. A set referenced by a source's
`last_seen_scan_id`, a replay revision's `scan_set_id`, or running state is never
eligible. One transaction removes at most 64 sets with set-based SQL and no
history-sized Rust collection. The same bounded operation may be repeated explicitly
to recover an older backlog; a failed prune rolls back the set close and all removals.
Before the first staging append, an adapter may prepare only its exact untouched
pending source with a validated zero-offset incremental checkpoint. Preparation is
revision/epoch compare-and-swap, may replace only path-private physical identity,
parser version, and bounded opaque resume state, MUST preserve the registered logical
identity, and advances the evidence epoch atomically. It cannot modify a current
generation, a touched source, observations, chunks, selections, or canonical pages.
The explicit 256-key manifest remains only a bounded test/repair input and cannot seal
a subset. Seal MUST prove the entire fixed all-registered-source manifest, exact
checkpoint/chunk coverage, replay-overlay coverage, accounting versions, exhausted
durable work, and foreign-key integrity.
Promotion MUST require zero pending observations and atomically publish the
deterministic union of new eligible selections and explicitly retained prior
replay-verified events. A replay-only fingerprint with no eligible or conflict
observation removes the prior contribution. An absent or conflict-only fingerprint
is carried with its original selection provenance and marked retained; conflict
quality remains visible. Unrebuilt legacy rows are not carried into replay-verified
truth because their old identity may double count; the immutable legacy snapshot
preserves them separately. Source generations and revision state swap in the same
transaction, and any failure leaves the previous current state intact. An explicit
epoch-checked discard may remove only unpublished staging state; it MUST NOT mutate
the current revision, legacy snapshot, or canonical event page.

The one-shot engine begins replay only after the archive closes the exact scan set as
complete. It streams discoveries directly to that set and verifies every discovered
source belongs to the currently enumerated scope. A fixed 32-byte logical-file key is
part of engine source identity; provider/profile/source ID alone is not file identity.
Full rebuild re-enumerates each scope and lends one descriptor-bound reader at a time.
The engine retains no descriptor list, validates every returned batch against the
exact logical file, and relies on archive preparation plus final seal for complete
disk-backed second-pass membership. During replay it retains only the latest exact
revision/epoch handle and accepts a returned handle only for the same revision with a
non-regressing epoch. Cancellation, deadline, incomplete second-pass quality, invalid
progress, port fault, or stale state after replay begin invokes exact discard of the
last confirmed unpublished handle; a failed discard remains an explicit recovery
state and never authorizes publication.

The cross-process writer sidecar is durable but contains exactly zero bytes. It stores
no PID, timestamp, owner, path, diagnostic, credential, or lease history. Lock ownership
exists only in one OS file handle and is released by guard drop or process death. The
sidecar is never deleted on normal unlock.

Canonical replay events and late session relations from one reader batch are one
`ReplayAppendBatch` authority unit. Both collections are independently capped at 256.
They share the same expected revision/epoch and source/checkpoint boundary; event
overlay, relation/session state, replay selection, work queue, chunks, checkpoint,
source completion state, and evidence epoch are committed atomically. The epoch
advances exactly once for the entire batch, never once per relation. A failure after
event work or after relation work leaves the exact pre-batch state.

Steady-state refresh is revision- and archive-generation-aware. An exact complete
scan may advance freshness for the same current revision and provision new path-
private sources. Non-empty new sources keep publication `partial` until their bounded
checkpoint reads finish; empty sources may be complete immediately. Existing sources
omitted by a complete scan remain historical evidence and are not deleted. Current
tail append compares revision epoch, archive generation, source generation, identity,
offsets, and proof state. Observations, replay state, affected-fingerprint projection,
relations, work, chunks, checkpoint, source state, both CAS tokens, and publication
quality commit atomically. Four injected boundaries prove exact rollback. Multiple
batches and multiple newly admitted sources remain resumable.

An unchanged refresh may probe metadata and one bounded anchor but reads zero
historical JSONL payload bytes and commits no tail batch. Replacement, rewrite,
truncation, identity mismatch, or a changed provider/profile scope never erases current
truth: it advances the archive generation into durable `recovery_pending`, after which
only a non-destructive full rebuild may return the publication to `complete`. That
rebuild may replace only an exact unadmitted generation-zero provisional source with
no replay source, observation, or chunk state. If one scan discovers more than the
fixed provisional-admission bound, it requests rebuild before retaining another key.
Malformed, incomplete, or oversized relevant input cannot advance an adapter checkpoint
or authorize a complete rebuild; the failed attempt retains prior canonical truth and
the durable recovery marker until valid authoritative input succeeds.

The worker retains one coordinator, one optional not-yet-started permit, one wake
token, one completion, one owned thread handle, and fixed phase/supersession counters.
Ten thousand active-time hints still retain only one merged follow-up. Completion
replacement never stores a result history, and panic/fault state stores no payload,
provider/source identity, path, checkpoint, observation, or adapter error text.
Worker state is runtime-only and is not archive, settings, diagnostic, or recovery
authority.

The scheduler retains one atomic flag word, latest monotonic hint tick, watcher-health
byte, lifecycle byte, two checked scalar counters, one capacity-one wake, and one owned
thread. The flag word contains only dirty, force, highest-urgency, overflow, and clock-
discontinuity bits. Ten thousand hints allocate no event/path queue and yield at most
one worker follow-up. A watcher generation retains at most 64 canonical configured
roots inside the pinned backend; callback events and errors are dropped immediately.
Missing roots retain no backend watch, and old generation callbacks are non-authority.
No scheduler/watcher state is persisted or treated as scan, replay, checkpoint, or
publication evidence.

The live composition retains one adapter discovery snapshot, one SQLite writer
connection, one reusable lease object, one worker, one scheduler, one current watcher
generation, one bounded prior-root vector, one admission flag, and fixed lifecycle/
latest-result snapshots. It retains no refresh history, event path, raw line, provider
payload, staging page, or unbounded retry queue. Startup recovery reads at most the
fixed scan-scope page and performs at most the fixed replay-continuation limit.

P1-E adds exactly one fixed engine-publication state. Its immutable public value copies
only an in-process generation, persisted archive generation, optional replay revision,
optional latest complete scan set, that exact set's completion time, publication
quality, and fixed diagnostic counters. The publication state is at most 256 bytes in
the supported 64-bit build and retains no prior snapshot. Ten thousand equal candidates
change only checked scalar counters. Archive generation must be strictly newer before
replacement; equal or older candidates cannot change the snapshot generation. Counter
or generation overflow sets a fail-closed flag and never wraps. Failed, busy,
cancelled, or deadline work cannot manufacture a newer archive snapshot. In-process
generation may restart with the process; persisted archive generation is the durable
ordering authority.

The Windows power adapter adds one process-wide static signal containing one pending
event byte, three checked counters, and one overflow flag. It retains no event queue,
callback context allocation, thread, window, OS handle in engine state, timestamp, or
history. Repeated equal notifications change only a checked coalesced counter; a later
different notification atomically replaces the pending event.

Truncation, physical replacement, or source absence is not destructive authority. A
complete sealed overlay may promote while carrying omitted prior replay-verified
events. Incomplete, partial, cancelled, pending, mismatched, or invalid evidence still
cannot promote. Carry-forward records accounting history; it does not claim that the
original source still exists.

## TM-DATA-005 — SQLite policy

The usage archive has a strict versioned schema. Schema v3 removed the historical
256-source revision constraint. Schema v4 makes the canonical projection
self-contained and adds publishing revision, origin revision, and retained state, so
obsolete generations can be removed without fabricating provenance. Schema v5 adds
provider-qualified scan sets, coherent child terminal state, exact last-seen
references, running-scope exclusivity, and optional scan-set provenance for migrated
replay revisions. Schema v6 adds a singleton archive generation, current revision,
latest complete scan set, and explicit `empty|complete|partial|recovery_pending`
publication state. Exact v1-v5 archives migrate non-destructively through validated
create/copy/drop/rename steps; populated v4 scan ownership is derived only from its
exact referenced sources, otherwise marked `legacy-unverified`. Ambiguous or
incoherent state fails closed. V2 foreign keys are disabled only outside the
revision-table migration transaction, checked before commit, and restored on every
tested exit. V3-to-v4, v4-to-v5, and v5-to-v6 migrations use immediate transactions
with exact logical-copy and injected rollback checks.
File-backed connections MUST use WAL,
FULL synchronous writes, foreign keys, a bounded busy timeout, bounded journal/cache
policy, and disabled mmap. Collections and complete-manifest validation are
keyset-paged at no more than 256 rows. Scan-history cleanup uses only scan-related
foreign-key checks rather than rescanning the complete usage-event archive.

The query path uses a distinct `READ_ONLY|NO_MUTEX` connection and never calls the
writable open/migration path. It requires exact schema v6 and bundled SQLite identity,
WAL, foreign keys, query-only and defensive modes, trusted-schema/DQS disabled,
query-planner stability, no checkpoint on close, 250 ms busy timeout, 4 MiB cache,
file-backed temporary storage, and zero mmap. Each result captures archive generation,
dataset identity, scan completion/manifest, and at most 256 events plus one lookahead in
one deferred transaction, then returns only owned data. Continuation without the exact
dataset identity is invalid.

The capture also reads the current replay revision's stored canonicalizer, fingerprint,
and replay-signature versions in the same snapshot. A revision with obsolete accounting
versions remains readable for bounded diagnosis but the query facade MUST mark it
`unknown` with `accounting_version_stale`; it MUST NOT describe that data as
authoritative. Query consumers retain at most one immutable result. The P2-A
100,000-event contract covers a 256-row first/cursor page; million-row dashboards are
served only by the future transactional materialized aggregates in P2-B.

## TM-DATA-006 — Bounds

Reader lines are limited to 16 MiB. Resume metadata is capped at 32 KiB. General
display metadata is UTF-8 bounded; tool names, collection counts, profile roots,
source directories, and UI snapshots have explicit contract limits.

The encoded Codex adapter checkpoint, including its fixed header and parser resume,
is capped at 32 KiB total. It stores no path or raw source bytes. Bootstrap retains at
most the provider discovery bounds, the engine scope manifest, one descriptor-bound
reader, one reader/canonical batch, and exact store handles. Full rebuild uses two
linear enumerations; source count does not size a JSONL descriptor collection. The
store's zero-based scan/revision/epoch values and engine nonzero values are related
only by checked one-to-one runtime translation and are never persisted in the other
representation.

## TM-DATA-007 — Replay classification

Every current observation has one replay disposition: `eligible`, `replay`, `pending`,
or `conflict`. Canonical selection uses only `eligible`. All observations remain
available for bounded reconciliation and quality counts. Session ancestry traversal
is capped at 32 levels and one transaction re-evaluates at most 256 direct children.

## TM-DATA-008 — Quota epochs and reset transitions

A provider quota sample is immutable and keyed by provider, account/workspace scope,
stable window ID, observation ID, and observation time. It carries provider-defined
window/reset semantics, optional used/remaining ratios, optional capacity/units,
advertised reset time, freshness, quality, evidence source, and confidence. Missing
values are never inferred from local usage.

A full-reset transition references the last trustworthy pre-reset sample and first
trustworthy post-reset sample and records the closed epoch's maximum observed use,
old/new advertised reset times, scheduled/early/manual-or-banked/unknown kind, and an
observation interval if the exact instant is unknown. Capacity changes are orthogonal
transitions and may accompany a reset. Transition identity is deterministic so
restart/retry cannot duplicate it. Poll samples, epochs, transitions, and aggregates
have explicit per-window retention/page bounds.

## TM-DATA-009 — Banked reset inventory and activation receipts

A provider benefit lot is immutable evidence scoped by provider, account/workspace,
benefit kind, target window, identity, observation revision, quantity, typed expiry,
state, source, freshness, and confidence. Banked rate-limit resets, credits, temporary
usage, and unknown benefits are distinct kinds. Different expirations remain separate;
date-only and timezone-unknown expirations are never silently promoted to exact UTC.

Current inventory is a bounded projection over immutable change points. A reminder
profile has one revision and at most eight unique normalized lead times; inherited and
explicitly overridden profiles remain distinguishable. Reminder delivery is
deduplicated by lot revision, threshold, and channel. An activation writes
a deterministic intent before external mutation and a normalized receipt afterward.
Unresolved or ambiguous intents survive retention. A confirmed receipt may reference
one `manual_or_banked_reset` quota transition, but neither inventory nor local usage
may invent provider capacity. Current lots, changes, reminders, intents, receipts,
pages, and maintenance work have explicit per-scope bounds.

Only explicit provider ancestry identifies a parent. A strong signature covers the
normalized model, emitted delta, and provider cumulative snapshot. A weak signature
covers model and delta only and cannot suppress a pre-divergence event by itself.
Once a child diverges from a fixed parent relation, later events remain eligible.

The pure classifier validates matching provider/profile scope, declared parent
session, and equal child/parent ordinal before comparing signatures. Depth or direct
fanout exhaustion is `pending` and requires continuation; it is not evidence of a
cycle or contradictory relation.

If a child's ordinal is beyond the observed tail of a parent that has not been proved
complete, the child remains `pending`. Only a complete fixed manifest and exact
full-prefix source proof may make that missing-parent work actionable and prove that
the child outgrew its parent before final seal.
