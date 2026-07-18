# Changelog

All notable changes are recorded here.

## Unreleased

### Added

- Implemented P3-D.0 Task 12B.2b.1 bounded operation execution. The production app now
  owns one standard-library operation thread over the sole command coordinator, one
  capacity-one wake, active plus one follow-up, and one latest-only completion. Work
  runs outside the mutex; exact cancel, irreversible state, retry, caught-panic fault,
  explicit shutdown, and `Drop` all retain fixed path-private outcomes and joined
  ownership. Manual backup is the first real binding and executes through the existing
  atomic maintenance receipt wait off the Slint thread.
- Added sealed application config export/import operations. `.tmconfig` now has a
  separate 2 MiB encoded reader/writer ceiling. Export stages portable settings into an
  already controlled create-new target, crosses irreversible state before publication,
  then reopens and fully verifies it. Import fully verifies an already open reader,
  retains one bounded category/count/base-identity preview, and commits that exact
  candidate while preserving device-local settings. Native file dialogs and UI
  preview/confirm binding remain open and no UI path authority is claimed.
- Added nine operation-worker contracts, two app config lifecycle contracts, a package
  fail-fast bound contract, real application manual-backup command coverage, and seven
  new source-policy mutations covering duplicate/unbounded worker ownership, lost
  binding, detached shutdown, and sealed/bounded config capabilities.
  Task 12B.2b still owns native-file/UI config binding, verify/selected-restore/rebuild
  execution, complete cancellation propagation, and no-backup reconstruction.
- Closed the Task 12B.2b.1 developer gate with clean-root, formatting/diff, warnings-as-
  errors locked workspace Clippy, the complete locked workspace test/doctest suite in
  502.1 seconds, the release composition audit, reliable-state 55/55 plus application
  38/38 authority mutations, and an independent Critical/Important/Minor 0/0/0 review.
  The authenticated live Codex contract remains intentionally ignored without its
  explicit environment binding; product and release acceptance remain unclaimed.

- Implemented P3-D.0 Task 12B.2a selected restore composition. A UI generation/ordinal
  is sealed into one opaque verified package identity before admission closes; the
  selected point is held by one RAII pin shared with every post-publication deletion,
  including cycles admitted before selection, while a verified `PreRestore` package is
  published. Every old bundle owner then joins, one fresh fixed archive guard enters
  journaled restore, and the returned recovery receipt is durably bound to the current
  run before any replacement lifecycle work.
- Added restored-archive schema routing. Current archives rebuild one fresh bundle;
  supported legacy archives repeat mandatory verified `PreMigration` and
  `PostMigration` gates with the durable pending obligation before returning live.
  Clean shutdown accepts the exact recovery generation, so the completed journal is
  not replayed on the next start. The shared bounded catalog is published as immutable
  `Arc` snapshots, keeping heavy snapshot/verification/recovery work outside its mutex.
- Added catalog-reordering/byte-replacement/directory-drift, late-pin retention, current/legacy
  selected-restore, stale-selection, shutdown, recovery-replay, and source-policy
  regressions. Task 12B.2b still owns the operation worker, UI/native-file binding,
  config import/export, verify/rebuild, cancellation propagation, and authoritative
  no-backup reconstruction; this entry does not claim those or release acceptance.
- Closed the Task 12B.2a developer gate with catalog 7/7, retention 3/3, app 14 unit
  plus 7 integration, application policy 31/31, reliable-state authority 55/55,
  clean-root, formatting, warnings-as-errors locked workspace Clippy, the complete
  locked workspace test/doctest gate in 507.5 seconds, release composition audits, and
  an independent Critical/Important/Minor 0/0/0 review. The live-auth Codex executable
  contract remains intentionally ignored when its explicit environment binding is absent.

- Began P3-D.0 Task 12B with a path-free, constant-state application command
  coordinator. Typed config/backup/verify/restore/rebuild intents retain at most one
  active operation and one follow-up, coalesce 10,000 identical hints, reject a third
  distinct request busy, support exact queued/active cancellation, and stop
  cancellation at one irreversible boundary. Actual operation-worker and native-file
  bindings remain Task 12B.2.
- Added controlled current-bundle restart without reconstructing the Slint window. It
  pauses command admission, discards only the queued follow-up, joins every old owner,
  acquires a fresh archive lease, starts one new bundle generation, and resumes
  admission. Completion notifiers are generation-bound under the bundle mutex, so an
  obsolete notifier cannot publish into the replacement bundle.
- Verified Task 12B.1 with 7/7 command contracts, 14 app unit plus 7 app integration
  contracts, 23/23 application authority contracts, the complete clean-root/format/
  strict-Clippy/workspace-test gate, release application composition audit, reliable-
  state audit, 55/55 reliable-state authority mutations, and independent follow-up
  review with no findings. This does not claim product acceptance or release readiness.

- Implemented P3-D.0 Task 12A application reliability composition. One state owner runs
  before live/query/controller construction; safe mode owns no archive user; a healthy
  bundle owns one capacity-one maintenance runtime and a complete online-snapshot ->
  verify -> typed-package -> verify -> publish -> catalog-bind -> retain operation.
- Added mandatory verified pre/post-migration safety points that remain active when
  periodic backup is disabled. Redundant run-state schema v2 accepts legacy v1, records
  a pending source/target pair before writable migration, survives a committed-
  migration/post-backup failure, and forces restart to complete the post point before
  live or clean. Pre-open failure still preserves the old archive and pinned package.
- Added atomic deadline-bounded exact-root maintenance receipt waiting without polling or a new
  thread, exact verified-package catalog binding without leaking a platform token, and
  first-install WAL snapshot support while standalone candidates remain strict and
  completely verified. Cold catalog verification stays on the worker, verified proofs
  survive unchanged rebuilds, and 19 real sequential backups remain within the default
  15-point retention bound. Typed restore/restart/rebuild commands remain Task 12B.
- Verified Task 12A with focused store/state/app contracts, 17/17 application and 55/55
  reliable-state authority mutations, the complete clean-root/format/strict-Clippy/
  workspace-test gate, and the release application composition audit. This does not
  claim product acceptance or release readiness.

- Implemented P3-D.0 Task 11A pre-open state bootstrap. Strict A/B run records publish
  and reread `unclean` before catalog, package, or SQLite access; an exactly clean prior
  run uses bounded normal read-only inspection, while unclean/missing/invalid state adds
  `quick_check(100)`. Pending recovery resumes first, supported legacy/newer schemas do
  not migrate, and first install is distinguished from a missing damaged main by
  bounded owned evidence.
- Added newest-first fully reverified corruption-only automatic data-only recovery,
  corrupt-newer candidate skipping, exact recovery-generation acceptance, and a
  two-unclean-launch bound before safe mode. No valid backup and every non-corruption
  result preserve existing evidence. Zero-length SQLite WAL/SHM sidecars are retained
  as valid exact facts while a zero-length main remains invalid.
