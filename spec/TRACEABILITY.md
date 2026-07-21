# TokenMaster traceability

Status values are explicit: `implemented`, `partial`, `planned`, or `open evidence`.
A design or plan is not implementation evidence.

| Requirement | Status | Implementation or planned owner | Evidence or next gate |
| --- | --- | --- | --- |
| TM-FUNC-001 | implemented | `crates/provider`, Codex roots/files | provider, discovery, enumeration contracts |
| TM-FUNC-002 | implemented | Codex reader plus store/runtime incremental path | zero-payload unchanged, exact tail bytes, multi-batch restart, new/missing source, replacement/truncation/profile-scope and malformed-input recovery contracts |
| TM-FUNC-003 | partial | domain/accounting/Codex parser, schema-v9 price facts, indexed aggregate query facade, P3-C Dashboard, P3-D.1 History, P3-D.2 Sessions list/detail/later-page navigation, P3-D.3 Models, P3-D.4 Projects, and P3-D.5 Recent activity | prior evidence plus private-cursor replace-only Sessions navigation with exact terminal no-snapshot rollback and retained-page newest recovery, one shared 1/7/30 recent-usage envelope (default/max 30) for History/Models/Projects, and a bounded latest-event token page pass; interactive range behavior, rhythm aggregation, remaining exploration, and CLI/MCP remain |
| TM-FUNC-004 | partial | exact joined product status, immutable reducer, complete bounded data routes, Help/About, P3-E.1 route command palette, P3-E.2 compact quota mode, P3-E.3 typed tray lifecycle, P3-E.4 current-session activation, P3-E.5 current-user startup, app-owned in-app expiry presentation, and global reminder settings editor | palette and compact reuse existing route/quota truth; one five-intent tray router, one capacity-one secondary/hotkey Show bridge, and three explicit path-free startup intents reach only typed app/platform boundaries; global portable desired state synchronizes reminders through startup, Save, and confirmed import; per-scope editing, reminder OS/tray delivery, usage alerts, and remaining presentation work remain |
| TM-FUNC-005 | partial | separate `crates/desktop` frontend, sole `crates/app` production binary, `tokenmaster-platform` current-session/current-user startup owners, and P4-C complete presentation owner | P4-C persists schema-v3 density+skin, migrates strict v1/v2 in memory, binds typed package source versions, applies one exact Rust-owned 15-role Refined/Graphite/Ember palette before metadata, admits before apply, retains one active plus one latest complete payload, and proves 10,000 mixed-axis switches. Layouts/schemes/locales, typography/row sizing, accessibility/DPI/paint/resource verification, and live Windows acceptance remain P4/P6 |
| TM-FUNC-006 | planned | separate CLI and MCP adapters over query facade | P5 strict JSON/stdin MCP conformance tests after the complete UI |
| TM-FUNC-007 | implemented | accounting lineage/classifier, scalable replay archive, P0-E composition, and P1-A retention | real JSONL baseline/append/restart/replay/quality/atomic-replacement/truncation-retention/failure contracts pass; live scheduling remains TM-FUNC-008/P1 |
| TM-FUNC-008 | partial | provider-neutral drafts/engine plus built-in Codex live runtime, immutable publication, and Windows power binding; external plugin host pending | P1-C, P1-D.0-P1-D.6, and P1-E.1-P1-E.3 logical-file/codec/bootstrap/exact-scan/CAS/recovery/lease/scheduling/lifecycle/publication/power contracts; external plugin conformance remains |
| TM-FUNC-009 | partial | prior quota stack plus explicit all-current overview and dynamic P3-C Plan Usage rows | 32-window one-revision discovery, missing-value truth, dynamic ratios/reset labels, no fixed five-hour/weekly UI, and privacy/audit gates pass; reset-history exploration and alerts remain |
| TM-FUNC-010 | partial | prior benefit/reminder stack, expiry-safety center, app-owned in-app presentation receipts, and global reminder settings synchronization | portable settings map generation `N` to global revision `N + 1`; inherited due rebuild preserves overrides and delivery/ack/provider evidence, aggregate results cover valid overridden scopes without post-commit fallibility, and startup Busy/unavailable keeps the exact policy Pending while runtime health degrades independently; snooze, quiet hours, OS delivery, and activation remain |
| TM-FUNC-011 | partial | prior Git stack plus P3-C checked Code Output card and P3-D.4 Projects | exact safe-alias per-project commits/added/removed/net/efficiency across at most 32 repositories pass with checked sums, one-count project cost, separate UTC-today evidence, and no identity/path rows; detail/range controls remain |
| TM-FUNC-012 | implemented | complete P3-D.0 reliable-state stack through Task 18 | settings/config, online/compact backup, typed packages, optional manual age protection, bounded catalog/retention, journaled restore/reconstruction, sealed app/UI operations, adversarial/privacy audits, 256-cycle resource return, exact disk plateau, spanning backup/UI latency, and clean-identity P3-D.0 developer acceptance pass |
| TM-UI-001 | partial | complete bounded data routes, static Help/About, P3-E.1 command palette, P3-E.2 compact quota mode, P3-E.3 tray lifecycle, P3-E.4 Show/restore/focus activation, P3-E.5 startup-status/actions, and transient accessible expiry panel | real Slint proves palette/compact activation, exact five-action native menu mapping, bounded hotkey delivery, and Disabled/Enabled/Stale/Conflict/Denied/Unavailable startup presentation with four explicit accessible actions; unified localization and live Windows focus/accessibility/sign-in acceptance remain P4/P6 |
| TM-UI-002 | partial | immutable publication, bounded routes/palette/compact projection, independent notification epoch bridge, and P4-C complete presentation owner | P4-C fixes three density and three skin keys/indices, schema-v3 complete-pair persistence with v1/v2 migration, saved/saving/not-saved semantics, seven density token tables, three exact 15-role palettes, admission-before-apply, one owner, and exact application plus compiled-UI 10,000-cycle proofs without new authority. Layouts/color schemes/locales, typography/row-size behavior, accessibility/DPI/paint/resource verification, and hot locale/layout settings remain P4 |
| TM-PERF-001 | partial | bounded backend, one-current projections, capped P3-E.1/P3-E.2 models, queue-free P3-E.3 lifecycle router, one P3-E.4 native thread, one notification receipt worker, and constant-capacity bridges | palette/compact/notification stress bounds remain; current-session activation adds one auto-reset bit, one pending bit, one scheduled task, no payload/queue/timer/polling, 10,000-signal coalescing, panic containment, and a 4,096-cycle fixed resource envelope; live hotkey/multi-process and long-run P4/P6 evidence remain |
| TM-PERF-002 | open evidence | software renderer and M0 resource gates | uninterrupted soak and interactive receipts remain absent |
| TM-PERF-003 | partial | immutable publication, bounded query controller, newest-only snapshot bridge, capacity-one Sessions terminal rollback, independent notification epoch bridge, and live app hints | Sessions completion uses the existing worker and event-loop task with one latest intent and no polling requirement; notification callbacks retain their prior bounded release/re-pump/join guarantees; P4 paint and final release evidence remain |
| TM-PERF-004 | implemented | bounded reliable-state path, one joined constant-state app worker, immutable catalog/UI projections, and event-driven reconstruction barrier | prior fixed bounds plus deterministic 8/96 MiB automatic/normal/compact throughput, 64 KiB I/O, 8 MiB decoder window, less-than-database 64 MiB private-growth ceiling, one sampler-only thread delta, 10,000-trigger/resume coalescing, 64+256 backup cycles, 16 acquired-candidate cancel/recover and 16 real restore cycles, exact 15-point/disk plateau, resource return, and spanning Dashboard/software-paint p95 delta at most 10 ms pass |
| TM-REL-001 | partial | M0 scripts, product receipt schemas, separate P3-D.0 developer receipt, and `P3E_ACCEPTANCE.md` | the strict P3-E operator-receipt schema/local preflight is fixed without claiming authenticated actions or package provenance; the P6 producer/manifest binding, external receipt, current schema-3 P3-D.0 exact-commit receipt, and final product packaging evidence remain pending |
| TM-REL-002 | open evidence | `M0_ACCEPTANCE.md`, `P3E_ACCEPTANCE.md` | packaged P3-E shell interaction, interactive Windows/DPI/accessibility, and uninterrupted soak receipts absent |
| TM-REL-003 | partial | P3-D.7 standard pinned Slint attribution surface plus P6 explicit MSVC signed portable package and supply-chain gates | in-product attribution exists; GNU/MSVC comparison, generated notices/SBOM, public-download attribution, advisory/source/license/secret/action/attestation audits, deterministic package and clean-room launch remain pending |
| TM-DATA-001 | partial | prior privacy boundaries plus path-private app root, bounded data models, static Help/About, route-only P3-E.1 palette, and quota-only P3-E.2 compact mode | palette and compact surfaces contain only fixed route truth or existing identity-free quota/freshness rows and expose no general command, content, credential, path, or identity surface; remaining P3-E-P5 surfaces repeat gates |
| TM-DATA-002 | implemented | domain drafts plus exclusive `tokenmaster-accounting` canonicalizer | canonicalizer vectors, compile-fail authority tests, Codex/store contracts |
| TM-DATA-003 | implemented | file identity and reader checkpoint | physical identity live/persisted round-trip, checkpoint conversion, resume bound, and restart contracts |
| TM-DATA-004 | implemented | scoped scan/rebuild plus replay-aware current publication, paired-CAS tail facts, exact admission, durable partial/recovery, retained promotion/discard | atomic faults, stale CAS, unchanged/append/multi-batch/new/missing/restart/deadline/rebuild contracts pass |
| TM-DATA-005 | implemented | writable usage/quota/benefit/Git store plus separate `UsageReadStore` | strict schema v13 retains prior facts and adds independent Git state plus one exact scalar cross-family product-status transaction with defensive deadline/corruption/progress cleanup and no historical scan |
| TM-DATA-006 | partial | prior bounded publication plus current data projections, replace-only P3-D.2c Sessions navigation, static Help/About, and P3-E.2 compact projection | prior caps remain; Sessions retains one at-most-64 page, one pending intent, and one capacity-one terminal rollback slot with no cursor history, while Compact reuses the current at-most-32 quota rows, one always-mounted view and one geometry slot with 10,000-switch proof; rhythm aggregates and P4 switch limits remain |
| TM-DATA-007 | implemented | replay facts/classifier in a private overlay plus schema-v4 self-contained canonical projection with deterministic selection/retention | v1/v2/v3-to-v4 migration plus replay/append/restart/300-file/atomic-replacement/truncation truth-table/failure contracts pass |
| TM-DATA-008 | implemented | exact quota values/evaluator, strict schema-v10 transactional storage, bounded retention, defensive reads, and immutable public quota facade | complete domain/write/retention/read/query contracts plus request ordering, explicit unavailable windows, provider freshness, quality aggregation, generation neutrality, adversarial inference matrix, restart/maintenance/current-and-legacy coexistence, 1,000-transition paging, resource plateau, and offline authority audit pass |
| TM-DATA-009 | partial | typed benefit inventory, schema-v12 due/outbox/ack, durable reminder runtime, app-owned visible presentation, and global profile projection | global profile replacement is immediate and inherited-only; returned counts are validated before commit and desired settings survive startup Busy/unavailable as exact retryable Pending independently of optional runtime health, while OS delivery and activation intents/receipts remain |
| TM-DATA-010 | implemented | non-serializable repository hints, bounded opaque Git values, durable schema-v13 projection, bounded runtime population, and immutable public join | local-path side-channel privacy, salted identities, 32-repository/4,096-association/400-day/8-category/16-warning caps, immutable generations, exact append/rebuild/unavailable/stale truth, bounded read/join/runtime/resource/authority gates pass |
| TM-DATA-011 | implemented | complete P3-D.0 reliable-state data/application/UI contour through Task 18 | prior evidence plus exhaustive mutation/truncation and WAL/SHM recovery, executable app/state matrix, deterministic fixtures, fixed streaming/high-water bounds, 64+256 lifecycle plateau, 16 cancel/recovery and restore cycles, exact retention/staging return, and version/fixture/command/resource/latency identity receipt pass |
| TM-SEC-001 | partial | local-only product plus validated installed/portable app root, no listener, credential-blind Codex transport, shell-free runtimes, and in-process native dialogs | app audit permits only fixed environment/root composition and proves zero HTTP/browser/shell/socket/SQL/arbitrary-root surfaces; Task 14 additionally source-pins Common Item Dialog with zero shell/process authority; future MCP stdio network-denial tests pending |
| TM-SEC-002 | partial | strict app/Desktop authority split, privacy-bounded data UI, Help/About, P3-E palette/compact/startup surfaces, and present-only notification DTO | only app/platform touch reminder or startup mutation; Desktop startup adds three typed intents and one six-state presenter, while the dedicated audit proves fixed HKCU, no HKLM/shell/process/polling/arbitrary registry input, and zero portable fields; remaining P4-P5/plugin and interactive accessibility/security suites remain |
| TM-SEC-003 | implemented | provider/Codex/store/engine errors, value types, redacted worker panic boundary, redacted quota surfaces, and sealed repository activity paths | prior privacy evidence plus non-serializable repository hints, canonical local-only namespace/reparse validation, latest-only transport, invalid-candidate clearing, and checkpoint/archive/Debug path exclusion pass |
| TM-SEC-004 | implemented | transactional archive authority, exact rebuild/recovery, OS lease, pathless watcher, ordered live lifecycle, static power callback | P1-D.3 rollback/recovery, P1-D.4 process lease, P1-D.5 callback privacy, P1-D.6 lease-first recovery/admission/shutdown, and P1-E.3 power isolation contracts |
| TM-SEC-005 | partial | P4-C built-in skins are fixed Rust-owned application data with exact palettes and no file/provider/theme authority | external skin package schema, validation, signatures, isolation, and hot reload remain unimplemented |
| TM-SEC-006 | planned | built-in Codex exists; isolated plugin host deferred | provider plugin design and future 1.1 conformance/security gates |
| TM-SEC-007 | planned | host-owned banked reset activation capability and policy boundary | no-scrape/no-authority-escalation/idempotency/ambiguous-outcome security gates pending |
| TM-SEC-008 | partial | sealed reliable-state layers plus complete Windows application/UI, adversarial, and resource contour | prior evidence plus every package prefix/one-bit mutation, coherent WAL/SHM recovery, twenty-three attack anchors, SHA-pinned dependencies/features/licenses, privacy canaries, zero codec process/network/shell/extraction/plugin/UI/SQL authority, zero child processes, fixed handle/thread/USER/GDI return, and clean P3-D.0 evidence identity pass; hostile-race Unix cleanup hardening remains before any future Unix native selector |

