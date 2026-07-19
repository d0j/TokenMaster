# TokenMaster current state

## Product identity

TokenMaster is the sole product. It is an original Rust/Slint/SQLite implementation
in one root workspace. WhereMyTokens is the UI/product reference and ccusage is the
usage-analysis reference; both remain external, MIT-pinned provenance only.

## Implemented

- Global reminder settings synchronization/editor: portable settings are desired-state
  authority; generation `N` maps to global profile revision `N + 1` for first-install,
  current, migrated, reconstructed, restored, explicit Save, and confirmed config
  import paths. Pending precedes durable mutation and remains retryable when archive
  synchronization is Busy/unavailable. Startup reports optional reminder runtime
  StoreUnavailable separately without erasing the exact enabled/leads Pending
  projection or changing settings bytes. The fixed responsive editor supports enable/
  disable, five recommended leads, and up to eight normalized custom leads while global
  edits preserve scope overrides, deliveries, acknowledgements, and provider evidence.
  Per-scope editing, snooze, quiet hours, reminder OS/tray delivery, usage alerts, activation,
  P4/P5/P6, M0 acceptance, package/signing/soak, and release remain incomplete.

- P3-E.1 bounded route command palette: the production `MainWindow` owns one final
  full-window overlay over the existing immutable 11-route projection. Ctrl+K and the
  visible header action open it; focused text entry, Escape, wrapped Up/Down, Enter,
  pointer activation, and accessibility default action reuse the existing route
  selection callbacks. The query is truncated to 64 Unicode scalar values, the one
  replace-only result model never exceeds 11 rows, and an accepted snapshot refreshes
  an open palette without retaining prior models or holding the Desktop mutex through
  Slint setters. Runtime UI tests include 10,000-scalar input, pointer/accessibility,
  and open-snapshot refresh; the release desktop audit and 134 mutation cases pass.
  Clean-root, formatting, strict workspace Clippy, and the complete locked workspace
  test/doctest gate pass; the latter completed in 879.2 seconds. That P3-E.1 receipt
  did not claim native lifecycle; P3-E.3/P3-E.4 status is recorded below. Current-user
  startup, P4/P5/P6, M0, packaging, signing, soak, and release remain incomplete.

- P3-E.2 bounded compact quota mode: the existing `compact_widget` route now switches
  the sole always-mounted `MainWindow` to a 420 by 560 logical-pixel quota surface and
  restores one previously captured valid normal physical size on return. It reuses the
  exact current Dashboard quota model, renders all provider-defined windows up to the
  existing 32-row cap, keeps unavailable and unknown-ratio truth explicit, and adds one
  keyboard/pointer/accessibility Dashboard return. It adds no query, snapshot, window,
  worker, timer, queue, cache, controller, provider assumption, or native authority.
  Compiled coverage proves 32 rows, explicit unknown ratio, same-component identity,
  exact restoration, and 10,000 mode cycles; release audit, 141 mutations, strict
  Desktop Clippy/tests, and independent 0/0/0 review pass. Clean-root, formatting,
  strict workspace Clippy, and the complete locked workspace test/doctest gate also
  pass; the latter completed in 753.4 seconds. That P3-E.2 receipt did not claim native
  lifecycle; P3-E.3/P3-E.4 status is recorded below. Current-user startup, interactive
  Windows/DPI/screen-reader acceptance, P4/P5/P6, M0, packaging, signing, soak, and
  release remain incomplete.

- P3-E.3 production tray lifecycle: the release composition constructs exactly one
  isolated Windows tray owner after showing the sole `MainWindow`. Icon click and the
  fixed menu emit only Show, Hide, OpenCompact, OpenDashboard, and Quit through one
  single-install queue-free router. Explorer recovery uses a hidden top-level tool
  window and checks re-registration: failure marks the tray unavailable, shows the
  main window, and changes close from hide to explicit quit. Show/route actions
  unminimize, show, raise, and request foreground focus for the same window; Quit still
  returns through joined shutdown before the sole clean mark. Slint `system-tray` is
  absent from production Desktop and remains only in the separate M0 probe. Desktop/
  app release audits, 226 combined mutation cases, strict package Clippy, and full
  Desktop/app tests pass. P3-E.4 current-session status is recorded below. Current-user
  startup, actual Explorer/focus behavior, interactive Windows/DPI/screen-reader/
  resource acceptance, P4/P5/P6, M0, packaging, signing, soak, and release remain
  incomplete.

- P3-E.4 current-session single-instance activation/global hotkey: `run()` claims the
  fixed non-inheritable auto-reset event
  `Local\TokenMaster.CurrentSession.Activation.v1` before renderer, environment,
  data-root, SQLite, or runtime construction. An existing-event process performs only
  `SetEvent`, closes its capability, and exits; failures expose the stable path-free
  `current_session_unavailable` code and never fall through to a second runtime. The
  primary starts one joined `tokenmaster-session-integration` owner after the weak-
  window sink exists, registers fixed `Ctrl+Alt+T` with `MOD_NOREPEAT`, and normalizes
  hotkey/secondary input to the existing Show/restore/focus path. The app bridge keeps
  one pending bit and one scheduled Slint task; 10,000 requests coalesce, scheduling
  failure retains one startup-flushable bit, and delivery/native sink panic is
  contained. Shutdown closes admission and joins/unregisters before the clean mark.
  Focused platform/app tests, 84 application-audit mutations, strict focused Clippy,
  4,096 test-owner cycles with handle growth bounded to eight and thread/USER/GDI
  growth to one under concurrent harness noise, and independent Critical/Important/
  Minor 0/0/0 review pass. Live two-process arbitration, occupied hotkey,
  foreground policy, cross-token ACL, sleep/resume, and real RegisterHotKey resource
  return remain interactive evidence. After the validator correction, the exact clean-
  root/fmt/strict-workspace-Clippy/full locked test-doctest chain passed in 1,001.5
  seconds total; application and Desktop release audits passed in 187.3 and 150.6
  seconds. Current-user startup is the next P3-E slice;
  P4/P5/P6, M0, packaging, signing, soak, and release remain incomplete.

- M0 native architecture proof: one process, software-rendered Slint UI, tray
  lifecycle, three layouts, three skins, English/Russian/pseudo localization,
  bounded chart/session models, and explicit resource-gate contracts.
- P3-A production desktop foundation: a separate `tokenmaster-desktop` package maps
  one current `ProductSnapshot` into exactly 11 fixed route rows, rejects equal/older
  generations, preserves validated selection, and renders one original software-only
  Slint header/navigation/state shell with no probe dependency or mock usage data.
- P3-B.1 bounded desktop controller: one reused refresh worker owns one typed query
  source and `ProductReducer`, coalesces refresh intents into at most one follow-up,
  and replaces one latest immutable snapshot only after a complete attempt. Query,
  cancellation, deadline, redaction, and shutdown contracts pass without Slint-thread
  blocking or partial visible publication.
- P3-B.2 capacity-one event bridge: the controller and Slint share the same latest
  snapshot mailbox, one weak notifier queues at most one event, and the UI applies
  only the newest generation with no polling timer, extra worker, second result slot,
  or strong window cycle. Race, retry, window-close, 10,000-notification coalescing,
  and real headless Slint event-loop contracts pass.
- P3-C quota-first Dashboard: explicit all-current quota and benefit overview reads
  feed one identity-free immutable six-section projection. The compiled responsive
  Slint board renders real today, quota/reset, Git, trend, session, activity, and model
  truth with 32/32/240/12/8/12 caps, section-local degradation, unknown values, and
  route-only in-place navigation without timers, animation, polling, or model rebuild.
- P3-D.1 bounded History route: a separate product section resolves the latest 30
  civil days through the existing capacity-one query worker and publishes independent
  failure/retention truth. One identity-free desktop projection and Slint model render
  overview tokens/cost/events, exact range/timezone/evidence, daily trend, and responsive
  newest-first details without route-time queries, timers, caches, or prior-range state.
  The expanded desktop audit passes 30/30 mutation cases and records one History model,
  one application path, a 30-day maximum, and zero polling/private-ID/direct-authority
  surfaces. Strict workspace Clippy and the complete locked test/doctest suite pass;
  the full suite completed in 710.7 seconds.
- P3-D.2a bounded Sessions list: the existing desktop query plan requests one all-time
  newest-first page capped at 64 while Dashboard keeps its 12-row summary. A separate
  identity-free projection and responsive Slint route render last activity, duration,
  events, every token bucket, total, cost, freshness/quality, and explicit `has_more`.
  Opaque keys/cursors never enter Slint; route switching adds no query, model rebuild,
  timer, worker, cache, or archive handle. Focused desktop/controller/UI contracts and
  the 33/33 mutation audit plus release audit/build pass. The complete post-slice
  locked workspace test/doctest suite and strict warnings-as-errors Clippy pass; the
  full suite completed in 725.2 seconds.