- Added root/capability rebinding before mutation and continuous startup-guard handoff
  into `LiveRuntime`. A real integration contract carries one first-install guard
  through archive open/startup recovery, then separately proves joined shutdown before
  clean publication; legacy runtime
  starts retain their behavior. Focused platform/store/state/runtime contracts and the
  strict locked workspace Clippy, reliable-state audit, and 55/55 authority mutations
  pass; the complete locked workspace test/doctest suite passes in 571.4 seconds.
  The changed platform capability passes the explicit `x86_64-pc-windows-msvc`
  warnings-as-errors target check.
  Application-owned migration points,
  no-backup authoritative reconstruction, safe mode, and final all-owner lifecycle are
  explicitly Task 12.

- Implemented P3-D.0 Task 10 durable restore through a sealed three-layer boundary.
  Platform binds the exact archive lease to fixed main/WAL/SHM, recovery staging, and
  three never-auto-deleted quarantine sets; store copies a path-free reader into its
  bounded namespace and applies the complete SQLite/schema/FK/semantic verifier; state
  owns only typed orchestration and the redundant six-phase journal.
- Added restart-idempotent old-or-new replacement and rollback, including explicit
  process-death coverage after sidecar quarantine, main promotion, and portable-
  settings commit but before the corresponding journal advance. Manual data-only,
  manual data-plus-portable-settings, and corruption-only automatic data-only restore
  preserve device-local settings; portable settings commit by an exact prepared
  generation/digest and cannot publish a duplicate generation after restart.
- Bounded the complete recovery staging namespace to three exact artifacts across
  both platform and store allocators. The actual-free-space preflight covers the
  larger of candidate expansion plus candidate verification (`2B`) and candidate
  expansion plus active-corruption verification (`B+A`), plus an 8 MiB reserve.
  Recognized abandoned staging is discarded only when an absent or completed journal
  proves that no restore is pending. Unknown names, links/reparse
  points, multiple links, invalid dual journals, stale package/candidate/active facts,
  wrong leases, and ambiguous forward/rollback state fail closed with evidence kept.
  Focused platform/store/journal/restore contracts, strict Clippy, the reliable-state
  audit, and 52/52 authority mutations pass; safe-mode/app composition and UI remain
  later Tasks 11-13.
- Closed independent review findings: recovery now binds the lease to the physical
  lock file, reserves operation IDs with create-new markers, persists the fixed backup
  slot rather than a process-local catalog generation, permits later independent
  journal generations, proves corruption internally, rolls forward an already-
  published settings target, and classifies native replacement failures only from
  exact old/new namespace facts. Kill tests cover store-verifier files, first journal
  publication, settings publication, and every durable recovery phase.
- Recovery now revalidates the physical writer guard before any store or platform
  staging cleanup. Wrong-archive authority preserves all pre-journal evidence, and a
  live peak contract proves both verifier windows contain exactly three artifacts.
  Final independent rereview reports Critical 0, Important 0, Minor 0 and `Ready`.
  The final Task 10 baseline passes clean-root in 14.899 seconds, formatting in 1.396
  seconds, strict locked workspace Clippy in 9.169 seconds, and the complete locked
  workspace test/doctest suite in 545.3 seconds. The reliable-state audit and 52/52
  mutations pass, and the changed platform boundary passes an MSVC target check.

- Implemented P3-D.0 Task 9 capacity-one backup maintenance. The native runtime owns
  exactly one worker and one scheduler/shared timer, one active request, one urgency-
  merged follow-up, one latest completion, and one latest mandatory-guard completion.
  Ten thousand hints remain constant state; pause/resume/shutdown/Drop join all owned
  work and a Windows 12-warm-up/24-measured success/failure/cancel gate returns thread,
  handle, private-memory, USER, and GDI topology to its post-warm-up envelope.
- Added scalar automatic scheduling: `Healthy` restart truth seeds a new monotonic
  interval anchor while `HealthyUnpublished` stays closed; the default is five-minute
  quiet plus six-hour ordinary minimum with one resume/clock-rollback catch-up.
  Periodic disablement drops an already merged periodic-origin follow-up but cannot
  disable pre-migration, pre-restore, pre-destructive guards, or an owned internal
  retry. Retry is internal urgency, not a submit purpose; it keeps the original root
  request and package purpose, and two same-identity failures enter `Suspect`.
- Linked `MaintenancePermit` cancellation directly to store `BackupControl` without
  exposing raw atomics, made final publication non-cancellable, and reject `Published`
  before or `Cancelled` after that state boundary. Added the bounded
  path-free `VerifiedBackupCandidateReader` and sole typed state/store package bridge;
  replacement, truncation, append, cancellation, codec/output, or seal failure rechecks
  identity/length/SHA-256 and irreversibly poisons the unpublished stage.
- Added backward-compatible backup purpose value 6 for pre-destructive maintenance,
  leaving the first five v1 values unchanged. Added 17 maintenance contracts, one
  Windows resource contract, two verified-reader store contracts, and success plus
  three-mutation state/store composition coverage. Focused strict state Clippy and the
  reliable-state source/workspace authority audit and 47 mutation tests pass. The
  post-fix independent rereview reports Critical 0, Important 0, Minor 0 and `Ready`;
  the final baseline passes clean-root in 14.940 seconds, formatting in 1.256 seconds,
  strict locked workspace Clippy in 12.340 seconds, and the complete locked workspace
  test/doctest suite in 507.6 seconds.

- Implemented P3-D.0 Task 8 sealed backup catalog and bounded retention. Platform owns
  the canonical local `backups` directory, exactly 32 private package slots, opaque
  physical entry/generation tokens, bounded staging/read/publication, link/reparse/
  hardlink/duplicate-identity rejection, and write-through one-file tombstone deletion.
- Added the exact production proof chain: typed package write into a sealed unpublished
  slot, full path-free package verification, no-delete retention admission, directory-
  owned publication with seal recheck, catalog rebuild/bind, and exact confirmation.
  Cold catalog rows remain `HeaderValid`/`Corrupt`; only current complete proof becomes
  `Verified`.
- Added deterministic protected retention under 32 files, 15 verified points, and the
  checked 256 MiB-through-64 GiB byte range: candidate/newest-two/pre-migration
  protection plus shared four-newest/seven-UTC-day/four-ISO-week tiers. Every deletion
  revalidates the complete current verified set and exact target, deletes at most one
  oldest unprotected point, then requires rebuild/replan. Same-length corruption of
  candidate, target, or another protected point preserves all files.
- Added four catalog, two retention, five backup-directory, deletion-boundary, mixed-
  failure precedence, stage cleanup/poison, namespace/privacy, and stale-proof
  regressions. The reliable-state source audit passes and its mutation suite is now
  42/42, including exact typed stage writer/verifier allowlists and sealed stage-method
  enforcement. Independent third review is Critical 0, Important 0, Minor 0,
  `Ready: Yes`; the complete locked workspace baseline passes in 566.3 seconds.
- Implemented P3-D.0 Task 7 optional manual age v1 backup protection. Pinned
  `age = 0.12.1` with default features disabled and no CLI/plugin/SSH/armor/async/
  unstable/web feature. Manual export uses the standard scrypt recipient with fixed
  `log_n = 16`; import caps accepted work at 16 before derivation. Automatic encryption
  is explicitly rejected and automatic recovery stores no secret.
