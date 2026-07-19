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

## TM-DATA-010 — Repository activity and Git output projection

A repository activity hint is a transient provider-neutral value containing exact
provider, profile, source, session, event time, optional safe project alias, and one
sealed canonical local-directory candidate. The candidate MUST reject relative,
traversal, network, device, mapped-remote, symlink, and reparse-point ancestry before
use. Only one latest hint may exist per source batch. The hint and candidate MUST NOT
implement serialization and MUST use fully redacted `Debug`.

Repository paths are excluded from parser resume, adapter checkpoints, observations,
canonical batches, SQLite, query values, diagnostics, logs, and errors. A consumer
MUST take the side-channel hint immediately after its source batch; a later read may
replace it. Explicit invalid `cwd` clears prior transient association rather than
reusing an older repository.

The Git runtime may retain at most 32 latest canonical candidates only in process
memory. Each candidate has one checked sequence and at most one raw-head frontier.
Pause and power recovery MUST invalidate every frontier and sequence before a result
can publish; shutdown MUST clear candidates and frontiers. Candidates and raw Git
object IDs MUST NOT cross restart, serialization, SQLite, health, query, or Debug
boundaries. Count-only health contains only stable codes, outcomes, counts, elapsed
time, and bounded scheduler/worker topology.

The durable Git projection uses installation-salted opaque repository and activity
association identities. It retains bounded scan/publication state and aggregate
facts only: exact day/category line metrics, commit and merge counts, explicit
quality, warnings, unavailable reasons, freshness, and omission counters. It MUST
NOT retain a repository path, executable path, author email, ref, commit identity or
message, file path/content, raw command output, or provider transcript.

Schema v13 owns one random 32-byte installation salt, one independent monotonic Git
publication state, at most 32 repositories, at most 4,096 activity associations, and
one immutable active aggregate generation per repository. A generation contains at
most the latest 400 daily rows, exactly eight category rows per retained day, exactly
eight all-time category rows, and at most 16 ordered warnings. All-time totals remain
independent of the daily retention window. If any older daily fact is dropped,
`daily_history_truncated` is mandatory, quality is partial, and queries expose the
oldest retained day plus whether the requested range is complete.

Only salted opaque repository, association, project, ref-set, mailmap, and author-set
fingerprints are durable. A project key is present in a read capture only when every
association for that repository has the same non-null key. Missing or conflicting
keys produce `association_incomplete`; replacing an association with no safe project
key clears the earlier key. Unavailable repositories have no cache identity,
aggregate, category, daily, or warning rows and therefore represent absence rather
than fabricated zero activity.

The project key is a domain-separated SHA-256 fingerprint of the exact safe
`ProjectAlias` from the transient provider hint plus the installation salt. Public
query code never receives that salt. It supplies at most 256 already-bounded usage
project candidates to one fixed store matcher and receives only candidate indices.
This is an exact association to the usage contract, not a repository-basename guess.

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
writable open/migration path. It requires exact schema v13 and bundled SQLite identity,
WAL, foreign keys, query-only and defensive modes, trusted-schema/DQS disabled,
query-planner stability, no checkpoint on close, 250 ms busy timeout, 4 MiB cache,
file-backed temporary storage, and zero mmap. Each result captures archive generation,
dataset identity, scan completion/manifest, and at most 256 events plus one lookahead in
one deferred transaction, then returns only owned data. Continuation without the exact
dataset identity is invalid.

Current dataset identity is the checked pair
`(replay_revision_id, dataset_generation)`, not revision ID or replay evidence epoch
alone. Schema v7 advances dataset generation inside the same transaction after every
canonical event insert, delete, or update. Freshness-only scan publication changes
neither member even when replay/CAS evidence advances. This makes old keyset cursors
fail closed after every row-set mutation without resetting them for a no-change scan.

The capture also reads the current replay revision's stored canonicalizer, fingerprint,
and replay-signature versions in the same snapshot. A revision with obsolete accounting
versions remains readable for bounded diagnosis but the query facade MUST mark it
`unknown` with `accounting_version_stale`; it MUST NOT describe that data as
authoritative. Query consumers retain at most one immutable result. The P2-A
100,000-event contract covers a 256-row first/cursor page; million-row dashboards are
served only by transactional materialized aggregates in P2-B.

Schema v8 makes current canonical events provider-self-contained and adds one exact
aggregate state plus generation-qualified UTC minute/hour and session rollups. Current
events maintain those rows transactionally when state is `ready`; insert, delete,
update, dataset generation, event count, and all aggregate contributions commit or
roll back together. Missing token components retain known-count/known-sum algebra and
never become zero. Current and immutable legacy datasets remain distinct.

Schema v9 adds optional source-reported USD microdollars and generation-qualified
`usage_price_time_rollup` / `usage_price_session_rollup` facts. Each canonical event
contributes at most one minute, one hour, and one session price row keyed by exact
provider/profile, model, bounded project partition, normalized service tier,
long-context state, and reported-cost state. Rows retain event/calculable/reported
counts plus checked uncached-input, cached-input, billable-output, and reported-cost
sums; they never retain source IDs, paths, prompts, responses, commands, reasoning
text, or a calculated monetary estimate. Current mutations update price facts in the
same transaction as dataset generation and token rollups. Recovery and immutable-
legacy rebuilds populate the same inactive aggregate generation and publish it only
after the existing exact generation/count checks.

