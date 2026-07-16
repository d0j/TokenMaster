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
- P2-B aggregate storage foundation: strict schema v8 migrates exact v7 archives with
  rollback at every provider/aggregate boundary, makes current canonical events
  provider-self-contained, and adds generation-qualified UTC minute/hour and session
  rollups with exact unavailable/partial token algebra. All current event mutation
  paths converge on transactional invariant triggers; overflow, missing state, or a
  missing expected published rollup aborts event, dataset generation, counts, and
  rollups together. Non-empty archives rebuild through persisted keyset pages capped at
  2,048 events, bounded disk-backed cleanup/staging, reopen resume, generation-mismatch
  restart, and one checked active-generation publication. Focused store tests pass.
  Reference-machine release p95 is 1.814 ms for one event, 19.888 ms for 32, and
  230.620 ms for 256, each within its corrected absolute and relative baseline gate.
- P2-B fixed overview read: `UsageReadStore` captures publication identity, ready
  aggregate generation, and exact overview metrics in one deferred transaction. One
  request is limited to 32 unique scopes and three adjacent aligned minute/hour UTC
  segments, allowing exact DST boundary composition without raw-event reads. Missing
  token components retain known-count/sum truth, result addition is checked, stale or
  rebuilding generations fail closed, deadline cleanup is reusable, and query-plan plus
  boundary fixtures prove no raw table, `OFFSET`, gap, overlap, or double counting.
- P2-B combined analytics read: one call now captures that overview together with up
  to 400 exact adjacent series points and any unique subset of model, project, provider,
  and profile breakdowns. All payloads share one publication/generation snapshot.
  Zero-duration minute-aligned points preserve skipped civil dates; each breakdown
  retains 256 of 257 ordered groups and reports truncation. Scope filtering, exact
  known/partial algebra, real `EXPLAIN`, concurrent state change, forced cancellation
  cleanup, and the full focused store suite pass without raw-event or `OFFSET` access.
- P2-B session reads: `UsageReadStore` now returns all-time session summaries through
  indexed mixed-order keyset pages capped at 256+1 and exact detail from only session
  model/project rollups. Opaque keys/cursors bind raw session identity to the exact
  dataset without a getter or Debug leak. Current/scoped/rebuilt-legacy, equal-time
  ordering, missing detail, 257-row truncation, stale identity, real plan, concurrent
  state, and forced-cancellation cleanup contracts pass.
- P2-B calendar/facade: `tokenmaster-query` now pins Jiff 0.2.32 behind private
  boundary types and resolves explicit IANA or positively identified system zones
  without UTC fallback. UTC, Asia/Jerusalem, America/New_York, Australia/Lord_Howe,
  Asia/Kathmandu, Pacific/Apia, leap/year edges, every week start, 23/25-hour days,
  skipped dates, and a historical sub-minute failure pass. The locked bundled chain is
  `jiff-tzdb-platform` 0.1.3 plus `jiff-tzdb` 0.1.8 / IANA tzdb 2026c.
- P2-B immutable public values: `QueryService` now returns exact today/day/week/month/
  bounded-custom analytics with optional 400-point daily series, known/partial/
  unavailable tokens, activity counters, four fixed breakdowns, and owned opaque
  session page/detail values. Session continuation binds dataset plus canonical scope
  filters; rebuild/stale/failed reads consume no snapshot generation. Focused strict
  Clippy and all `tokenmaster-query` tests pass.
  A visible `aggregate_rebuilding` warning belongs to the future joined P2-F status
  snapshot; analytics itself returns stable `unavailable` because no truthful payload
  exists during rebuild.
- P2-B scale/privacy/resource closure: deterministic release-mode current and
  immutable-legacy one-million-event fixtures rebuild in 75.528/81.142 seconds at
  13,240/12,324 events/s. Rebuild-page p95 is 246.558/268.305 ms; main+WAL+SHM
  amplification is 1.483x/1.568x. Cold overview is 174.318/178.241 ms, cached p95 is
  0.543/0.365 ms, full 400-point/four-breakdown p95 is 151.043/141.192 ms, all-32-scope
  full analytics is 165.120/139.040 ms, and session first/cursor p95 remains below
  0.75 ms. Repeated snapshot/session replacement and cooperative rebuild reopen cycles
  retain private-memory/handle/thread/USER/GDI plateaus. The prior 256-event rebuild
  cap failed throughput and was replaced by the measured 2,048-event bounded cap.
  The post-synchronization baseline gate passed clean-root audit, formatting, strict
  locked workspace Clippy, every locked workspace test/doctest, and diff-check in
  79.326 seconds.
