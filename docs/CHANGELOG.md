# Changelog

All notable changes are recorded here.

- Provider-ready internal runtime seam: schema-v13 opaque resume payloads with
  descriptor-bound reconstruction, injected providers, one bounded live ownership
  path, and provider-owned quota/health polling capped at 32 windows. Codex remains
  built in; external plugin hosting and packaging remain planned 1.1 work.

## Unreleased

### Added

- Added an isolated trusted GitHub workflow for the canonical unsigned Windows ZIP:
  default-branch manual dispatch or `v*` tag push only, immutable action commits,
  minimal OIDC/attestation permissions, explicit no-OCI/no-storage-record behavior,
  and package → attestation → ZIP/receipt upload ordering. It prepares the artifact
  provenance receipt path but does not claim an attestation until a trusted remote run
  is downloaded and independently verified.

- Changed TokenMaster's own product license to Apache-2.0 while preserving external
  MIT and Slint attribution, and added a SHA-pinned Gitleaks 8.30.1 gate over one clean
  committed Git history plus the validated closed Windows ZIP. Redacted temporary
  reports are deleted; the bounded receipt retains only commit/tool/package identities
  and fixed scan modes. This closes the TM-REL-003 secret-scan item, not signing,
  attestation, interactive Windows evidence, or soak.

- Added a pinned `cargo-deny` 0.20.2 Windows bootstrap and a locked all-features MSVC
  dependency policy. Advisory, reviewed-license, and crates.io-only source checks pass;
  no advisory is ignored. Pre/post commit, worktree, tool, policy, and lock hashes
  prevent a concurrent-checkout receipt TOCTOU, and task-owned RustSec scratch cleanup
  handles read-only Git packs. This closes only the TM-REL-003 advisory/source/license
  item; secret scan, attestation, signing, interactive evidence, and soak remain.

- Added P4-G unified en/ru/pseudo localization on schema v7 across Slint views and
  Rust-composed labels. Direct, persisted, and notifier changes hot-reproject while
  stable payloads remain unchanged. In-app presentation keeps one Shell-lifetime
  `Option<DesktopInAppNotificationBatch>` capped at 256 safe DTO rows; focused in-app
  2/2, localization 22/22, UI 16/16, format, strict Desktop Clippy, and diff checks
  pass. Full desktop aggregate attempts timed out/lost, but the final clean-root,
  format, warnings-as-errors workspace Clippy, and complete locked workspace test/
  doctest baseline passes after correcting a stale future-schema test from v7 to v8.
  The desktop textual source audit remains open under `AUDIT_HARDENING_LOOP`; live
  Windows/interactive acceptance remains pending. This is not M0, RC, or release.

- Added P4-F durable board preferences: schema v6 extends complete presentation with
  a strict fixed six-key manifest (`plan_usage`, `code_output`, `trend`, `sessions`,
  `activity`, `models`), in-memory v1-v5 canonical migration without startup write,
  accessible Up/Down/Visible/Collapse/Reset controls, compact hidden rows, and
  all-six payload retention. Focused evidence passes board UI 5/5 including 10,000
  edits, layout 4/4, state 24/24, package 9/9, and app 80+1+7+57. TM-FUNC-004 remains
  partial; locale/language, typography/accessibility/DPI/paint/resource and live
  Windows acceptance, P5/P6, M0, package/signing/soak, and release remain open.

- Added P4-E durable presentation layouts: schema v5 persists the complete
  density/skin/color-scheme/layout value, strict v1-v4 migrate to Refined, and the
  wide Dashboard supports Refined, Control Center, and Workbench over existing
  bounded models. Narrow width remains single-column with durable selection. The
  existing worker proves 81 combinations and 10,000 switches. Board reorder/hide/
  collapse, locale/language, typography/accessibility/DPI/paint/resource, P5/P6, M0,
  packaging/signing/soak, and release acceptance remain open.

- Added bounded full rhythm aggregation for Activity: 24 hourly and seven Monday-Sunday
  buckets, capped at 30 civil days, 768 occurrences, and 2,304 segments. The rollup
  remains in the shared analytics transaction, preserves DST/fractional/skipped-date
  and metric/exposure/occurrence metadata, and leaves Recent activity independent.
- Corrected the service-restart integration contract to await the asynchronous initial
  controller refresh before reading its published product generation.

- Added P3-D interactive History ranges (Tasks 1-5): fixed 1/7/30 rolling civil-day
  presets with default/max 30, one worker/slot, and a shared History/Models/Projects
  envelope. Exact epoch/product/range fences, accepted-only publication, terminal
  rollback, refresh rebind, bounded replacement, and path/private-content guards are
  covered by the recorded focused, Desktop, application, and parser-audit receipts.
  The final strict workspace baseline passes with serialized Windows GNU linking.
  This does not claim P4/P5/P6/M0/package/signing/soak/release acceptance.

