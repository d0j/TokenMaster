# TokenMaster current state

## Product identity

TokenMaster is the sole product. It is an original Rust/Slint/SQLite implementation
in one root workspace. WhereMyTokens is the UI/product reference and ccusage is the
usage-analysis reference; both remain external, MIT-pinned provenance only.

## Implemented

- M0 native architecture proof: one process, software-rendered Slint UI, tray
  lifecycle, three layouts, three skins, English/Russian/pseudo localization,
  bounded chart/session models, and explicit resource-gate contracts.
- M1 usage foundation: bounded provider roots, path-private source discovery,
  reparse-safe streaming enumeration, typed bounded JSONL parser, cumulative token
  state, physical/logical source identity, byte framing, revalidation, strict SQLite
  usage schema, keyset reads, and atomic current-generation append.
- P0 authority and Codex-lineage boundary: provider-neutral bounded observation and
  session-relation drafts, parser resume v2 with ordinals/cumulative snapshots,
  exclusive `tokenmaster-accounting` canonicalization, fingerprint v2, replay
  signature v1/evidence, opaque canonical events, and store-only canonical input.
- P0-C pure replay classifier: root/matching/diverged/pending/conflict transitions,
  strong/weak proof rules, provider/profile/parent/ordinal validation, irreversible
  divergence, and pending (not conflict) depth/fanout exhaustion.
- P0-D/P0-D.1 replay archive: strict SQLite replay overlay, transactional exact-v1
  migration, non-destructive exact-v2 migration with fault-tested foreign-key policy
  restoration,
  immutable legacy snapshot, explicit archive modes, fixed/version-owned replay
  manifests, invisible staging generations, transactional classified replay append,
  deterministic eligible selection, epoch CAS, and fail-closed persisted-version
  validation. Late explicit relations invalidate old selections atomically and use
  restart-safe ordinal/child keyset work capped at 32 ancestry links and 256 direct
  descendants per transaction. Conflicts/cycles are permanent; bound exhaustion is
  durable pending work without epoch spin. Staging rows never affect current event
  pages or source metadata. Product begin snapshots every registered source with
  set-based SQLite operations and checked 64-bit counts without a history-sized Rust
  manifest. The explicit 256-key manifest remains bounded test/repair input and cannot
  seal a subset. Exact all-source seal validates 256-row `file_key` keyset pages,
  full-prefix checkpoints,
  chunk and overlay coverage, exhausted work, and foreign keys. Zero-pending promotion
  atomically materializes eligible rows and swaps generations with injected-failure
  rollback; incomplete replacements cannot silently omit prior visible evidence.
  Replay reclassification may intentionally change which accounted events are
  canonical. Exact-epoch
  discard removes only unpublished staging and leaves current/legacy state unchanged.
- P0-E transactional composition proof: real synthetic Codex JSONL discovery and
  streaming enumeration feed bounded reader batches through the accounting authority
  into the replay archive. The proof includes exact replay/eligible totals and quality,
  staging invisibility, append rebuild, reopen after the first of multiple batches,
  300 observations, 300 files, one-chunk-at-a-time full-prefix verification, Windows
  atomic physical replacement, cancellation, malformed JSON, incomplete tail, and
  complete-line truncation. A constrained exact-epoch `prepare_replay_source` supplies
  a valid adapter-owned empty resume and live physical identity only to untouched
  staging; two bounded reads recover its checkpoint and one chunk after restart.
  P0-E originally proved omitted prior evidence failed closed before retention
  authority existed.
- P1-A retained canonical projection: strict schema v4 migrates exact v1/v2/v3
  archives through validated create/copy/drop/rename steps and three injected
  rollback boundaries. The indexed canonical page is self-contained and records its
  publishing revision, last direct origin revision, and retained state, so obsolete
  source generations can be removed without false provenance or unbounded history
  retention. Atomic promotion installs eligible selections directly, suppresses
  replay-only prior contributions, and carries absent/conflict-only replay-verified
  events. Legacy-unverified rows remain only in their immutable snapshot. Store
  contracts prove the complete truth table, exact provenance after reopen, invalid
  owner rejection, and rollback of values/provenance/generations/revision. The real
  Codex JSONL truncation fixture now promotes while preserving the 2-event/26-token
  canonical result; cancellation, malformed data, incomplete tails, and pending work
  remain non-promotable.