- P2-C deterministic pricing: `tokenmaster-pricing` contains the reviewed embedded
  catalog, checked integer USD-micro arithmetic with one final rounding, exact aliases,
  standard/priority/long-context rules, and immutable override snapshots capped at
  512 entries. `auto`, `calculated`, and `reported` selection distinguishes complete,
  partial, unavailable, and legitimate zero; it exposes provenance, catalog/override
  identity, assumed/unpriced/omitted/conflict counters, and bounded missing reasons.
- P2-C schema and recovery: strict schema v9 migrates exact v8 archives, adds optional
  source-reported cost plus time/session price-basis rollups, and retains bounded
  project partition in the same constant event row. Current insert/update/delete and
  recovery/current/legacy rebuild contracts preserve price facts transactionally in
  the active aggregate generation without storing calculated cost or private content.
- P2-C cost facade: overview plus up to 400 series points uses one fair 401-target,
  512-key batch; breakdown and session page/detail costs use bounded 256-target batches.
  Every public overview, point, breakdown item, and session summary now carries a
  dataset-exact `CostResult`. A 32-scope tuple-plan regression found during the red
  million gate was replaced by bounded scope CTE plus composite-index seeks.
- P2-C release evidence: current/legacy one-million gates pass at 1.862x/2.010x
  database amplification, 148.168/156.080 ms full-analytics p95, 158.588/162.504 ms
  all-32-scope analytics, session-page p95 below 14 ms, and session-detail p95 below
  1 ms. Repeated catalog/override/mode/query cycles retain private-memory, handle,
  thread, USER, and GDI plateaus. The production dependency/source/release-library
  audit finds no runtime pricing network path.
- P2-D Task 1 exact quota domain: the M0 `QuotaTarget`/`f64` placeholder is removed.
  Provider/account/workspace/window/unit/provider-epoch IDs are bounded ASCII values;
  observations use exact redacted 32-byte IDs; ratios are integer parts per million;
  absolute units remain optional; fixed-window thresholds are provider-defined; and
  normalized definitions/samples validate revision, time ordering, reset evidence,
  optional absence, capacity coherence, and nested serde fields. Eleven quota
  contracts plus the complete domain suite and strict domain Clippy pass. No detector,
  SQLite quota tables, provider transport, inventory, reminder, query, or UI
  capability is claimed.
- P2-D Task 2 pure quota evaluator: the new `tokenmaster-quota` crate has only
  `tokenmaster-domain` and `sha2` as direct production dependencies and performs no
  I/O. Domain-separated, length-framed SHA-256 scope/epoch/transition identities use
  architecture-independent integer encoding and redacted `Debug`. Constant-state
  evaluation covers start/advance, identical and conflicting duplicates, stale
  samples, window/state continuity, provider-epoch and explicit resets,
  manual/banked resets, provider-threshold scheduled/early/unknown resets, standalone
  and reset-accompanying allowance changes, comparable maximum use, repeated resets,
  exact checked sequences, overflow, rolling/drop-only rejection, and untrusted
  inference gates. A review-found restart defect is covered: the epoch-opening
  definition revision is retained separately from the latest applied revision, so an
  update neither invents a reset nor makes restored state invalid. Eleven focused
  detector/identity contracts and strict crate Clippy pass. Ratio and absolute-unit
  maxima retain independent observation IDs in current state and transitions, so
  later retention can protect exact evidence even when the maxima occur in different
  samples. No schema, persistence, retention, public query, transport,
  inventory/reminder, UI, or automation capability is claimed.