- Added P3-D.2c bounded Sessions navigation. `Next page` and `Back to newest` route
  direction-only intents through the current weak application bundle and existing
  capacity-one controller worker. The opaque continuation remains worker-local; one
  latest pending intent and one replace-only at-most-64-row model retain no cursor/page
  history. Refresh and Back reset to newest, stale epoch/product/navigation generations
  fail closed, and page changes clear exact detail. Real pointer/Enter/Space/Tab,
  narrow/wide accessibility, model-identity, sink-lifetime, rapid-input, refresh-race,
  cancellation, and mutation contracts pass. This does not claim interactive History
  ranges, P5 JSON/MCP, M0, package/signing/soak, or release acceptance.

- Fixed terminal Sessions navigation recovery. Cancellation, deadline, abandoned, and
  stale no-snapshot completions now release only the matching pending intent through one
  existing event-loop task; refresh supersession is lock-safe, callback/poll receipt
  handling is idempotent, synchronous errors remain visible, retained failed pages can
  return to newest, and initial unavailable state stays closed. No polling loop, worker,
  queue, cursor history, or second page/model was added.
  The corrective receipt passes 242/242 Desktop and 105/105 application mutations,
  three release audits, and the exact clean-root/fmt/strict-Clippy/locked-workspace
  baseline; independent final code and audit reviews are 0/0/0.

- Added the historical P4-B durable density verification: at that boundary settings schema v2 persisted one
  fixed three-value density, strict v1 migrates in memory without a startup write,
  typed packages bind their declared settings source schema, and Desktop applies an
  admitted value through the existing latest-only operation worker. Saved/saving/
  not-saved projection, startup hydration, source mutations, and 10,000 switches are
  deterministic developer evidence. P4-C later superseded its skin status; layouts,
  color schemes, locales, remaining typography/row-size behavior,
  accessibility/DPI/paint/resource gates, P5/P6, M0, package/signing/soak, and release
  remain open.

- Added P4-A developer-only runtime density verification: one checked fixed three-value
  `DesktopPresentationStyle` owner, one root Slint binding/callback, seven exact
  spacing/radius token tables, and a 10,000-switch contract. Deterministic source and
  mutation checks reject key/index/revision drift and new timer/worker/query/window or
  retained authority. This does not claim full skins/layouts/color schemes/locales/
  languages, persistence/migration, remaining density typography/row-size behavior,
  accessibility/DPI/paint/resource verification, P5/P6, M0, package/sign/signing/soak,
  or release.

- Added P3-E.5 explicit current-user startup without introducing a second preference
  store. One fixed `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` `TokenMaster`
  `REG_SZ` value is the sole device-local truth. The platform writes only the exact
  quoted current executable path without arguments, rejects non-file/reparse identity,
  verifies physical identity and exact post-mutation readback, and makes stale repair
  explicit. A foreign/conflicting value is never overwritten or deleted. Startup
  inspection is read-only and non-fatal; Settings exposes six path-free statuses and
  three explicit actions. No startup value/path is copied into reliable settings,
  config, backup, logs, or Desktop models, and no shell, process, elevation, machine
  hive, timer, retry, polling, worker, or queue authority was added. Focused platform,
  application, compiled UI, source-audit, and mutation contracts pass without changing
  the real registry; focused platform 9+2, UI 2, app 1, portable-settings 12, all 20
  audit mutations, strict focused Clippy, and independent Critical/Important/Minor
  0/0/0 Ready review pass. Live sign-in, relocation, denied-ACL, and resource-return proof
  remains interactive; P3-E closure, P4/P5/P6, M0, package/signing/soak, and release
  are not claimed.

- Added P3-E.4 current-session single-instance activation and the fixed global
  `Ctrl+Alt+T` shortcut. The sole production entry claims one non-inheritable auto-reset
  `Local\TokenMaster.CurrentSession.Activation.v1` event before renderer/data startup;
  an existing-event process only signals Show and exits, while claim/signal failures
  fail closed as `current_session_unavailable`. The primary later owns one joined
  message-driven thread and one unnamed shutdown event. Both sources reuse the existing
  Show/restore/focus path through one pending bit and one scheduled Slint task, with
  startup retry retention and panic containment. Ten thousand requests remain constant-
  capacity. Across 4,096 test-owner cycles, handle growth is bounded to eight and
  thread/USER/GDI growth to one under concurrent harness noise.
  Focused platform/app tests, strict focused Clippy, 84 application-audit mutations,
  independent Critical/Important/Minor 0/0/0 product review, the corrected 1,001.5-
  second full baseline, and 187.3/150.6-second application/Desktop release audits pass.
  Live two-process,
  occupied-hotkey, foreground-policy, cross-token ACL, sleep/resume, and real hotkey
  resource acceptance remain interactive. P3-E.5 current-user startup is recorded
  above; P4/P5/P6, M0, packaging, signing, soak, and release are not claimed.

