# TokenMaster traceability

Status values are explicit: `implemented`, `partial`, `planned`, or `open evidence`.
A design or plan is not implementation evidence.

| Requirement | Status | Implementation or planned owner | Evidence or next gate |
| --- | --- | --- | --- |
| TM-FUNC-001 | implemented | `crates/provider`, Codex roots/files | provider, discovery, enumeration contracts |
| TM-FUNC-002 | implemented | `crates/codex/src/reader` | framing, checkpoint, append/truncate/rewrite/revalidation contracts |
| TM-FUNC-003 | partial | domain usage, Codex parser; pricing/analytics planned | usage, parser-state, parser-adversarial contracts |
| TM-FUNC-004 | planned | query snapshots and complete Slint product routes | P4 UI plan after P2/P3 contracts |
| TM-FUNC-005 | partial | `crates/probe-app`; product shell later | lifecycle, presentation, skin-runtime, metrics, stress contracts |
| TM-FUNC-006 | planned | separate CLI and MCP adapters over query facade | P3 strict JSON/stdin MCP conformance tests |
| TM-FUNC-007 | partial | domain lineage values exist; accounting/store replay pending | P0-A through P0-E plans and replay fixtures |
| TM-FUNC-008 | partial | built-in provider/discovery exists; neutral draft/engine/plugin host pending | P0-A/P1; plugin design is 1.1 planning only |
| TM-UI-001 | planned | complete Slint board and supporting views | granular parity matrix and P4 accessibility/UI tests |
| TM-UI-002 | partial | `crates/probe-app` presentation generations | presentation/skin contracts; archive-independent product snapshots pending |
| TM-PERF-001 | partial | parser, reader, domain, store bounds | adversarial/fixed-capacity tests; future engine/query/plugin bounds pending |
| TM-PERF-002 | open evidence | software renderer and M0 resource gates | uninterrupted soak and interactive receipts remain absent |
| TM-PERF-003 | partial | keyset store reads implemented; immutable snapshots planned | SQLite/read contracts; P2 query snapshot gates pending |
| TM-REL-001 | partial | M0 scripts and receipt schemas | identity checks exist; final product packaging evidence pending |
| TM-REL-002 | open evidence | `M0_ACCEPTANCE.md` | interactive Windows/DPI/accessibility and uninterrupted soak receipts absent |
| TM-DATA-001 | partial | domain/provider/Codex/store privacy boundaries | adversarial/debug/path privacy tests; future surfaces must repeat gates |
| TM-DATA-002 | partial | current canonical event exists but authority boundary is unsafe | P0-A replaces it with draft plus exclusive canonicalizer |
| TM-DATA-003 | implemented | file identity and reader checkpoint | physical identity, checkpoint, resume bound contracts |
| TM-DATA-004 | partial | atomic current append implemented; staging planned | usage-ingest rollback/CAS tests; P0-D/P1 promotion pending |
| TM-DATA-005 | implemented | `crates/store/src/usage` | strict schema, pragmas, keyset paging, ingest contracts |
| TM-DATA-006 | partial | reader/parser/store limits | line/resume/batch/page bounds; full UI/query/plugin limits pending |
| TM-DATA-007 | partial | domain replay values only | P0-B through P0-E classifier/schema/pipeline tests pending |
| TM-SEC-001 | partial | local-only product and no listener today | future quota HTTPS opt-in and MCP stdio security tests pending |
| TM-SEC-002 | partial | current JSONL/store boundaries validate types and sizes | future config/CLI/MCP/plugin boundary suites pending |
| TM-SEC-003 | implemented | provider/Codex/store errors and value types | serialized/debug privacy and path-redaction contracts |
| TM-SEC-004 | partial | current append transaction/CAS | staging promotion and failed-scan reconciliation tests pending |
| TM-SEC-005 | partial | M0 skins are declarative application data | external skin package schema/validation not implemented |
| TM-SEC-006 | planned | built-in Codex exists; isolated plugin host deferred | provider plugin design and future 1.1 conformance/security gates |

The approved audit resolutions and delivery order are in
`docs/AUDIT_AND_MASTER_PLAN.md`. P0-A is executable through
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-authority-boundary.md`. Tasks 3+
in the older replay plan are historical and superseded.

The clean-root invariant is implemented by `scripts/audit-clean-root.ps1` and its
Pester contracts.