- P2-D Task 3 strict quota schema: schema v10 adds seven quota-owned `STRICT` tables,
  one independent quota revision, immutable definition/sample/history contracts,
  current epoch/window projections, fixed indexes, and exact trigger SQL. Exact
  same-window/revision composite foreign keys prevent cross-scope sample or epoch
  binding. Allowance changes require complete old/new units and a kind consistent with
  unit/capacity direction. Exact v9 migration creates only empty quota state inside one
  immediate transaction, preserves usage/dataset/aggregate/price facts, and rolls back
  to residue-free v9 at an injected post-create fault. Five focused schema contracts,
  49 store unit tests, complete locked workspace tests/doctests, strict workspace
  Clippy, formatting, and clean-root pass. Transactional quota writes, retention,
  reads, query, transport, inventory/reminders, UI, and automation remain
  unimplemented.
- P2-D Task 4 transactional quota writes: `UsageStore::apply_quota_observation` now
  evaluates and publishes one normalized definition/sample pair inside one immediate
  transaction. Duplicate/stale inputs are exact no-ops; start/advance/allowance/reset
  results insert immutable facts, update exact current epoch/window state, optionally
  close one epoch and insert one transition, and advance the independent quota
  revision once. Global observation content reuse, definition mutation/regression,
  transition/SQLite overflow, and missing or mismatched current projection fail
  closed. Five injected publication faults roll back definition, sample, epoch,
  transition, projection, and revision to the exact prior state; retry is deterministic.
  Eight focused write contracts, 51 store unit tests, the complete locked workspace
  tests/doctests, strict workspace Clippy, formatting, and clean-root pass. Two fresh
  isolated query resource processes also pass after the warm-up floor was hardened
  against one transient low allocator sample without weakening sustained-growth or
  structural gates. Reads/query, transport, inventory/reminders, UI, and automation
  remain unimplemented.
- P2-D Task 5 bounded quota retention: exported per-window defaults are 512 samples
  and 256 closed epochs/transitions; hard caps are 2,048 samples and 1,024 closed
  epochs/transitions; maintenance pages are capped at 256. Equivalent polling removes
  only the previous unprotected same-definition sample after current publication, so
  10,000 identical polls retain protected first/latest evidence. Explicit maintenance
  deletes only old unreferenced same-window samples with a newer normalized
  equivalent and never removes first/current/last, independent ratio/unit maxima, or
  transition pre/post/max evidence. Meaningful 513-sample history and 1,024 reset
  transitions remain intact above soft defaults. Applying an over-cap sample/reset
  rolls back; reopen rejects a tampered internally-count-consistent over-cap archive.
  Two injected maintenance boundaries restore both rows and global retained count,
  then deterministic retry succeeds. Seven focused retention contracts, 52 store unit
  tests, clean-root, formatting, strict locked workspace Clippy, and the complete
  locked workspace test/doctest suite pass. This retention task adds no public query,
  transport, inventory/reminder, UI, or automation capability.
- P2-D Task 6 defensive quota reads: `UsageReadStore` now captures zero through 32
  unique exact current-window keys or one exact transition history page. Current
  capture owns validated definitions, samples, current epoch plus first sample, and an
  optional exact last transition under one quota revision. Transition history is
  newest-first, revision/filter-bound, opaque-keyset paged at 256+1, and returns owned
  pre/post samples without `OFFSET`, usage tables, price tables, caller SQL, or
  caller-defined sorting. Missing windows remain absent.
  Critical review added deterministic transition restoration to the quota authority
  crate and relational read checks: current epoch/provider-reset projection and
  transition source, reset times, allowance units, detection interval, ordering, and
  reset epoch identity must match their boundary samples. Post-open drift and a
  missing referenced last transition fail `InvalidStoredValue`. Total deadline is
  enforced even across multiple short statements and the progress handler is cleared
  after every return. Six focused query contracts, 56 store unit tests, exact index
  plan checks, quota/store strict Clippy, and the complete locked workspace gate pass.
  Public quota query values/service, transport, inventory/reminders, UI, and
  automation remain unimplemented.
