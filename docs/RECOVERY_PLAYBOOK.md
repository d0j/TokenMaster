# TokenMaster recovery playbook

1. Confirm the current branch and clean worktree with `git status --short`.
2. Read `AGENTS.md`, the contracts in `spec/`, and `docs/HANDOFF.md`.
3. Run `pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path`.
4. Run the focused test for the affected crate before changing behavior.
5. If source data or SQLite state is involved, preserve user data and reproduce with a
   synthetic fixture; never persist or attach private JSONL content.
6. Run the workspace gate before updating handoff documents.

## Replay staging recovery

1. Read `archive_state()` first. Canonical reads continue from the current compatible
   or stale replay revision, immutable legacy snapshot, or empty state; they never
   read the staging overlay.
2. For an unsealed staging revision, resume only with its exact revision ID and
   evidence epoch. Before its first append, prepare each untouched pending source with
   its validated zero-offset adapter checkpoint; this binds the live path-private
   physical identity and a valid bounded resume payload without touching current.
   After any append/reopen, recover only through `replay_generation_snapshot` and
   fetch full-prefix proofs one chunk at a time through `source_chunk`. Run bounded
   continuation until no actionable work remains, then seal only after the persisted
   complete scan-set ID and exact present membership are revalidated. Product rebuilds
   use `begin_replay_revision_for_scan_set`; `begin_replay_revision_all_sources` and
   the explicit 256-key manifest remain bounded composition/test/repair compatibility.
   If a later scan changes membership, discard the stale revision and restart from
   the newer complete set. A zero-source bound revision has no source checkpoint work
   and may seal/promote retention-only without changing missing-source generations.
   Submit canonical events and late session relations from one reader pull only as one
   bounded `ReplayAppendBatch`; never restore the old per-relation commit loop. One
   successful batch advances the evidence epoch once. Any batch error leaves events,
   relation/session state, selections, work, chunks, checkpoint, source state, and
   epoch unchanged, so retry only with the same exact prior handle and input.
3. A sealed revision with pending quality evidence is intentionally not promotable.
   Preserve it for bounded `replay_quality()` inspection or explicitly call
   `discard_replay_revision` with its exact revision ID and latest epoch.
4. Discard is the only supported retry reset. It removes the staging revision and all
   staging generations in one immediate transaction, validates foreign keys, and
   leaves current events, current source pointers, and the immutable legacy snapshot
   unchanged. A stale epoch or integrity fault writes nothing.
5. Never treat `Truncated`, `IdentityChanged`, missing, or rewrite detection as
   permission to erase prior visible usage. A complete sealed revision may promote
   through P1-A: eligible replaces, replay-only suppresses, and absent/conflict-only
   replay-verified events carry with older origin provenance. Partial, cancelled,
   pending, stale, or invalid staging must still be discarded or resumed by exact
   epoch; it cannot invoke carry-forward.
6. Never delete SQLite rows, generations, the legacy snapshot, or the archive file to
   recover a rebuild. P0-E proves exact discard through a test-only driver; no
   user-facing recovery command exists yet.

## Scan-set recovery

1. Read `running_scan_set()` after reopen. Resume only that exact set and its exact
   provider/profile child scans; never synthesize replacement IDs or treat append
   activity as observation.
2. A running child may accept idempotent observations only for registered sources in
   its own scope. Finish it with the truthful explicit outcome. Only `complete`
   finalizes presence; every other outcome preserves the prior `missing` values.
3. Close the parent only after every child is terminal. A mixed parent truthfully
   aggregates to failed, timed out, cancelled, or partial and cannot authorize the
   production replay path. Do not edit scan or source rows manually.
4. Parent close automatically performs one reference-safe pruning batch. For an older
   backlog, call the internal `prune_scan_history_batch()` repeatedly until it returns
   zero. Each call removes at most 64 whole closed sets only when every child scope has
   32 newer closed sets. Running sets and source/replay references are preserved. A
   failure writes nothing; never replace this operation with ad hoc SQL.

## Writer lease recovery