- P1-B.1 scoped scan authority: strict schema v5 adds one bounded global scan set
  with one provider/profile-qualified child per scope, coherent terminal states,
  exact last-seen references, and exclusive running indexes. Exact v1-v4 migrations
  preserve populated source/scan/replay state and fault-test every v4 create/copy/drop
  boundary. Store-owned lifecycle contracts prove multi-provider scope isolation,
  idempotent observation, complete-only missing finalization, later restoration,
  reopen, late registration, and atomic rollback after parent creation or presence
  mutation. Ordinary append no longer creates or clears scan authority.
- P1-B.2 scan-bound replay: production begin stores one exact complete `scan_set_id`
  and stages only its present members with set-based SQL. Continuation, seal, and
  promotion revalidate parent/child completion plus bidirectional membership, so a
  later scan invalidates stale staging. Same-profile Codex/Hermes scopes compose
  without collision. A zero-present-source set survives reopen, creates no staging
  generation, preserves missing sources/current generations, and atomically publishes
  retained canonical truth. Faults after revision or generation creation roll back
  all staging state. The real synthetic Codex pipeline now creates/observes/finalizes
  a scan set before replay; partial enumeration closes partial and cannot authorize
  replay. All seven pipeline contracts remain green without a production dependency
  from the Codex adapter to the store.
- P1-B.3 bounded scan history: parent close prunes only whole closed scan sets when
  every child scope has 32 newer closed sets. A source `last_seen_scan_id`, replay
  `scan_set_id`, or running state protects the entire set. Each transaction removes at
  most 64 candidates through a SQLite temporary table and scan-related foreign-key
  checks, without scanning canonical usage events or collecting history in Rust.
  Contracts prove steady-state plateau, repeated bounded backlog recovery, preservation
  of running/replay-referenced sets, checked parent/child ID exhaustion, reopen, and
  rollback of both close and pruning after an injected fault.
- P1-C.1 constant-state refresh coordinator: the new provider-neutral
  `tokenmaster-engine` crate separates admission from terminal execution, uses checked
  monotonic request IDs/deadlines and cooperative atomic cancellation, and retains one
  active permit plus at most one aggregate follow-up. Ten thousand hints collapse to
  one highest-urgency follow-up without path/source/request history. Contracts cover
  immediate and active deadline expiry, cancellation precedence, busy, stale IDs,
  exactly-one follow-up, and both direct and follow-up ID exhaustion without wrap.
- P1-C.2 bounded runtime ports: sealed provider/profile/source identities, opaque
  checkpoints capped at 32 KiB, fixed diagnostic counters, 18-update chunk-proof
  batches, scope-exact 256-observation/256-relation adapter and canonical batches,
  and the original bounded replay-page seam. Object-safe synchronous `Adapter`, `Archive`,
  monotonic `Clock`, and `WriterLease` contracts keep provider I/O, archive authority,
  raw bytes, paths, Slint, OS handles, and async runtimes structurally separate.
  Compile-fail contracts prove sealed identity, path rejection, and canonical-only
  archive writes; debug/errors expose stable codes/counts only.
- P1-C.3 one-shot execution: `OneShotExecutor` acquires the writer lease before any
  provider/archive work, streams scope-exact discovery without retaining a source
  list, closes incomplete scans truthfully, and replays only an all-complete exact set.
  It canonicalizes one bounded batch at a time, validates revision/epoch continuity,
  rejects non-progress and cross-scope data, caps continuation work at 4,096, then
  seals/promotes one small result. Cancellation and deadlines are proven across every
  execution boundary; replay faults discard only the last confirmed unpublished
  handle and report cleanup failure separately. Eighteen focused contracts pass.