- P2-D Task 7 immutable public quota facade: `tokenmaster-query` now owns
  `QuotaQueryHeader`, `QuotaEnvelope<T>`, request-ordered current-window results,
  query-owned definition/sample/epoch/transition values, and an opaque
  quota-revision/filter-bound continuation. Quota freshness uses exact provider
  sample boundaries rather than the usage TTL; aggregate quality is the worst
  truthful selected state; missing windows are explicit unavailable results; and
  stale/failed calls do not consume snapshot generation. Public Debug redacts
  account/window/filter/label/provider-epoch/cursor identity. Four focused contracts,
  the complete locked query suite, and strict query Clippy pass.
- P2-D Task 8 core acceptance: the adversarial matrix proves rolling/unknown windows
  and low-quality/low-confidence fixed-window recoveries cannot infer automatic
  resets. The release gate covers 32 windows, 1,000 scheduled/early/manual repeated
  transitions, 10,000 duplicate polls, writer/reader restart, current reads,
  256-row history continuation, bounded maintenance, and current plus migrated legacy
  usage coexistence. Maximum measured calls are 3.429 ms write, 0.228 ms duplicate,
  2.774 ms current-32, and 1.256 ms history-256. Repeated quota current/history/reopen
  cycles pass the existing Windows private-memory/handle/thread/USER/GDI plateau.
  The offline authority audit covers 76 production dependency packages,
  43 production files, and three current release libraries with zero forbidden
  network/browser/shell matches.
- Codex quota wire normalization: `tokenmaster-codex` now strictly decodes the
  official account/rate-limit response into owned provider-neutral definitions and
  samples. A ChatGPT email is transiently normalized into a domain-separated
  pseudonymous account ID and cannot escape through values, errors, or `Debug`.
  Non-empty `rateLimitsByLimitId` is authoritative over the legacy duplicate;
  primary/secondary windows expand to at most 32 definitions. Ratios, seconds,
  durations, freshness, IDs, provider evidence, and reset thresholds use checked
  integer/domain contracts. Up to 64 reset-credit rows are validated transiently,
  raw IDs and untrusted text are discarded, and one separate provider-neutral benefit
  observation may leave normalization.
- Codex quota process transport: `CodexQuotaTransport` accepts only one already
  resolved absolute regular native executable and fixed `app-server --stdio`
  arguments. It performs the stable non-experimental initialize/account/rate-limit
  protocol pinned to app-server `0.144.1`, opts out of the observed quota and
  remote-control status notifications, caps a frame at 256 KiB, total stdout at
  1 MiB, frames at 64, and the complete deadline at 30 seconds. One poll owns one
  hidden Windows child and one helper thread; success, error, EOF, and timeout always
  terminate/reap/join before return. Unknown fields, IDs, methods, versions, or
  provider values fail closed under stable redacted error codes.
- Codex quota connector acceptance: a credential-free live smoke against the
  installed supported Codex returned two normalized observations and completed in
  0.70-0.94 seconds across repeated runs. The deterministic fixture matrix covers
  success, stderr, RPC failure, unsupported version, malformed/unknown/blank/
  oversized envelopes, notification, wrong/duplicate/out-of-order/negative IDs,
  early EOF, and timeout.
  The isolated Windows resource gate completed 16 warm-up and 64 measured rounds;
  every round included success, RPC failure, and forced timeout. The retained parent
  plateau was about 1.4 MiB private memory; focused/full-workspace runs observed
  topology-stable 131-135 handles, four threads, USER=1, GDI=0, and no task-owned
  child remaining. The release authority audit covers 72 production dependency
  packages, 22 production library source files, and one release library with zero
  forbidden network/browser/cookie/private-endpoint/credential-file/shell/socket
  matches.
- Codex quota runtime: `CodexQuotaRuntimeConfig` accepts authoritative explicit
  native selection or a fresh bounded `PATH` search over at most 64 KiB/128 entries
  for exact `codex.exe`/`codex` filenames only. A separate scheduler/worker performs
  app-server I/O before trying the shared writer lease, opens SQLite only under the
  non-waiting guard, publishes at most 32 independent idempotent quota transactions
  plus one optional independent benefit transaction, and exposes one count/time/code-
  only health snapshot separate from usage-engine health. Quota and benefit
  processed/status/failure/last-success facts remain distinct; a benefit failure never
  rolls back quota and a quota failure may still leave a truthful benefit success.
  Cancellation after source I/O writes nothing; partial store failure reports exact
  committed transactions. Normal/accelerated cadence is 15 minutes/60 seconds, with
  permanent incompatibility kept on the normal cadence.