1. `busy` means another live handle owns the empty archive sidecar. Do not delete the
   sidecar, write a PID/timestamp into it, or bypass the lease with a SQLite-only
   writer. Wait for the current operation to finish and retry through normal admission.
2. Process death releases the OS lock. The zero-byte sidecar intentionally persists;
   its existence is not evidence of a live owner and it must not be removed on startup
   or unlock. Reacquire the same canonical archive identity through
   `RuntimeWriterLease`.
3. A non-empty, symlink/reparse, relative, remote, or device sidecar/location fails
   closed. Preserve only the stable error category; never log the resolved path or OS
   message. Repair the controlled data-directory configuration or remove unexpected
   payload only through explicit operator-owned inspection, not automatically.

## One-shot engine recovery

1. Treat a `busy` result as writer-lease admission backpressure only. The executor has
   not started adapter I/O or archive mutation in that case. A `busy` port code after
   lease acquisition is reported as `failed`, not as safe backpressure.
2. For `cancelled`, `deadline_exceeded`, or `failed`, inspect the bounded result's
   scan-set ID, original stable error code, and `ReplayCleanup` state. `Discarded`
   means the exact last confirmed unpublished revision/epoch was removed. `Failed`
   means cleanup itself failed and startup recovery must inspect the one staging
   revision by exact ID/epoch before retrying. Never guess a newer handle or delete
   rows manually.
3. A failure before replay may leave a closed partial/failed scan, or a running scan
   only when the archive itself rejected closure. Follow scan-set recovery above. A
   failure after replay begin never authorizes publication; prior canonical state
   remains the read surface under the archive transaction contract.
4. `InvalidData` for cross-scope discovery, cross-logical-file batch identity,
   extra/duplicate second-pass source, omitted source at exact seal, unchanged
   non-terminal checkpoint, changed revision, or regressed epoch is a boundary/
   integrity fault. Full rebuild must re-enumerate the exact completed scopes and lend
   one fresh descriptor-bound reader per source; never reconstruct descriptors from
   archive rows or cache a history-sized path list.
   Do not retry the same adapter/archive state indefinitely. Preserve bounded codes
   and synthetic reproduction evidence; never log the source, checkpoint, or path.

## Bootstrap runtime recovery

1. Treat `OneShotExecutor` through `tokenmaster-runtime` as the bootstrap/full-rebuild
   path only. A successful bootstrap must end in exact seal and promotion; use the
   separate incremental entry point for tails and do not reuse either as a watcher.
2. A Codex checkpoint decode failure is `invalid_data`. Re-enumerate the current
   configured root and create a fresh zero-offset checkpoint by open/metadata probe.
   Never edit envelope bytes, restore a path from SQLite, or log checkpoint content.
3. If bootstrap fails after replay begin, honor the returned `ReplayCleanup`. Exact
   `Discarded` leaves no staging revision/generation and preserves the prior canonical
   projection. For `Failed`, inspect the exact stored revision/epoch through replay
   recovery above; do not guess through the runtime's checked ID translation.
4. A missing/rejected configured profile is partial and cannot replace prior presence
   or canonical usage. An available, completely enumerated empty profile is the only
   zero-source authoritative case.

## Incremental runtime recovery

1. Read `archive_publication()` together with `current_replay_revision()`. Resume only
   an exact `partial` revision epoch/archive-generation pair. Never call the legacy
   canonical-only append path after replay promotion and never edit a checkpoint.
2. If partial state has replay work, run bounded current continuation. If it has no
   work, enumerate the current exact scopes again and resume each present source from
   its stored path-free checkpoint. A no-work pending probe does not advance either
   CAS token. New non-empty sources remain pending; an empty admitted source may
   already be complete.
3. Before tail writes, revalidate every present source's logical/physical identity,
   observed length/time, and bounded anchor. `RebuildRequired` must CAS the
   publication to `recovery_pending`; it is not permission to overwrite identity,
   truncate observations, remove missing history, or retry incremental append.
4. `recovery_pending` is durable full-rebuild selection. Run the exact bootstrap path
   against a new staging revision; only seal/promotion may restore `complete`. Prior
   canonical pages remain readable until that promotion. A repeated incremental call
   reports rebuild required without advancing the archive generation.