Schema v10 adds a quota-owned singleton revision, immutable definition revisions and
samples, current/closed epoch projections, reset/allowance transitions, and one exact
current window projection without changing usage or price row shape. Every quota table
is `STRICT`; opaque IDs are exact 32-byte blobs; text, enums, ratios, times, counts,
units, and allowance-change direction are checked. Composite foreign keys prevent a
current projection or retained evidence row from binding a sample, epoch, or
definition from another scope/window or revision. Published definitions, samples,
closed epochs, and transitions reject `UPDATE`; later store-owned bounded retention
may delete only whole unreferenced retained rows.

The exact v9-to-v10 path first validates the v9 archive, then creates and seeds only
the empty quota schema inside one immediate transaction, sets `user_version=10`,
validates the complete v10 contract, and commits. Any quota-schema fault rolls back to
exact v9 with no quota objects. The migration does not rewrite or reclassify usage,
aggregate, or price rows.

Non-empty migration publishes no partial aggregate. A rebuild is bound to one expected
dataset generation, uses a persisted fingerprint keyset cursor, and processes at most
2,048 events per call into disk-backed unpublished rows. Cleanup is also paged at no
more than nine rollup rows per requested event, or 18,432 rows at the hard cap. Reopen
resumes exact state; a canonical
mutation invalidates only staging and requires restart. Publication is one active-
generation update after exact processed/total and dataset-generation checks. Aggregate
reads require `ready`; no other state may fall back to a whole-history query.

An exact overview bucket is described to the store by at most three ordered adjacent
UTC segments over minute/hour rollups. Starts and ends are aligned to the selected
width and ranges are half-open. This is sufficient for the private calendar layer to
compose a boundary-minute prefix, full-hour middle, and boundary-minute suffix while
keeping timezone rules and Jiff types outside the archive contract.

An analytics series is an ordered exact partition of its overview range and contains
at most 400 owned points. A zero-duration minute-aligned point carries zero metrics and
represents a civil bucket skipped by timezone history. Breakdowns group only stored
rollup rows: model/project use their dimension rows; provider/profile use `all` rows.
Each requested kind is unique, capped independently at 256 retained items with explicit
truncation, and cannot multiply into a caller-defined cube.

Session summaries are all-time facts over one provider/profile/session key; period
filters apply to time-rollup analytics, not to whole-session metrics. The canonical
order is last UTC instant descending, then provider, profile, and private session
identity ascending. A continuation retains that exact key plus dataset identity and
the public facade binds it to the canonical applied scope-filter set; it returns at
most 256 rows with one internal lookahead. Raw session identity has no public getter
and is redacted from Debug. Exact detail returns the same summary plus
independently capped model and project dimension rows from `usage_session_rollup`;
project absence is typed. A valid key missing from the exact unchanged dataset returns
no detail rather than fabricated metrics.

Public calendar values contain only validated Gregorian dates, a canonical IANA zone
identity, configurable week start, exact UTC boundaries, and owned metrics. Jiff and
timezone-rule objects remain private. A public token aggregate is exactly
`unavailable`, `known(sum)`, or `partial(known_sum, known_count, event_count)`; no
missing component is converted to zero. Daily series are optional and capped at 400.

Public cost is selected from immutable price-basis captures and an immutable pricing
engine. Money is unsigned integer USD microdollars; rates are integer microdollars per
million tokens; all accumulation is checked and rounded once. `auto`, `calculated`,
and `reported` modes return `complete`, `partial`, `unavailable`, or legitimate
`zero`, source composition, catalog/override identity, counters, conflicts, and a
bounded missing-reason set. Unknown models, tiers, contexts, token relationships, and
key truncation never become zero. One overview plus up to 400 series targets uses one
batch capped at 401 targets and 512 returned price keys. Breakdown and session batches
retain at most 256 targets and the same global 512-key detail cap with exact per-target
omitted counts. No result issues one SQL query per visible point or session.

The joined product status capture is one exact scalar read model over schema v13. One
short deferred transaction binds usage publication/dataset/aggregate state, independent
quota and benefit revisions, and independent Git publication state. It returns only
owned counts, checked revisions, quality/freshness inputs, aggregate progress, and
stable availability facts; it never returns source, account, window, lot, repository,
project, or archive identities. Missing independent domains remain explicitly absent
or unavailable and do not erase readable sibling truth.

The product projection retains one immutable current snapshot. Its data sections are
keyed by checked refresh-attempt generations that are distinct from source snapshot
generations; its runtime sections use a separate checked runtime generation. A failed
compatible refresh keeps the last successful payload plus a bounded stable failure,
while incompatible durable identity invalidates that payload. Exactly 11 fixed routes
derive readiness from a fixed-width reason set; no dynamic route, reason, history,
queue, runtime owner, path, identifier, or database value is retained by the reducer.

## TM-DATA-006 — Bounds

Reader lines are limited to 16 MiB. Resume metadata is capped at 32 KiB. General
display metadata is UTF-8 bounded; tool names, collection counts, profile roots,
source directories, and UI snapshots have explicit contract limits.
The product status warning set is capped at 16, route reasons are represented by one
`u16`, and each route exposes at most eight currently defined stable reasons. Snapshot
replacement retains one `Arc`-owned current value and no prior snapshot history.

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