- Bound encryption to an opaque `VerifiedBackupPackage` and recheck its exact length
  and complete-file SHA-256 in the encryption pass. Added non-cloneable redacted
  zeroizing passphrases, exact 12-through-128 Unicode-scalar confirmation with no trim
  or normalization, immediate caller-buffer clearing, sealed output receipts, direct
  authenticated-inner-package verification, and irreversible ciphertext/database
  stage discard on every failure.
- Added seven grouped encryption contracts covering standard age round-trip, fixed and
  malicious work factors, wrong password, header/MAC/body/final-tag corruption,
  truncation/trailing data, authenticated non-package plaintext, destination capacity,
  cleanup failure, changed and same-length-substituted sources, automatic-mode
  rejection, stage poison/removal, passphrase boundaries/redaction, and typed inner
  package validity. The generic inner parser remains fully private behind a typed
  authenticated-payload bridge. Reliable-state authority mutations pass 37/37 and
  final independent security rereview is Critical 0, Important 0, Minor 0,
  `Ready: Yes`.
- Implemented P3-D.0 Task 6 fixed typed `.tmconfig`/`.tmbackup` v1 packages. The
  deterministic header/manifest/settings/database/footer grammar has exact checked
  little-endian fields, expanded entry SHA-256, descriptor binding, preceding-byte
  package SHA-256, and an independently sealed complete-file receipt; it is not a
  generic archive or extractor.
- Pinned `zstd` 0.13.3 with default features disabled. Each entry is one checksummed,
  content-sized frame at level 6/12/19, one thread, an 8 MiB decoder window, fixed
  64 KiB buffers, and an independent expanded-byte counter. Bounds are 1 MiB settings,
  64 GiB database, eight entries/64 KiB manifest at the version boundary, and checked
  64 GiB-plus-2-MiB total/encoded ceilings.
- Added platform-owned bounded durable readers with early-EOF/appended-byte detection
  and irreversible staged-file discard/poison. Public package APIs accept only these
  capabilities; every codec/final-seal failure prevents later write, seal, and publish.
  The authority audit also rejects future public generic stream methods.
- Added a frozen 405-byte config vector, all three profiles across the then-five backup
  purposes, 24 MiB streaming coverage, structural flips/truncation, false/overflowing
  lengths, unknown/duplicate/concatenated/trailing frames, resealed missing-end,
  checksum/digest, 16 MiB-window, 300-to-256 bomb, privacy, late-footer, and partial-
  writer regressions. Package 5/5, adversarial 10/10, durable-file 17/17, and authority
  mutations 36/36 pass; final independent review is Critical 0, Important 0, Minor 0,
  `Ready: Yes`.
- Implemented P3-D.0 Task 5 store-owned SQLite Online Backup primitives. Page-stepped
  snapshots include committed WAL truth, use fixed create-new staging names, bound
  busy retry/cancellation/deadline/output size, and never treat a copied live main
  file as a backup.
- Added defensive standalone candidate verification with exact bundled SQLite
  identity, query-only/defensive/trusted-schema/DQS/cell-size/mmap/cache policy,
  explicit SQLite value/SQL/column limits, integrity and foreign-key checks, exact
  schema/index validation, stored count/generation checks, and application semantic
  validation. Old supported versions are inspected without migration; newer versions
  remain non-mutating typed failures.
- Added verified `VACUUM INTO` compaction from isolated candidates, physical-file plus
  length/SHA-256 binding before and after every consumer, stale-path rejection,
  explicit discard, bounded cleanup health, and fixed-name abandoned-candidate
  recovery. Five backup, ten adversarial, and one page-step barrier contract pass;
  final independent review reports Critical 0, Important 0, Minor 0.
- Implemented P3-D.0 Task 4 typed version-1 settings over the private redundant-record
  core. The exact schema persists only a canonical bounded in-app reminder default,
  validated automatic-backup enabled/quiet/interval/retention policy, and one device-
  local route; future skins/locales/OS notifications/pricing/providers and forbidden
  private state are not placeholders in v1.
- Added fixed-purpose `SettingsStore` current/fallback/default load outcomes, strict
  migration/version/unknown/range/relationship gates, explicit two-invalid-slot save,
  portable-only category/count preview, stale confirmation rejection, idempotent
  commit, and device-local preservation. Successful publication returns a nonzero
  generation/portable-digest target that can be reconstructed and reread-verified.
- Added ten settings contracts covering exact JSON, 1 MiB cap, bounded reminder decode,
  duplicate rejection, safe defaults without evidence mutation, corrupt-newest
  fallback, valid-envelope newer-schema write protection, unsupported/malformed
  imports, generation overflow, staging collision, privacy canaries, portable-only
  backup candidates, and target verification. The authority audit still permits only
  six literal record children, limits the directory capability to the exact typed
  constructor, and passes all 34 mutation cases.
- Implemented P3-D.0 Task 3 crate-private redundant records: six fixed settings/run/
  recovery A/B children, a versioned 64-byte header and 40-byte footer, 1 MiB strict-
  JSON cap, checked generation, payload/record SHA-256, highest-valid selection,
  corrupt-newest fallback, equal-generation conflict detection, and explicit
  `NoValidRecord` without destructive repair.
- Added a bounded two-pass record writer that measures/hashes without retaining the
  encoded payload, streams the second pass in at most 256 KiB calls, rejects
  nondeterministic serialization, publishes only a sealed inactive slot, rereads both
  records, and maps every post-publication uncertainty to `RecoveryRequired`.
- Added caller-bounded exact-child platform reads and inactive-slot replacement with
  no third backup. Record evidence covers 13 unit contracts and generation-3 process
  death during partial write, after seal/before publish, and after publish/before
  reread. Platform evidence adds an injected before/after redundant replacement
  boundary, 40 deterministic kills, and 20 replacement-entry races.
- Strengthened the reliable-state authority audit to keep generic record/file authority
  crate-private, permit only six literal children and bounded `io` result/error uses,
  and reject approved-alias reuse. All 33 Pester mutations, strict state/platform
  Clippy, focused suites, workspace authority audit, and independent final review pass.
- Implemented P3-D.0 Task 2 controlled durable files in `tokenmaster-platform`:
  restricted exact-child targets, 32-slot create-new staging, 64 GiB plus 2 MiB file
  and 256 KiB call bounds, streaming length/SHA-256 receipts, flush/close/reopen, and
  path-private fixed failures.
- Added Windows same-volume `MoveFileExW(MOVEFILE_WRITE_THROUGH)` publication without
  copy fallback and `ReplaceFileW` replacement with independently verified exact old-
  target backup. Post-publication hook/sync/verification uncertainty is always
  `RecoveryRequired`; failed rollback preserves staged and backup artifacts.
- Added Unix no-overwrite hard-link publication and exact-backup atomic replacement,
  deterministic partial-write/recovery regressions, 40 handshake-controlled pre/post
  child kills, and 20 replacement-entry race kills. Focused strict Clippy, 9 library
  tests, 11 durable integration tests, and final independent review pass.
- Implemented P3-D.0 Task 1: the library-only `tokenmaster-state` workspace package,
  nine stable path-private error categories, checked byte/item limits, and a
  deterministic Rust/Pester authority contract. The audit pins five direct
  dependencies and rejects bin/build targets, process/network/shell/SQL/Slint/archive
  authority, direct filesystem/path authority, source inclusion, public arbitrary-path
  constructors, false workspace membership, standard-library/platform aliases and
  re-exports, declarative macros, and forbidden transitive packages. Its 29 mutation
  cases include the independent-review bypass corpus.