- P1-C.4 deterministic worker: `RefreshWorker` owns one named thread, a capacity-one
  wake token, and a capacity-one latest completion. Ten thousand hints retain one
  aggregate and execute at most one follow-up; unread completion replacement is
  non-blocking and counted. Stale cancellation, pre-execution deadline/cancellation,
  idempotent shutdown, `Drop` join, and external Clock lock order are deterministic.
  Callback panic becomes fixed `failed`/`panicked`, abandons its follow-up, redacts
  process-hook payload output only on the marked worker thread, and closes admission
  as `faulted`; an outer boundary also clears state for other worker-port panics. Ten
  focused worker contracts pass without async, provider, filesystem, platform, Slint,
  or UI dependencies; incompatible `panic=abort` builds fail at compile time.
- P1-D.0 real-source port repair: the engine now distinguishes every logical file by
  a fixed 32-byte key even when many Codex JSONL files share one provider source ID.
  Full rebuild performs two linear streaming passes and receives at most one temporary
  descriptor-bound reader at a time; path/file-handle/raw bytes never cross the port.
  Exact preparation rejects extra or duplicate sources, exact seal rejects omissions,
  and cross-scope/cross-file batches or incomplete second-pass quality fail closed with
  exact staging cleanup. The focused contract repeats a 300-file shared-root rebuild
  three times, proves 300 appends, one maximum live reader, zero retained reader after
  each run, and exact promotion. This supersedes the P1-C replay-page/cursor
  assumption; it does not implement live Codex scheduling or tail-only refresh.
- P1-D.1 atomic replay facts: `ReplayAppendBatch` now owns independently bounded
  256-event and 256-late-relation collections. Observation/overlay state, relation
  reconciliation, selection invalidation, continuation work, chunks, checkpoint,
  source completion, and evidence epoch commit in one immediate transaction and
  advance the epoch exactly once. Faults after event work and after relation work
  restore the exact pre-batch rows/checkpoint/epoch. The real synthetic Codex pipeline
  submits reader relations in that batch and no longer commits them one by one.
- P1-D.2 production bootstrap composition: the new `tokenmaster-runtime` crate bridges
  bounded Codex discovery/readers and the real SQLite archive through engine ports.
  `CodexCheckpointV1` is a strict path-free manual binary envelope capped at 32 KiB
  total; initial checkpoints use open/metadata probe without reading JSONL content.
  Runtime/store ID translation is checked and errors remain stable/path-free. Seven
  real runtime contracts prove baseline/reopen, 300 logical files sharing one source
  ID, authoritative zero-source, missing-profile partial retention, append rebuild,
  Windows atomic replacement, truncation carry-forward, pre-start cancellation, and
  exact staging discard after replay begin. This P1-D.2 slice is bootstrap/full
  rebuild only; later slices below add the incremental tail and real OS writer lease.
- P1-D.3 replay-aware incremental archive: strict schema v6 adds one checked current
  publication generation with exact complete/partial/recovery truth and exact v5
  rollback-tested migration. Current append compares revision epoch plus archive
  generation, applies replay facts and only affected fingerprints atomically, and
  disables the replay-bypassing canonical append path. Runtime performs exact scan
  freshness/admission, all-source identity/anchor preflight, then reads only persisted
  tails. Real contracts prove zero payload bytes when unchanged, exact one-line bytes,
  300-event multi-batch, several new plus empty sources, missing-source retention,
  cancellation, deadline-after-first-batch restart without duplicates, Windows
  replacement, truncation, changed-profile recovery, safe full-rebuild takeover of
  provisional sources, durable `recovery_pending`, reopen semantics, and rollback at
  four current-append boundaries.
- P1-D.4 portable process-owned writer lease: `tokenmaster-platform` canonicalizes one
  controlled local parent and uses a persistent empty sidecar plus Rust 1.97
  `File::try_lock`. One guard owns one handle; drop, normal process exit, and forced
  termination release the OS lock. Separate same-process and child-process handles,
  canonical `.` aliasing, UNC/device/mapped-remote rejection, payload rejection, empty
  persistence, redacted Debug, reacquisition, and runtime `busy` mapping are proven. A
  4,096-cycle Windows acquire/drop contract also proves that the process handle count
  does not grow. No PID, timestamp, path, owner payload, polling thread, or retained
  lock history exists. This slice does not contain watcher/scheduler or live lifecycle
  assembly; the watcher/scheduler is delivered separately below.