The implemented domain representation uses bounded ASCII identifiers of at most 128
bytes for account, workspace, window, unit, and provider epoch; provider identity
retains its existing 64-byte contract. Observation identity is exactly 32 bytes with
redacted `Debug`. Ratios are exact integer parts per million in
`0..=1_000_000`; no floating-point quota value remains. A sample enforces
`0 < observed_at <= fresh_until <= stale_after`, contains at least one quota/reset
fact, preserves absent values as `None`, and accepts an exact reset occurrence only
with explicit evidence inside `1..=observed_at`. Absolute used/remaining values cannot
exceed a present capacity. Reset thresholds require a post-reset boundary and are
valid only for fixed windows. Deserialization repeats these validations and rejects
unknown nested scope/window fields.

A full-reset transition references the last trustworthy pre-reset sample and first
trustworthy post-reset sample and records the closed epoch's maximum observed use,
old/new advertised reset times, scheduled/early/manual-or-banked/unknown kind, and an
observation interval if the exact instant is unknown. Capacity changes are orthogonal
transitions and may accompany a reset. Transition identity is deterministic so
restart/retry cannot duplicate it. Poll samples, epochs, transitions, and aggregates
have explicit per-window retention/page bounds.

The pure quota evaluator is implemented without I/O or mutable global state. It
rejects mismatched windows, conflicting duplicate identities, incoherent previous
state, definition-revision regression, non-exact transition sequences, and sequence
overflow. Ratio/amount drops alone and rolling-window recovery do not create resets.
Provider epoch, explicit reset, manual/banked, and provider-threshold evidence follow
the documented precedence and preserve scheduled, early, manual/banked, or unknown
classification plus exact-or-interval detection time. Open epoch identity retains its
opening definition revision while the state separately tracks the latest applied
definition revision, so definition updates survive restart without false resets.
Maximum used ratio and maximum comparable absolute units each retain the observation
identity that established that maximum; the identities may differ and are copied into
reset transitions for exact retention/provenance.

The implemented quota write path loads only one scope/window's latest definition,
current epoch, exact last sample, and transition sequence inside one immediate
transaction, then delegates classification to the pure evaluator. Identical duplicate
and stale observations commit no row or revision change. Every visible start, advance,
allowance change, or reset inserts one immutable normalized sample, updates the exact
current epoch/window projection, optionally closes one epoch and inserts one immutable
transition, and advances the independent quota revision exactly once. Reset plus
allowance change remains one reset transition with complete allowance facts.

Observation identity is global and content-stable; reusing it with different normalized
content fails before publication. A stored definition revision cannot change content
and a lower definition revision fails stale. Current epoch, current window, and last
sample must agree exactly on revision, identities, times, evidence metadata, and
transition sequence on writable use and reopen. Missing or mismatched projection state,
sequence/revision/count overflow, or an injected fault after sample, epoch, transition,
current projection, or revision fails closed and rolls back to the exact prior state.

Quota retention is implemented with exported per-window soft defaults of 512 samples
and 256 closed epochs/transitions, hard caps of 2,048 samples and 1,024 closed
epochs/transitions, and maintenance pages capped at 256 candidates. A consecutive
equivalent poll may remove only its previous unprotected same-definition sample after
the current pointer moves. Paged maintenance selects only older unprotected samples
that have a newer normalized equivalent in the same window and definition revision.
First, last/current, ratio maximum, unit maximum, closed-epoch, and transition
pre/post/max evidence remain protected. Meaningful samples and all transitions may
remain above their soft defaults; Task 5 never merges transitions or closed epochs.
Crossing a hard cap rolls back the applying observation, and reopen rejects stored
per-window counts above a hard cap even when singleton counts were altered to match.
Maintenance changes only retained detail/counts, not semantic quota revision, and its
delete/state fault boundaries restore the exact prior archive.

Defensive quota reads are implemented on the separate query-only `UsageReadStore`.
One deferred transaction first captures the independent quota revision, then loads at
most 32 exact current windows or one newest-first transition page with at most 256+1
rows. Missing current windows remain absent. Transition continuation is keyset-based
on sequence and opaque identity, bound to the exact window and captured revision, and
never uses `OFFSET`.

Every returned value is reconstructed through the domain/quota constructors rather
than copied into an unchecked read DTO. Current definition/sample/epoch/current-row
projections must agree on exact key, revision, identities, times, provider epoch,
advertised reset, evidence metadata, and transition sequence. Transition IDs are
recomputed from their normalized identity fields; kind/epoch shape, allowance
direction, detection interval, pre/post ordering, old/new advertised resets, source,
allowance boundary units, and reset-current epoch identity must agree with the joined
boundary samples. Any malformed or post-open altered projection fails with
`InvalidStoredValue`; no plausible partial snapshot is returned.

The public quota facade is implemented separately from usage dataset identity.
`QuotaQueryHeader` carries a checked process-local snapshot generation, exact quota
revision, generated/data-through time, aggregate freshness/quality, exact bounded
window filters, and stable warnings. Current requests preserve caller order and
return one explicit result per requested window; missing windows are unavailable,
never zero. Transition pages expose query-owned immutable values and an opaque cursor
bound to the exact filter and quota revision. Public `Debug` redacts account,
workspace, window, provider-epoch, and cursor identity.

The release-scale contract covers 32 windows, 1,000 immutable transitions, 10,000
duplicate polls, scheduled/early/manual repeated resets, writer and reader restart,
256-row continuation, bounded maintenance, and both current and migrated immutable
legacy usage archives. Measured maximum calls are 3.429 ms for a visible write,
0.228 ms for a duplicate poll, 2.774 ms for a 32-window current snapshot, and
1.256 ms for a 256-row history page on the reference machine.

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