- Added the P3-E.3 production tray lifecycle around the sole production window: one
  pinned TokenMaster SVG design, one isolated Windows tray owner, five typed actions
  (Show, Hide, OpenCompact, OpenDashboard, Quit), one queue-free single-install router,
  and one weak-window application sink. The hidden owner is a top-level tool window so
  it receives Explorer recreation; checked re-add failure shows the main window and
  switches close from hide to quit. Show/route actions restore, raise, and request
  foreground focus; explicit Quit returns through joined shutdown before the sole
  clean mark. Production Desktop no longer enables Slint `system-tray`. No new thread,
  timer, polling retry, queue, cache, snapshot, controller, store, or provider owner
  was added. Native callback state is exact-readback verified before icon registration
  or Available publication. Desktop/application release audits, 226 combined audit mutations, strict
  package Clippy, and full package tests pass. Live Explorer/focus/sleep/resource
  return remains interactive evidence. P3-E.4 current-session activation is recorded
  above; current-user startup, P4/P5/P6, M0, packaging, signing, soak, and release are
  not claimed.

- Added the P3-E.1 bounded route command palette as the final full-window layer of the
  sole production `MainWindow`. It reuses the fixed immutable 11-route projection,
  limits the query to 64 Unicode scalar values, keeps one replace-only result model,
  refreshes safely on accepted snapshots, and adds no query service, timer, worker,
  cache, mutation command, or native authority. Ctrl+K/header opening, focused text,
  Escape/Up/Down/Enter, pointer activation, accessibility default action, 10,000-scalar
  input, and live snapshot refresh have executable/source-audit coverage. The release
  Desktop audit, 134 mutation cases, clean-root, fmt, strict workspace Clippy, and the
  complete locked workspace test/doctest gate pass. Compact content and the remaining
  production shell/native lifecycle are still open.

- Implemented generation-bound global reminder settings synchronization. Portable
  settings remain desired-state authority; startup, explicit Save, and confirmed import
  share a Pending-first retryable synchronizer, while global edits preserve scope
  overrides, deliveries, acknowledgements, and provider evidence. Startup archive
  contention now keeps the exact durable policy Pending and retryable while optional
  runtime health independently reports StoreUnavailable.
- Added the fixed responsive Settings editor: enable/disable, five recommended leads,
  and up to eight normalized custom leads with checked conversion, dirty-draft
  retention, accessible labels, and no UI runtime/store/timer/polling/queue authority.
  Computed application/Desktop/benefit receipts and 194 Pester mutation tests pass.
  Per-scope editing, snooze, quiet hours, OS/tray delivery, usage alerts, activation,
  P4/P5/P6, M0 acceptance, package/signing/soak, and release remain incomplete.

- Implemented the app-owned in-app expiry presentation contour. One leased reminder
  batch maps into at most 256 provider-neutral, identity-free Desktop rows; one checked
  weak-window Slint callback replaces the transient model/count/visible state before
  emitting the one-shot `Presented` receipt. Dismissal clears the model without a timer,
  animation, polling loop, or automatic acknowledgement.
- Added one condition-variable receipt worker outside the UI thread. It acknowledges
  only after visible application and retries acknowledgement only for Busy/
  StoreUnavailable after exactly 60 seconds. Confirmed release after failed presentation
  re-pumps on that same worker; terminal acknowledgement error releases without
  automatic re-presentation. `Err`/`false` release keeps local backpressure, while an immediate
  external re-presentation wakes receipt handling without waiting for the retry. Ten
  thousand pumps still coalesce behind one take/presentation.
- Added panic-safe real runtime/SQLite acknowledgement: a panic is redacted and rolls
  `Acknowledging` back to `Leased`, while the narrow fallback release can recover outer
  runtime-mutex poison. Desktop now clears bridge-busy state before invoking the receipt
  and announces both visible benefit and kind labels. Focused lifecycle/UI tests,
  computed Desktop/application/benefit source receipts, ADR-073, and strengthened
  mutations pass; the combined Desktop/application audit is 177/177.
  Repeated independent lifecycle review closed at Critical 0 / Important 0 / Minor 0,
  and the exact clean-root/fmt/workspace-Clippy/workspace-test developer baseline passed.
  This evidence does not claim M0, soak, package, signing, or release acceptance.
  Settings synchronization/editing, snooze, quiet hours, OS/tray delivery, usage alerts,
  activation, M0, packaging, signing, and release acceptance remain separate.