5. A cancellation/deadline after one committed tail batch may leave truthful
   `partial`. Retry from the stored checkpoint and latest paired CAS values. Current
   append is transaction-atomic across replay facts, affected projection, relations,
   work, chunks, checkpoint, source state, revision epoch, archive generation, and
   publication quality; never compensate with ad hoc SQL.

## Watcher and scheduler recovery

1. Watcher events are only lossy hints. Never recover a source, checkpoint, archive
   generation, or scan membership from an event path or kind. The callback discards
   event/error paths and retains only fixed dirty/force/urgency/health state.
2. A watcher error, rescan flag, root/settings generation change, resume, or monotonic
   rollback forces one `recovery` urgency. A missing configured root creates no broad
   ancestor watch and uses the 60 second degraded reconciliation until discovery finds
   it. A healthy watcher still receives a mandatory 15 minute reconciliation.
3. Invalid, duplicate, oversized, reparse/symlink, unsupported-namespace, or over-64
   root sets fail closed without replacing the prior generation. Backend setup failure
   degrades scheduling but does not authorize deletion or make old callback generations
   authoritative.
4. Scheduler `faulted` means its submission callback failed or fixed counters/time
   arithmetic exhausted. Stop watcher admission, join/drop the scheduler and watcher,
   inspect the worker/archive through fixed codes, and construct a fresh runtime. Do
   not replay queued event paths: none exist.

## Live runtime startup and lifecycle recovery

1. Startup acquires the archive sidecar writer lease before opening or migrating
   SQLite. `busy` therefore means no startup scan, staging repair, or asynchronous
   admission occurred; retry only after the other owner releases its OS handle.
2. Under that same guard, close at most the one bounded running scan set as failed.
   Resume staging only when status, accounting versions, scan binding, revision, and
   epoch are exact. Promote a sealed exact revision directly; discard only the exact
   invalid unpublished revision. Preserve state and fail startup when store access or
   returned identity is unavailable or cannot safely authorize deletion.
3. A replay-verified complete or partial current publication resumes incrementally.
   Empty, legacy, stale, or recovery-pending truth uses the full rebuild. A typed
   incremental `rebuild_required` switches to full rebuild under the same refresh
   permit and writer guard; it never exposes the recovery marker as fresh truth.
4. Pause closes scheduler-to-worker admission before pausing and cancelling the exact
   active permit. Resume resets watcher assumptions and forces one recovery refresh.
   Shutdown closes admission, drops the watcher, joins the scheduler, then cancels and
   joins the worker. A fault does not waive cleanup; call shutdown or drop the runtime
   and construct a new instance only after owned resources have joined.

## Worker recovery

1. `running` accepts work; `shutting_down` and `stopped` return `closed`; `faulted`
   returns `faulted`. A `coalesced` admission is one aggregate hint, not a queued job.
   Drain the capacity-one latest completion and use `superseded_results` to know that
   an older unread completion was intentionally replaced; there is no hidden history
   to recover.
2. On ordinary `failed`, the one merged follow-up may still run. On `panicked`, the
   callback result is fixed `failed`/`panicked`, its allocated follow-up is abandoned,
   admission closes, and the worker thread exits. First inspect the one-shot cleanup
   and scan/replay state above; then drop or shut down the faulted worker and create one
   new worker. Never reset SQLite rows or reuse a stale request ID as recovery.
3. `shutdown` and `Drop` are cooperative: they cancel the exact active permit, wake an
   idle worker, and join. The adapter/executor must observe its token at documented
   boundaries; never force-terminate the thread or interrupt a SQLite transaction.
4. First worker creation wraps the current process panic hook and suppresses payload
   output only on TokenMaster's thread-local marked worker. Install any application
   crash hook before creating the worker and do not replace it while workers exist.
   A non-callback worker-port panic yields `faulted` with cleared runtime coordinator
   state and no completion payload; reproduce only with synthetic data and fixed codes.

## Schema recovery