Schema v11 implements the read-only inventory/reminder foundation with one independent
benefit publication revision, strict scope/current/material-revision/change/profile/
threshold/due/delivery objects, exact v10 migration, immutable change and delivery
facts, 64 current lots, 8 thresholds, 512-change/256-delivery soft retention,
2,048-change/1,024-delivery hard limits, and 256-row maintenance. The newest change
per lot is protected as the terminal/reappearance revision cursor. Activation
intents/receipts remain unimplemented and confer no current mutation authority.

The defensive benefit read projection is also implemented over the same schema-v12
archive. Current and history captures read the global benefit revision and all
scope-owned rows in one deferred transaction. Current rows are capped at 64 and
ordered by conservative expiry, unknown expiry, explicit kind rank, and opaque lot
ID. History uses descending `(sequence, change_id)` keyset paging with 256+1
lookahead; continuations bind the exact scope hash and benefit revision. Current
redundant columns must exactly match their immutable material revision, scope row
counts must match captured rows, and missing/invalid material fails as
`InvalidStoredValue`. Profile inheritance and nearest due facts come from indexed
benefit-owned tables. No benefit query references usage events, usage rollups,
provider payloads, source paths, or prompts.

The built-in Codex refresh runtime now publishes the normalized benefit observation
without merging it into quota state. One provider poll precedes one non-waiting writer
lease attempt and one writable store open. The same process guard covers deterministic
quota publication and the optional benefit publication, while every quota window and
the benefit inventory keep independent transactions and revisions. Runtime health
records separate bounded quota and benefit observation/processed/status/failure
counts, benefit material-change and pending-due counts, and separate last-success
times. A failed domain never rewrites the other domain's result or implies
cross-domain atomicity.

The durable in-app reminder queue is now executable under schema v12. One store-owned
immediate transaction first replays at most 256 immutable delivery/outbox rows that
lack an acknowledgement. When none exist, it examines at most 256 indexed due rows,
removes expired entries, selects the most urgent useful overdue threshold for each lot
revision/channel, records its immutable delivery/outbox row, removes only the examined
due rows, updates aggregate queue/receipt counts, and returns the next due time.
Runtime never receives SQL or public scope/lot/delivery identities. A recorded
threshold suppresses equal and less-urgent missed thresholds across profile rebuild
and restart while preserving a future more-urgent threshold.

Schema v12 adds a separate immutable `benefit_reminder_ack` relation. Presentation
leases an in-memory copy without changing durable truth. Only an explicit
post-presentation acknowledgement inserts the corresponding row idempotently.
Unacknowledged outbox rows survive restart and profile/inventory rebuild and are
ineligible for retention. Exact v11 migration marks pre-v12 delivery receipts as
acknowledged because the old runtime already treated them as consumed.

The reminder runtime retains one scheduler, one worker, one nearest deadline, one
capacity-one coalesced request, one latest count-only snapshot, and at most one owned
delivery batch of 256 items. No per-scope or per-lot timer, thread, channel, callback,
or retained source/provider payload exists. Notification backpressure stops later
queue mutation until the batch is acknowledged. A failed presentation can release the
lease; process failure before acknowledgement replays it. This foundation publishes
typed in-app events but does not claim that the unfinished P3 UI rendered them; OS
delivery, snooze, quiet hours, activation intents, and activation receipts remain
absent.

## TM-DATA-011 — Reliable-state records and packages

Settings, run state, and recovery intent use two alternating bounded records. Each
record has exact magic and version, a checked monotonic generation, exact payload
length, SHA-256 payload digest, and strict bounded JSON. Readers select the highest
valid generation. One invalid slot falls back; two invalid recovery slots plus staged
artifacts fail to safe mode rather than inferring ownership.

The implemented Task 3 record envelope is `TMREC001`, little-endian version 1, a
64-byte header, strict JSON payload of at most 1 MiB, and a 40-byte `TMEND001` footer
whose SHA-256 binds header, payload, and footer marker. Header flags are zero and every
stored generation is nonzero. Decode checks the actual bounded file size before the
declared payload length, rejects trailing bytes and unknown versions, verifies both
digests, and only then deserializes the typed value. Equal generations are accepted
only when their payload digests agree; a disagreement is integrity failure rather than
absence/default authority. Save measures and hashes without retaining encoded JSON,
streams a second deterministic pass into the inactive slot, seals before publication,
and rereads both slots. Any uncertainty after publication is recovery-required. The
generic record/file authority remains crate-private; the typed settings store and
later typed run/recovery stores are the only intended public surfaces.

Implemented settings schema version 1 contains exactly `portable` and `device`
classes. Portable state contains one canonical in-app reminder profile (enabled plus
one through eight unique lead seconds from 60 through 31,536,000) and automatic-
backup policy (periodic enabled, quiet seconds 300..3,600, interval seconds
21,600..604,800 with quiet strictly below interval, and retention budget 256 MiB..
64 GiB). Defaults are the recommended 7d/24h/12h/6h/1h reminder leads, five-minute
quiet, six-hour interval, and 2 GiB budget. Device state contains only one of the 11
implemented route keys. The schema stores no future presentation/provider field and
no forbidden private state. Portable candidates have their own strict versioned JSON
envelope, SHA-256 digest, bounded category/count preview, and never carry device
state. A committed target is the exact nonzero settings generation plus portable
digest and can be independently reread-verified.