- Codex quota runtime acceptance: focused discovery/execution/lifecycle/public
  contracts, concurrent usage-runtime/quota-worker fault isolation, and strict
  runtime Clippy pass. The isolated Windows gate completed
  16 warm-up and 48 measured rounds; every success round published quota plus a real
  reset-credit benefit, and every round also covered RPC failure, forced timeout,
  writer contention, and pause/resume. The latest retained private floor was
  3,432,448 bytes, sampled high 6,139,904 bytes, handles 131, threads four, USER=1,
  GDI=0, with no task-owned fixture child remaining. The release audit covers 115
  production dependency packages, the production portions of six quota-runtime
  source files, and one release library with zero forbidden network/browser/cookie/
  private-endpoint/credential-file/shell/socket/direct-SQL or foreign-runtime matches.
  The local process `PATH`
  contains npm `.ps1`/`.cmd` wrappers, but exact-native discovery selects the
  installed Windows app `codex.exe`.
- Verification correction: the first post-Task-1 workspace run reproduced an existing
  query resource-test defect. A default Rust test harness changed its own worker
  threads during process-wide `PrivateUsage` sampling, while allocator spikes later
  returned below the earlier floor. A later full gate proved that fixed warm-up could
  still end before the process topology and allocator phase converged. The resource
  gate now runs as a Cargo `harness = false` single-thread process and performs at
  most 64 warm-up rounds until two eight-round windows have identical
  handle/thread/USER/GDI topology and converged retained floors. Measurement retains
  the original 1 MiB open/drop and 2 MiB aggregate/rebuild budgets plus per-sample
  structural bounds. Deterministic fixtures reject topology-phase contamination,
  sustained growth, and incomplete windows; two fresh focused processes and the full
  workspace build pass. The clean-root audit, formatting, strict locked workspace
  Clippy, complete locked workspace tests/doctests, and diff-check pass after the
  correction.

## Next implementation slice