- Corrected the future recovery crash fixture to reuse an integration-test executable
  instead of introducing a `tokenmaster-state` binary target.
- Approved P3-D.0 Reliable State design and 18-task TDD rail covering redundant typed
  settings, strict `.tmconfig`/`.tmbackup` import/export, SQLite Online Backup,
  streaming Zstandard, optional bounded age protection for manual exports,
  self-describing retention, and Data & Recovery/safe-mode UI.
- Frozen corruption containment around the existing fixed archive/writer identity:
  complete main/WAL/SHM quarantine, redundant six-state idempotent restore journal,
  distinct existing/missing/new-install paths, Windows atomic replacement/rollback,
  newest-first revalidation, crash-loop stop, and explicit authoritative-source
  rebuild when no backup is usable.
- Defined manual data-only versus data-plus-portable-settings restore, data-only
  automatic recovery, settings-publication rollback/resume, mandatory safety points
  independent of the periodic schedule, exact 256 MiB-through-64 GiB retention budget,
  and a whole-package footer digest. Device-local settings are never restored.
- Added planned TM-FUNC-012, TM-PERF-004, TM-DATA-011, TM-SEC-008, and ADR-054
  contracts. These planning entries do not claim that backup or recovery behavior is
  implemented.

- P3-C explicit all-current quota and benefit overview contracts with exact-empty
  semantics, one-revision capture, 32-window/32-scope/256-lot hard bounds, immutable
  public envelopes, and section-local product/controller publication.
- Pure identity-free `DesktopDashboardProjection` with six ordered sections and hard
  caps of 32 quota rows, 32 benefit summaries, 240 trend points, 12 sessions, eight
  activity categories, 12 models, and checked Git aggregation over 32 repositories.
- Responsive semantic Slint Dashboard rendering real today, Plan Usage/banked reset,
  Code Output, trend, session, activity, and model truth. Missing values remain
  unavailable; narrow/wide navigation preserves the window and Dashboard models.
- Route-only UI application path so navigation no longer rebuilds seven Dashboard
  list models. After initial construction, accepted newer snapshots replace each
  bounded model once; the UI adds no
  timer, animation, polling, query/SQL/runtime authority, or seeded metrics.
- Twenty adversarial desktop audit contracts covering empty-filter discovery drift,
  fixed quota rows, seeded values, private IDs, UI authority/polling/animation,
  presentation bounds, route-triggered rebuilds, worker/slot/event duplication,
  diagnostic renderer/probe drift, and the seven-Rust/nine-Slint source boundary.

- P3-B.3 `tokenmaster-app` composition package as the sole owner of
  `TokenMaster.exe`; `tokenmaster-desktop` is now a six-Rust-file library-only
  frontend with no new platform/runtime/store/provider authority.
- Deterministic installed/portable data-root policy: an exact empty
  `tokenmaster.portable` marker selects adjacent `data`, absence selects
  `%LOCALAPPDATA%\TokenMaster`, and invalid intent fails without fallback or path
  disclosure.
- Optional engine completion notifier propagated through usage, nested Git, quota,
  and reminder runtimes, with receipt-before-hint ordering, lock-free callback entry,
  panic isolation, no dispatcher/timer/queue, and legacy constructor compatibility.
- One generation-ordered desktop runtime-health observation slot joining four copied
  product health/error values on the existing query worker; 10,000 replacements and
  active-query race/coalescing contracts pass.
- Application composition/data-root/live-bundle tests plus 21 adversarial desktop/app
  Pester cases and release audits proving one binary, one owner per runtime/controller/
  bridge, software-only rendering, zero polling/arbitrary-root surfaces, and zero
  forbidden private/old-project binary strings.
- Deterministic Git process regression for a deadline reached before the fixture can
  create its PID receipt. Cleanup tests now inspect the exact executable path as well
  as any published PIDs, so a valid kill-and-reap cannot flake under workspace load.

- P3-B.2 capacity-one Slint event-loop bridge sharing the controller's sole latest
  snapshot mailbox, with one idle-only weak notifier, one atomic scheduled gate, one
  weak window, newest-generation application, and a post-drain race recheck.
- Stable fixed bridge health for scheduled/coalesced/delivered/ignored/failure counts
  and last generation/failure, with retryable event-loop unavailability and explicit
  terminated/window/state lifecycle truth.
- Six bridge unit/race contracts, eight controller contracts including populated-
  idle attachment wakeup, and a real headless Slint integration event loop.
- Expanded desktop audit with 12 adversarial Pester contracts and exact invariants:
  the then-seven Rust/five Slint files, one worker, one shared result slot, one
  event-loop site, zero bridge polling, and no strong window retention or direct
  authority.

- P3-B.1 bounded desktop controller over one proven refresh worker, one typed query
  source, one worker-confined product reducer, and one replaceable latest immutable
  snapshot; Slint callbacks remain non-blocking and query-free.
- Typed refresh urgency, admission, receipt/attempt, completion, cancellation, and
  stable error contracts. One thousand hints collapse into one follow-up, and
  cancellation/deadline termination discards partial visible publication.
- Real empty schema-v13 controller integration, sibling query-fault isolation,
  deterministic shutdown/post-close rejection, and archive-path redaction tests.
- Expanded desktop audit with eight adversarial Pester contracts, explicit one-worker/
  one-slot/UI-query checks, five allowed production dependencies, and zero forbidden
  source or release-binary matches across six Rust and five Slint files.

- P3-A separate production `tokenmaster-desktop` package with a package-specific
  software-only Slint graph; the M0 probe remains an evidence artifact and is not a
  runtime or source dependency.
- One immutable bounded desktop projection of all 11 product routes, stable route/
  label/reason codes, one retained selection/model, and equal/older generation
  rejection before Slint replacement.
- Original compiled `TokenMaster` header/navigation/state shell driven by the real
  initial product snapshot with no seeded quota, session, chart, cost, or reset data.
- Desktop source/release audit plus the original six adversarial Pester contracts covering probe
  dependencies, mock data, FemtoVG, route drift, direct authority, and forbidden
  filesystem/network/process/SQL/browser/credential surfaces. The release audit
  reported five Rust and five Slint files at P3-A closure, one model, 11 routes/reasons maximum, and
  zero forbidden dependency/source/private-canary matches.

- P2-F joined product status: one exact defensive schema-v13 scalar transaction binds
  usage publication/aggregate progress with independent quota, benefit, and Git state,
  then maps it into a bounded schema-v1 envelope without consuming a generation on
  failure.
- New leaf `tokenmaster-product` reducer with one current immutable snapshot, separate
  checked attempt/source/runtime generations, compatible last-good retention,
  incompatible-identity invalidation, and stale asynchronous result rejection.
- Eleven fixed route-readiness projections with a `u16` reason set. Aggregate rebuild
  keeps Activity and Data Health reachable, degrades Dashboard section by section, and
  makes only History, Sessions, Models, and Projects unavailable.
- Count-only product runtime health for usage, quota/benefit, reminder, and Git owners;
  the product layer retains no runtime, worker, callback, lease, path, identity, queue,
  SQLite handle, or snapshot history.