The implemented Task 5 SQLite candidate is a temporary controlled file, not yet a
`.tmbackup`. Online Backup copies at most 64 pages per step from the fixed live
archive, retries at most eight consecutive busy/locked steps, and retains no page or
row history in Rust. Verification uses fixed streaming buffers and applies 16 MiB
SQLite value, 256 KiB SQL, and 256-column limits before reading untrusted schema.
Schema enumeration retains at most the expected table count, names are capped at
128 bytes, and schema SQL at 256 KiB. The accepted value carries only schema version,
length, defensive-policy facts, physical-file identity, and SHA-256; it exposes no
path or data content. Temporary snapshot/compact names are capped at 32 each. Cleanup
health is one saturating counter, and recovery scans exactly those 64 names plus their
fixed SQLite sidecars.

Implemented Task 6 `.tmconfig` and `.tmbackup` use one fixed typed little-endian
container, not a general archive. The exact v1 order is a 32-byte `TMPKG001` header,
one 40-byte `TMMNF001` manifest, then one settings entry and, for `.tmbackup` only,
one database entry. Each entry is a 64-byte `TMENTR01` descriptor, exactly one Zstd
frame, and a 24-byte `TMENEND1` suffix. The footer is the SHA-256 binding of the
manifest plus every entry descriptor/suffix, the exact `TMEND001` marker, and the
stored SHA-256 of every preceding byte. The complete file also receives an independent
SHA-256 before its controlled stage is sealable.

Header and manifest kind/count must agree: config is exactly one settings entry;
backup is exactly settings then database. The manifest carries settings schema 1,
database schema (zero only for config), compression profile, creation time in
0..253402300799999 UTC milliseconds, and one of periodic/manual/pre-migration/
post-migration/pre-restore/pre-destructive-maintenance for backup. The sixth value is
an additive v1 enum value; the five existing wire values remain unchanged. Reserved
bytes/flags are zero. The implemented
limits are eight entries and 64 KiB manifest at the version boundary, 1 MiB expanded
settings, a separate 2 MiB encoded `.tmconfig` ceiling checked before parsing, one
64 GiB database, a 64 GiB plus 2 MiB checked backup total/encoded ceiling, and 64 KiB
codec buffers.

Every entry descriptor binds kind, Zstd codec 1, profile level 6/12/19, checksum plus
content-size flags, expanded length/SHA-256, and window log 23. Dictionary IDs,
reserved bits, concatenated frames, missing frame ends, trailing bytes, a frame
content-size mismatch, windows above 8 MiB, expanded output above the independent
counter, suffix-length mismatch, unknown values, overflow, and any digest mismatch
fail closed. Codec input is only `DurableFileReader`; output is either
`DurableStagedFile` or the sealed exact-slot `BackupStagedFile` used by the typed
backup writer. No public generic extractor exists. A codec or final-seal failure
irreversibly discards and poisons the output stage, so later write, seal, or
publication cannot recover partial bytes as truth.

Application config preview retains exactly one decoded `PortableSettingsCandidate`,
its base settings generation/record digest, at most three ordered change categories,
one changed-field count, creation time, and package byte count. It retains no source
path, filename, reader, raw package bytes, digest, history, or device-local candidate.
Commit consumes the preview, rejects base identity drift, preserves the current device
settings, and publishes through the existing redundant settings record. Export receipts
contain only creation time and package byte count; package and durable receipts remain
redacted capabilities.

Task 14 native selection retains no history. One input capability owns only its open
bounded reader. One output capability owns one private target descriptor plus either
`Absent` or one opaque physical identity; it may additionally own one adjacent bounded
create-new stage while an export is in progress. During existing-target publication,
one adjacent displaced file is retained only until the new file is identity/byte-
verified; ambiguity retains it as recovery evidence instead of deleting it. On Windows,
a stage owns one delete-capable cleanup handle, not a path-only cleanup promise. Unix
controlled-selection cleanup retains an open identity and revalidates the namespace but
does not claim containment of a hostile same-user unlink race. Selected path/name and
COM result
strings are transient platform-only values and never enter config/package bytes,
settings preview, archive, logs, diagnostics, errors, `Debug`, Product, Desktop, or
future CLI/MCP. A deterministic controlled selector retains one redacted target or one
cancelled/stable-failure value, never a queue.

The wire format contains no filenames, paths, links, permissions, devices,
credentials, prompts, responses, reasoning, commands, output, source content, or raw
provider data. Implemented optional manual protection wraps only the exact
length/SHA-256 identity of an opaque `VerifiedBackupPackage` in a binary standard age
v1 passphrase envelope. Its recipient stanza uses scrypt `log_n = 16`; import caps the
accepted value at 16 before derivation. Passphrase bytes never enter package metadata,
receipts, stable errors, `Debug`, process arguments, environment, settings, or health.
Automatic recovery packages remain unencrypted and store no decryption secret.

Implemented Task 8 automatic retention owns exactly 32 private package slots,
`point-00.tmbackup` through `point-31.tmbackup`, below the fixed `backups` child. Slot
names never encode time, purpose, profile, identity, or user data. A platform entry
binds directory scope, ordinal, observed length, and physical identity; the complete
scan generation hashes only those bounded physical facts. Unexpected names/types,
links/reparse points, hard links, duplicate physical identities, and controlled
stage/deletion remnants fail closed.