- P3-D.2b exact Sessions detail: each live controller/bridge bundle owns a checked
  backend epoch, and every click binds that epoch, the viewed immutable product
  generation, and a nonzero selection generation plus visible ordinal. One latest-only
  slot multiplexes exact detail with refresh on the existing worker; the opaque query
  key is resolved only there and never enters product correlation, Desktop, Slint, logs,
  or serialization. The responsive card updates highlight/loading synchronously and
  renders explicit idle/loading/ready/missing/unavailable states, exact summary and
  freshness/quality, plus at most 32 model and 32 approved path-free project-alias rows.
  Narrow layout retains every token bucket including reasoning. Current-bundle admission
  is nonblocking and rejects immediately while another operation owns the bundle mutex.
  Focused product/controller/projection/application/real-Slint contracts, strict package
  Clippy, 93 desktop/application audit cases, and product/desktop/application release
  audits pass. The compiled-UI contract sends real pointer, Enter, and Space events; a
  source mutation pins Tab navigation. No worker, queue, timer, cache, polling site,
  database owner, dependency, or second snapshot slot was added.
  Independent follow-up review returned READY with Critical/Important/Minor 0/0/0 after
  its one audit-scope Minor was closed by the 41/41 Desktop suite. The final clean-root/format/strict-
  Clippy/complete locked test-doctest baseline passed in 820.7 seconds overall (18.845,
  1.611, and 22.080 seconds for the first three stages). The credential-dependent live
  Codex contract remains explicitly ignored; M0 and product-release gates are separate.
- P3-D.3 bounded Models route: the fixed recent-30-day History request now captures
  Model and Project breakdowns in one immutable envelope, so History, Models, and the
  Projects route share exact range/timezone/freshness without a third analytics
  query. One identity-free projection retains at most 64 canonical model rows with
  input/cached/output/reasoning/total tokens, typed cost availability/mode/composition,
  events, relative distribution, and explicit backend/frontend truncation. The responsive
  compiled Slint view keeps every component in wide and narrow layouts, visibly and
  accessibly distinguishes partial plus calculated/reported/mixed cost evidence, and
  switches in place without rebuilding the
  model or adding a worker, timer, queue, cache, connection, dependency, private ID, or
  authority. Focused product/Desktop tests and all 47 production Desktop mutation
  audits pass. Independent re-review returned READY with Critical/Important/Minor
  0/0/0 after partial-cost/provenance and acceptance-matrix findings were closed. The
  clean-root, formatting, strict warnings-as-errors workspace Clippy, and complete
  locked workspace test/doctest baseline pass; the full suite completed in 790 seconds.
- P3-D.4 bounded Projects route: one usage-centric projection consumes the already
  prefetched recent-30-day Project breakdown and the existing UTC-today Git envelope,
  with no query or runtime owner added. It retains at most 32 safe alias/`Unassociated`
  rows, full token/cost/event truth, and optional exact-alias commits/added/removed/net/
  efficiency. Recent usage and Today code ranges/timezones/evidence stay visibly and
  accessibly separate. Same-alias repositories use checked sums and count project cost
  once; unassociated and Git-only aliases never become false joins/zero-usage rows.
  The responsive compiled Slint view switches in place and replaces one bounded model
  only for an accepted generation. Focused projection/UI/full Desktop tests, strict
  Desktop Clippy, 57/57 mutation audits, source authority/privacy checks, 256+lookahead
  Project and 32-store/16-query Git truncation fixtures, retained failure, partial cost,
  mismatched identity/cost, zero divisor, checked overflow, and 10,000-replacement
  release tests pass. Git-unavailable and not-linked rows expose no fabricated
  repository/code zeroes, including through wide/narrow accessibility. Independent
  re-review returned READY with Critical/Important/Minor 0/0/0. Clean-root, formatting,
  strict warnings-as-errors workspace Clippy, release composition, and the complete
  locked workspace test/doctest suite pass; the full suite completed in 807 seconds
  with serialized Windows GNU linking. P3-D.4 is closed; no release acceptance is
  inferred.
- P3-D.5 bounded Recent activity route: one projection consumes the existing
  `LatestActivityRequest::first(12)` product page and adds no query or runtime owner.
  It retains at most 12 newest-first UTC timestamp/canonical-model rows with typed
  input/cached/output/reasoning/total tokens, freshness/quality, optional `has_more`,
  and explicit empty/unavailable/retained-failure/backend/frontend truncation truth.
  Scope, event/dataset/provider/profile/source/session/project identity, cursor/
  fingerprint/key, paths, content, and authority do not cross Desktop/Slint. The
  responsive wide/narrow compiled view preserves the full accessible row meaning,
  switches in place, and remains available during aggregate rebuild. Focused 9/9
  projection, compiled UI, full Desktop package, strict Desktop Clippy, source/release
  audits, and 67/67 mutation cases pass. The route does not claim rhythm/heatmap
  parity; a bounded timezone/DST-aware aggregate remains future work. Independent
  review found two Important empty/evidence-state intersections; red/green fixes now
  degrade partial empty pages and distinguish retained empty pages from unavailable
  evidence. Re-review returned READY with Critical/Important/Minor 0/0/0. Clean-root,
  formatting, strict warnings-as-errors workspace Clippy, release composition, and the
  complete locked workspace test/doctest gate pass; the full baseline completed in
  1,035 seconds. P3-D.5 is closed; no release acceptance is inferred.
- P3-D.6 bounded Notifications route: one read-only projection consumes the existing
  all-current benefit overview and adds no query or runtime owner. It retains at most
  32 effective reminder-profile rows, 256 separate current-lot rows, and eight leads
  per profile while preserving exact/bounded/provider-local/provider-date/unknown
  expiry, inherited/override source, disabled/in-app-only coverage, evidence, warnings,
  due time, and explicit truncation. One scope and one lot Slint model render responsive
  wide/narrow and accessible expiry-safety views; navigation performs no query, model
  rebuild, notification take/ack/release, settings mutation, or activation. Focused
  projection/UI/full Desktop tests, strict Desktop Clippy, source audit, and 82/82
  mutation contracts pass. App-owned presentation receipts are implemented separately;
  per-scope editing, snooze, quiet hours, OS delivery, usage alerts, and activation
  remain unfinished capabilities. Independent re-review returned READY with Critical/
  Important/Minor 0/0/0 after lossless millisecond UTC, waiting-state, audit-authority,
  wide-completeness, and populated-replacement findings were closed. Clean-root,
  formatting, strict warnings-as-errors workspace Clippy, release composition, and the
  complete locked workspace test/doctest gate pass in 1,216.4 seconds overall
  (25.8/1.7/76.0 seconds for clean-root/fmt/Clippy). P3-D.6 is closed; no release
  acceptance is inferred.
- App-owned in-app expiry presentation: one runtime adapter leases and immediately
  maps at most 256 rows into an identity-free Desktop batch. One checked weak-window
  event applies a single transient model/count/visible state before `Presented`; one
  condition-variable worker acknowledges off the UI thread, retries acknowledgement
  only for Busy/StoreUnavailable after 60 seconds, and re-pumps a released failed
  presentation without an unrelated completion. A terminal acknowledgement error is
  released without automatic re-presentation. Runtime panic rolls acknowledgement back to a
  releasable lease; `Err`/`false` release retains local backpressure, and Desktop becomes
  ready before invoking the receipt. Focused Desktop/app/reminder-runtime tests, source
  receipts, and the combined 177/177 Desktop/application mutation suite pass. Global
  settings synchronization/editing is implemented separately; per-scope editing,
  snooze, quiet hours, OS/tray delivery, usage alerts, and activation remain open.
  The exact clean-root/fmt/workspace-Clippy/workspace-test developer baseline also
  passes; this is not M0, interactive Windows, soak, package, signing, or release
  acceptance.
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
  The P2-F joined status now exposes `aggregate_rebuilding`; analytics itself returns
  stable `unavailable` because no truthful aggregate payload exists during rebuild.
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

## P2-E Git output foundation

Tasks 1-4 of
`docs/superpowers/plans/2026-07-16-tokenmaster-git-output.md` are implemented. The
domain now owns opaque repository/activity identities, exact bounded line/day/category
metrics, quality, warning, and unavailable contracts. `tokenmaster-git` owns the
bounded streaming NUL parser, deterministic aggregate core, and exact native read-only
backend with fixed commands, explicit executable discovery, concurrent capped output,
deadline/cancellation cleanup, and synthetic coverage for root/merge/octopus/rename/
binary/gitlink/worktree/mailmap/shallow/history-change semantics.

