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
| TM-FUNC-007 | partial | accounting lineage/classifier plus v2 replay archive, staged append, and durable late-relation continuation implemented | replay contracts cover restart, deterministic relation identity, depth/fanout bounds, cycles, stale work, and selection invalidation; seal/promotion and P0-E remain |
| TM-FUNC-008 | partial | built-in provider and neutral draft seam implemented; engine/plugin host pending | provider/Codex/accounting contracts; P1 and 1.1 remain |
| TM-UI-001 | planned | complete Slint board and supporting views | granular parity matrix and P4 accessibility/UI tests |
| TM-UI-002 | partial | `crates/probe-app` presentation generations | presentation/skin contracts; archive-independent product snapshots pending |
| TM-PERF-001 | partial | parser, reader, domain, store bounds | adversarial/fixed-capacity tests; future engine/query/plugin bounds pending |
| TM-PERF-002 | open evidence | software renderer and M0 resource gates | uninterrupted soak and interactive receipts remain absent |
| TM-PERF-003 | partial | keyset store reads implemented; immutable snapshots planned | SQLite/read contracts; P2 query snapshot gates pending |
| TM-REL-001 | partial | M0 scripts and receipt schemas | identity checks exist; final product packaging evidence pending |
| TM-REL-002 | open evidence | `M0_ACCEPTANCE.md` | interactive Windows/DPI/accessibility and uninterrupted soak receipts absent |
| TM-DATA-001 | partial | domain/provider/Codex/store privacy boundaries | adversarial/debug/path privacy tests; future surfaces must repeat gates |
| TM-DATA-002 | implemented | domain drafts plus exclusive `tokenmaster-accounting` canonicalizer | canonicalizer vectors, compile-fail authority tests, Codex/store contracts |
| TM-DATA-003 | implemented | file identity and reader checkpoint | physical identity, checkpoint, resume bound contracts |
| TM-DATA-004 | partial | atomic identity-checked current append plus fixed-manifest replay staging implemented | current/replay append rollback, CAS, provider/version mismatch, and staging-invisibility tests; seal/promotion and P1 runtime integration pending |
| TM-DATA-005 | implemented | `crates/store/src/usage` | strict schema, pragmas, keyset paging, ingest contracts |
| TM-DATA-006 | partial | reader/parser/store limits | line/resume/batch/page bounds; full UI/query/plugin limits pending |
| TM-DATA-007 | partial | replay facts/classifier persisted in a strict v2 private overlay with deterministic selection and bounded durable reconciliation | migration/read/append/relation/restart/keyset/cycle contracts pass; seal/promotion and P0-E remain |
| TM-SEC-001 | partial | local-only product and no listener today | future quota HTTPS opt-in and MCP stdio security tests pending |
| TM-SEC-002 | partial | current JSONL/store boundaries validate types and sizes | future config/CLI/MCP/plugin boundary suites pending |
| TM-SEC-003 | implemented | provider/Codex/store errors and value types | serialized/debug privacy and path-redaction contracts |
| TM-SEC-004 | partial | current/replay transactions and CAS, immutable legacy snapshot, fixed manifest, version/work-epoch fail-closed checks | append/relation/continuation rollback, restart, and invisibility tests pass; seal/promotion failure injection and failed-scan reconciliation remain |
| TM-SEC-005 | partial | M0 skins are declarative application data | external skin package schema/validation not implemented |
| TM-SEC-006 | planned | built-in Codex exists; isolated plugin host deferred | provider plugin design and future 1.1 conformance/security gates |

The approved audit resolutions and delivery order are in
`docs/AUDIT_AND_MASTER_PLAN.md`. P0-A/P0-B and P0-C have completed executable plans.
P0-D schema, migration, archive state, fixed-manifest staging, classified append, and
bounded late-relation continuation are implemented; atomic seal/promotion is next.
Tasks 3+ in the older replay plan are historical and superseded.

The clean-root invariant is implemented by `scripts/audit-clean-root.ps1` and its
Pester contracts.