`BackupCatalog` is process-local and disposable. Rebuild streams every complete file
with one 64 KiB buffer, retains only fixed header/manifest metadata plus complete-file
SHA-256, and rejects duplicate file content. Cold rows are `header_valid` or
`corrupt`, never `verified`. Prior `verified` state carries forward only when slot
physical identity, length, complete-file SHA-256, and typed metadata are unchanged;
an explicit current `VerifiedBackupPackage` proof must match all of them before bind.
Public catalog values expose only checked catalog generation, bounded ordinal, UTC
time, compressed size, purpose, schema/compression, and health.

Retention keeps at most 15 verified restore points under a default 2 GiB compressed-
byte budget configurable only from 256 MiB through 64 GiB. Protection is selected
first: the admitted candidate, newest two verified points, and newest pre-migration
point until a later verified post-migration point exists. The remaining deterministic
UTC tiers are four newest, at most seven distinct calendar-day representatives, and at
most four distinct ISO-week representatives, all under the shared 15-point cap.
Unchecked/corrupt bytes count against the budget but are never deletion-eligible.

Admission is a pure no-delete check over one fully verified unpublished candidate and
requires a free slot. The candidate stage is fully parsed through a sealed path-free
reader before admission. Only after exact publication, catalog-generation increment,
candidate bind, and preservation of every prior package may a retention cycle select
one oldest verified unprotected point. Immediately before deletion it streams and
rechecks the complete current verified set, rechecks the exact deletion target and
directory generation, then uses a write-through same-volume tombstone and removes at
most that one file. The caller must rebuild and replan before another deletion.

Implemented Task 9 maintenance state is constant-size. It contains one active permit,
one merged pending request, a checked request counter, one previous source-failure
identity/count pair, one latest general completion, one latest mandatory-guard
completion, and fixed success/failure/byte counters. A retry has a new attempt ID but
retains the original root request and backup purpose; only its scheduling urgency
becomes `source_retry`. Thus a pre-migration retry can never be mislabeled periodic or
authorize the guarded mutation with the wrong restore-point purpose.

The automatic schedule stores only enabled/healthy/dirty/paused/catch-up flags and
five scalar ticks. `Healthy` startup seeds the already-proved publication flag and its
first interval anchor at the current monotonic tick; `HealthyUnpublished`, empty,
suspect, and quarantined states do not. It otherwise emits no automatic request before
the first healthy publication, enforces both quiet time and the ordinary minimum
interval, consumes one catch-up after a missed resume interval or clock rollback, and
drops a merged periodic-origin follow-up when periodic scheduling is disabled. Source
retry exists only as internal urgency and always retains the root purpose. One worker
thread and one scheduler thread communicate through capacity-one wake channels. A
permit owns a typed `BackupControl` linked to the same cancellation state; cancellation
becomes immutable when the permit enters final publication, and execution-state
validation rejects `Published` before or `Cancelled` after that boundary. Runtime health contains no path, SQL, source
content, prompt, response, command, or history collection.

The store-owned `VerifiedBackupCandidateReader` exposes a bounded path-free chunk
capability over one exact verified SQLite candidate. Opening rechecks physical
identity, length, and full SHA-256. Complete package consumption recounts and rehashes
the open handle, rejects early EOF or appended bytes, and rechecks the namespace
identity after EOF. Replacement, truncation, append, cancellation, codec failure, or
destination failure discards and poisons the unpublished package stage.

Quarantine retains at most three complete main/WAL/SHM sets and never deletes them
automatically.

Recovery staging is a distinct fixed namespace. It recognizes only opaque
`restore-<operation>.sqlite3` candidates and their platform-generated durable-stage
children plus a zero-byte create-new reservation, retains at most three artifacts
globally, and never derives a name from package or UI data. When and only when the
redundant journal is absent or complete, startup/resume may
discard these unpublished artifacts. Unknown names/types, links/reparse points, and
multiple links remain preserved and block cleanup.
Platform and store independently enforce the shared three-artifact ceiling before
creating a recovery child. With selected database length `B` and observed active-main
length `A`, admission requires actual free space of `max(2B, B+A) + 8 MiB`; the first
store verifier is released before active-corruption verification, and the active facts
must still match the preflight observation before journal publication.

Recovery journals the exact states `prepared`, `sidecars_quarantined`,
`main_replaced`, `reopened_verified`, `settings_published`, and `complete`. It stores
only a checked operation generation, fixed backup slot plus bounded opaque package/operation/candidate
identity, exact prior main/WAL/SHM presence/length/SHA-256 facts, optional portable-
settings target generation/digest, data-only/data-plus-portable-settings/automatic-
data-only mode, attempt, and state. A data-only restore journals an explicit settings
no-op. Automatic mode is corruption-only and limited to two attempts. It never stores
or accepts an arbitrary path. Every transition is idempotent, including completed
sidecar/main/settings mutations before journal advance; uncertainty preserves all
artifacts and enters safe mode.

Implemented Task 11A run state uses only `run-a.tms` and `run-b.tms`. Task 12A advances
the strict current schema to version 2 while accepting schema-v1 records as an exact
legacy subset with no migration obligation. A launch first inspects the prior highest
valid record as `clean`, `unclean`,
`missing`, or `invalid`, then durably publishes and rereads a new `unclean` generation
before catalog, package, or SQLite access. Only an exactly clean prior generation may
use normal startup inspection; every other condition adds bounded `quick_check(100)`.
The retained `RunSession` binds its clean publication to the exact unclean generation
and digest, so a changed record cannot be accepted. Clean publication is separately
authorized and occurs only after application-owned work has joined.

