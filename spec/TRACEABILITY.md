# TokenMaster traceability

Status values are explicit: `implemented`, `partial`, `planned`, or `open evidence`.
A design or plan is not implementation evidence.

| Requirement | Status | Implementation or planned owner | Evidence or next gate |
| --- | --- | --- | --- |
| TM-FUNC-001 | implemented | `crates/provider`, Codex roots/files | provider, discovery, enumeration contracts |
| TM-FUNC-002 | implemented | `crates/codex/src/reader` | framing, checkpoint, append/truncate/rewrite/revalidation contracts |
| TM-FUNC-003 | partial | domain/accounting/Codex parser; pricing/analytics planned | usage, canonicalizer, parser-state, parser-adversarial contracts |
| TM-FUNC-004 | planned | query snapshots and complete Slint product routes | P4 UI plan after P2/P3 contracts |
| TM-FUNC-005 | partial | `crates/probe-app`; product shell later | lifecycle, presentation, skin-runtime, metrics, stress contracts |
| TM-FUNC-006 | planned | separate CLI and MCP adapters over query facade | P3 strict JSON/stdin MCP conformance tests |
| TM-FUNC-007 | implemented | accounting lineage/classifier, scalable replay archive, P0-E composition, and P1-A retention | real JSONL baseline/append/restart/replay/quality/atomic-replacement/truncation-retention/failure contracts pass; live scheduling remains TM-FUNC-008/P1 |
| TM-FUNC-008 | partial | built-in provider-neutral drafts plus constant-state coordinator, corrected per-file/two-pass synchronous ports, one-shot executor, and deterministic worker; live Codex composition and plugin host pending | provider/Codex/accounting contracts plus P1-C engine and P1-D.0 logical-file/temporary-reader/exact-second-pass contracts |
| TM-FUNC-009 | planned | immutable provider quota epochs and weekly full-reset transitions | P2 quota reset plan; scheduled/early/repeated/reset+allowance/restart/UI/API fixtures pending |
| TM-FUNC-010 | planned | banked reset lots, selectable default/custom expiry reminders, activation intents/receipts | P2 banked reset plan; inventory/profile/reminder/reconciliation/UI/security fixtures pending |
| TM-UI-001 | planned | complete Slint board and supporting views | granular parity matrix and P4 accessibility/UI tests |
| TM-UI-002 | partial | `crates/probe-app` presentation generations | presentation/skin contracts; archive-independent product snapshots pending |
| TM-PERF-001 | partial | parser, reader, domain, store bounds, P0-E composition, P1-C engine core, and P1-D.0 two-pass repair | 300 shared-root logical files use two linear passes, one live temporary reader, no engine descriptor/page list, 256 observation/relation batches, 32-KiB checkpoints, 18 chunk updates, 4,096 continuation cap, reopen, capacity-one wake/result channels, and 10,000-hint one-follow-up contracts pass; live/query/plugin runtime evidence pending |
| TM-PERF-002 | open evidence | software renderer and M0 resource gates | uninterrupted soak and interactive receipts remain absent |
| TM-PERF-003 | partial | keyset store reads implemented; immutable snapshots planned | SQLite/read contracts; P2 query snapshot gates pending |
| TM-REL-001 | partial | M0 scripts and receipt schemas | identity checks exist; final product packaging evidence pending |
| TM-REL-002 | open evidence | `M0_ACCEPTANCE.md` | interactive Windows/DPI/accessibility and uninterrupted soak receipts absent |
| TM-DATA-001 | partial | domain/provider/Codex/store privacy boundaries | adversarial/debug/path privacy tests; future surfaces must repeat gates |
| TM-DATA-002 | implemented | domain drafts plus exclusive `tokenmaster-accounting` canonicalizer | canonicalizer vectors, compile-fail authority tests, Codex/store contracts |
| TM-DATA-003 | implemented | file identity and reader checkpoint | physical identity live/persisted round-trip, checkpoint conversion, resume bound, and restart contracts |
| TM-DATA-004 | implemented | atomic current append, scoped complete-only scan finalization, scan-bound and compatibility staging, bounded reference-safe scan pruning, exact preparation, atomic replay event/relation facts, paged seal, retained projection promotion, and staging discard | lifecycle/begin-fault/append-fault/membership/reopen/zero-source/300-source/repeated-scan/backlog/Codex pipeline contracts pass |
| TM-DATA-005 | implemented | `crates/store/src/usage` | strict schema v5, exact v1/v2/v3/v4 migration, provider-qualified scan sets, pragmas, keyset paging, ingest contracts |
| TM-DATA-006 | partial | reader/parser/store plus engine value/batch/temporary-reader/execution/worker limits | line/resume, 32-KiB opaque checkpoint, independent 256 event/relation batches, one temporary replay reader, 18 chunk-update, 4,096 continuation, fixed-result, one active/aggregate follow-up, capacity-one channels, and SQLite-i64 counter bounds; full UI/query/plugin limits pending |
| TM-DATA-007 | implemented | replay facts/classifier in a private overlay plus schema-v4 self-contained canonical projection with deterministic selection/retention | v1/v2/v3-to-v4 migration plus replay/append/restart/300-file/atomic-replacement/truncation truth-table/failure contracts pass |
| TM-DATA-008 | planned | immutable quota samples, epochs, reset and allowance transitions | P2 quota reset history schema/detection/retention contracts pending |
| TM-DATA-009 | planned | typed provider benefit inventory, versioned reminder profiles/delivery, activation intent/receipt projection | P2 banked reset schema/expiry/profile/dedup/CAS/retention contracts pending |
| TM-SEC-001 | partial | local-only product and no listener today | future quota HTTPS opt-in and MCP stdio security tests pending |
| TM-SEC-002 | partial | current JSONL/store boundaries validate types and sizes | future config/CLI/MCP/plugin boundary suites pending |
| TM-SEC-003 | implemented | provider/Codex/store/engine errors, value types, and redacted worker panic boundary | serialized/debug privacy, path-redaction, sealed identity, path-substitution, raw-archive-write compile-fail, fixed panic/fault completion, and panic-strategy compile guard contracts |
| TM-SEC-004 | partial | transactional scoped scan/replay authority, bounded pruning, CAS/preparation, atomic fact batches, immutable legacy, exact seal/carry-forward/discard, and one-shot fail-closed composition | P1-D.0 adds exact per-file/two-pass failure handling; P1-D.1 proves event/relation/selection/work/chunk/checkpoint/epoch rollback and one epoch advance per accepted batch; live Codex runtime remains |
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
P1-D.1 now applies replay events and late relations as one bounded atomic fact batch
with two rollback boundaries and one epoch increment. P1-D.2 bootstrap Codex
composition is next, followed by incremental live composition and the real OS writer
lease.
Tasks 3+ in the older replay plan are historical and superseded.

The clean-root invariant is implemented by `scripts/audit-clean-root.ps1` and its
Pester contracts.
