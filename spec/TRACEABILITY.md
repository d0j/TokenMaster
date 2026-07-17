# TokenMaster traceability

Status values are explicit: `implemented`, `partial`, `planned`, or `open evidence`.
A design or plan is not implementation evidence.

| Requirement | Status | Implementation or planned owner | Evidence or next gate |
| --- | --- | --- | --- |
| TM-FUNC-001 | implemented | `crates/provider`, Codex roots/files | provider, discovery, enumeration contracts |
| TM-FUNC-002 | implemented | Codex reader plus store/runtime incremental path | zero-payload unchanged, exact tail bytes, multi-batch restart, new/missing source, replacement/truncation/profile-scope and malformed-input recovery contracts |
| TM-FUNC-003 | partial | domain/accounting/Codex parser, schema-v9 price facts, pure pricing engine, and indexed aggregate query facade | exact token/cost overview, daily-series, breakdown, session page/detail mapping with availability, provenance, override, conflict, and unknown-price fixtures; UI/CLI/MCP presentation remains |
| TM-FUNC-004 | partial | exact joined product status, immutable product reducer, fixed route readiness, P3-A production Slint shell, P3-B.1 bounded query controller, and P3-B.2 capacity-one event delivery; route payloads remain | prior gates plus status-first typed reduction, section-local failure, real schema-v13 read, 1,000-hint coalescing, latest-only snapshot, 10,000-notification/one-event delivery, real headless Slint loop, cancellation/deadline/shutdown/redaction and desktop audit pass; P3-B.3-P3-E remain |
| TM-FUNC-005 | partial | `crates/probe-app` evidence plus separate `crates/desktop` production shell | M0 lifecycle/presentation/skin/stress contracts remain separate; P3-A software-only production window passes, while tray/hotkey/startup/compact lifecycle remain P3-E/P6 |
| TM-FUNC-006 | planned | separate CLI and MCP adapters over query facade | P5 strict JSON/stdin MCP conformance tests after the complete UI |
| TM-FUNC-007 | implemented | accounting lineage/classifier, scalable replay archive, P0-E composition, and P1-A retention | real JSONL baseline/append/restart/replay/quality/atomic-replacement/truncation-retention/failure contracts pass; live scheduling remains TM-FUNC-008/P1 |
| TM-FUNC-008 | partial | provider-neutral drafts/engine plus built-in Codex live runtime, immutable publication, and Windows power binding; external plugin host pending | P1-C, P1-D.0-P1-D.6, and P1-E.1-P1-E.3 logical-file/codec/bootstrap/exact-scan/CAS/recovery/lease/scheduling/lifecycle/publication/power contracts; external plugin conformance remains |
| TM-FUNC-009 | partial | exact quota domain/evaluator, strict schema-v10 storage, bounded retention, defensive reads, immutable public query facade, built-in official Codex quota normalizer/transport, and separate quota runtime publication; UI planned | prior quota gates plus exact native discovery, I/O-before-lease execution, bounded idempotent store publication, separate health/lifecycle, normal/accelerated cadence, concurrent usage/quota fault isolation, fail-closed public smoke, 16+48-round Windows runtime resource plateau, and release authority audit pass; quota UI remains |
| TM-FUNC-010 | partial | strict provider-neutral benefit lots, Codex reset-credit normalization, schema-v12 inventory/history/profile/due/outbox/ack storage, immutable current/history query facade, Codex publication, and one-timer durable in-app event runtime; visible UI and activation later | prior gates plus 256-row atomic due processing, durable urgent-threshold suppression, outbox-before-event, pre-ack restart replay, post-ack deduplication, release/retry, hint/resume/clock coalescing, backpressure, contention-before-SQLite, joined lifecycle, resource plateau, and release audit pass; P3 rendering/OS delivery/activation remain |
| TM-FUNC-011 | partial | strict Git domain/core/backend/hints, schema-v13 transactional projection, bounded runtime publication, immutable public query/efficiency facade, and joined product status | P2-E Tasks 1-8 plus P2-F exact scalar status/runtime/route fault-isolation gates pass; P3 visible Git card remains |
| TM-UI-001 | partial | P3-A original production Slint header/navigation/state shell; complete board/supporting views remain | exact 11-route compiled UI and in-place selection pass with no mock data; dashboard/routes/keyboard/accessibility remain P3-C-P4 |
| TM-UI-002 | partial | immutable runtime/query/product publication, P3-A one-current projection, P3-B.1 one-current controller snapshot, and P3-B.2 generation-ordered Slint delivery | prior gates plus attempt/receipt separation, one reducer, complete-attempt publication, cancellation/deadline discard, latest-only result, stale rejection, and real headless event-loop application pass; production composition and P4 switches remain |
| TM-PERF-001 | partial | bounded backend plus one-current product/desktop projections, one desktop query worker/latest slot, and one capacity-one event gate | prior gates plus 1,000 hints to one follow-up, 10,000 notifications to one queued event/newest delivery, one worker/slot/event audit, zero bridge polling, and no query call on the UI adapter pass; UI switch/resource and long-run release evidence remain P4/P6 |
| TM-PERF-002 | open evidence | software renderer and M0 resource gates | uninterrupted soak and interactive receipts remain absent |
| TM-PERF-003 | partial | immutable engine/product publication, indexed facades, stale-safe desktop projection, bounded query controller, and newest-only event-loop bridge | prior scale evidence plus exact controller attempt generations, coalesced follow-up/event scheduling, section-local failure, off-UI query ownership, race recheck, weak-window cleanup, and headless Slint delivery; P4 visible-paint evidence remains |
| TM-REL-001 | partial | M0 scripts and receipt schemas | identity checks exist; final product packaging evidence pending |
| TM-REL-002 | open evidence | `M0_ACCEPTANCE.md` | interactive Windows/DPI/accessibility and uninterrupted soak receipts absent |
| TM-REL-003 | planned | P6 explicit MSVC signed portable package and supply-chain gates | GNU/MSVC comparison, notices/SBOM, advisory/source/license/secret/action/attestation audits, deterministic package and clean-room launch pending |
| TM-DATA-001 | partial | domain/provider/Codex/store privacy boundaries, pseudonymous quota account normalization, count-only runtime health, and identity-free joined product status | prior privacy gates plus P2-F release-library scan proves no path, source/account/window/lot/repository/project identity, raw payload, quota/benefit value, inner error, or vendored upstream source escapes; P3/P5 must repeat gates |
| TM-DATA-002 | implemented | domain drafts plus exclusive `tokenmaster-accounting` canonicalizer | canonicalizer vectors, compile-fail authority tests, Codex/store contracts |
| TM-DATA-003 | implemented | file identity and reader checkpoint | physical identity live/persisted round-trip, checkpoint conversion, resume bound, and restart contracts |
| TM-DATA-004 | implemented | scoped scan/rebuild plus replay-aware current publication, paired-CAS tail facts, exact admission, durable partial/recovery, retained promotion/discard | atomic faults, stale CAS, unchanged/append/multi-batch/new/missing/restart/deadline/rebuild contracts pass |
| TM-DATA-005 | implemented | writable usage/quota/benefit/Git store plus separate `UsageReadStore` | strict schema v13 retains prior facts and adds independent Git state plus one exact scalar cross-family product-status transaction with defensive deadline/corruption/progress cleanup and no historical scan |
| TM-DATA-006 | partial | bounded backend, one-current product snapshot/route model, one-worker/one-latest-slot desktop controller, and shared-mailbox event bridge | prior limits plus fixed projection, typed capped requests, one active/one pending refresh, 1,000-hint/latest-only and 10,000-notification/one-event tests, one shared slot, and controller/bridge source audit pass; paged route payload and P4 switch limits remain |
| TM-DATA-007 | implemented | replay facts/classifier in a private overlay plus schema-v4 self-contained canonical projection with deterministic selection/retention | v1/v2/v3-to-v4 migration plus replay/append/restart/300-file/atomic-replacement/truncation truth-table/failure contracts pass |
| TM-DATA-008 | implemented | exact quota values/evaluator, strict schema-v10 transactional storage, bounded retention, defensive reads, and immutable public quota facade | complete domain/write/retention/read/query contracts plus request ordering, explicit unavailable windows, provider freshness, quality aggregation, generation neutrality, adversarial inference matrix, restart/maintenance/current-and-legacy coexistence, 1,000-transition paging, resource plateau, and offline authority audit pass |
| TM-DATA-009 | partial | typed provider benefit inventory, pure reconciliation/reminder planning, built-in Codex normalization, strict schema-v12 current/history/profile/due/outbox/ack foundation, immutable query facade, quota publication, and durable in-app event runtime; activation later | prior evidence plus bounded store-owned due and acknowledgement transactions, outbox-before-event, unacknowledged restart replay, acknowledged deduplication, profile-rebuild survival, future urgent preservation, expired drain, nearest-due scheduling, capacity-one leased batch, lifecycle/contention/resource/audit gates pass; P3 visible delivery and activation intents/receipts remain |
| TM-DATA-010 | implemented | non-serializable repository hints, bounded opaque Git values, durable schema-v13 projection, bounded runtime population, and immutable public join | local-path side-channel privacy, salted identities, 32-repository/4,096-association/400-day/8-category/16-warning caps, immutable generations, exact append/rebuild/unavailable/stale truth, bounded read/join/runtime/resource/authority gates pass |
| TM-SEC-001 | partial | local-only product, no listener, deterministic offline pricing/quota/Git core, credential-blind official Codex child transport, and shell-free quota/Git runtimes | specialized audits prove Git has exact native read-only process authority only and no HTTP/browser/cookie/private-endpoint/credential-file/shell/socket/direct-runtime-SQL or mutation path; future MCP stdio network-denial tests pending |
| TM-SEC-002 | partial | current JSONL/store/query/product boundaries plus strict app-server, quota-benefit, reminder, Git, desktop-controller, and Slint-bridge surfaces validate type, size, evidence, identity, continuity, deadline, ordering, isolation, and redaction | prior gates plus P3-B.1 typed-only query source and P3-B.2 one shared slot/one event/weak window/zero polling, cancellation/deadline partial-publication rejection, path-free stable failures, no UI query/direct store-runtime authority, and release-string privacy pass; P3-B.3-P5/plugin suites pending |
| TM-SEC-003 | implemented | provider/Codex/store/engine errors, value types, redacted worker panic boundary, redacted quota surfaces, and sealed repository activity paths | prior privacy evidence plus non-serializable repository hints, canonical local-only namespace/reparse validation, latest-only transport, invalid-candidate clearing, and checkpoint/archive/Debug path exclusion pass |
| TM-SEC-004 | implemented | transactional archive authority, exact rebuild/recovery, OS lease, pathless watcher, ordered live lifecycle, static power callback | P1-D.3 rollback/recovery, P1-D.4 process lease, P1-D.5 callback privacy, P1-D.6 lease-first recovery/admission/shutdown, and P1-E.3 power isolation contracts |
| TM-SEC-005 | partial | M0 skins are declarative application data | external skin package schema/validation not implemented |
| TM-SEC-006 | planned | built-in Codex exists; isolated plugin host deferred | provider plugin design and future 1.1 conformance/security gates |
| TM-SEC-007 | planned | host-owned banked reset activation capability and policy boundary | no-scrape/no-authority-escalation/idempotency/ambiguous-outcome security gates pending |

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