- Product-status acceptance: a 100,000-event/40-sample status p95 of 0.125 ms, 10,000
  reducer replacements, 1,152 isolated open/capture/drop cycles with stable handles,
  threads, USER/GDI objects and bounded private-memory return, plus a zero-match
  product authority/privacy audit.

- Bounded Git runtime publication with one constant-state scheduler/worker, at most 32
  latest transient candidates, one active scan/follow-up, unchanged zero-history path,
  ancestry-proven same-process append, rewrite/recovery rebuild, stale-sequence
  rejection, and Git-I/O-before-nonwaiting-lease/SQLite ordering.
- Exact Git lifecycle and failure truth: missing-author and other identifiable scan
  failures publish durable unavailable/rebuild-required state without erasing the last
  trustworthy generation; pause cancels/reaps the exact child and drops raw object-ID
  frontiers; resume forces rediscovery; shutdown and `Drop` join owned work.
- Git runtime acceptance for 32-candidate eviction, sibling fault isolation, writer
  contention, stale-result follow-up, Codex live side-channel routing, pause/resume,
  child cleanup, and 16 warm-up plus 48 measured Windows rounds at a 3,293,184-byte
  private floor, 6,422,528-byte sampled high, 118 handles, four threads, USER=1, and
  GDI=0.
- Git production authority audit across four packages, 126 production dependencies,
  19 production boundary files, and four release libraries, with zero forbidden
  dependency, foreign-language, network/browser/credential/shell/direct-SQL/mutation,
  vendored-upstream, or private binary-string matches.
- Immutable schema-v1 public Git output envelopes with checked process-local
  generation, independent Git publication revision, explicit UTC half-open ranges,
  owned all-time/range/category/day values, freshness/quality/retention truth, and
  32+1 repository lookahead.
- Exact private Git-to-usage project joining through a domain-separated installation-
  salted fingerprint and fixed store-owned 32-key/256-candidate matcher. The public
  snapshot receives only a matched safe alias, never the salt or opaque project key.
- Fixed-point cost per 100 added product-code lines only for exact complete non-stale
  UTC evidence, exact non-conflicting cost, and a nonzero denominator. Ambiguity,
  partial retention, stale/unavailable/corrupt usage, deadline, unknown cost, and zero
  lines remain typed unavailable; usage failure does not suppress independent Git
  facts.
- Git query acceptance for aggregate-only usage reads, no raw event scan, one shared
  two-second budget, failed-generation neutrality, restart/publication snapshot
  isolation, corruption rejection, 32 repositories by 400 days, repeated transaction
  return, and Windows handle stability.
- Strict SQLite schema v13 Git projection with a random installation salt,
  independent monotonic publication state, 32 opaque repositories, 4,096 activity
  associations, immutable rebuild/append aggregate generations, exact rollback-safe
  v12 migration, unchanged refresh, rebuild-required invalidation, and no durable
  path/ref/commit/file/command/raw-output values.
- Bounded defensive Git store capture with owned all-time/range totals, eight
  categories, latest-400-day retention, explicit `daily_history_truncated` boundary
  and range completeness, exact project-association clearing/ambiguity, 32+1
  repository lookahead, hard maximum two-second deadline, and progress cleanup.
- Store-owned durable in-app reminder processing: one immediate transaction first
  replays at most 256 unacknowledged immutable outbox rows or examines at most 256
  indexed due rows, drains expired entries, selects one most-urgent useful threshold
  per lot/channel, commits outbox before returning the event, updates exact counts/
  nearest due, and suppresses equal/less-urgent missed thresholds while preserving
  future more-urgent thresholds.
- Strict schema-v12 immutable reminder acknowledgement: presentation leases without
  claiming display, release retries a failed presenter, pre-ack crash/restart replays
  the outbox, post-ack restart deduplicates, exact v11 receipts migrate as already
  acknowledged, and retention cannot remove unacknowledged events.
- Isolated `BenefitReminderRuntime` with one scheduler, one bounded worker, one nearest
  wall-clock timer, capacity-one coalesced hints, one latest count-only health
  snapshot, one at-most-256 ready/leased notification batch with backpressure,
  startup/resume/hibernation/clock recovery, 60-second transient retry, joined
  shutdown, and thread-local scheduler panic redaction.
- Durable reminder acceptance for pre-ack restart replay, post-ack deduplication,
  release/retry, acknowledgement contention, 10,000 hints, profile rebuild, future
  urgent threshold, expired drain, outbox-before-event ordering, contention-before-
  SQLite, live-usage fault isolation, pause/resume, and deterministic nearest/retry
  scheduling. The 16+48 Windows resource gate returned at a 3,440,640-byte private
  floor, 5,799,936-byte sampled high, 117 handles, four threads, USER=1, and GDI=0.
- Benefit release-authority audit across four production packages, 125 production
  dependencies, four reminder source files, and four release libraries, with zero
  forbidden dependency, foreign-language, network/browser/credential/shell/direct-SQL,
  or private binary-string matches.
- Benefit contour closure with clean-root, formatting, warnings-as-errors locked
  workspace Clippy, complete locked workspace tests/doctests, specialized authority
  audit, complete-diff/dependency/language review, and task-owned process return all
  passing. This does not claim visible notification rendering or activation.
- One-poll Codex quota/benefit runtime publication: provider I/O still completes before
  one non-waiting writer-lease attempt and one store open; at most 32 quota windows
  and one optional benefit observation publish through separate exact transactions
  under the same guard, preserving committed sibling facts without claiming cross-
  domain atomicity.
- Separate validated runtime health for quota and benefits, including observed/
  processed/exact status/failure counts, benefit lot-change and pending-due counts,
  common versus domain failure stages, overall and per-domain last-success times,
  accelerated benefit-contention retry, inconsistent-report fail-closed behavior, and
  restart-idempotent duplicate publication.
- Combined quota-benefit Windows runtime acceptance with real reset-credit fixture:
  16 warm-up plus 48 measured success/RPC/timeout/contention/pause-resume rounds,
  3,432,448-byte private floor, 6,139,904-byte sampled high, 131 handles, four threads,
  USER=1, GDI=0, and no task-owned child process remaining.
- Immutable benefit query envelopes with independent benefit revision, one-transaction
  current/history capture, 64-lot conservative FEFO order, explicit absent/fresh/
  aging/stale and complete/quantity-partial/partial/unknown facts, inherited/override
  profile metadata, nearest conservative expiry/durable due, truthful `in_app_only`
  coverage, redacted scope/revision-bound 256+1 history continuation, and failed-call
  snapshot-generation neutrality.
- Benefit query acceptance at 64 current lots and 2,048 immutable changes: restart,
  concurrent snapshot isolation, deadline-handler cleanup, live redundant-projection
  corruption rejection, source-level no-usage-scan guard, 0.842 ms current read,
  4.904 ms maximum 256-row page, and 32 open/query/drop cycles returning at 116
  handles, five threads, USER=2, GDI=0, and 4,517,888 private bytes.
- Strict SQLite schema v12 benefit foundation with independent publication revision,
  exact rollback-safe v10-to-v11 and v11-to-v12 migrations, provider/account/workspace
  scopes, immutable
  material lot revisions and change points, current projection, inherited/override
  reminder profiles, durable due queue and delivery receipts, exact object validation,
  and no changes to existing usage/price/quota facts.
