# TokenMaster traceability

Status values are explicit: `implemented`, `partial`, `planned`, or `open evidence`.
A design or plan is not implementation evidence.

| Requirement | Status | Implementation or planned owner | Evidence or next gate |
| --- | --- | --- | --- |
| TM-FUNC-001 | implemented | `crates/provider`, Codex roots/files | provider, discovery, enumeration contracts |
| TM-FUNC-002 | implemented | Codex reader plus store/runtime incremental path | zero-payload unchanged, exact tail bytes, multi-batch restart, new/missing source, replacement/truncation/profile-scope and malformed-input recovery contracts |
| TM-FUNC-003 | partial | domain/accounting/Codex parser, schema-v9 price facts, pure pricing engine, and indexed aggregate query facade | exact token/cost overview, daily-series, breakdown, session page/detail mapping with availability, provenance, override, conflict, and unknown-price fixtures; UI/CLI/MCP presentation remains |
| TM-FUNC-004 | planned | query snapshots and complete Slint product routes | row-level parity ledger plus P3 UI plan after P2 query contracts |
| TM-FUNC-005 | partial | `crates/probe-app`; product shell later | lifecycle, presentation, skin-runtime, metrics, stress contracts |
| TM-FUNC-006 | planned | separate CLI and MCP adapters over query facade | P5 strict JSON/stdin MCP conformance tests after the complete UI |
| TM-FUNC-007 | implemented | accounting lineage/classifier, scalable replay archive, P0-E composition, and P1-A retention | real JSONL baseline/append/restart/replay/quality/atomic-replacement/truncation-retention/failure contracts pass; live scheduling remains TM-FUNC-008/P1 |
| TM-FUNC-008 | partial | provider-neutral drafts/engine plus built-in Codex live runtime, immutable publication, and Windows power binding; external plugin host pending | P1-C, P1-D.0-P1-D.6, and P1-E.1-P1-E.3 logical-file/codec/bootstrap/exact-scan/CAS/recovery/lease/scheduling/lifecycle/publication/power contracts; external plugin conformance remains |
| TM-FUNC-009 | partial | exact quota domain/evaluator plus strict schema-v10 transactional and bounded-retention storage; read/query/UI planned | bounded IDs, ppm ratios, optional units, deterministic identities, reset/allowance evaluation, exact v9 rollback migration, immutable transactional writes, duplicate/stale no-op, evidence-preserving 512/256 soft retention, 2,048/1,024 hard caps, 256-row maintenance, repeated reset/reopen/account isolation, publication/maintenance rollback, and projection/over-cap fail-closed contracts pass; read/query/provider/UI pending |
| TM-FUNC-010 | planned | banked reset lots, selectable default/custom expiry reminders, activation intents/receipts | P2 banked reset plan; inventory/profile/reminder/reconciliation/UI/security fixtures pending |
| TM-UI-001 | planned | complete Slint board and supporting views | granular parity ledger and P3 accessibility/UI tests |
| TM-UI-002 | partial | `crates/probe-app`, immutable runtime publication, and P2-A query snapshots | strictly newer consumer predicate, exact archive identity/data-through, equal/older rejection, no-change cursor continuity, and one-retained-envelope contract pass; product presentation snapshots pending |
| TM-PERF-001 | partial | bounded parser/reader/store/engine plus live runtime and query facade | unchanged payload bytes=0; exact tail; 300-event batches; 10,000-hint aggregate/one follow-up; one retained result across 10,000 candidates; 10,000 equivalent quota polls plateau at protected first/latest samples; watcher/live/power baselines and 256-cycle query resource plateau pass; UI/plugin evidence pending |
| TM-PERF-002 | open evidence | software renderer and M0 resource gates | uninterrupted soak and interactive receipts remain absent |
| TM-PERF-003 | partial | immutable engine publication, P2-A activity query, schema-v9 transactional token/price aggregates, and immutable cost facade | current/legacy P2-C million gates: rebuild 8,737/8,129 events/s, page p95 376.824/406.604 ms, amplification 1.862x/2.010x, cached overview p95 2.040/2.065 ms, 400-point/four-breakdown p95 148.168/156.080 ms, all-32-scope 158.588/162.504 ms, session page p95 below 14 ms and detail below 1 ms; isolated single-thread resource gate uses bounded topology-stable/converged warm-up and preserves 1/2 MiB plus structural bounds; P3 UI remains |
| TM-REL-001 | partial | M0 scripts and receipt schemas | identity checks exist; final product packaging evidence pending |
| TM-REL-002 | open evidence | `M0_ACCEPTANCE.md` | interactive Windows/DPI/accessibility and uninterrupted soak receipts absent |
| TM-REL-003 | planned | P6 explicit MSVC signed portable package and supply-chain gates | GNU/MSVC comparison, notices/SBOM, advisory/source/license/secret/action/attestation audits, deterministic package and clean-room launch pending |
| TM-DATA-001 | partial | domain/provider/Codex/store privacy boundaries | adversarial/debug/path privacy tests; future surfaces must repeat gates |
| TM-DATA-002 | implemented | domain drafts plus exclusive `tokenmaster-accounting` canonicalizer | canonicalizer vectors, compile-fail authority tests, Codex/store contracts |
| TM-DATA-003 | implemented | file identity and reader checkpoint | physical identity live/persisted round-trip, checkpoint conversion, resume bound, and restart contracts |
| TM-DATA-004 | implemented | scoped scan/rebuild plus replay-aware current publication, paired-CAS tail facts, exact admission, durable partial/recovery, retained promotion/discard | atomic faults, stale CAS, unchanged/append/multi-batch/new/missing/restart/deadline/rebuild contracts pass |
| TM-DATA-005 | implemented | writable usage/quota store plus separate `UsageReadStore` | strict schema v10 retaining dataset/token/price contracts plus quota-owned objects, exact v8 and v9 rollback migrations, and read-only query-only defensive 4-MiB/no-migration/no-checkpoint policy pass |
| TM-DATA-006 | partial | reader/parser/store, engine/runtime, pure pricing engine, and immutable P2 query facade | prior limits plus 401-target/512-key range cost batches, 256-target breakdown/session batches, exact omitted counters, calendar composition, optional 400-point daily series, four breakdowns, scope-bound cursors, 32 scopes, 16 warnings, two-second reads, and owned public values pass; UI/plugin limits pending |
| TM-DATA-007 | implemented | replay facts/classifier in a private overlay plus schema-v4 self-contained canonical projection with deterministic selection/retention | v1/v2/v3-to-v4 migration plus replay/append/restart/300-file/atomic-replacement/truncation truth-table/failure contracts pass |
| TM-DATA-008 | partial | exact quota values/evaluator plus strict schema-v10 transactional quota storage and bounded retention | constructors and detector reject invalid/mismatched state; exact v9 migration seeds empty quota revision without usage/price mutation; strict tables, immediate writer, and maintenance enforce bounded fields, exact same-window evidence, global observation identity, immutable definitions, semantic allowance changes, protected first/last/max/pre/post samples, 512/256 soft defaults, 2,048/1,024 hard caps, 256-row same-window compaction, exact current projection, revision/sequence bounds, publication/maintenance rollback, deterministic retry, and over-cap reopen rejection; read contracts pending |
| TM-DATA-009 | planned | typed provider benefit inventory, versioned reminder profiles/delivery, activation intent/receipt projection | P2 banked reset schema/expiry/profile/dedup/CAS/retention contracts pending |
| TM-SEC-001 | partial | local-only product, no listener, and deterministic offline pricing | pricing/query dependency/source/release-library audit proves no runtime pricing network path; permitted credential-free local/official quota source and future MCP stdio network-denial tests pending |
| TM-SEC-002 | partial | current JSONL/store boundaries plus exact quota domain, detector, strict quota schema, transactional writer, and fixed retention validate types, sizes, evidence, identity, and state continuity | quota values/evaluator reject invalid or inferred state; schema/write/maintenance path rejects cross-window evidence, incomplete allowance facts, observation/definition reuse, missing current projection, weakened schema, migration residue, partial publication, unbounded pages, evidence deletion, and persisted hard-cap breach; future provider/config/CLI/MCP/plugin boundary suites pending |
| TM-SEC-003 | implemented | provider/Codex/store/engine errors, value types, redacted worker panic boundary, and redacted quota identities | serialized/debug privacy, path-redaction, sealed identity, path-substitution, raw-archive-write compile-fail, redacted observation/scope/epoch/transition identity, stable quota error-code, fixed panic/fault completion, and panic-strategy compile guard contracts |
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
P2-D Tasks 1-5 exact quota values, pure detector, strict schema-v10 foundation,
transactional history writes, and evidence-preserving bounded retention are
implemented; defensive reads/query, transport, and reset inventory remain. P3 UI
evidence remains separate.
M0 interactive/
soak evidence remains.
Tasks 3+ in the older replay plan are historical and superseded.

The clean-root invariant is implemented by `scripts/audit-clean-root.ps1` and its
Pester contracts.