- Opening an exact schema-v1, v2, v3, v4, v5, v6, or v7 archive performs the
  non-destructive schema-v8 migration automatically. Preserve the original archive and
  reproduce any failure
  against a synthetic copy; do not edit `sqlite_schema`, rename tables manually, or
  disable foreign keys in an operator workflow.
- Migration validates the exact source schema before mutation. V2 revision migration
  restores `foreign_keys=ON` after every outcome; v3 canonical projection and v4
  scan-set, v5 publication-state, v6 dataset-generation, and v7 provider/aggregate
  migrations run in immediate transactions.
  Create/copy/drop faults roll
  back to the exact prior schema and logical rows. Ambiguous migrated scan ownership
  or incoherent terminal state fails closed. Never delete the database to bypass a
  migration error.

## Whole-file/configuration recovery status

P3-D.0 Tasks 1-11A now implement typed settings, verified Online Backup candidates,
strict `.tmconfig`/`.tmbackup`, sealed catalog/retention, capacity-one maintenance,
the durable restore journal/quarantine path, A/B run truth, read-only pre-open diagnosis,
cold journal resume, and verified-backup automatic recovery. Task 12 application
stop/restart, migration safety points, no-backup authoritative reconstruction, and safe
mode plus the later Data & Recovery UI are not implemented. Therefore Tasks 10-11A are
development infrastructure, not an operator command or proof that the current desktop
fully recovers a real installation.

The implemented bootstrap and later application integration follow this exact order:

1. Stop/join all runtime and SQLite owners, then acquire the matching archive writer
   lease before observing main/WAL/SHM, cleaning verifier/platform staging, or reading
   the recovery journal. A wrong physical guard must leave every artifact unchanged.
2. `Invalid` dual journal slots or unexpected staging/quarantine artifacts enter safe
   mode with every artifact preserved. `Absent` or `Complete` journal may remove only exact bounded
   unpublished recovery staging; it never removes quarantine evidence.
3. Resume a pending journal before any ordinary SQLite open. Reverify the selected
   package, candidate or already-promoted active main, and every persisted fixed-file
   fact. Never infer success merely from a phase label or missing staging name.
4. Automatic restore is permitted only after TokenMaster's own complete verifier proves
   definitive corruption, always data-only,
   and at most twice. Busy, access, disk-full, transient I/O, unsupported location, or
   newer schema are not corruption authority. Manual full restore must explicitly
   choose data-only or data-plus-portable-settings; device-local settings never move.
5. Before writing a recovery candidate, let `B` be the selected database length and
   `A` the observed active-main length; require `max(2B, B+A) + 8 MiB` of actual free
   staging space and recheck the active observation before publication. Do not delete
   the journal or any quarantine set after `complete`. Later retention/
   operator policy must own evidence disposal explicitly; Task 10 never auto-deletes it.
6. Before ordinary startup, inspect prior run state, publish/reread the next `unclean`
   generation, resume any pending journal, and use normal read-only inspection only for
   an exactly clean prior run. Every other prior condition adds `quick_check(100)`.
   Keep the same writer guard through live-runtime archive open and startup recovery,
   then release it; later mutations reacquire the same fixed lease per operation.
   Publish clean only after every application owner joins.
7. If no backup is usable, preserve the corrupt set and return recovery-required. Task
   12 alone may create a fresh archive through normal store code and repopulate only
   reconstructible data from authoritative local providers. Never do this in state or
   silently replace damage with an empty database.

For a current real incident, stop TokenMaster normally if possible, preserve the
complete data directory as an operator-owned copy, and reproduce only against a
synthetic/copy fixture. Until Task 12 and the Data & Recovery UI expose the typed flow,
do not copy only `tokenmaster.sqlite3`, move WAL/SHM/writer-lock files, call internal restore APIs from
an ad hoc helper, run arbitrary SQL, or treat SQLite `.recover` output as authoritative.

Generated `target/`, `reports/`, and `dist/` content is disposable developer output.
Do not use it as a release claim. M0 acceptance requires the exact external receipts
listed in `M0_ACCEPTANCE.md`.