Codex metadata and turn context now emit at most one latest provider-neutral
`RepositoryActivityHint` per source batch. Its canonical local path is sealed,
non-serializable, fully redacted, excluded from resume/checkpoint/events/SQLite, and
transported only through the descriptor-bound reader side channel. Relative,
traversal, network/device/mapped-remote, symlink, and reparse ancestry fail closed;
an explicit invalid `cwd` clears the prior transient association. Exact provider,
profile, source, session, event time, and safe project alias are preserved. Existing
provider readers default to no hint. The Git backend applies the same validation again
to the executable parent, candidate, common directory, and worktree root immediately
before command use.

Task 5 is now implemented. Strict schema v13 adds a random installation salt,
independent monotonic Git publication state, at most 32 opaque repositories, 4,096
activity associations, immutable aggregate generations, exact all-time totals/eight
categories, the latest 400 daily points with eight day-category rows each, and at most
16 stable warnings. Exact v12 migration and injected schema failure roll back without
touching usage, price, quota, benefit, reminder, or acknowledgement state.

Authoritative rebuild, same-process CAS-proven append, unchanged refresh, and
rebuild-required invalidation are atomic. Changed/incompatible authority preserves the
prior generation stale. Unavailable publication stores no fabricated cache identity
or zero series. Daily retention becomes explicit partial
`daily_history_truncated` with an oldest-retained boundary and per-request completeness.
An absent project key clears the earlier value; multiple associations expose a project
key only when all agree, otherwise the capture adds `association_incomplete`.
`UsageReadStore` returns owned all-time/range totals, categories, days, warnings and
quality through a 32+1 repository lookahead, at most 400 inclusive days, and a hard
maximum two-second deadline whose progress handler is cleared before reuse.

Task 6 is now implemented. `QueryService::git_output` returns owned schema-v1
envelopes with checked process-local generation, independent Git publication
revision, explicit UTC half-open range, per-repository freshness/quality, all-time and
range totals/categories, retained days, warnings, unavailable/retention truth, and
lookahead. Exact safe project aliases are recovered only through a store-owned
domain-separated salted matcher over at most 32 keys and 256 materialized project
candidates; salt and project keys never enter the product snapshot.

The optional fixed-point cost-per-100-product-code-lines join shares one maximum
two-second budget with Git capture and reads only materialized usage/price aggregates.
It requires exact association/range, complete Git evidence, current compatible usage,
exact non-conflicting cost, and a nonzero denominator. Ambiguous/unmatched projects,
partial ranges, stale Git or usage, unknown cost, zero lines, usage rebuild/deadline/
corruption remain typed unavailable. Usage failure does not hide independent Git
metrics. Focused contracts cover UTC/privacy, aggregate-only reads, failed-generation
neutrality, restart and concurrent publication isolation, corruption rejection,
32 repositories by 400 days, repeated handle/transaction return, and sub-two-second
maximum reads.

Tasks 7-8 are now implemented. `LiveRuntime` routes each successful Codex repository
side-channel hint into one independent `GitRuntime` without changing usage accounting.
The runtime owns one constant-state scheduler/worker, one active scan, one follow-up,
and at most 32 latest transient candidates. Unchanged refs skip history, compatible
same-process refs scan only newly reachable commits, and rewrite/recovery/pause-resume
force an authoritative rebuild. Git I/O and exact child cleanup finish before one
non-waiting writer lease and one SQLite open. Superseded sequences cannot publish;
known scan failures publish explicit unavailable truth or preserve the prior generation
as rebuild-required. Health is count-only and path-free.

Focused contracts cover 32-candidate eviction, sibling isolation, contention after Git
I/O and before SQLite, stale-result rejection plus one follow-up, missing-author durable
unavailable state, pause cancellation/reap, automatic resume rediscovery, and joined
shutdown. The Windows 16-warm-up/48-measured resource gate passed with a 3,293,184-byte
private floor, 6,422,528-byte sampled high, 118 handles, four threads, USER=1, and
GDI=0. `scripts/audit-git-output.ps1` passed across 126 production dependencies,
19 production boundary files, and four release libraries with zero forbidden
dependency, foreign-language, network/browser/credential/shell/direct-SQL/mutation,
vendored-upstream, or private binary-string matches. P2-E is complete; this does not
claim P3 UI, P5 CLI/MCP, M0 acceptance, packaging, signing, or release.

## P2-F joined product status

P2-F is implemented under
`docs/superpowers/plans/2026-07-17-tokenmaster-p2f-product-status.md`. The read-only
store captures usage publication/dataset/aggregate status plus independent quota,
benefit, and Git revisions in one defensive schema-v13 deferred transaction. The
maximum deadline is two seconds, progress cleanup is exact, mapping failures consume no
public generation, and fixed statements never scan event, rollup, quota-sample,
benefit-change, or Git-day history.

`QueryService::product_data_status` returns the bounded schema-v1 joined envelope. The
new leaf `tokenmaster-product` crate retains one current `Arc<ProductSnapshot>` and no
history. Data sections use a checked attempt generation distinct from their durable
source generation; runtime health uses a separate generation. Older asynchronous work
is rejected, compatible refresh failures retain the last successful payload with a
stable path-free code, and incompatible durable identity invalidates only the affected
payload.

Exactly 11 routes derive ready/degraded/unavailable state from one `u16` reason set.
Settings and Help/About need no archive. During aggregate rebuild, Dashboard degrades
section by section, Activity and Data Health remain reachable, and History, Sessions,
Models, and Projects are unavailable. Usage, quota/benefit, reminder, and Git runtime
owners remain outside the product layer; only bounded count/lifecycle/retry/failure
projections are copied. Real pause/resume, reminder contention, quota transport fault,
and sibling-isolation contracts pass.

The 100,000-event status fixture measured 0.125 ms p95 over 40 samples against the
25 ms gate. Ten thousand reducer replacements retain one current payload. The isolated
Windows gate completed 1,152 open/capture/drop cycles with stable 111 handles, four
threads, USER=1, GDI=0, and private memory returning below the original +2 MiB budget
after topology/convergence warm-up. `scripts/audit-product-status.ps1` passed with one
leaf package, six production files, no dynamic status collections/runtime ownership,
no direct filesystem/network/process/SQL/UI authority, no forbidden whole-history
status scan, 11 fixed routes, and zero vendored-source/release-string matches. This is
developer evidence for P2-F only; visible P3 UI and release acceptance remain separate.
The post-synchronization clean-root audit, formatting check, warnings-as-errors locked
workspace Clippy, and complete locked workspace tests/doctests pass.

## P3-A production desktop shell

P3-A is implemented under
`docs/superpowers/plans/2026-07-17-tokenmaster-p3a-desktop-shell.md`. The existing M0
probe remains unchanged as evidence; the new `tokenmaster-desktop` frontend has a
package-specific software-only Slint graph and no M0 dependency. One fixed projection
maps `ProductRoute::ALL` in canonical order, copies at most 11 stable reason codes per
route, retains one current generation/selection, and rejects equal or older updates.

The compiled `TokenMaster` shell contains one window, header, 11-route navigation, and
truthful route state/reason panel. It starts from the real initial product snapshot;
there are no seeded quota/session/chart values. Six adversarial Pester contracts and
the production audit reject probe dependencies, mock/seed data, FemtoVG, route drift,
direct store/runtime authority, and filesystem/network/process/SQL/browser/credential
surfaces. The release executable audit reports five Rust files, five Slint files, one
retained route model, 11 routes/reasons maximum, and zero forbidden source, dependency,
probe, renderer, or private-canary matches.

This closes only P3-A. It does not claim live archive/controller publication, the six
dashboard sections, exploration payloads, visible reminder acknowledgement, compact
widget lifecycle, P4 skins/localization/accessibility/paint gates, M0 acceptance,
packaging, signing, or release.

## P3-B.1 bounded desktop controller

P3-B.1 is implemented under
`docs/superpowers/plans/2026-07-17-tokenmaster-p3b-controller.md`. The desktop now
depends directly only on the read-only `tokenmaster-query` facade and the proven
`tokenmaster-engine` refresh coordinator in addition to product/Slint support. One
worker-confined query source and reducer publish through one capacity-one latest
snapshot slot. Started attempts are distinct from coalesced intent receipts; the
eventual follow-up receives its own product attempt generation.

