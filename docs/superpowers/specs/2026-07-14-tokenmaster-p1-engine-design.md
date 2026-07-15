# TokenMaster P1 Runtime Engine and Retention Design

**Status:** Approved for autonomous execution. P0-E, P1-A, and P1-B.1/P1-B.2 are
complete; P1-B.3 bounded scan history and recovery are the next implementation slice.

## 1. Goal and boundary

P1 turns the verified Codex-to-archive composition into one provider-neutral live
runtime. It owns refresh scheduling, complete scan epochs, source-set finalization,
bounded reads, cancellation, change coalescing, writer exclusion, sleep/resume, and
restart recovery. The store remains the only transaction authority and the current
canonical projection remains readable throughout staging.

P1 does not add analytics, pricing, quota transport, Git metrics, CLI, MCP, the full
desktop UI, skins, or external Wasm providers. It establishes the immutable revision
and refresh contracts those later surfaces consume.

The work is split because retention authority must be correct before a scheduler can
exercise it continuously:

1. **P1-A — retained canonical projection:** schema v4 and atomic carry-forward.
2. **P1-B — scan sets, scoped epochs, and source finalization:** exact
   provider/profile scope manifests, complete/partial/cancelled scan lifecycle,
   bounded seen-set authority, explicit missing-source state, and scan-set-bound
   replay revisions.
3. **P1-C — provider-neutral engine core:** adapter seam, bounded state machine,
   coalescing, cancellation, deadlines, and one-shot refresh.
4. **P1-D — Codex live integration:** compiled-in adapter, watcher hints, periodic
   reconciliation, sleep/resume, and crash recovery.
5. **P1-E — revision publication:** monotonic immutable engine snapshots and race,
   stress, resource, and failure gates.

## 2. Critical retention decision

### 2.1 Problem

The schema-v3 `usage_event` projection has a deferred foreign key to the selected
`usage_observation`. Promotion deletes the old current generations. This is safe only
while every previously visible fingerprint is represented by the replacement overlay.
It cannot carry a historical event across a truncated, replaced, or later missing
source: keeping the old row would reference a deleted generation, and attaching it to
the new generation would fabricate provenance.

Reader outcomes such as truncation, replacement, or missing are evidence about a
source, not authority to erase already accounted usage.

### 2.2 Alternatives

| Alternative | Result | Decision |
| --- | --- | --- |
| Keep old generations forever | Preserves the foreign key but can retain complete obsolete files and grow without bound | Rejected |
| Copy old events into a synthetic new observation | Keeps the old schema but invents source evidence and offsets | Rejected |
| Duplicate the entire canonical page in every staging revision | Correct and restart-safe, but doubles a million-row projection during every refresh | Rejected |
| Make the canonical projection self-contained and record retention provenance | Set-based, bounded-memory, atomic, truthful historical carry-forward | Selected |

### 2.3 Schema-v4 projection

`usage_event` remains the one indexed canonical projection but no longer has a foreign
key to a deletable observation generation. It keeps the original selected source key,
generation, offset, and fingerprint as historical provenance and adds:

- `projection_revision_id`: the current replay revision that publishes the row, or
  `NULL` only for an unrebuilt legacy projection;
- `origin_revision_id`: the replay revision that last selected the event directly, or
  `NULL` for legacy evidence;
- `retained`: `0` for a direct selection in the publishing revision and `1` for an
  event carried from the prior canonical projection.

The publishing revision has a deferred foreign key to `usage_replay_revision`.
Historical origin revision IDs deliberately have no foreign key because obsolete
revision rows are removed after atomic promotion. Strict checks require:

- a legacy row to have no projection/origin revision and not be marked retained;
- a direct replay selection to have equal non-null origin and projection revisions;
- a carried replay row to have a non-null projection revision and a strictly older
  replay origin.

Migration v3-to-v4 uses SQLite's create/copy/drop/rename procedure in one immediate
transaction. Existing schema and foreign keys are validated before copying. If a
current replay revision exists, migrated rows are direct selections owned by it;
otherwise they remain legacy rows. Row count and every copied logical column are
verified before commit. v1 and v2 archives continue through their exact validation
and non-destructive legacy-snapshot paths before reaching v4.