P2-D quota history core is complete under
`docs/superpowers/plans/2026-07-16-tokenmaster-p2-quota-core.md`: Tasks 1-8 cover
exact domain values, deterministic identities, pure reset/allowance evaluation,
strict schema v10, exact v9 migration, transactional writes, bounded retention,
defensive store reads, immutable public query values/service, adversarial and
release-scale evidence, Windows resource return, and offline authority audit.
The permitted credential-free Codex quota normalizer, short-lived official app-server
transport, exact-native executable discovery, and separate bounded quota runtime are
now implemented and verified. Benefit Tasks 1-8 are also implemented and verified:
typed reset-
credit inventory, expiration reconciliation, default/custom reminder profiles,
immutable read snapshots, and publication through the existing Codex runtime with
separate domain health, plus the store-owned due transaction and one-timer durable
in-app event runtime, authority audit, complete project-truth closure, and full
workspace quality gate. The immediate next slice is P2-E Git output. Activation
remains a later independently authorized
capability; visible notifications and the complete UI follow the completed data
contracts. No quota value may be inferred from local token/cost facts and no browser/
private-endpoint authority may be added.
P2-E Git output and P2-F joined product status remain after P2-D; P3 complete UI
follows the product-data contracts.

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
deterministic resource evidence. P2-A now completes the first immutable indexed query
snapshot. M0 interactive hibernation and uninterrupted-soak receipts remain separate
frozen-candidate acceptance work.
P2-A is now executable under
`docs/superpowers/plans/2026-07-16-tokenmaster-p2-query-foundation.md`. Its approved
design separates publication generation from dataset identity, where a current dataset
is replay revision plus schema-v7 dataset generation, and uses a dedicated
query-only SQLite store and short exact read transactions, keeps the facade synchronous,
and starts with the existing composite-index latest-activity page. P2-A Task 1 is now
implemented in `tokenmaster-query`: schema-v1 headers/envelopes, checked generations,
publication/dataset identity, an injected exact clock sample, stable path-free errors,
bounded scopes/warnings/pages, and fingerprint-redacted activity cursors. Task 2, the
separate query-only SQLite store and exact transaction capture, is also complete.
`UsageReadStore` opens schema v12 read-only without migration, enforces defensive
query-only policy with a 4 MiB cache, captures publication/scan truth and current or
legacy keyset pages in one deferred transaction, rejects stale continuation identity,
uses indexed `pageSize + 1`, and clears its deadline handler on every result.
`QueryService` now completes Tasks 3-5: successful captures receive strictly ordered
process-local generations; freshness uses the 20-minute/2-hour policy; partial,
recovery, legacy, clock-discontinuity, and obsolete accounting-version states remain
truthful; no-change publication preserves cursor identity; and one consumer slot
retains no history. Focused contracts prove a 100,000-event cold first page in 35.65 ms,
a warm cursor page in 1.10 ms, and a 256-cycle Windows open/query/drop resource plateau.
The audited cursor correction is complete: replay evidence can advance on a no-change
scan, so it is no longer dataset identity. Schema v7 adds a dedicated transactional
dataset generation with exact v6 migration/rollback, overflow, real no-change scan,
and current append proofs. P2-B Tasks 2-4 now add schema-v8 provider identity,
transactional materialization, and bounded resumable publication. Tasks 6-8 are complete:
fixed overview/series, independently capped breakdowns, opaque keyset session
page/detail reads, private calendar/timezone composition, and immutable public facade
values and million-row/storage/privacy/resource evidence are green. P2-C schema-v9
price facts, fixed-point selection, bounded overrides, public costs, and scale/
resource/offline evidence are complete. P2-D quota values, evaluator, schema-v10
storage, transactional history writes, bounded retention, defensive reads, immutable
public quota query, adversarial/scale/resource gates, and offline authority audit are
complete. The built-in Codex quota normalizer, bounded official app-server transport,
exact-native executable discovery, and dedicated refresh/store-publication worker are
complete for the pinned version. The benefit foundation Tasks 1-8 are also complete:
strict provider-neutral values, pure reconciliation/reminder planning, privacy-safe
Codex reset-credit normalization, and schema-v12 transactional inventory/history/
profile/due/outbox/ack storage with bounded maintenance, exact v11 migration, and
rollback, plus immutable current and history snapshots with FEFO order, explicit
absence/freshness/completeness/unknown
facts, inherited/override profiles, nearest expiry/due, revision-bound 256-row
continuation, corruption rejection, failed-call generation neutrality, and combined
quota-runtime publication from one poll/lease/store open with independent transaction,
failure, and last-success truth. The
64-lot/2,048-change gate measured 0.842 ms for current and 4.904 ms for the slowest
256-row page; repeated open/query/drop returned with five threads, 116 handles,
USER=2, GDI=0, and 4,517,888 private bytes. The combined runtime gate passed 16+48
rounds at 131 handles, four threads, USER=1, GDI=0, a 3,432,448-byte private floor,
and a 6,139,904-byte sampled high.

The store-owned reminder operation first replays at most 256 unacknowledged immutable
outbox rows or atomically examines at most 256 indexed due rows, records new outbox
rows before returning typed events, suppresses already-missed less-urgent thresholds,
preserves future more-urgent thresholds, drains expired rows, and returns the next
in-app deadline. Presentation leases but does not acknowledge a batch; release retries
failed presentation and explicit post-presentation acknowledgement is idempotent.
The isolated `BenefitReminderRuntime` owns one scheduler, one worker, one nearest
timer, one coalesced request, one latest count-only snapshot, and one ready/leased
bounded batch. Startup/restart, pre-ack replay, post-ack deduplication, 10,000 hints,
pause/resume/hibernation recovery, clock hints, acknowledgement contention, fault
isolation, and notification backpressure pass. Its latest 16+48 Windows gate returned
at 117 handles, four threads, USER=1, GDI=0, a 3,440,640-byte private floor, and a
5,799,936-byte sampled high. The four-package
benefit authority audit found 125 production dependencies, four reminder production
source files, four release libraries, and zero forbidden dependency/source/binary
matches. Actual P3 rendering, OS/tray delivery, snooze, quiet hours, and activation
remain later.
No view-time grouping of the full event table is allowed.
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