- Transactional benefit observation/profile persistence with deterministic pure-core
  reconciliation, duplicate/stale no-op, freshness-only publication, missing/
  ambiguous/reappearance/terminal handling, monotonic terminal cursor recovery after
  restart, due rebuild, injected rollback at history/current/due/revision boundaries,
  and path/identity-private errors.
- Bounded benefit retention with 512-change/256-delivery soft defaults,
  2,048-change/1,024-delivery hard limits, one total 256-row maintenance budget,
  protected newest/current/terminal evidence, orphan material-revision cleanup,
  noncurrent delivery compaction, exact state counts, and rollback-safe retry.
- Provider-neutral benefit domain values and pure `tokenmaster-benefits`
  reconciliation/reminder core, plus built-in Codex detailed reset-credit
  normalization with account-separated opaque IDs, discarded untrusted text,
  preserved independent expirations, and explicit aggregate unknown-expiry remainder.
- Separate bounded Codex quota runtime with one independent scheduler/worker,
  immediate startup/recovery refresh, capacity-one coalescing, 15-minute normal and
  transient-only 60-second accelerated cadence, pause/resume/suspend/shutdown,
  I/O-before-lease ordering, non-waiting shared writer admission, at-most-32
  idempotent per-window publications, partial-prefix accounting, and count/time/code-
  only health isolated from usage-engine state.
- Exact-native Codex executable selection with authoritative explicit configuration,
  fresh automatic process-`PATH` discovery bounded to 64 KiB/128 entries, exact
  `codex.exe`/`codex` matching, relative/script/`PATHEXT`/package-wrapper rejection,
  path-private config/discovery errors, public fail-closed smoke, 16+48-round
  success/RPC/timeout/busy/pause-resume Windows resource plateau, and a release
  dependency/production-source/library audit with explicit `#[cfg(test)]` exclusion
  and zero forbidden network/browser/credential/shell/socket/direct-SQL/
  foreign-runtime matches.
- Built-in credential-blind Codex quota connector using one exact short-lived
  `app-server --stdio` child, stable non-experimental protocol pinned to `0.144.1`,
  strict account/multi-bucket rate-limit schemas, pseudonymous account identity,
  primary/secondary fixed-point window normalization, deterministic observations,
  provider-defined reset thresholds, 20-minute/2-hour freshness, and transient
  bounded reset-credit validation without benefit persistence.
- Bounded Codex transport/process acceptance: exact fixed argv and notification
  opt-outs, 256-KiB frame/1-MiB stdout/64-frame/32-window/64-credit caps, redacted
  stable errors, hidden Windows child, complete timeout/reap/join cleanup, authenticated
  live two-window smoke, adversarial envelope/ID/version/EOF/error matrix, 64-round
  success/error/timeout Windows resource plateau, orphan-process check, and a release
  dependency/source/library authority audit with zero forbidden network/browser/
  cookie/private-endpoint/credential-file/shell/socket matches.
- Immutable public quota query envelopes with independent quota revision, exact
  provider freshness, worst truthful quality, request-ordered explicit unavailable
  windows, query-owned reset/allowance values, opaque filter/revision-bound history
  continuation, failed-call snapshot-generation neutrality, and redacted public
  `Debug`.
- P2-D quota-core acceptance gates: adversarial no-inference matrix, 32-window/
  1,000-transition/10,000-duplicate release fixture, writer/reader restart, 256-row
  continuation, bounded maintenance, current/migrated-legacy usage coexistence,
  repeated Windows private-memory/handle/thread/USER/GDI plateau, and a release
  dependency/source/library audit forbidding network, browser, cookie, shell, socket,
  and async-client authority.
- Defensive quota store snapshots with zero-to-32 exact current windows, owned
  definition/sample/epoch/last-transition values, revision-bound newest-first
  transition history capped at 256+1, opaque filter-bound keyset cursors, fixed
  quota-only indexed SQL, strict two-second total deadlines, guaranteed progress
  cleanup, deterministic transition restoration, boundary-projection reconciliation,
  redacted capture `Debug`, and post-open drift rejection.
- Evidence-preserving bounded quota retention with exported 512-sample/256-epoch-
  transition soft defaults, 2,048/1,024 per-window hard caps, 256-candidate
  maintenance pages, automatic equivalent-poll pruning, same-definition/same-window
  compaction, protected first/current/last/ratio-max/unit-max/reset evidence,
  over-cap write/reopen rejection, count-only results, and rollback-safe retry at both
  maintenance fault boundaries.
- Transactional quota observation persistence with exact duplicate/stale no-op,
  immutable definition/sample/closed-epoch/transition facts, independent one-step
  quota revision publication, repeated-reset and reopen continuity, account isolation,
  global observation identity, definition immutability, current-projection validation,
  checked sequence/capacity errors, deterministic retry, and rollback at five injected
  publication boundaries.
- Strict SQLite schema v10 quota storage with an independent revision, seven bounded
  tables, exact table/index/trigger validation, same-window/revision evidence foreign
  keys, semantic allowance-change checks, exact non-mutating v9 migration, malformed
  reopen rejection, and residue-free injected rollback.
- Pure constant-state quota epoch evaluator with domain-separated deterministic
  scope/epoch/transition identities, exact duplicate/stale/conflict handling,
  scheduled/early/manual/unknown full resets, orthogonal allowance changes,
  maximum comparable use with independent ratio/unit evidence identities,
  restart-safe definition revisions, checked transition sequences, and explicit
  rejection of drop-only or rolling-window reset inference.
- Exact provider-neutral quota domain values replacing the floating-point M0
  placeholder: bounded scope/window/unit/epoch IDs, redacted 32-byte observation IDs,
  parts-per-million ratios, optional coherent units, fixed-window provider thresholds,
  normalized definitions/samples, explicit reset evidence, and validated strict serde.
- Deterministic release-pinned fixed-point pricing with exact reviewed aliases,
  standard/cached/output/long-context/priority rates, explicit zero, checked arithmetic,
  one-time rounding, and no runtime price fetch.
- Validated immutable price overrides capped at 512 entries, stable revision identity,
  atomic rejection, source-reported/calculated/auto selection, explicit availability,
  provenance, conflict, and bounded missing-key evidence.
- Strict SQLite schema v9 price-basis facts for time and session aggregates, exact v8
  migration, transactional live maintenance, resumable legacy rebuild, bounded indexed
  batch reads, and exact dataset identity.
- Dataset-exact costs on overview, calendar series, model/project/provider/profile
  breakdowns, and opaque session page/detail values without raw-history or per-item
  query fallback.
- Current/legacy million-event pricing gates, catalog/override/query-switch Windows
  resource plateaus with an isolated single-thread measurement process, bounded
  topology-stable warm-up, converged retained-return windows, unchanged 1/2 MiB
  budgets, structural high-water checks, scoped price query-plan assertions, and a
  production dependency/binary-string audit that forbids runtime pricing network
  paths.