Focused contracts prove status-first reduction, one attempt generation across all
sections, section-local failure, real empty schema-v13 reads, 1,000 hints collapsing
to one follow-up, latest-only result retention, cancellation/deadline without partial
publication, deterministic shutdown/post-close rejection, and path-free open errors.
The desktop audit now has eight adversarial Pester contracts and reports six Rust/
five Slint files, one controller worker, one retained snapshot slot, no UI-thread
query surface, no direct store/provider/runtime/network/shell/SQL authority, and no
forbidden release-binary strings.

At P3-B.1 closure this was the controller core only. P3-B.2 now marshals its latest
snapshot through one coalesced Slint event, and P3-B.3 now owns the validated data root
and sole live-runtime composition. Safe benefit scope discovery remains an explicit
query contract before the benefit card can be ready.

The post-synchronization clean-root audit, format check, warnings-as-errors locked
workspace Clippy, and complete locked workspace tests/doctests pass. No task-owned
Cargo, compiler, test, or TokenMaster process remains.

## P3-B.2 capacity-one Slint event-loop bridge

P3-B.2 is implemented under
`docs/superpowers/plans/2026-07-17-tokenmaster-p3b2-event-bridge.md`. The existing
controller mailbox remains the only retained `ProductSnapshot` result. One idle-only
notifier attachment holds a weak bridge reference; attaching after an idle
publication immediately wakes the populated mailbox. One atomic scheduled flag
coalesces all notifications into at most one Slint event. The event takes only the
newest snapshot, upgrades one weak window, applies only a newer generation, clears the
flag, and performs one post-drain race recheck.

The bridge owns no timer, polling thread, query, store, runtime, path, data source,
second queue, or strong window cycle. Fixed saturating counters and stable failure
codes expose delivery health. Deterministic contracts prove 10,000 notifications
retain one event and deliver generation 10,000 only, a publication during delivery
gets exactly one follow-up, a scheduler-unavailable snapshot retries without loss,
bridge/window close stops scheduling, and bridge/observer handles are `Send + Sync`.
A real headless Slint integration event loop applies a controller-produced snapshot
to the generated `MainWindow` and exits deterministically.

At P3-B.2 closure the desktop audit had 12 adversarial Pester contracts and reported
seven Rust/five
Slint files, one controller worker, one retained snapshot slot shared with the bridge,
one event-loop schedule site, and zero bridge polling surfaces. It also rejects a
second slot/event site, timer/thread polling, strong window retention, UI queries, and
all prior direct-authority, renderer, probe, seeded-data, and private-string drift.

At P3-B.2 closure the production executable still started from the truthful
initial snapshot because P3-B.3 must select the installed/portable archive root and
compose the existing live runtime as sole ingestion owner. Safe benefit-scope
discovery, visible route payloads, P4 paint/resource gates, M0 acceptance, packaging,
signing, and release remain unclaimed.

## P3-B.3 deterministic application composition

P3-B.3 is implemented under
`docs/superpowers/plans/2026-07-17-tokenmaster-p3b3-application-composition.md`.
`tokenmaster-app` now owns the only production `TokenMaster.exe`; the six-Rust-file
`tokenmaster-desktop` package is library-only and remains directly limited to Slint,
engine, product, query, and error-context support.

An exact empty `tokenmaster.portable` file beside the validated executable selects the
adjacent `data` child; absence selects `%LOCALAPPDATA%\TokenMaster`. Both resolve to
one canonical local non-reparse directory and `tokenmaster.sqlite3`. Invalid marker,
missing installed base, unsupported media, or creation failure is stable/path-free and
never changes mode or falls back to CWD.

The app starts one mandatory `LiveRuntime` with nested Git plus independently
degradable quota and reminder runtimes, one desktop controller, and one Slint bridge.
The existing workers emit optional lossy completion hints after receipt publication.
One weak app notifier copies four fixed product health/error values under a checked
generation into one capacity-one controller observation; the existing controller and
bridge coalesce query/event work. There is no new timer, polling thread, queue, second
ingestion owner, runtime in desktop, or strong window/application cycle.

Focused evidence includes two notifier-order/panic contracts, four runtime propagation
contracts, typed product-health publication, 10,000 desktop observation replacement,
an active-query follow-up race, five data-root contracts, early-notification and
generation-overflow tests, a real live-bundle health/join/shutdown test, 21 adversarial
Pester cases, both source/release audits, and a successful release application build.
The fresh post-P3-B.3 workspace gate also passes clean-root, formatting, warnings-as-
errors Clippy, all tests, and all doctests. Its Windows-load-only Git process-test
race is now a deterministic delayed-start regression: deadline/cancellation cleanup
checks the exact fixture executable even when the child is reaped before publishing a
PID receipt.
P3-C subsequently completed visible quota-first route payloads and safe all-current
benefit-scope discovery. P3-D supporting views and P3-E desktop integration are now
the remaining P3 work; skins/locales/P4, automation/P5, release/P6, activation, and
acceptance evidence remain unclaimed.

## P3-C quota-first Dashboard

P3-C is implemented under
`docs/superpowers/plans/2026-07-17-tokenmaster-p3c-dashboard.md`. Separate quota and
benefit overview contracts preserve exact-empty filter semantics and capture at most
32 current windows, 32 scopes, and 256 lots under exact revisions. Product/controller
publication remains section-local and uses the existing one worker, reducer, snapshot
mailbox, and Slint event gate.

One pure `DesktopDashboardProjection` maps a current immutable snapshot into exactly
six ordered sections: Plan Usage, Code Output, Usage and Cost Trend, Sessions,
Activity, and Model Usage. Retained bounds are 32 quota rows, 32 benefit summaries,
240 trend points, 12 sessions, eight fixed activity rows, 12 models, and checked Git
aggregation over at most 32 repositories. Missing facts stay unavailable/partial;
banked resets, credits, temporary usage, and unavailable lots remain separate. No
account/workspace/window/lot/repository/project/session/event/source ID enters the UI.

The responsive Slint Dashboard uses semantic models/components/tokens and switches
between narrow and wide layouts without recreating `MainWindow`. After initial
construction, an accepted newer snapshot replaces each of seven bounded list models
once; route-only selection updates only
navigation state. The UI owns no query, SQL, runtime, timer, animation, polling thread,
filesystem/network/process/browser/shell/credential authority, or seeded metric.
Focused tests include real fixture values, 32 dynamic quota rows, reset separation,
unknown truth, in-place navigation, section-local degradation, checked Git sums, and
10,000 projection replacements.

The desktop adversarial suite now has 20 cases and the source receipt reports seven
Rust files, nine Slint files, six Dashboard sections, seven bounded list replacements,
one Dashboard application path, one controller worker, one snapshot slot, one event
site, and zero polling/private-ID surfaces. P3-D/P3-E, P4 skin/locale/accessibility/
paint/resource acceptance, P5 automation, activation, M0 acceptance, packaging,
signing, and release remain unclaimed.

## P3-D.0 reliable-state implementation record

The reliable-state design and 18-task TDD rail are approved in
`docs/superpowers/specs/2026-07-17-tokenmaster-reliable-state-design.md` and
`docs/superpowers/plans/2026-07-17-tokenmaster-reliable-state.md`. They keep the
implemented fixed `tokenmaster.sqlite3` and writer sidecar rather than introducing a
second live database identity. Task 1 now adds the library-only `tokenmaster-state`
workspace package with nine stable path-private error codes, checked byte/item limits,
and a Pester/workspace authority audit. After Task 6, the current workspace receipt
records six exact direct dependencies, one exact workspace member, zero bin/build
targets, zero forbidden
filesystem/process/network/shell/SQL/UI/archive/external-source authority, zero public
arbitrary-path constructors, and zero forbidden transitive dependencies. The original
mutation corpus now totals 36 cases guarding observed source, approved-alias reuse,
fixed-child, visibility, generic-stream, alias/re-export, and metadata bypasses.

Task 2 now adds controlled `tokenmaster-platform` durable files. One validated local
directory plus a restricted exact child name is the only public target constructor;
staging has 32 create-new slots, a 64 GiB plus 2 MiB file ceiling, 256 KiB write chunks,
streaming SHA-256/length verification, flush/close/reopen, and path-private fixed
errors. Windows publication uses `MoveFileExW(MOVEFILE_WRITE_THROUGH)` without copy
fallback and existing-target replacement uses `ReplaceFileW` with an exact old-target
backup. Every post-publication uncertainty is `RecoveryRequired`; ambiguous rollback
preserves staged and backup artifacts. The Unix fallback uses no-overwrite hard links,
atomic rename, file/directory synchronization, and makes no Windows durability claim.
Focused evidence is green: strict platform Clippy, 9 library contracts, 11 durable
integration contracts, 40 deterministic before/after child-process kills, 20
replacement-entry race kills, and a final independent read-only review.