Legacy-unverified rows are never carried into a replay-verified projection. The first
successful replay promotion replaces them only when the old prior-coverage rule is
satisfied; their immutable snapshot remains available. This prevents fingerprint-v1
or otherwise unverified history from being mixed into fingerprint-v2 totals.

### 2.4 Promotion policy

After a revision is sealed and its fixed manifest, replay overlay, selections, and
continuation state are validated, one immediate transaction applies this truth table:

| New complete overlay for a prior fingerprint | Published result |
| --- | --- |
| At least one deterministic eligible selection | Replace with that direct selection |
| Replay only, with no eligible selection | Remove the prior contribution |
| Conflict, with no eligible selection | Carry the prior event and expose conflict quality |
| Fingerprint absent | Carry the prior event |
| Pending | Promotion remains blocked |

The replay disposition is accounting evidence that a logical event duplicates an
already represented prefix; it may suppress that contribution. Source absence,
truncation, replacement, or conflict is not destructive authority.

Promotion first marks surviving prior rows as retained under the new projection,
then upserts deterministic selections as direct rows, removes replay-only prior rows,
swaps source generations, and changes revision status. Expected union counts,
provenance state, foreign keys, and fault-injection boundaries are validated before
commit. Any error rolls back the projection, generations, and revision together.

## 3. Scan sets, scoped epochs, and source finalization

The existing `usage_scan` and `usage_source.last_seen_scan_id` columns become public
store contracts in P1-B; append calls no longer manufacture scan authority. A scan is
scoped by the pair `(provider_id, profile_id)`, never by `profile_id` alone. Provider
scope is required so an external provider cannot collide with the built-in Codex
profile namespace.

A replay revision is global across the archive and may contain several scopes. It
therefore binds to one `scan_set_id`, not to one profile scan. The scan set owns a
fixed, bounded, duplicate-free scope manifest and its child scan IDs:

- `begin_scan_set(scopes, started_at)` creates one running set and one running child
  epoch for each exact provider/profile scope. IDs are typed and monotonically
  increasing. A second running set or overlapping running scope fails closed.
- `observe_scan_source(scan_id, source_key)` records a seen source only when the
  source's provider/profile pair exactly matches the running child scan. Registration,
  observation, reading, and replay staging remain separate operations.
- `finish_scan(scan_id, outcome, completed_at, counters)` closes the child exactly
  once. The store derives the distinct `sources_seen` value instead of trusting a
  caller counter. Only `complete` finalizes that scope: seen sources become present
  and registered unseen sources become missing in the same transaction.
- `finish_scan_set(scan_set_id, completed_at)` closes the set only after every child
  is terminal. It is `complete` only when every child is complete; other aggregate
  outcomes cannot authorize a replay revision.
- partial, failed, timed-out, or cancelled scans retain bounded counters but cannot
  mark unseen sources missing, delete an event, seal a missing source, or authorize
  promotion.
- a later complete scan restores a source from missing without changing its stable
  key. Missing source evidence and its one current generation remain until the source
  can be verified or an explicit future user retention operation exists.

`usage_source.last_seen_scan_id` is the bounded seen set: one pointer per registered
source, not an ever-growing `(scan, source)` history table. Closed unreferenced scan
history is pruned to a fixed recent window per scope; rows still referenced by a
source remain. Disk growth is therefore bounded by registered sources plus the fixed
operational window.

A scan-bound replay revision contains exactly the present sources finalized by its
complete scan set. Seal revalidates the same scan-set identity and source membership;
it cannot mix scopes or finalization from another epoch. A complete zero-source set is
valid and produces a retention-only revision rather than fabricating a source or
leaving the archive permanently unrefreshable.

## 4. Provider-neutral engine contract

P1-C adds `tokenmaster-engine`; it depends on provider-neutral domain/accounting/store
contracts, not on Codex paths or JSONL. The statically linked desktop assembly supplies
a Codex adapter implementation.

The adapter is synchronous streaming pull under engine-owned control:

- enumerate descriptors into a bounded callback sink and return an exact completion
  state;
- initialize or restore one opaque bounded adapter checkpoint;
- read at most one bounded draft batch and chunk-proof batch;
- verify a complete prefix;
- return stable path-free diagnostic codes and checked counters.

