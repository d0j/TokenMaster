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
| TM-FUNC-007 | implemented | accounting lineage/classifier plus scalable replay archive and P0-E composition proof | real JSONL baseline/append/restart/replay/quality/atomic-replacement/failure contracts pass; live scheduling remains TM-FUNC-008/P1 |
| TM-FUNC-008 | partial | built-in provider and neutral draft seam implemented; engine/plugin host pending | provider/Codex/accounting contracts; P1 and 1.1 remain |
| TM-UI-001 | planned | complete Slint board and supporting views | granular parity matrix and P4 accessibility/UI tests |
| TM-UI-002 | partial | `crates/probe-app` presentation generations | presentation/skin contracts; archive-independent product snapshots pending |
| TM-PERF-001 | partial | parser, reader, domain, store bounds plus P0-E bounded composition | 300-file/300-event, 256-batch/page, reopen contracts pass; future engine/query/plugin bounds pending |
| TM-PERF-002 | open evidence | software renderer and M0 resource gates | uninterrupted soak and interactive receipts remain absent |
| TM-PERF-003 | partial | keyset store reads implemented; immutable snapshots planned | SQLite/read contracts; P2 query snapshot gates pending |
| TM-REL-001 | partial | M0 scripts and receipt schemas | identity checks exist; final product packaging evidence pending |
| TM-REL-002 | open evidence | `M0_ACCEPTANCE.md` | interactive Windows/DPI/accessibility and uninterrupted soak receipts absent |
| TM-DATA-001 | partial | domain/provider/Codex/store privacy boundaries | adversarial/debug/path privacy tests; future surfaces must repeat gates |
| TM-DATA-002 | implemented | domain drafts plus exclusive `tokenmaster-accounting` canonicalizer | canonicalizer vectors, compile-fail authority tests, Codex/store contracts |
| TM-DATA-003 | implemented | file identity and reader checkpoint | physical identity live/persisted round-trip, checkpoint conversion, resume bound, and restart contracts |
| TM-DATA-004 | implemented | atomic current append, SQLite-owned all-source staging, exact preparation, paged seal, rollback-safe promotion, and staging discard | migration/fault/all-source/300-source/P0-E lifecycle/reopen/cancellation contracts pass |
| TM-DATA-005 | implemented | `crates/store/src/usage` | strict schema, pragmas, keyset paging, ingest contracts |
| TM-DATA-006 | partial | reader/parser/store limits | line/resume/batch/page bounds; full UI/query/plugin limits pending |
| TM-DATA-007 | implemented | replay facts/classifier persisted in a strict v3 private overlay with deterministic selection, bounded reconciliation, seal, promotion, and real Codex composition | v1/v2/v3 migration plus baseline/replay/append/restart/300-file/atomic-replacement/failure contracts pass |
| TM-SEC-001 | partial | local-only product and no listener today | future quota HTTPS opt-in and MCP stdio security tests pending |
| TM-SEC-002 | partial | current JSONL/store boundaries validate types and sizes | future config/CLI/MCP/plugin boundary suites pending |
| TM-SEC-003 | implemented | provider/Codex/store errors and value types | serialized/debug privacy and path-redaction contracts |
| TM-SEC-004 | partial | transactional CAS/preparation, immutable legacy, exact paged seal, atomic promotion, rollback, and exact-epoch discard | P0-E proves cancellation/malformed/incomplete/truncation fail closed; P1 still owns live runtime and carry-forward policy |
| TM-SEC-005 | partial | M0 skins are declarative application data | external skin package schema/validation not implemented |
| TM-SEC-006 | planned | built-in Codex exists; isolated plugin host deferred | provider plugin design and future 1.1 conformance/security gates |

The approved audit resolutions and delivery order are in
`docs/AUDIT_AND_MASTER_PLAN.md`. P0-A/P0-B and P0-C have completed executable plans.
P0-D transaction, classification, seal, promotion, rollback, and recovery semantics
are implemented. P0-D.1 removed the historical 256-file product blocker with exact
schema-v2-to-v3 migration, disk-backed all-source begin, checked 64-bit counts, and
256-row keyset-paged validation. P0-E proves the real synthetic Codex-to-archive path,
including bounded restart, atomic replacement, failure discard, totals, and quality.
The production runtime engine and explicit prior-evidence carry-forward remain P1.
Tasks 3+ in the older replay plan are historical and superseded.

The clean-root invariant is implemented by `scripts/audit-clean-root.ps1` and its
Pester contracts.