- Single-root TokenMaster workspace and clean-history product boundary.
- Root-only Rust M0 scripts and CI.
- Clean-root audit for product-tree invariants.
- TokenMaster-only contracts, handoff, roadmap, feature matrix, and provenance.
- Critical architecture audit with a single approved delivery rail.
- Architecture/release closure review with a blocking row-level reference parity
  ledger, UI-before-automation phase order, permitted Codex quota-source policy,
  release-pinned pricing, canonical MSVC signed portable package, Slint attribution,
  no-updater 1.0 boundary, and explicit supply-chain evidence gates.
- Provider-neutral observation/canonicalization TDD plan and complete requirement status matrix.
- Provider-neutral observation and late session-relation drafts with Codex ancestry,
  ordinal, cumulative, and resume-v2 contracts.
- Exclusive `tokenmaster-accounting` crate with versioned deterministic fingerprint
  and replay identities, evidence, opaque canonical events, and compile-fail authority
  proofs.
- Allocation-free provider-neutral replay classifier with explicit typed states,
  scope/ordinal validation, conservative weak evidence, and bounded-work semantics.
- Strict SQLite schema v4 with exact v1/v2/v3 migration and canonical projection
  publishing/origin/retained provenance.
- Atomic carry-forward for absent or conflicting replay-verified evidence, with
  truth-table, truncation, reopen, tamper, and fault-rollback contracts.
- Strict SQLite schema v5 with exact v1-v4 migration, bounded provider-qualified
  scan sets, complete-only source presence, coherent terminal state, and lifecycle
  rollback contracts.
- Exact scan-bound replay with persisted provenance, bidirectional membership
  revalidation, multi-provider scopes, atomic begin faults, missing-generation
  preservation, and zero-source retention-only promotion after reopen.
- Real synthetic Codex pipeline composition over complete/partial scan authority
  without adding a store dependency to the production Codex adapter.
- Reference-safe scan-history retention: 32 closed sets per scope, at most 64 whole
  unreferenced sets pruned per transaction, running/source/replay protection, bounded
  backlog recovery, checked ID exhaustion, and injected rollback proof.
- Provider-neutral constant-state refresh coordinator with monotonic checked IDs and
  deadlines, cooperative cancellation, explicit admission/terminal outcomes, one
  active permit, and one aggregate follow-up across 10,000-hint bursts.
- Bounded provider-neutral engine runtime contracts: sealed scope/source identities,
  32-KiB opaque checkpoints, 18 chunk-proof updates, scope-exact 256-item adapter and
  canonical batches, temporary source readers, stable coded errors, and object-safe
  synchronous adapter/archive/clock/writer-lease ports with compile-fail privacy gates.
- Provider-neutral one-shot refresh execution with lease-first admission, streamed
  scope-exact discovery, all-complete replay, core canonicalization, exact replay-handle
  continuity, bounded continuation, phase-complete cancellation/deadline handling, and
  explicit last-confirmed staging cleanup.
- Deterministic provider-neutral refresh worker with one owned thread, capacity-one
  wake/latest-result channels, non-blocking checked supersession, constant-state
  10,000-hint coalescing, cooperative shutdown/Drop join, stale-ID safety, fixed
  completion/snapshot values, and redacted panic/fault containment.
- Exact per-logical-file engine identity plus a descriptor-private two-pass rebuild
  seam that lends one temporary source reader at a time. Contracts cover shared
  provider source IDs, extra/duplicate/omitted or mismatched second-pass input,
  incomplete quality, and repeated 300-file promotion with one maximum live reader.
- Bounded atomic replay fact batches containing up to 256 canonical events and 256
  late relations with one revision/epoch advance and full event/relation/selection/
  work/chunk/checkpoint rollback at two injected transaction boundaries.
- Production `tokenmaster-runtime` bootstrap composition with the built-in Codex
  adapter, checked SQLite archive bridge, strict path-free 32-KiB checkpoint envelope,
  300-file/reopen/zero/missing-profile/Windows-replacement/truncation contracts, and
  exact post-begin staging cleanup.
- Replay-aware incremental archive and runtime: strict schema v6 publication
  generations, exact complete-scan freshness and source admission, paired revision/
  archive CAS, targeted fingerprint materialization, zero-payload unchanged refresh,
  persisted-offset multi-batch tails, bounded partial restart, multiple new/empty
  sources, missing-history retention, and durable non-destructive rebuild state.
- Portable process-owned writer lease using one persistent empty sidecar and Rust 1.97
  `File::try_lock`, with same-process/cross-process contention, normal/forced process
  release, canonical parent alias, unsupported namespace/mapped remote drive, privacy,
  and runtime bridge contracts.
- Bounded pathless filesystem scheduling with exact `notify = 8.2.0`, one fixed atomic
  hint aggregate, capacity-one wake, one scheduler thread, 250 ms quiet coalescing,
  15 minute healthy/60 second degraded reconciliation, checked clock rollback,
  64-root generation bounds, and Windows handle/thread return-to-baseline evidence.
- Lease-first live runtime composition with exact startup scan/staging recovery,
  incremental/rebuild selection, one worker-owned Codex/archive/lease state,
  admission-safe pause/resume, ordered joined shutdown, stable path-free snapshots,
  partial/reopen recovery, and combined Windows handle/thread return evidence.
- Startup-seeded immutable engine publication with checked in-process generation,
  exact archive generation/revision/scan/data-through/quality, fixed diagnostics,
  strict newer-only consumer ordering, one-state retention across 10,000 candidates,
  fail-closed overflow, and writer-busy/older-result rejection.
- Immutable publication race/recovery coverage for no-change freshness, pause/resume,
  persisted versus in-process restart ordering, malformed-truncation
  `recovery_pending`, canonical retention, and successful repair.
- Windows 8+ suspend/resume registration with one process-wide static capacity-one
  signal, no helper thread/window/callback allocation, last-event-wins reduction,
  forced reconciliation on every resume, private diagnostics, and 4,096-cycle private-
  memory/handle/thread/USER/GDI resource bounds.
- Approved a provider-neutral weekly quota reset history: immutable pre/post epochs,
  scheduled/early/repeated reset transitions, allowance-change separation, bounded
  retention, and shared UI/CLI/MCP semantics for P2.
- Approved separate banked reset inventory and expiry safety: independently expiring
  lots, a selectable 7d/24h/12h/6h/1h default profile plus bounded custom reminders,
  truthful notification coverage, assisted activation, and official-capability-only
  crash-safe automatic policy for P2.
- Approved the executable P2-A query foundation: separate publication and dataset
  identity, dedicated SQLite read-only/query-only store, short exact transactions,
  injected clock/deadline, bounded immutable envelopes, and composite keyset activity
  paging before materialized aggregates.
- Approved the self-reviewed P2-B schema-v8 aggregate design: provider-self-contained
  current events, exact missing-token algebra, UTC hour/minute and session rollups,
  resumable generation-bound rebuild, explicit IANA/DST rules, independently capped
  breakdowns, and blocking migration/performance/storage/privacy/resource gates.
- Added the first `tokenmaster-query` slice with schema-v1 immutable headers and
  envelopes, checked publication/dataset identities, injected exact clock samples,
  bounded pages/scopes/warnings, stable path-free errors, and fingerprint-redacted
  activity cursors.