- P1-D.5 bounded scheduler and filesystem hints: exact `notify = 8.2.0` is isolated
  inside `tokenmaster-runtime`. Its callback drops event/error paths immediately and
  updates only one fixed atomic aggregate plus a capacity-one wake. One owned scheduler
  thread enforces immediate startup recovery, a 250 ms quiet window, 15 minute healthy
  and 60 second degraded reconciliation, monotonic rollback recovery, fixed pause/
  resume/stopping state, and stable fault handling. Root sets are capped at 64,
  canonicalized, reparse/symlink/duplicate/unsupported namespaces fail closed, missing
  roots create no backend watch, and generation replacement invalidates old callbacks.
  Five scheduler contracts prove 10,000-hint collapse and at most one real engine
  follow-up; five watcher contracts prove real create/append/rename hints, bounded
  generations, missing-root recovery, and return of Windows handles/threads to baseline
  after 32 replacements.
- P1-D.6 live runtime and restart recovery: `LiveRuntime` acquires the exact OS writer
  lease before SQLite open/migration/recovery, closes one bounded orphan running scan
  set as failed, and resumes or exact-discards only the validated staging revision.
  The adapter, archive, and lease live inside one worker execution object. A refresh
  selects incremental only for replay-verified complete/partial truth, falls back to
  full rebuild on durable rebuild-required state, and replaces watcher roots only
  after successful authoritative discovery. Pause closes admission before scheduler
  pause and exact active-request cancellation; resume resets watcher assumptions and
  forces recovery; shutdown drops the watcher, joins the scheduler, then cancels and
  joins the worker. Three live contracts cover startup, append plus new source in one
  publication, 10,000-hint burst, pause/resume, replacement, truncation, current
  partial resume to 301 exact events, reopen, path-private Debug, and combined Windows
  handle/thread return. Four recovery contracts cover lease contention before SQLite
  creation, orphan closure, zero/nonempty staging resume, and incomplete discard.
- P1-E.1 immutable engine publication: startup copies current archive truth before the
  worker is admitted, then retains one fixed publication state (at most 256 bytes in
  the supported 64-bit build). A strictly newer persisted archive generation advances
  the checked in-process generation and copies optional revision, latest complete scan
  set, its exact completion time, quality, and fixed checked diagnostics. Equal/older
  candidates and writer-busy work cannot replace the snapshot; 10,000 candidates
  retain no history; overflow fails closed without wrap. Store/runtime contracts cover
  exact scan lookup, stale ID, append generation, consumer ordering, busy recovery,
  archive identity match, and path-private Debug.
- P1-E.2 race/recovery closure: unchanged scans advance only freshness, pause/resume
  forces authoritative reconciliation, process restart resets only in-process order,
  and persisted archive generation/revision remain exact. A malformed truncation now
  fails before checkpoint/batch commit, publishes a newer `recovery_pending` snapshot
  without erasing the prior two canonical events, then returns to `complete` only after
  valid input rebuilds successfully. Existing burst, cancellation, and stale-request
  contracts complete the race matrix.
- P1-E.3 Windows power binding: `tokenmaster-platform` registers the Windows 8+
  suspend/resume callback into one static capacity-one atomic signal with no helper
  thread, hidden window, callback allocation, or runtime/archive reference. Runtime
  applies the last event through an idempotent command; every resume forces exact
  reconciliation even if suspend was missed. Unit/integration contracts cover every
  resume code, duplicate/last-wins behavior, shutdown privacy, and 4,096 registration
  cycles with bounded private bytes and no handle/thread/USER/GDI growth.

## Next implementation slice

The architecture/release closure review is approved in
`docs/superpowers/specs/2026-07-16-tokenmaster-plan-closure-design.md`. It freezes the
native stack, UI-before-automation rail, row-level parity ledger, permitted Codex
quota sources, embedded pricing update policy, canonical MSVC signed portable ZIP,
Slint attribution route, no-updater 1.0 boundary, and supply-chain evidence. These are
design/acceptance decisions; they do not claim that P2-P6 functionality is implemented.