- Implemented P3-D.7 as an archive-independent responsive Help/About route with six
  fixed accessible sections for navigation, source/evidence truth, privacy, recovery,
  current automation availability, and licenses. It reflows without a list model and
  remains ready when product data is unavailable.
- Applied the compile-time Cargo package version exactly once during window
  construction and mounted exactly one pinned standard `AboutSlint` attribution
  widget. TokenMaster adds no arbitrary URL/browser/session surface, callback, query,
  dynamic diagnostics, runtime owner, timer, queue, cache, polling, or release claim.
- Added a compiled real-Slint route contract and expanded the deterministic Desktop
  audit to 104/104 mutation cases covering the real mount, exact five-guide-plus-one-
  attribution section instances, responsive breakpoint, accessible privacy/automation
  truth, compile-time version identity,
  standard attribution, zero list-model/control/open-URL authority, and false release/
  automation/all-provider claims. The complete Desktop package and strict package
  Clippy pass. Independent final review returned Critical/Important/Minor 0/0/0; the
  release audit and exact clean-root/fmt/strict workspace Clippy/locked workspace test-
  doctest baseline pass in 879.3 seconds. P4 localization, P5 CLI/MCP, and P6 notices/
  SBOM/MSVC/package/signing/public-download/release evidence remain separate.

- Implemented P3-D.6 as a responsive read-only Notifications expiry-safety center over
  the existing all-current benefit overview. It retains at most 32 effective reminder
  profiles, 256 separate current lots, and eight leads/profile while preserving exact/
  bounded/provider-local/provider-date/unknown expiry, inherited/override source,
  disabled/in-app-only coverage, evidence, warnings, due time, and truncation.
- Added one scope and one lot Slint model with complete wide/narrow/accessibility truth,
  explicit waiting/unavailable/retained/degraded/empty states, and millisecond-preserving
  exact/bounded UTC presentation. Navigation remains query- and rebuild-free.
- Kept delivery authority out of the route: no reminder take/ack/release, settings
  mutation, timer, worker, queue, cache, connection, polling, or activation callback
  was added. The source audit computes zero owner/control receipts and 82/82 mutation
  contracts cover the boundary. App-owned presentation receipts are now implemented by
  the separate contour above; settings editing, snooze/quiet hours, OS delivery, usage
  alerts, and activation remain future slices.
- Closed the initial review's three Important and one Minor findings with lossless
  millisecond UTC formatting, explicit waiting truth, complete owner/control mutations,
  visible wide completeness, and a populated replacement plateau. Re-review returned
  Critical/Important/Minor 0/0/0. Clean-root, formatting, strict workspace Clippy,
  release composition, and the complete locked workspace test/doctest gate pass; the
  uninterrupted baseline completed in 1,216.4 seconds. P3-D.6 is closed without
  claiming packaging, M0, or release acceptance.

- Implemented P3-D.5 as a responsive bounded Recent activity route over the existing
  `LatestActivityRequest::first(12)` product page. No query, worker, timer, queue,
  cache, connection, callback, schema, dependency, or route-time model rebuild was
  added; Activity remains available during aggregate rebuild.
- Added one newest-first 12-row projection and Slint model containing only UTC
  timestamp, canonical model, typed input/cached/output/reasoning/total tokens,
  freshness/quality, optional `has_more`, and explicit empty/unavailable/retained-
  failure/backend/frontend truncation truth. Private identity, provenance, content,
  paths, cursor/fingerprint, and authority remain outside Desktop/Slint.
- Added a compiled wide/narrow Recent activity view with complete accessible row
  meaning and in-place route switching. Focused 9/9 projection, compiled UI, full
  Desktop package, strict Desktop Clippy, source/release audits, and 67/67 mutation
  cases pass. The route intentionally does not claim WMT rhythm/heatmap parity; the
  bounded timezone/DST-aware aggregate remains future work.
- Closed two independent-review Important findings with explicit empty-page evidence
  degradation and a safe `page-available` presentation fact, so a retained empty page
  cannot render as unavailable. Exact fractional UTC timestamps are preserved and
  invalid nanoseconds fail closed. Re-review returned Critical/Important/Minor 0/0/0;
  clean-root, formatting, strict workspace Clippy, release composition, and the complete
  locked workspace test/doctest baseline pass in 1,035 seconds. P3-D.5 is closed;
  packaging, signing, M0, and release acceptance remain separate.