- Added the separate schema-v8 `UsageReadStore`: SQLite read-only/query-only and
  defensive policy, fixed 4 MiB cache, exact short read transactions, current/legacy
  composite-keyset pages, `pageSize + 1` lookahead, stale dataset rejection, and
  deadline interruption with guaranteed handler cleanup.
- Completed P2-A `QueryService`: exact freshness/quality mapping, explicit stale-
  accounting downgrade, no-change cursor continuity, changed-dataset rejection,
  successful-only snapshot generations, and one-result older/equal/newer ordering.
- Added P2-A evidence for a 100,000-event activity archive (35.65 ms new connection plus
  first page; 1.10 ms warm cursor page), 10,000 candidate replacements with one retained
  payload, private Debug/error fixtures, and a 256-cycle Windows resource plateau.
- Added exact schema-v8 migration with provider-self-contained current events,
  generation-qualified UTC minute/hour and session rollups, transactional known/
  partial/unavailable token algebra, and fail-closed invariant triggers.
- Added bounded resumable aggregate rebuilding with measured 2,048-event keyset pages, disk-backed
  unpublished generations, bounded cleanup, reopen resume, mutation restart, fault
  rollback, and one checked active-generation publication.
- Added measured release append gates for 1/32/256-event paths and corrected storage
  accounting to include SQLite main, WAL, and SHM files.
- Added the first P2-B fixed aggregate read: one exact deferred snapshot binds
  publication/dataset identity to a ready aggregate generation and sums at most three
  adjacent aligned UTC minute/hour segments across at most 32 typed scopes. Boundary,
  stale/rebuild, deadline cleanup, missing-token algebra, and raw-table-free query-plan
  contracts pass.
- Added one combined analytics snapshot with an exact 400-point maximum series and
  unique model/project/provider/profile breakdowns capped independently at 256+1.
  Series partition, skipped-date zero points, canonical ordering, typed unassociated
  project, scope filtering, truncation, concurrent-state, cancellation cleanup, and
  real fixed-query-plan contracts pass.
- Added all-time session first/cursor pages with mixed timestamp/identity keyset order,
  exact dataset-bound opaque keys, 32-scope and 256+1 bounds, and no raw session Debug
  exposure. Exact detail returns capped model/project rollups or typed absence without
  raw-event fallback. Current, rebuilt legacy, stale, equal-time, missing, scope,
  limit, cancellation, concurrent-state, and real query-plan contracts pass.
- Added exact private calendar composition with pinned Jiff 0.2.32, explicit IANA or
  resolved-system zones, all seven week starts, DST gap/fold, skipped-date, leap/year,
  half/quarter-hour, and historical sub-minute fail-closed contracts. The locked
  bundled Windows chain records `jiff-tzdb` 0.1.8 / IANA tzdb 2026c.
- Added immutable public analytics and session facade values: today/day/week/month/
  bounded custom ranges, optional 400-point daily series, known/partial/unavailable
  token facts, activity counters, four independently capped breakdowns, and owned
  opaque session page/detail results. Session continuation now binds both dataset and
  canonical scopes, and failed/rebuild/stale captures do not consume snapshot
  generation.
- Added explicit P2-B current/legacy million-event release-mode gates for rebuild
  throughput/page latency, cold/cached overview, full 400-point/four-breakdown/
  32-scope analytics, session pages, main+WAL+SHM amplification, privacy, and repeated
  snapshot/rebuild resource plateaus. The measured results are 12,324-13,240 events/s,
  246.558-268.305 ms rebuild-page p95, 1.483x-1.568x storage amplification, below
  179 ms cold overview, below 0.55 ms cached overview p95, below 166 ms full analytics,
  and below 0.75 ms session-page p95.

### Fixed

- Removed a startup race in `RefreshScheduler::spawn_paused`: paused instances now
  begin with no preloaded recovery flags, and `resume()` installs the single recovery
  request. A fast scheduler thread can no longer submit both the constructor flag and
  the resume flag before the first quota refresh completes.
- Hardened the isolated Windows query resource warm-up floor against one transient low
  allocator sample while retaining measured-window return minima, sustained-growth
  rejection, and exact handle/thread/USER/GDI gates.
- Replaced the overly conservative 256-event aggregate rebuild cap after its
  deterministic current-million red run retained only about 2,850 events/s. The
  2,048-event cap keeps the persisted cursor/generation/crash boundary, caps derived
  work at 18,432 rows, and passes the independent 500 ms page-p95/resource gates.
- Bound current replay cursors to revision ID plus the dedicated schema-v7 dataset
  generation. Insert/update/delete advance it transactionally; real no-change scan
  publication may advance replay evidence but keeps dataset identity stable. Exact v6
  migration rollback and generation overflow fail closed.
- Preserved query deadline semantics across the store-to-runtime port mapping; the full
  workspace gate exposed and now covers the previously missing exhaustive error case.
- M0 verification no longer depends on foreign runtime toolchains.
- Corrected the stale M0 development decision reference to ADR-006.
- Removed public canonical/replay constructors from domain and Codex-owned
  fingerprinting; the store now accepts accounting output only.
- Made legacy parser resume fail closed instead of guessing an ordinal that could
  collide with prior events.
- Made store append fail closed when a canonical event provider does not match the
  registered source provider, in addition to existing profile/source checks.
- Removed the canonical page's lifetime dependency on obsolete source generations;
  complete truncation/replacement now preserves accounted usage without synthetic
  observations or unbounded generation retention.
- Removed scan authority from ordinary append and made post-scan source registration
  remain missing until a later complete matching-scope scan observes it.
- Rejected cross-scope adapter discovery and non-progressing checkpoints before they
  can loop or mutate the wrong archive scope.
- Removed the archive replay-page/cursor descriptor-recovery assumption, which aliased
  real Codex files sharing one source ID and could not recover a live path-private
  descriptor without unbounded caching or repeated enumeration.
- Disabled canonical-only append after replay promotion and removed false `complete`
  windows during new-source admission; current append, checkpoint, replay projection,
  publication quality, epoch, and archive generation now roll back together.
- Allowed a valid existing-source tail to commit while an exact scan has also admitted
  a new pending source; current replay membership remains required and all paired CAS
  checks remain unchanged.
- Classified profile-scope changes as durable rebuild requirements and made full
  rebuild safely recover an unadmitted provisional source instead of leaving the
  archive blocked after interrupted incremental admission.
- Converted changed provisional identity and over-bound new-source admission into a
  typed non-destructive rebuild path instead of a database conflict or retry loop.
- Removed the synthetic Codex pipeline's per-relation transaction loop; reader events
  and relations now reach the store as one exact batch, preventing stale engine handles
  after a partial fact commit.
- Reserved terminal `busy` for writer-lease admission so later port faults cannot be
  mislabeled as harmless backpressure.
- Prevented external Clock or execution callbacks from running under the worker state
  mutex, and made stopped/faulted admission reject before consulting the Clock.
- Made callback panic dominate concurrent cancellation as fixed `failed`/`panicked`,
  abandon the one follow-up, suppress worker-only panic payload output, clear runtime
  state on other worker-port panics, preserve faulted join ownership, and reject
  incompatible `panic=abort` engine builds.
- Made malformed, incomplete, or oversized relevant live input fail before checkpoint
  or batch commit, preventing a degraded full rebuild from being mislabeled complete.