The product architecture, universal automation connector, complete UI, dynamic quota
bars, skins, layouts, density, and localization are approved in
`docs/superpowers/specs/2026-07-14-tokenmaster-product-architecture-design.md`. Its
weekly quota contract now keeps immutable before/after epochs for scheduled, early,
and repeated full resets under the P2 plan
`docs/superpowers/plans/2026-07-15-tokenmaster-quota-reset-history.md`.
The separately approved P2 banked-reset plan models multiple quantities/expirations,
bounded reminders, notification coverage, immutable activation receipts, and a future
official-capability-only automatic policy:
`docs/superpowers/plans/2026-07-15-tokenmaster-banked-reset-inventory.md`. It is design
only; current reset discovery, notifications, assisted activation, and automatic
activation are not implemented.
The source-adapter seam keeps the current local Codex reader replaceable by future
sandboxed bounded provider plugins without coupling storage, analytics, automation,
or UI to Codex JSONL. The selected future format is a `.tmplugin` WebAssembly
Component executed in an isolated on-demand host; Codex remains compiled in and pays
no plugin runtime cost. No plugin implementation is claimed by that design.

The repeated critical audit is recorded in `docs/AUDIT_AND_MASTER_PLAN.md`. P0-A and
the P0-B Codex-lineage surface are implemented under the completed executable TDD plan
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-authority-boundary.md`.

P0-D.1 is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-scalable-replay-manifest.md`. Its
300-source contract crosses two manifest pages, promotes, reopens, and preserves
late-source fail-closed behavior. P0-E is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-e-pipeline-proof.md`; it is a
transactional cross-crate proof, not the production scheduler. P1-A is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p1-retained-projection.md`. P1-B.1 and
P1-B.2 now own strict scan-set presence and exact replay binding, including a
zero-present-source retention-only revision. P1-B.3 completes reference-safe 32/64
scan-history retention, ID exhaustion, and recovery. P1-C.1 supplies the
constant-state coordinator, P1-C.2 supplies bounded adapter/archive/clock/
writer-lease ports, P1-C.3 supplies the one-shot executor, and P1-C.4 supplies the
bounded deterministic worker. P1-D.0 corrects the real per-file/two-pass seam under
`docs/superpowers/plans/2026-07-15-tokenmaster-p1-d-live-runtime.md`. P1-D.1 makes
replay events and late relations one atomic store batch, and P1-D.2 composes the real
Codex bootstrap reader with the store archive. P1-D.3 adds the replay-aware current
archive and real tail-only refresh, P1-D.4 adds the portable process-owned writer
lease, and P1-D.5 adds bounded pathless watcher/periodic scheduling. P1-D.6 completes
lease-first startup recovery and live lifecycle assembly. P1-E.1 now exposes the
immutable bounded engine publication without sharing the writer connection with UI,
CLI, or MCP readers. P1-E.2 closes the race/recovery/restart matrix and makes degraded
live input fail closed. P1-E.3 completes the isolated Windows power binding and
deterministic resource evidence. The next implementation gate is P2 immutable indexed
query snapshots; M0 interactive hibernation and uninterrupted-soak receipts remain
separate frozen-candidate acceptance work.
Parser resume v1 still fails closed because its event ordinal cannot be inferred
safely; legacy data remains immutable and must be rebuilt, never reinterpreted.

## Release truth

M0 is not accepted. The required interactive Windows/DPI/accessibility receipt and an
uninterrupted 24-hour software-soak receipt are absent. No package, signing, or
product-release claim is authorized by the current developer evidence.

The future canonical Windows 1.0 artifact is an MSVC signed portable ZIP, but none has
been built or accepted. The current GNU lane remains developer/M0 evidence. P6
GNU/MSVC comparison, attribution/notices, SBOM, advisory/source/license/secret/action
audits, attestation, deterministic package, clean-room launch, signing, interactive
matrix, and release-candidate soak all remain unverified.

The clean-root audit, all three Pester contract files, root format check, strict
Clippy with `RUSTFLAGS=-Dwarnings`, full locked Rust workspace tests, release build,
and M0 developer stress verification pass from the root workspace. The exact commands
are recorded in `docs/HANDOFF.md` and the M0 script; this does not replace external
acceptance evidence.