The engine owns cancellation tokens, deadlines, backpressure, ordering, revision/scan
CAS, and store commits. An adapter never receives a store handle, revision authority,
canonical fingerprint constructor, UI object, arbitrary network capability, or an
unbounded sender.

The initial engine is synchronous and one-shot internally. The desktop may run it on
one dedicated worker. This avoids an async runtime and makes ownership, shutdown, and
memory behavior deterministic. The fixed topology is one coordinator, at most one
active writer refresh, and bounded request/result channels. No worker or channel is
created per source.

## 5. Scheduling, coalescing, and cancellation

Refresh requests have monotonically increasing request IDs and one of four outcomes:
`completed`, `coalesced`, `busy`, or `deadline_exceeded`. While a refresh is active,
additional hints set one bounded dirty flag plus the highest requested urgency; they
do not queue paths or duplicate work. At most one follow-up refresh is scheduled after
the current operation.

Filesystem notifications are hints only. A 250 ms quiet window coalesces bursts, and
a periodic complete reconciliation remains authoritative. Overflow, watcher failure,
sleep, clock discontinuity, or resume forces complete reconciliation rather than
destructive inference.

Cancellation is checked between enumeration callbacks, reader batches, continuation
pages, and store transactions. It stops new work, discards only the exact unpublished
revision, closes the scan as cancelled, releases the writer lease, and publishes a
bounded status result. It never interrupts a SQLite commit in the middle.

## 6. Writer lease and restart recovery

The GUI, CLI refresh, and future MCP refresh share one cross-process writer lease.
P1-C first uses an OS-owned exclusive lock scoped to the archive identity, not a
timestamp-only SQLite row. Process death releases it automatically, so a suspended or
crashed writer cannot leave a false permanent owner. SQLite still supplies the final
transactional exclusion and bounded busy timeout.

On startup, the lease owner inspects the one allowed staging revision:

- if the revision and scan are exact, unsealed, and restartable, resume from stored
  checkpoints and durable continuation work;
- if evidence is stale, incomplete, version-mismatched, or belongs to a non-complete
  source epoch, discard it only through the exact revision/epoch API;
- never delete the database, current revision, legacy snapshot, or canonical page as
  recovery.

Sleep pauses new refresh work and cancels at a safe boundary. Resume invalidates
watcher assumptions, reopens source handles, and schedules one complete scan. Wall
clock changes never decide source identity or ordering.

## 7. Immutable publication contract

Every completed promotion emits one small owned `EngineSnapshot` with a monotonically
increasing in-process generation, archive revision, scan-set ID, freshness/quality state,
data-through timestamp, and bounded diagnostics counters. It contains no event
history, path, source contents, SQLite transaction, store connection, or UI handle.

Consumers replace a snapshot only if its generation is newer. A cancelled, failed,
partial, or stale async result cannot overwrite a newer snapshot. P2 later builds
indexed query snapshots from the same archive revision.

## 8. Bounds and failure invariants

- one active refresh and at most one coalesced follow-up;
- reader/accounting/store batches remain at most 256 events;
- manifest and source validation remain keyset-paged at 256 rows;
- no complete descriptor, event, chunk, diagnostic, watcher, or request history in
  memory;
- no long-lived SQLite transaction across provider I/O;
- all counters use checked `u64` values within SQLite's signed ceiling;
- cancellation, partial enumeration, timeout, adapter error, disk-full, busy, stale
  epoch, or injected promotion fault leaves prior canonical truth readable;
- no prompt, response, reasoning, command, output, source content, raw tail,
  credential, or absolute path reaches the store, status snapshot, Debug, or report.

## 9. Acceptance sequence

P1-A must first prove exact v1/v2/v3-to-v4 migration, strict current-schema rejection,
carry/replay/eligible/conflict truth-table behavior, replacement/truncation integration,
reopen, fault rollback, bounded SQL operation, and privacy gates.

P1-B then proves complete-scan-only missing authority and scan-set/revision identity. P1-C
proves the pure engine state machine with a fake adapter. P1-D uses the native Codex
adapter and real synthetic JSONL. P1-E adds race, burst, sleep/resume, restart,
cross-process lease, memory, handle, thread, and CPU evidence.

No P1 slice accepts M0, proves the complete interactive Windows product, packages an
artifact, or claims a release.