Task 3 now adds the crate-private redundant-record core. Its fixed envelope has a
64-byte `TMREC001` header, strict JSON payload capped at 1 MiB, and a 40-byte
`TMEND001` footer with payload and whole-record SHA-256 binding. Decode validates
actual file bounds, exact version/header/flags/generation/length, both digests, no
trailing bytes, and typed JSON before selection. The highest valid generation wins;
one invalid slot is a typed fallback, equal generations require equal payload digests,
and a conflict or two invalid slots cannot authorize repair. Save measures/hashes
without retaining encoded JSON, streams a second deterministic pass in at most
256 KiB calls, seals the inactive slot, publishes, and rereads both slots. Every
post-publication uncertainty is `RecoveryRequired`.

The new platform support is limited to caller-bounded exact-child reads and replacement
of an inactive A/B slot without a third backup. Focused evidence passes 13 record unit
contracts, two public authority contracts, 10 platform unit contracts, 14 durable-file
integration contracts, the now-expanded 34/34 Pester mutations, an injected redundant before/after OS
boundary, 40 redundant boundary kills, 20 redundant entry races, and state process
death during partial write, after seal/before publish, and after publish/before reread
of generation 3. Generic record/file authority is not reexported from
`tokenmaster-state`; Task 4 wraps it in fixed-purpose typed settings APIs. Final
independent review reports no Critical or Important finding. A no-follow/open-handle
identity check for a hostile same-user path-replacement race remains recorded as
defensive hardening outside the current threat boundary.

Task 4 now adds public typed version-1 settings without exposing generic record or
path authority. The exact schema stores only the implemented provider-neutral in-app
reminder default, automatic-backup enabled/quiet/interval/retention policy, and the
device-local last route. Reminder lists are canonical, unique, range checked, and
capped at eight; backup quiet/interval relationships and the 256 MiB-through-64 GiB
budget are validated at minimum five-minute quiet and six-hour interval. Unsupported
versions, including a newer schema inside a valid record envelope, unknown/duplicate
fields, invalid enum
values, relationships, and payloads above 1 MiB fail before publication. No skin,
locale, OS notification, pricing, provider, credential, source path, prompt, response,
command, or source-content placeholder is persisted.

`SettingsStore` returns stable `Current`, `Fallback`, or `Defaults` outcomes and
path-private health codes. A single missing peer is healthy first-generation state; a
corrupt peer is an explicit fallback. Two invalid slots load safe defaults without
touching evidence; only a later explicit validated save may replace one slot and it
keeps the other invalid file. Portable import preview reports only bounded changed
categories/counts, rejects stale confirmation, preserves the current device route,
and is idempotent. Commit receipts expose a nonzero generation plus portable SHA-256
target that can be reconstructed from a future journal and independently reread-
verified. Ten settings contracts, 13 record contracts, two public authority
contracts, strict state Clippy, the workspace state audit, and 34/34 authority
mutations pass. Independent high-risk review closed newer-schema overwrite, schedule-
floor, directory-capability bypass, and unbounded sequence findings; its final pass
reports no Critical, Important, or Minor issue and `Ready: Yes`. The fixed `.tmconfig`
container is now implemented by Task 6; Task 4 supplies its bounded portable-settings
entry without gaining package authority.

Task 5 now adds store-owned SQLite Online Backup and verified candidate capabilities.
The live main file is never copied as a backup: page-stepped snapshots include
committed WAL truth, bound busy retry, cancellation, deadline, output size, and fixed
staging names. Verification opens a standalone candidate under defensive/query-only
policy, zero mmap, a 4 MiB cache, `cell_size_check`, exact bundled SQLite identity,
and explicit SQLite value/SQL/column limits. Integrity, foreign keys, exact schema,
indexes, counts, generations, and application semantics are independent gates.
Supported old schemas are inspected without migration; newer schemas are classified
without mutation. Compact output is created only with `VACUUM INTO` from an isolated
verified snapshot and is reverified before acceptance.

A verified candidate is bound to physical file identity, length, and streaming
SHA-256 before and after verification and every compaction consumer. Path replacement
therefore fails as `StaleBackupCandidate`. Candidate cleanup has explicit discard,
one bounded failure counter, and a fixed 64-name abandoned-candidate recovery pass;
errors and Debug remain path/SQLite-text private. Fifteen backup contracts plus one
barrier test prove a writer commit strictly between Online Backup page steps. Final
independent high-risk review reports Critical 0, Important 0, Minor 0 and
`Ready: Yes`.

Task 6 now adds the fixed `.tmconfig`/`.tmbackup` v1 codec. The deterministic layout
is one 32-byte header, one 40-byte self-describing manifest, ordered typed entries
with 64-byte descriptors and 24-byte suffixes, then descriptor binding, `TMEND001`,
and a preceding-byte SHA-256; the complete controlled file is independently hashed
and sealed. Config contains exactly portable settings. Backup contains settings then
one database plus creation time, database schema, compression profile, and one of
five periodic/manual/pre-/post-migration/pre-restore purposes.

`zstd` is pinned to 0.13.3 with default features off. Every entry is one checksummed,
content-sized frame at level 6/12/19, one thread, window log 23, exact expanded length
and SHA-256. Parsing uses 64 KiB buffers, an independent output counter, 1 MiB
settings, 64 GiB database, and checked 64 GiB-plus-2-MiB ceilings. Public APIs accept
only `DurableFileReader`/`DurableStagedFile`; raw stream helpers are private and the
audit rejects public generic stream authority. Codec/final-seal failure irreversibly
discards and poisons output so later write/seal/publish is `InvalidState`.

Evidence passes a frozen 405-byte config vector with complete-file SHA-256, all three
profiles times the then-five purposes, a 24 MiB streaming database, every structural
flip/truncation class, overflow/unknown/duplicate/concat/trailing/checksum/digest
mutations, an independently resealed missing frame end, a 16 MiB-window frame, and a
300-to-256 content-size bomb. Focused package tests are 5/5 and adversarial tests
10/10; platform durable contracts are 17/17; strict Clippy, the workspace authority
audit, and 36/36 Pester mutations pass. Final independent review reports Critical 0,
Important 0, Minor 0 and `Ready: Yes`.

Task 7 now adds optional binary age v1 protection for manual backup exports.
`age = 0.12.1` is exact with default features disabled; no CLI, plugin, SSH, armor,
async, unstable, or web feature is enabled. Encryption accepts only
`ManualExport`, a controlled reader, and an opaque `VerifiedBackupPackage`; the same
streaming pass rechecks exact source length and complete-file SHA-256 before the
ciphertext stage remains sealable. Automatic mode and same-length source substitution
fail closed. Export fixes scrypt `log_n = 16`; import sets maximum 16 before stanza
unwrap, so a malicious larger factor returns before attacker-selected derivation.

`BackupPassphrase` is a non-cloneable redacted zeroizing secret. New values require an
exact confirmation and 12 through 128 Unicode scalar values; neither trim nor
normalization occurs. Both caller-owned fields are taken and cleared on every outcome.
Decrypt authenticates the complete age stream and immediately feeds it to the private
typed `BackupPackage` parser; only the verified inner database stage is sealed.
Authenticated non-package plaintext, wrong password, malformed/non-scrypt header,
header MAC/body/final-tag corruption, truncation, trailing data, platform I/O, output
capacity failure, and final seal failure all irreversibly discard/poison the output;
cleanup uncertainty is `RecoveryRequired`. Seven grouped encryption contracts cover
the full matrix. Strict state Clippy, the workspace authority audit, exact feature
inspection, and 37/37 Pester mutations pass. The generic inner stream parser is fully
private behind a typed authenticated-payload bridge; final independent security
rereview reports Critical 0, Important 0, Minor 0 and `Ready: Yes`. The final
unchanged-source component baseline passes clean-root, formatting, strict locked
full-workspace Clippy, and the complete locked workspace test/doctest suite in 544.1
seconds combined.

Task 8 now adds the sealed automatic-backup directory, disposable catalog, and
deterministic protected retention. `tokenmaster-platform` owns the canonical local
`backups` child and exactly 32 private slots. Enumeration rejects unexpected names/
types, symlinks, reparse points, hard links, duplicate physical identities, stale
tokens, and crash remnants. Stages expose bounded write/seal/discard plus a path-free
reader only after seal; only the owning directory can publish. Deletion uses a
write-through exact tombstone: a before-move interruption leaves the point unchanged,
while every post-move or uncertain boundary becomes `RecoveryRequired`.