The approved audit resolutions and delivery order are in
`docs/AUDIT_AND_MASTER_PLAN.md`. P0-A/P0-B and P0-C have completed executable plans.
P0-D transaction, classification, seal, promotion, rollback, and recovery semantics
are implemented. P0-D.1 removed the historical 256-file product blocker with exact
schema-v2-to-v3 migration, disk-backed all-source begin, checked 64-bit counts, and
256-row keyset-paged validation. P0-E proves the real synthetic Codex-to-archive path,
including bounded restart, atomic replacement, failure discard, totals, and quality.
P1-A adds schema-v4 provenance and explicit prior-evidence carry-forward without
retaining obsolete generations. P1-B.1 adds strict schema v5 and provider-qualified,
complete-only scan-set presence authority. P1-B.2 binds the production replay and
real synthetic Codex composition to that authority, including zero-source retention.
P1-B.3 bounds closed scan history to 32 per scope with 64-set reference-safe batches,
running recovery, checked ID exhaustion, and atomic fault rollback. P1-C.1 and P1-C.2
provide constant-state refresh admission plus bounded provider-neutral runtime ports.
P1-C.3 composes them into a scope-exact, bounded one-shot execution with full phase
cancellation/deadline coverage and exact replay cleanup. P1-C.4 completes the engine
core with one owned worker thread, capacity-one wake/latest-result channels, fixed
supersession state, panic/fault containment, stale-ID safety, and cancel/wake/join
shutdown/Drop ownership. P1-D.0 then corrects the real multi-file seam with a fixed
logical-file key and two linear streaming passes that lend at most one temporary
reader; the 300-file contract proves no engine replay-page/descriptor collection.
P1-D.1 applies replay events and late relations as one bounded atomic fact batch with
two rollback boundaries and one epoch increment. P1-D.2 now supplies the real
path-private Codex adapter, strict checkpoint codec, and store archive bootstrap
composition. P1-D.3 now adds schema v6 publication truth, exact scan freshness/source
admission, paired-CAS replay-aware tail append, bounded continuation, durable partial/
recovery state, and real Codex zero-payload/append/multi-batch/restart/replacement
plus profile-scope/full-rebuild recovery contracts. P1-D.4 adds the portable empty-
sidecar process-owned writer lease, mapped-remote fail-closed classification, and a
4,096-cycle Windows handle-plateau contract. P1-D.5 adds the pinned pathless watcher,
fixed atomic aggregate, quiet/periodic scheduler, 10,000-hint one-follow-up proof, and
32-generation Windows resource return. P1-D.6 adds lease-first startup recovery,
incremental/rebuild selection, admission-safe pause/resume, current-partial restart,
ordered joined shutdown, and combined Windows resource return. P1-E.1 now adds
startup-seeded immutable engine publication, strict archive-generation ordering,
exact revision/scan/data-through, fixed checked diagnostics, 10,000 equal-candidate
retention, and busy/older-result rejection. P1-E.2 closes no-change, pause/resume,
process-restart, malformed-truncation `recovery_pending`, canonical-retention, and
successful-repair publication contracts. P1-E.3 adds real Windows callback
registration, capacity-one event reduction, resume-without-suspend reconciliation,
duplicate/shutdown behavior, and 4,096-cycle private-memory/handle/thread/USER/GDI
resource bounds. P1 and P2-A are implemented. P2-B provider identity, aggregate
schema/triggers, bounded rebuild, overview/series/breakdown reads, and opaque keyset
session page/detail reads, exact private calendar composition, immutable aggregate
values, facade mapping, and million-row/storage/privacy/resource evidence are
implemented. P2-C schema-v9 price facts, fixed-point pricing, bounded overrides,
batched cost facade, scale, offline, privacy, and resource gates are implemented.
P2-D Tasks 1-8 exact quota values, pure detector, strict schema-v10 foundation,
transactional history writes, evidence-preserving bounded retention, defensive store
snapshots/keyset history, immutable public query values/service, adversarial/scale/
resource evidence, and offline authority audit are implemented. The built-in Codex
quota normalizer and short-lived official app-server transport are also implemented
and live-verified for the pinned version. Exact-native executable discovery, the
dedicated quota scheduler/worker, I/O-before-lease publication, and separate bounded
runtime health are implemented. Benefit inventory Tasks 1-8 now cover values, pure
reconciliation/reminder planning, Codex normalization, schema-v12 persistence/
retention, immutable query snapshots, combined quota-runtime publication, and one-
timer crash-safe durable in-app event delivery with explicit acknowledgement. P2-E
Tasks 1-8 now add the bounded Git domain/backend/hint/projection/query/runtime contour,
including same-process incremental authority, pause/resume recovery, durable
unavailable truth, a Windows resource plateau, and release-library authority audit.
P2-F joined product status is implemented with exact scalar capture, immutable section
reduction, fixed route readiness, copied runtime health, scale/resource evidence, and
its authority audit. P3 visible notification/UI evidence remains.
M0 interactive/
soak evidence remains.
Tasks 3+ in the older replay plan are historical and superseded.

The clean-root invariant is implemented by `scripts/audit-clean-root.ps1` and its
Pester contracts.

P4-C: `TM-FUNC-005` and `TM-UI-002` extend to schema-v3 complete presentation,
Refined/Graphite/Ember exact identity, one palette owner, admission-first application,
and latest-wins complete payload persistence. `audit-desktop-shell` and
`audit-application-composition` source/Pester mutation rails are the developer receipt;
interactive and release traceability remains open.