One run record may retain only the current recovery operation generation, exact
candidate identity, a saturating launch count, and at most one path-free pending
migration source/target schema pair. The pair is published only after the verified pre
point and before writable migration, is preserved by every new unclean generation, and
is cleared only after the verified post point. A clean record cannot retain it. The same recovered candidate may be
launched twice after unclean exits; the third attempt enters safe mode. A later clean
run accepts that operation generation, while a historical completed journal cannot
start a false retry loop or block a later independent recovery generation. No run
record stores a path, timestamp history, process identity, error text, or usage data.

Implemented Task 12A composes one application-owned backup operation over the existing
fixed records and slots. A published point is admitted only after online snapshot,
strict candidate verification, typed package write, complete package verification,
sealed publication, exact verified-package catalog binding, and bounded retention.
Retention deletes at most one admitted victim per iteration and never exposes a slot
or path outside the sealed owners. The operation retains one catalog projection. Its
first worker execution completely verifies every header-valid package in the bounded
directory; each rebuild carries verification only for unchanged length/digest/metadata
identity. Package corruption becomes explicit catalog corruption, while transient or
ambiguous directory failure aborts the pass. This prevents proof loss from turning
retention into unbounded file accumulation after restart.

For an exact supported legacy archive, `PreMigration` and `PostMigration` are distinct
mandatory package purposes and receipts. The pre point records the old schema and stays
pinned until the post point is verified. Periodic policy does not alter this rule. A
failure before writable migration retains the old archive; a later failure retains the
migrated archive with the durable pending-post pair. Both publish no live bundle, and
restart completes the post point before clearing the pair.

Implemented Task 12B.1 command state is process-local and constant: one optional active
permit, one optional pending command, one optional last-retryable command, one checked
next request ID, and closed/paused flags. A restore selection is only a nonzero catalog
generation and bounded ordinal; it contains no path, slot, filename, digest, or package
metadata. Active permits retain one atomic running/cancelled/irreversible byte. Bundle
state retains one checked generation and one optional current bundle. These values are
not persisted and never become recovery authority.

Implemented Task 12B.2a seals a generation/ordinal selection into one opaque identity
containing only the private physical slot, package length, and full package digest. It
can be resolved after catalog generation or ordinal changes only when those exact bytes
remain fully verified; deletion, slot reuse, byte replacement, ambiguity, or non-
verified health fails closed. One RAII pin serializes current-directory binding with
each actual retention deletion. A cycle admitted before the pin must replan with that
identity protected in addition to the candidate and ordinary protected tiers. The pin
remains through the statically protected `PreRestore` publication, then clears before
journaled replacement. Catalog projections remain bounded and immutable behind short-
lived `Arc` replacement; no operation retains catalog history.

After journal completion, the recovery operation generation and candidate identity are
written into the existing run session before any restored-archive lifecycle work. A
clean joined shutdown accepts that exact generation. A supported legacy candidate is
not treated as current: it receives a new verified old-schema `PreMigration` point,
durable source/target pending pair, guarded migration, and verified current-schema
`PostMigration` point before the fresh bundle is exposed.

Implemented Task 12B.2b/Task 15 adds one latest-only bounded reliable-state projection
and one latest-only operation completion. Public state contains fixed health, backup
policy, scalar counts/times, at most fifteen generation/ordinal restore choices, one
optional config preview, one optional operation, and one optional path-free recovery
receipt. It contains no path, slot, digest, archive identity, source identity, raw
settings, password, or history-sized collection. `AtomicPromotion` always clears the
cancellable flag and is published at the exact irreversible boundary rather than at
operation admission. Counts and published bytes are typed optional values, so an
unavailable projection cannot fabricate zero history. Restore preview retains one
exact reviewed generation/ordinal value; confirmation consumes that value instead of
resolving the current row again. Running is published when each permit actually begins
execution, including a promoted follow-up. Manual backup stays cancellable until its
maintenance permit crosses the irreversible boundary and then publishes
`AtomicPromotion`.

The redundant recovery journal keeps its existing backup identity for verified restore
and stores no backup identity for authoritative-source reconstruction. A missing backup
identity is valid only for the explicit reconstruction mode; older valid restore
records remain compatible and every other missing/ambiguous identity fails closed. The
reconstruction candidate is a fresh normal-schema archive created through the ordinary
store constructor, fully verified, staged, reverified, and promoted only after active
corruption is proved. Main, WAL, and SHM evidence remains quarantined. A mandatory
bounded recovery-urgency source refresh must complete before healthy publication and
backup scheduling. The durable public receipt marks reconstruction and non-
reconstructible quota, reset-credit, reminder, and Git history loss; these domains are
unavailable, never fabricated zero. A complete no-backup journal plus an active
recovery launch becomes a preflight source-reconciliation obligation. It survives a
restart, keeps the effective outcome recovery-required, and is cleared only after the
bounded refresh completes; retry reuses the promoted archive and never reruns corrupt-
archive replacement.