`BackupPackage` now composes directly with the sealed exact-slot stage. The production
sequence is write, fully parse/verify the still-unpublished stage, no-delete retention
admission, publish with seal recheck, catalog rebuild/bind, and exact confirmation.
Cold catalog rebuild streams all complete bytes but reports only `HeaderValid` or
`Corrupt`; `Verified` requires exact current package proof. Warm proof carries only
across unchanged physical identity, length, complete-file SHA-256, and typed metadata.

Retention protects the candidate, newest two verified points, and the newest
pre-migration point until later verified post-migration evidence, then applies the
shared four-newest/seven-UTC-day/four-ISO-week tiers under 15 points and the checked
256 MiB-through-64 GiB byte budget. Admission deletes nothing. Each deletion first
rehashes the complete current verified set, rechecks the exact target and directory
generation, removes at most one oldest unprotected verified file, and requires rebuild/
replan. Same-length corruption of the candidate, target, or another protected point
preserves all files. Focused catalog 4/4, retention 2/2, platform directory 5/5,
mixed-error unit, source authority audit, and 42/42 Pester mutations pass.
Independent third review reports Critical 0, Important 0, Minor 0 and `Ready: Yes`.
The final Task 8 workspace baseline passes clean-root (17.4 seconds), formatting (1.3
seconds), strict locked full-workspace Clippy (13.3 seconds), and the complete locked
workspace test/doctest suite (566.3 seconds total).

Task 9 now adds constant-size backup maintenance. `MaintenanceCoordinator` retains
one active request and one urgency-merged follow-up; 10,000 concurrent hints do not
create a queue. Mandatory safety points outrank manual, source retry, and periodic
work, while a second unresolved mandatory guard is explicitly busy. A retry gets a
fresh attempt ID and lower scheduling urgency but preserves the original root request
and backup purpose, so a failed pre-migration point cannot turn into periodic truth.
Two failures against the same opaque source identity enter `Suspect`.

`BackupMaintenanceRuntime` owns exactly one worker and one scheduler/shared timer,
with joined pause/resume/shutdown/Drop lifecycle. Exact `Healthy` restart truth seeds
the first interval at the current monotonic tick while `HealthyUnpublished` remains
closed. Automatic work requires both quiet and ordinary interval gates, emits one
resume/clock-rollback catch-up, and periodic disablement discards a merged periodic-
origin follow-up without discarding an internal retry or mandatory guard. Health is fixed latest completion/counters plus one exact
mandatory-guard completion; no request/progress history is retained. A permit creates
a store-owned `BackupControl` linked to the same cancellation state, and final
publication is a non-cancellable compare-exchange boundary. `Published` before or
`Cancelled` after that boundary is an internal-invariant failure.

The store now exposes one bounded path-free `VerifiedBackupCandidateReader`. It checks
physical identity, length, and full SHA-256 before open and after complete streamed
consumption. The sole state/store package bridge writes that reader directly into a
sealed unpublished stage and discards/poisons output on replacement, truncation,
append, cancellation, source/destination, codec, or seal failure. The package format
adds backward-compatible purpose value 6 for pre-destructive maintenance; prior values
remain unchanged. Focused maintenance 17/17, resource 1/1, store backup 7/7, catalog
6/6, strict state Clippy, workspace authority audit, and 47/47 mutation tests pass.
The first independent review's four Important findings and one Minor evidence finding
have corresponding regressions and fixes; post-fix rereview reports Critical 0,
Important 0, Minor 0 and `Ready`. The final Task 9 baseline passes clean-root in 14.940
seconds, formatting in 1.256 seconds, strict locked workspace Clippy in 12.340 seconds,
and the complete locked workspace test/doctest suite in 507.6 seconds. The live-auth
Codex transport test remains the one expected environment-gated ignored test.

Remaining ownership is: store for SQLite Online Backup and candidate verification,
platform for durable same-volume replacement and sealed file selection, state for
settings/packages/retention/recovery, and app for runtime shutdown/restart and safe
mode. Product/Desktop receive copied bounded health and typed intents only.

The v1 contract uses redundant settings/run/recovery records, strict
`.tmconfig`/`.tmbackup` packages, streaming Zstandard levels 6/12/19 with an 8 MiB
window, optional bounded age passphrase protection for manual exports, default
four-newest/seven-daily/four-weekly retention under 15 points and 2 GiB, a maximum
three quarantine sets, and an idempotent six-state restore journal. Manual restore
selects data only or data plus portable settings; automatic recovery is data only and
device-local settings remain untouched. Definitive corruption alone may authorize
automatic restore; busy, permission, disk-full,
transient-I/O, unsupported-location, and schema-too-new results preserve current
truth. No valid backup leads to explicit quarantine and authoritative-source rebuild,
never fabricated zero or automatic corrupt-row salvage.

Tasks 1-11A are implemented. Task 10 adds the physically lease-bound platform recovery
scope, path-free store verification, repeatable six-phase redundant journal, three-set
quarantine, old-or-new promotion/rollback, prepared settings target, manual and
corruption-only automatic modes, restart at pre-journal mutation boundaries, and
three-artifact absent/completed-journal staging cleanup with actual-free-space
preflight. Both platform and store enforce the shared cap; the peak is the larger of
`2B` and `B+A`, plus an 8 MiB reserve, and the physical lease is authorized before
any verifier/platform cleanup. Corruption authority is internal verifier evidence, never a caller
assertion; a verified backup remains prior-install evidence when main is missing.
Final independent Task 10 rereview reports Critical 0, Important 0, Minor 0 and
`Ready`. The final baseline passes clean-root in 14.899 seconds, formatting in 1.396
seconds, strict locked workspace Clippy in 9.169 seconds, and the complete locked
workspace test/doctest suite in 545.3 seconds; the reliable-state audit, 52/52
mutations, and the changed platform MSVC target check also pass. Task 10 is accepted
as developer evidence, not as product release or M0 acceptance.

Task 11A adds strict `run-a.tms`/`run-b.tms` launch truth and `StateBootstrap` before
ordinary SQLite open. Bootstrap validates that every data/reliable-state capability is
bound to one root, observes prior artifacts, durably publishes and rereads `unclean`,
resumes a pending journal, and performs read-only startup inspection. Only an exactly
clean prior run skips `quick_check(100)`; missing, invalid, and unclean prior state add
it. Supported legacy and newer schemas return migration/upgrade-required without
mutation. Definitive corruption or a missing main with prior backup evidence selects
newest-first and fully reverifies each candidate; corrupt newer points are skipped and
automatic restore is always data-only. Busy/access/disk/cancel/transient/policy results
and no usable backup preserve the active/corrupt set.

The same recovered candidate may fail two launches; a third enters safe mode. A clean
run accepts the recovery generation so a completed historical journal neither creates
a false crash loop nor blocks a later independent recovery. `LiveRuntime` now consumes
the already-held platform guard through archive open and startup recovery, then releases
that startup guard; later mutations acquire the same fixed lease per operation. Legacy
starts retain their behavior. A runtime integration contract proves first-install
bootstrap, continuous guarded startup, live archive creation/use, joined shutdown, and only then clean
publication. Zero-length WAL/SHM facts are valid and identity-bound; zero-length main
is rejected. Focused platform 13/13, writer-lease 9/9, store startup 5/5, state restore
20/20, bootstrap 12/12, automatic recovery 7/7, and full runtime tests pass; strict
locked workspace Clippy, the reliable-state workspace audit, and 55/55 authority
mutations also pass after independent review caught two redundant attributes. The
complete locked workspace test/doctest suite passes in 571.4 seconds. The changed
platform capability also passes the explicit `x86_64-pc-windows-msvc`
warnings-as-errors target check.

Task 12A is implemented in the working tree. `ApplicationStateOwner` runs Task 11A
preflight before live/query/controller construction, and safe mode retains the Slint
shell without any archive/runtime/query/maintenance owner. A healthy bundle owns one
capacity-one maintenance runtime whose concrete operation performs SQLite Online
Backup, full candidate verification, typed package staging and verification, sealed
publication, verified-package catalog binding, and bounded one-at-a-time retention.
Cold catalog verification runs on the worker; unchanged package proofs survive rebuilds
and the final projection remains owned by the operation. Terminal receipts use one
deadline-bounded condition-variable wait; no polling thread or UI timer was added.