- Implemented P3-D.4 Projects as one responsive usage-centric route over the already
  captured recent-30-day Project breakdown and existing UTC-today Git envelope. The
  UI labels both periods independently and adds no analytics/Git query or runtime owner.
- Added one bounded 32-row Projects projection with safe aliases/`Unassociated`, full
  token mix, typed cost provenance, relative usage, and optional exact-alias commits,
  added/removed/net, and efficiency. Same-alias repository sums are checked and project
  usage cost is counted once; unmatched, partial, retained-failure, and truncation truth
  remain explicit.
- Added the compiled wide/narrow Projects view with complete accessible Recent usage
  and Today code row meaning, explicit unavailable/not-linked code truth, and in-place
  route switching. Expanded the production Desktop audit to 57 mutation contracts
  covering caps, exact-only joins, dual ranges, cost-once aggregation, visible status/
  reasons, privacy, one model/application site, and zero route-time work.
- Closed P3-D.4 after independent re-review returned Critical/Important/Minor 0/0/0,
  clean-root/format/full warnings-as-errors Clippy passed, and the complete locked
  workspace test/doctest suite passed in 807 seconds with serialized Windows GNU
  linking. Packaging, signing, M0, and product release acceptance remain separate.

- Implemented P3-D.3 Models as a real recent-30-day route. The existing History
  analytics request now captures Model and Project breakdowns, so History, Models, and
  the Projects view share one dataset/range/timezone/evidence envelope and the
  controller still performs exactly two analytics calls per refresh.
- Added one bounded 64-row Models projection and responsive compiled Slint view with
  canonical model keys, events, input/cached/output/reasoning/total tokens, typed cost
  availability/mode/composition, visible and accessible partial-cost provenance,
  relative distribution, exact evidence, and explicit backend/frontend truncation.
  Route switching remains query- and rebuild-free.
- Expanded the production Desktop audit to 47 mutation contracts and exact Models
  request/bound/model/application/view receipts. Focused product/Desktop/UI/resource
  tests pass with one worker/snapshot slot, zero new query/thread/timer/queue/cache/
  connection/dependency, and no private identity or authority crossing the frontend.
- Closed P3-D.3 after independent review returned Critical/Important/Minor 0/0/0 and
  clean-root, formatting, strict workspace Clippy, and the complete locked workspace
  test/doctest baseline passed; the full suite completed in 790 seconds.

- Implemented P3-D.2b exact Sessions detail with checked backend epochs and independent
  viewed-product/selection generations. Slint submits only a visible ordinal; the current
  app bundle routes one typed intent to the existing capacity-one worker, which resolves
  the opaque session key transiently and publishes only the latest matching selection.
- Added immediate selected/loading UI state and one responsive detail card with explicit
  idle/loading/ready/missing/unavailable truth, exact timestamps/duration/events/token
  buckets/cost/freshness/quality, and a combined maximum of 32 model plus 32 approved
  path-free project-alias rows. Failure, cancellation, dataset drift, and backend
  replacement never retain or show another row's payload.
- Expanded product/Desktop/application fail-closed audits for identity-free correlation,
  one latest-only work slot, current-bundle routing, exact bounds/model replacement,
  nonblocking bundle admission, keyboard/focus/hover selection, and no UI query
  authority. All 93 desktop/application
  audit cases and the three release audits pass with one controller worker/snapshot
  slot, zero retained session keys in product/UI, and no new dependency, timer, queue,
  cache, polling site, or authority surface.
- Closed independent review findings: a busy application bundle now rejects detail
  admission with `try_lock` instead of waiting on the UI thread; sub-second duration
  formatting borrows nanoseconds exactly; narrow summary/breakdown rows retain reasoning
  and every other aggregate; the headless compiled-UI contract now sends real pointer,
  Enter, and Space events, while a mutation rail pins explicit Tab navigation.
  The session-detail queue rail is scoped to detail fields/aliases, so unrelated bounded
  controller vectors remain permitted and are covered by a negative false-positive case.
- The final four-stage workspace baseline passed in 820.7 seconds overall: clean-root
  18.845 seconds, formatting 1.611 seconds, strict locked workspace Clippy 22.080 seconds,
  then the complete locked tests/doctests. The credential-dependent live Codex contract
  remains explicitly ignored; no M0, package, signing, or release acceptance is claimed.

