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

Canonical replay events and late session relations from one reader batch are one
`ReplayAppendBatch` authority unit. Both collections are independently capped at 256.
They share the same expected revision/epoch and source/checkpoint boundary; event
overlay, relation/session state, replay selection, work queue, chunks, checkpoint,
source completion state, and evidence epoch are committed atomically. The epoch
advances exactly once for the entire batch, never once per relation. A failure after
event work or after relation work leaves the exact pre-batch state.

The worker retains one coordinator, one optional not-yet-started permit, one wake
token, one completion, one owned thread handle, and fixed phase/supersession counters.
Ten thousand active-time hints still retain only one merged follow-up. Completion
replacement never stores a result history, and panic/fault state stores no payload,
provider/source identity, path, checkpoint, observation, or adapter error text.
Worker state is runtime-only and is not archive, settings, diagnostic, or recovery
authority.

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
replay revisions. Exact v1-v4 archives migrate non-destructively through validated
create/copy/drop/rename steps; populated v4 scan ownership is derived only from its
exact referenced sources, otherwise marked `legacy-unverified`. Ambiguous or
incoherent state fails closed. V2 foreign keys are disabled only outside the
revision-table migration transaction, checked before commit, and restored on every
tested exit. V3-to-v4 and v4-to-v5 migrations use immediate transactions with exact
logical-copy and injected rollback checks.
File-backed connections MUST use WAL,
FULL synchronous writes, foreign keys, a bounded busy timeout, bounded journal/cache
policy, and disabled mmap. Collections and complete-manifest validation are
keyset-paged at no more than 256 rows. Scan-history cleanup uses only scan-related
foreign-key checks rather than rescanning the complete usage-event archive.

## TM-DATA-006 — Bounds

Reader lines are limited to 16 MiB. Resume metadata is capped at 32 KiB. General
display metadata is UTF-8 bounded; tool names, collection counts, profile roots,
source directories, and UI snapshots have explicit contract limits.

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