Supported legacy startup now publishes a verified pre-migration package before writable
open/migration, then records one path-free pending source/target pair in redundant
run-state schema v2. A verified post-migration package clears that pair before exposing
the bundle. Both points remain mandatory when periodic backup is disabled. Failure
before writable open leaves the old schema intact; failure after migration leaves the
migrated archive and pending obligation in safe mode. Restart completes the post point
before live, and `clean` rejects a pending pair. First-install live WAL
snapshots are supported without weakening standalone package validation: only a live
source with a regular WAL may have schema-format byte zero; copied candidates still
require format four and the complete verifier. Healthy shutdown pauses and joins
maintenance, then controller/quota/reminder/live owners, and only then publishes clean.

Task 12B.1 is also implemented. One application-owned typed command
coordinator keeps one active request and at most one distinct follow-up, coalesces
10,000 identical hints, rejects a third distinct request, supports exact active/queued
cancel plus explicit retry, and makes cancellation impossible after the irreversible
boundary. Restart pauses admission and discards only the follow-up, joins the current
bundle, acquires a fresh archive guard, builds one higher bundle generation, preserves
the Slint window, and resumes admission. Runtime notifiers carry the exact bundle
generation and compare it under the same slot mutex, so obsolete completion hints are
discarded before allocating a product-runtime generation.
Focused Task 12B.1 evidence passes: 7/7 command contracts, 14 app unit plus 7 app
integration contracts, strict app Clippy, and 23/23 application authority contracts,
including a clean-composition control and grouped process-import rejection.
The required clean-root, formatting, and warnings-as-errors locked workspace Clippy
gates pass on this tree. The complete locked workspace test/doctest gate then passes
in 481.6 seconds. The release application composition audit passes with one production
binary/artifact and exactly one command/restart/runtime owner surface; the reliable-
state workspace audit and 55/55 authority mutations also pass. Independent follow-up
review reports Critical/Important/Minor 0/0/0. This is developer evidence, not product
or release acceptance.

Task 12B.2a selected restore composition is implemented. A current
generation/ordinal choice is revalidated against the complete directory and becomes one
opaque RAII identity pin. Every in-flight retention deletion consults the same pin gate,
including cycles admitted before the command, so the selected package survives old-
maintenance join and protected `PreRestore` publication. Catalog projections are
immutable bounded `Arc` snapshots; heavy snapshot, verify, retention planning, and
recovery I/O do not hold the projection mutex.

After all old owners join, a fresh fixed guard enters the existing journaled restore.
The exact recovery receipt is durably bound to the current run session before any
restored lifecycle work. Current schema rebuilds one fresh bundle; supported legacy
schema repeats verified pre/post migration points and the pending source/target pair.
Stale selections and shutdown attempts fail before mutation; any later ambiguity leaves
safe mode with zero bundle owners. The sequential lifecycle regression covers current
and v12 selected restore, stale reuse, bounded retention, clean recovery acceptance,
and final shutdown. Catalog 7/7, retention 3/3, app 14 unit plus 7 integration, and
31/31 application source-policy mutations pass. Clean-root, formatting,
warnings-as-errors locked workspace Clippy, and the complete locked workspace
test/doctest gate pass; the latter takes 507.5 seconds. Release composition and the
reliable-state workspace audit pass, as do all 55/55 reliable-state authority mutations.
Independent final review reports Critical/Important/Minor 0/0/0. The live-auth Codex
executable contract remains intentionally ignored without its explicit environment
binding. This is Task 12B.2a developer evidence, not product or release acceptance.

Task 12B.2b.1 is implemented in the current development contour. The production app
owns one joined standard-library operation worker containing the sole command
coordinator, one capacity-one wake, active plus one follow-up, and one latest-only
completion. Execution occurs outside the worker mutex; cancellation, irreversible
transition, retry, panic fault/closure, explicit shutdown, and `Drop` remain constant-
state and path-private. The first real binding runs manual backup through the existing
atomic maintenance receipt wait off the Slint thread while keeping the current bundle
generation stable. Clean publication now also requires the operation worker to join.

Sealed config operations are implemented below the application/UI boundary. A separate
2 MiB encoded `.tmconfig` ceiling fails before parsing. Export writes portable settings
to an already controlled create-new target and reread-verifies the published package.
Import fully verifies an already open reader, retains one typed base-bound preview with
at most three categories and scalar counts, and commits only that candidate while
preserving device-local settings. Nine worker, two app config, six package, 25 app unit
plus 7 app integration, strict focused Clippy, and 38/38 application source-policy
contracts pass. Clean-root, formatting/diff, warnings-as-errors locked workspace Clippy,
the complete locked workspace test/doctest suite in 502.1 seconds, release composition,
and reliable-state 55/55 mutations also pass. Independent final review reports
Critical/Important/Minor 0/0/0 and `Ready`; the authenticated live Codex contract remains
intentionally ignored without explicit environment binding. This is developer evidence,
not product or release acceptance.

Task 12B.2b is now implemented through the application/UI boundary. Config selection,
preview/confirm/cancel, normal/compact/encrypted backup, verification, confirmed
selected restore, rebuild, retry/cancel, and backup-policy updates are fixed path-free
intents. Native dialogs run on their owning Slint/STA thread and only sealed input/output
capabilities cross to the single operation worker. Each mutating path publishes
`AtomicPromotion` and clears cancellation at its exact irreversible boundary.

Task 14 sealed native file selection is implemented below that application/UI boundary.
`tokenmaster-platform` uses the existing pinned Windows bindings for the Common Item
Dialog, exact `.tmconfig`/`.tmbackup`/`.tmbackup.age` filters, balanced STA COM lifetime,
and explicit user-cancel classification. Selection returns only an already open bounded
single-link no-follow input or an output capability bound to absent/existing physical
identity plus the selected parent's physical identity. Windows output retains an exact
cleanup handle, writes a bounded create-new adjacent stage, and leaves an existing target
untouched until sealed atomic publication. Existing replace captures and validates the
displaced target, rolls back post-check identity drift, and deletes old bytes only after
the new identity/bytes verify. Local/reparse/hard-link/type/extension/path/size and
selection-drift failures are stable and path-private; the deterministic selector supports
tests and unsupported hosts. The native selector is thread-affine and requires an active
owner. File-dialog 11/11, the 19-test platform unit set including five new race/recovery
contracts, full platform tests, and strict platform Clippy pass. Application binding is
now supplied by Task 15; interactive Windows evidence remains open. Independent final rereview reports Critical/
Important/Minor 0/0/0 and `Ready`. Clean-root, formatting/diff, warnings-as-errors
locked workspace Clippy, release application composition, application 38/38 and reliable-
state 55/55 authority mutations, and the complete locked workspace test/doctest suite
in approximately 473 seconds pass. Interactive behavior and release acceptance remain
unclaimed.

Task 15 now adds the bounded Data & Recovery and Settings surfaces. One latest-only
`DesktopReliableStateProjection` carries fixed health/policy/optional counters/times, at most
fifteen generation/ordinal restore points, one config preview, one operation, and one
optional path-free recovery receipt. It is kept outside the archive-backed product
snapshot so safe mode can render it with no query/controller/runtime owner. Slint owns
no path, file, SQLite, state/store/runtime/platform, provider, or recovery capability;
there is no polling timer, progress queue, or per-operation history. Restore requires a
second age/size/quality review and explicit data-only or data-plus-portable-settings
choice. Confirmation consumes the exact reviewed selection even if a newer projection
reorders the row. Unknown counts and bytes render unavailable, not zero. Passphrases
are redacted and cleared after admission. The UI exposes semantic
accessibility/high-contrast/reduced-motion and narrow/wide hooks; hot en/ru locale and
interactive Windows/accessibility evidence remain later P4/Task 17 gates.

The no-backup rebuild now exists and remains fail-closed. Only proven definitive active
corruption plus no usable reverified backup authorizes a fresh archive created through
the ordinary store schema. State fully verifies it, stages and reverifies it, records an
explicit reconstruction journal with no backup identity, preserves main/WAL/SHM in
bounded quarantine, atomically promotes, and fully verifies the active result. The app
then starts one guarded live runtime, forces a recovery-urgency refresh, and waits on the
bounded worker completion path until authoritative local Codex reconciliation is done
and no refresh remains pending. Backup maintenance becomes `Healthy` only after this
barrier. A durable banner explicitly marks quota, reset-credit, reminder, and Git
history unavailable rather than displaying fabricated zeros. Complete reconstruction
journal evidence keeps this source-reconciliation obligation across cold restart,
same-process failure, and the bounded two-launch Safe Mode; explicit retry reuses the
promoted archive instead of attempting destructive reconstruction again.