- Implemented P3-D.2a as the bounded Sessions list. The existing desktop worker now
  requests one all-time newest-first page capped at 64; Dashboard retains its first 12
  rows while the independent product section preserves explicit `has_more`.
- Added one identity-free Sessions projection and responsive compiled Slint route with
  last activity, duration, events, input/cached/output/reasoning/total tokens, cost,
  freshness/quality, and accessible wide/narrow row meaning. Opaque keys/cursors remain
  controller-owned; route switching adds no query, rebuild, timer, worker, cache, or
  archive handle.
- Expanded the production desktop audit to 33 mutation contracts and exact Sessions
  bounds/model/application receipts. Focused controller/projection/package/UI tests and
  the release audit/build pass with 10 Rust/16 Slint files, one worker/slot, a 64-row
  Sessions cap, and zero polling/private-ID/direct-authority surfaces. Clean-root,
  formatting, strict warnings-as-errors workspace Clippy, and the complete locked
  workspace test/doctest suite pass; the full suite completed in 725.2 seconds.

- Implemented P3-D.1 as the first complete supporting data route. Query now resolves
  an exact bounded `recent_days(30)` range in the selected IANA timezone, including
  DST-correct civil-day partitions and the existing 400-day hard ceiling.
- Added an independent History product section and one sequential request on the
  existing capacity-one desktop worker. Compatible failures remain section-local,
  dataset changes invalidate stale History, and cancellation/deadline still prevent
  partial-attempt publication.
- Added one identity-free 30-row newest-first desktop projection and responsive compiled
  Slint History view with overview tokens/cost/events, exact range/timezone/evidence,
  daily trend, and wide/narrow detail tables. Route switching remains query-free and
  adds no timer, worker, cache, prior-range history, database handle, or dependency.
- Expanded the production desktop audit to 30 mutation contracts and exact History
  bounds/model/application receipts. Clean-root, formatting, strict warnings-as-errors
  workspace Clippy, release desktop audit/build, and the complete locked workspace
  test/doctest suite pass; the full suite completed in 710.7 seconds.

- Completed P3-D.0 Tasks 17-18 with a separate fail-closed Reliable State developer
  acceptance rail. New release contracts measure deterministic 8/96 MiB schema-13
  automatic/normal/compact backup throughput, fixed 64 KiB streaming and 8 MiB decoder
  window, a 64 MiB private-growth ceiling with database-size headroom, sampler-only
  thread delta, 10,000-trigger/resume coalescing, and reproducible fixture SHA-256.
- Added 64 warm-up plus 256 backup/package/verify/import-cancel/retention cycles, 16
  cancellations after recovery source/candidate acquisition, and 16 complete isolated
  data-only restores. Every measured pass returns verification staging to zero and the
  retained catalog to the exact 15-point byte plateau. Private memory, handles, threads,
  USER/GDI objects, child processes, and manual compact age-encryption return are bound
  to one original all-contours post-warm-up envelope.
- Added real Slint software-paint evidence under one identity-tracked 96 MiB automatic
  backup cycle that spans every loaded Dashboard query and route-input-to-paint sample.
  The strict P3-D.0 receipt binds a clean commit, application SHA-256, exact format and
  dependency versions, deterministic fixtures, command arrays, durations, metrics,
  limits, and eleven individual gates. Independent first review found four Important
  harness gaps; corrected rereview reports Critical/Important/Minor 0/0/0. This remains
  developer evidence and does not accept interactive Windows, M0, soak, packaging,
  signing, or product release.
- Implemented P3-D.0 Task 16 adversarial/privacy closure. Dedicated state contracts
  reject every proper config/backup package prefix, every one-bit mutation, and every
  WAL/SHM add/remove/change drift case. The RED matrix found and fixed both pre-existing
  drift moving WAL and conflicting/resumed partial layouts. Platform now preflights main
  plus both active/quarantine sidecar locations, accepts only an exact same-operation
  partial resume, and retains per-move race checks. A dedicated app target directly
  executes 57 journal/crash/rollback/automatic-recovery/mandatory-safety tests without
  adding test-only production authority.
- Added `audit-backup-package.ps1` and fourteen mutation contracts. The rail pins the
  seven codec files, twenty-three security/compatibility/execution anchors, exact
  SHA-256 identities for the 196-package name/version/license and enabled-feature
  closures, both MIT upstream notices, 247 production-source plus synthetic-export and
  release privacy canaries, and zero process/network/shell/generic-extraction/plugin/UI/
  SQL codec authority. Focused state 4/4, app 57/57, platform 13/13, and combined Pester
  120/120 pass. The post-review locked workspace test/doctest suite passes in 604.1
  seconds, strict warnings-as-errors workspace Clippy passes, and independent rereview
  reports Critical/Important/Minor 0/0/0 with `READY`.