Task 17 closes the bounded data-plane evidence without adding a production schema or
public payload. The release-only fixtures normalize the schema-13 installation salt to
a fixed test value and bind exact byte length plus SHA-256 for 8 MiB and 96 MiB
freelist databases. Package I/O remains 64 KiB, the Zstd decoder window remains at most
8 MiB, and sampled private growth remains within 64 MiB with more than 16 MiB headroom
to the large database. The lifecycle fixture fills the exact daily/weekly retention
tiers before its baseline, then requires all 256 measured publications to return to
the same 15-point byte total and verification staging to zero. Sixteen cancellations
occur only after a recovery source reader and candidate exist; sixteen independent
data-only restores use the real journal/coordinator/promotion contour. The evidence
retains only scalar counts, durations, hashes, resource counters, limits, and gate
results and never a fixture path or database content.

### P3-C bounded Dashboard projection

The read-only quota overview discovers at most 32 current window keys in one deferred
transaction and restores every window under the same quota revision. The benefit
overview captures at most 32 scopes and 256 current lots in one deferred transaction.
Each plus-one case fails closed; neither overview treats an empty exact filter as an
all-current request.

`DesktopDashboardProjection` owns only copied presentation facts from one immutable
`ProductSnapshot`. It retains exactly six section states and caps quota rows at 32,
benefit summaries at 32, trend points at 240, session summaries at 12, activity rows
at eight fixed categories, model rows at 12, and checked Git aggregation at 32
repositories. Account, workspace, quota-window, benefit-lot, repository, project,
session, event, and source identities do not cross this boundary. A missing scalar is
typed unavailable or partial before formatting and is not stored as a display zero.

### P3-D.1 bounded History projection

`UsageRange::recent_days` accepts only 1 through 400 days. Resolution samples the
query clock once, uses the requested explicit/system IANA timezone, and produces the
exact half-open interval `[today - (days - 1), tomorrow)`. The daily series remains
an ordered exact partition, including civil days with no events, and therefore cannot
silently collapse gaps or substitute UTC boundaries.

The product snapshot owns a History analytics section independent from the today-only
Dashboard analytics section. Compatible failure may retain only its own last-good
payload as degraded; dataset-identity change invalidates it. The desktop projection
copies at most 30 daily rows, reverses them newest-first for display, retains one
overview and exact range/timezone/evidence facts, and owns no query service, cursor,
archive handle, prior range, or row identity. Missing token components remain typed
unavailable/partial and cost preserves complete/partial/unavailable/legitimate-zero.

### P3-D.2a bounded Sessions projection

`DesktopQueryPlan` requests one all-time newest-first session page capped at 64 rows.
The product snapshot owns that page independently from the 12-row Dashboard summary.
Compatible failure may retain only its own last-good page as degraded; dataset-identity
change invalidates it. `has_more` remains explicit, including when exactly 64 rows are
published, so the frontend cannot mistake the bounded page for archive completeness.

`DesktopSessionsProjection` copies at most 64 rows and only aggregate presentation
facts: first/last UTC instant, event count, optional input/cached/output/reasoning/total
tokens, cost, freshness, quality, stable reasons, and continuation availability. It
owns no provider/profile/source/workspace/project/session identity, opaque key, cursor,
query service, archive handle, prior page, or detail cache. The raw dataset-bound key
remains inside query/controller state for the generation-bound exact-detail path and
never crosses into product correlation, Desktop projection, or Slint.

### P3-D.2b exact Sessions detail projection

Every live backend bundle owns one nonzero checked `DesktopSnapshotEpoch`. Inside one
epoch, product generations remain strictly newer-only; a higher epoch accepts a restarted
generation, rejects later output from an older backend, and clears the active selection.
One accepted click allocates a nonzero `ProductSessionDetailSelectionGeneration` and
correlates only that generation plus the zero-based visible ordinal. Product state owns
one optional correlation and one replace-only detail section; it never owns an opaque
`UsageSessionKey`, click history, result history, or cross-selection retained payload.

`DesktopSessionDetailProjection` has exactly `idle`, `loading`, `ready`, `missing`, and
`unavailable` states. Ready state copies the exact summary, envelope freshness/quality,
and at most 32 model plus 32 approved path-free project-alias aggregate rows. Each row
contains only display kind/label, event count, typed token components, total, and cost.
Query or projection omission sets explicit truncation. Provider/profile/source/session
keys, cursors, raw paths, prompts, responses, reasoning content, commands, and credentials
never enter the projection or Slint model.

### P3-D.3 bounded Models projection

The existing recent-30-day History request also requests Model and Project breakdowns.
It remains one query and one compatible product section: History consumes the daily
series, Models consumes the Model breakdown, and the future Projects route consumes the
prefetched Project breakdown. The query/store boundary retains at most 256 Model and
256 Project items plus explicit lookahead-derived truncation in the one current
immutable snapshot. No prior recent-usage envelope is cached.

`DesktopModelsProjection` accepts only `UsageBreakdownIdentity::Model`, preserves the
query's total-token/event/stable-key order, and copies at most 64 rows. Each row contains
only the canonical bounded model key, event count, typed input/cached/output/reasoning/
total values, and typed cost availability, selection mode, and calculated/reported/
mixed composition. Slint receives availability plus a visible/accessibility-safe
composition label; it never converts partial cost into complete cost. Backend
truncation or discarding rows beyond 64 produces
one explicit `models_truncated` reason. A missing Model breakdown is degraded truth,
not an empty exact range. The projection also copies only the shared overview,
half-open range, timezone, freshness, and quality; it owns no provider/profile/source/
account/workspace/project/session identity, key, cursor, path, filter, sort state,
query service, archive handle, runtime owner, or prior model.

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