The current developer gate passes clean-root, formatting, warnings-as-errors locked
workspace Clippy, all three source audits, application 46/46, reliable-state 56/56, and
desktop 28/28 policy mutations, plus the complete locked workspace test/doctest suite
in 540.8 seconds. The authenticated live Codex contract remains the single expected
environment-gated ignored test. Independent rereview reports Critical/Important/Minor
0/0/0 and `Ready` for Task 15.

Task 16 now closes the consolidated adversarial/privacy/compatibility contour. The new
state matrix rejects every proper package prefix and every one-bit package mutation and
tests WAL/SHM add/remove/change drift. That test exposed and fixed a real partial-move
defect: pre-existing SHM drift could move WAL before returning `ArtifactMismatch`.
Platform now verifies main and both active/quarantine sidecar locations before the first
new move, rejects conflicting targets before another active child moves, resumes an
exact already-moved sidecar, and retains the per-move checks. The application gate
executes the already stronger six-state journal, process-death, settings rollback,
automatic data-only, missing-main, and periodic-disabled safety contracts directly;
private application migration tests remain source-bound without opening test-only
production APIs.

`scripts/audit-backup-package.ps1` adds a distinct package security rail: seven exact
codec files, twenty-three coverage anchors, SHA-256 identities for the 196-package exact
name/version/license and enabled-feature closures, two MIT upstream notices, forbidden
process/network/shell/generic-archive/plugin/UI/SQL authority, 247 production-source
privacy scan, synthetic-export privacy proof, and the release executable are checked.
Focused state 4/4, app aggregate 57/57, platform archive-recovery 13/13, package Pester
14/14, and combined package/reliable-state/app Pester 120/120 pass. At that checkpoint Task 17-18,
interactive/M0/package/signing/release acceptance remain unclaimed.

The pre-review complete workspace gate passed in 476.7 seconds and exposed a pre-existing
Codex timeout-test race: the fixture could be killed before writing its PID receipt. The
contract now treats that receipt as optional only on the deadline path, proves no
task-owned process remains by exact executable path, and passed eleven consecutive
focused runs. The first independent Task 16 review reported 0 Critical, 4 Important,
and 1 Minor; all four technical gaps now have RED/GREEN coverage above. The fresh locked
workspace test/doctest suite passes on the post-review tree in 604.1 seconds, strict
warnings-as-errors workspace Clippy passes, and independent rereview reports Critical/
Important/Minor 0/0/0 with `READY`. Task 16 is closed; that evidence was not Task 17-18,
interactive/M0, packaging, signing, or release acceptance.

Task 12A focused store backup 8/8 and adversarial 10/10, catalog 6/6, maintenance
19/19, state bootstrap 13/13, app 6 unit plus 7 integration, application authority
17/17, and reliable-state authority 55/55 contracts pass. The app lifecycle contract
also executes 19 real sequential manual backups and proves the retained catalog remains
within the default 15-point bound. The prior required clean-root, formatting, strict locked
workspace Clippy, and complete locked workspace test/doctest gate passes in 617.2
seconds; the one live-auth Codex transport test remains intentionally environment-
gated. The release application composition audit also passes with one production binary,
one state/maintenance/live/quota/reminder/controller/bridge owner, both migration gates,
zero polling/arbitrary-root/forbidden-string surface, and one release artifact.

Task 17 is implemented and independently rereviewed at Critical/Important/Minor 0/0/0.
The deterministic release fixture is 9,125,888 bytes at 8 MiB target with SHA-256
`a5cf28d370d3d6a38f0c3588e2a41e614c693aa4a39ee4dbfb470ac87d9f5fdd` and
101,519,360 bytes at 96 MiB target with SHA-256
`292190892f7f067405a07f686f540b42bc6eed6f21b4f153a8c0c0697ccc1b78`.
Automatic/normal/compact large-fixture throughput measured 70.96/65.03/0.612 MiB/s;
the largest sampled private growth was 51,732,480 bytes, below the fixed 64 MiB limit
and more than 16 MiB below the database. Package I/O is 64 KiB, decoder window 8 MiB,
and the only thread delta is the sampler. The 10,000-trigger and resume gates retain
one active/follow-up and one catch-up without burst.

The lifecycle gate warms backup, acquired-candidate cancellation, and restore, then
passes 256 measured backup/verify/import-cancel/retention cycles, 16 forced candidate
cancel/recovery cycles, and 16 complete isolated restores. It retains 15 points and
returns to the exact filled per-run disk plateau on every cycle. Private memory returned
from 4,194,304 to 8,261,632 bytes, handles 151 to 152, threads 5 to 3, USER 2 to 2,
GDI 0 to 0, and child processes remained zero; encrypted compact high water was
75,558,912 bytes and returned to that same original envelope. One exact 96 MiB backup
cycle spans the loaded UI window: cached Dashboard query p95 delta was 0.0326 ms and
software-paint delta 0 ms against the 10 ms limits. Task 18 binds these gates through
`P3D0_ACCEPTANCE.md`; the ignored clean-commit receipt remains developer evidence only.
Fresh Task 18 clean-root/reliable-state/package/application/Desktop audits, formatting,
strict workspace Clippy, the complete locked workspace test/doctest gate in 665.4
seconds, and the GNU release application build pass. The only new ordinary-suite
behavior is an explicit debug skip for the release-only UI measurement; its exact
release target passes independently.

P3-D.7 Help/About replaces the final P3-D data-independent placeholder with one
archive-independent responsive guide. Six fixed accessible sections explain
navigation, source/evidence truth, privacy, recovery, current automation availability,
and licenses. `DesktopShell` applies the compile-time Cargo package version once; the
route owns no projection, model, query, runtime, diagnostics, callback, timer, queue,
cache, polling, or TokenMaster URL/browser surface. Exactly one pinned standard
`AboutSlint` supplies the selected in-product attribution. The compiled Slint contract,
source/release audits, full Desktop package, strict package Clippy, and 104/104 mutation
cases pass. Independent final review returned Critical/Important/Minor 0/0/0. The first
full baseline exposed a pre-existing bimodal warm-up false plateau in the product
private-byte resource test; its exact 16-sample regression now forces continued warm-
up without widening the 2 MiB return tolerance, passes in three independent processes,
and received a separate 0/0/0 review. The subsequent exact clean-root/fmt/strict
workspace Clippy/locked workspace test-doctest baseline passes in 879.3 seconds.
Unified en/ru/pseudo locale switching remains P4, CLI/MCP remains P5, and generated
notices/SBOM/MSVC/package/signing/public-download/release evidence remains P6.

## Next implementation slice

P3-D.0 Reliable State, P3-D.1 History, P3-D.2 bounded Sessions list/detail, P3-D.3
Models, P3-D.4 Projects, P3-D.5 Recent activity, and the bounded read-only P3-D.6
Notifications route plus P3-D.7 Help/About, the app-owned visible presentation/receipt
bridge, global reminder settings synchronization/editor, P3-E.1 route command palette,
P3-E.2 compact quota mode, P3-E.3 production tray lifecycle, and P3-E.4 current-session
single-instance activation/global hotkey are implemented. Continue with opt-in
current-user startup and remaining shell closure.
Later-page Sessions navigation and interactive History ranges remain bounded
replacements of their existing sections rather than new frontend query owners. Interactive
Windows, P4 presentation, P5 CLI/MCP, activation, M0, packaging, signing, soak, and
product release remain unclaimed.

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
workspace quality gate. P2-E, P2-F, P3-A, P3-B.1, P3-B.2, P3-B.3, and P3-C are
complete; P3-D.0 Reliable State is complete through Tasks 1-18, followed by P3-D
supporting data-bearing routes. Activation
remains a later independently authorized capability. No quota value may be inferred
from local token/cost facts and no browser/private-endpoint authority may be added.

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
`docs/superpowers/plans/2026-07-15-tokenmaster-banked-reset-inventory.md`. Official
Codex reset-credit discovery, durable in-app reminder events, and joined product status
are implemented; visible P3 notification rendering, OS delivery, assisted activation,
and automatic activation are not implemented.
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
`UsageReadStore` opens schema v13 read-only without migration, enforces defensive
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
matches. Dashboard rendering is now complete in P3-C; reminder OS/tray delivery,
snooze, quiet hours, and activation remain later.
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

The clean-root audit, required Pester contract files, root format check, strict
Clippy with `RUSTFLAGS=-Dwarnings`, full locked Rust workspace tests, release build,
and M0 developer stress verification pass from the root workspace. The exact commands
are recorded in `docs/HANDOFF.md` and the M0 script; this does not replace external
acceptance evidence.