- Made the existing Codex timeout cleanup contract deterministic under Windows process
  startup load. A deadline may legally expire before the fixture writes its PID receipt;
  that case now verifies cleanup by exact executable path. The target passes eleven
  consecutive runs and remains covered by the complete workspace suite.

- Implemented P3-D.0 Task 12B.2b/Task 15 application and UI reliable-state composition.
  The owning Slint/STA thread invokes the sealed config/backup dialogs and submits only
  controlled capabilities to the single joined operation worker. Config preview/
  confirm/cancel, normal/compact/encrypted backup, verification, confirmed selected
  restore, rebuild, retry/cancel, and backup-policy changes use fixed path-free intents.
  Each mutating path publishes `AtomicPromotion` and disables cancellation at its exact
  irreversible boundary.
- Added bounded Data & Recovery and Settings views with one latest-only projection, at
  most fifteen generation/ordinal restore points, one config preview, one operation,
  responsive/accessibility presentation hooks, destructive restore review, cleared
  passphrases, and no UI polling/progress queue. A durable path-free recovery banner
  distinguishes verified-backup restore from authoritative-source reconstruction.
- Added fail-closed no-backup reconstruction. Only proven definitive corruption plus no
  usable reverified point can create a fresh normal-schema archive. The archive is fully
  verified before/after staging and after atomic promotion; the prior main/WAL/SHM set
  remains quarantined and the redundant journal records explicit reconstruction without
  a backup identity. Application forces and awaits a bounded recovery-urgency local
  Codex reconciliation before healthy backup maintenance. Quota, reset-credit, reminder,
  and Git history remain explicitly unavailable rather than fabricated zero.
- Closed the independent Task 15 review findings. Restore confirmation now consumes
  the exact reviewed generation/ordinal identity after projection drift; promoted
  follow-ups publish `Running` at actual worker start; manual backup remains
  cancellable until its exact irreversible boundary and then publishes
  `AtomicPromotion`; unavailable counts/bytes are typed unknowns and render
  `Unavailable`. A complete no-backup journal now preserves the source-reconciliation
  obligation across restart, failed retry, and the bounded two-launch Safe Mode;
  explicit retry reconciles the promoted archive without repeating reconstruction.
- Strengthened application, desktop, and reliable-state audits to pin the rebuild
  binding, recovery urgency/completion barrier, exact irreversible phase count, visible
  non-reconstructible-loss receipt, reviewed restore identity, actual worker-start
  phase, typed unknown metrics, restart/retry reconciliation, and sole allowlisted
  store-to-state staging bridge. Focused application/engine/runtime/store/state/desktop
  tests and the application 46/46, reliable-state 56/56, and desktop 28/28 policy
  mutation suites pass. Clean-root, formatting, warnings-as-errors workspace Clippy,
  and the complete locked workspace test/doctest suite in 540.8 seconds also pass.
  Independent rereview reports Critical/Important/Minor 0/0/0 and `Ready`. Task 16-18
  adversarial/resource/acceptance and interactive Windows evidence remain open; no
  product or release acceptance is claimed.

- Implemented P3-D.0 Task 14 sealed native file selection in `tokenmaster-platform`.
  The existing pinned Windows bindings now call the Common Item Dialog directly with
  exact `.tmconfig`, `.tmbackup`, and `.tmbackup.age` filters, balanced STA COM lifetime,
  explicit cancellation, filesystem-only/no-link/no-process flags, and no new dependency.
  The thread-affine selector requires an active owner. Platform returns only an already
  open bounded single-link no-follow input or an output capability bound to the selected
  parent and selection-time absent/physical identity state. Windows output uses a
  retained exact cleanup handle and bounded adjacent create-new stage. Existing replace
  preserves the target until sealed atomic publish, validates the displaced identity
  after the syscall,
  rolls back a raced replacement, and deletes old bytes only after new identity/byte
  verification. Unicode names are supported while local/
  namespace/reparse/hard-link/type/extension/path/size failures stay stable and redacted.
- Added a deterministic controlled selector and eleven file-dialog contracts covering exact
  filters/defaults, cancellation, input bounds, Unicode, create/replace/no-truncate,
  identity drift, link/type rejection, source-pinned balanced COM cleanup, and zero
  shell/process authority. Five platform unit contracts additionally cover the post-check replace
  race, retained displaced recovery evidence, Windows handle-pinned cleanup namespace,
  and identity-query failure after both replace and rollback. One contract proves a
  selected output capability can grant only one stage for its complete lifetime.
  Full platform tests and strict platform Clippy pass. Application/UI binding and
  interactive Windows evidence remain open. Independent final rereview reports
  Critical/Important/Minor 0/0/0 and `Ready`.
- Updated the encryption cleanup-failure fixture to add a second physical hard link.
  This preserves a real ambiguous-cleanup fault after Windows handle-bound cleanup made
  the former path-rename-only sabotage safely removable. The complete locked workspace
  test/doctest suite passes in approximately 473 seconds alongside clean-root, strict
  workspace Clippy, release composition, and 38/38 plus 55/55 authority mutations.

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
  Task 12B.2b still owns application/UI config binding, verify/selected-restore/rebuild
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

- Pinned every GitHub Actions `uses:` reference to a reviewed full commit and added a
  bounded fail-closed immutable-action validator to the M0 verification chain.
- Bound P3-E interactive preflight to the deterministic P6 ZIP and producer receipt.
  Clean HEAD, BUILDINFO, closed manifest, package/EXE hashes, packaged executable,
  exact tested executable, and operator receipt must now identify the same artifact;
  ZIP expansion is bounded and temporary extraction is removed.
- Fixed late Codex session-identity conflict normalization, including zero-usage
  records, so valid rebuilds remain resumable and conflict-qualified.
- Kept invalid `cwd` diagnostics repository-hint-only instead of rejecting valid
  usage replay.
- Fixed false `ContinuationStalled` failures when replay maintenance advances the
  durable epoch without classifying an observation.
- Removed per-source whole-database FK rescans and short-circuited irreversible
  staging conflicts. A 6,400-event contract improved from about 11.5 to 2.8-3.3
  seconds with exact conflict/visibility truth; a clean portable MSVC live run
  sustained about 2,534 observations/second over the measured interval with bounded
  memory.
- Aligned Workbench with its accepted wide layout: Plan Usage remains full width,
  Code Output pairs with Sessions, Trend pairs with Model Usage, and Activity remains
  full width. Geometry contracts now verify row pairing and full-width behavior.
- Updated the application persistence contract to expect current settings schema v5.
- Fixed an older `RefreshScheduler` race exposed by the P3-E.5 full gate. A hint that
  arrived after the scheduler sampled monotonic time could previously look like a
  clock rollback and emit a spurious Recovery refresh. The scheduler now performs one
  bounded second clock sample after observing the newer hint, while genuine rollback
  still fails closed. Deterministic concurrent-progress/second-sample-rollback tests,
  500 pre-review and 300 post-review repeated scheduler runs, the full runtime suite,
  strict Clippy, and independent Critical/Important/Minor 0/0/0 review pass.
- Closed the reminder-settings final-review races. Rapid policy saves now retain one
  active operation plus one latest-wins pending payload instead of acknowledging and
  discarding a newer value. Explicit Save and confirmed config import wait for a
  bounded Slint acknowledgement of the visible Pending/atomic-promotion projection
  before settings bytes may change; publication failure aborts the mutation and keeps
  an import preview retryable.
- Widened the aggregate reminder-profile due count to `u64` for valid overridden-scope
  inventories, moved every fallible count conversion before SQLite commit, and added
  65,536-due retry/reopen coverage with unchanged durable revisions.
- Corrected current handoff and roadmap text that still routed the next implementation
  slice to the already completed global reminder settings editor. P3-E shell lifecycle
  is now the explicit next slice.
- Hardened the product resource-gate warm-up against the exact bimodal private-byte
  profile exposed by the first full P3-D.7 baseline. Warm-up now continues when its
  retained ceiling exceeds its floor by the unchanged 2 MiB return tolerance; a
  deterministic 16-sample regression, three independent live passes, independent
  0/0/0 review, and the subsequent complete workspace baseline pass.
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

- Added P4-C partial durable built-in skins: schema v3 complete density+skin settings,
  v1/v2 Refined migration, Refined/Graphite/Ember immutable 15-role Rust palettes,
  one admission-first presentation owner, and latest-wins atomic persist/restore.
  Layout, colour-scheme, locale, typography, interactive accessibility/DPI/paint/resource,
  P5/P6/M0, package/signing/soak, and release acceptance remain open.
- Hardened P4-C evidence against swapped nine-pair mappings, direct or aliased extra
  channels, any extra Slint `UiPalette` property or presentation callback, alternate
  pre-admission mutators, revision-write loss, and dead-code 10,000-cycle decoys. The
  current normative/status documents now consistently identify schema v3/P4-C while
  retaining P4-B only as compatibility history.
