# TokenMaster traceability

| Requirement | Implementation | Evidence |
| --- | --- | --- |
| TM-FUNC-001 | `crates/provider`, `crates/codex/src/roots.rs`, `files/` | provider, discovery, enumeration contracts |
| TM-FUNC-002 | `crates/codex/src/reader`, `reader_revalidation_contract.rs` | framing, checkpoint, append/truncate/rewrite contracts |
| TM-FUNC-003 | `crates/domain/src/usage.rs`, `crates/codex/src/parser` | usage, parser-state, parser-adversarial contracts |
| TM-FUNC-005 / TM-UI-002 | `crates/probe-app` | lifecycle, presentation, skin-runtime, metrics, stress contracts |
| TM-PERF-001 | parser, reader, domain bounds | adversarial and fixed-capacity contracts |
| TM-PERF-003 / TM-DATA-005 | `crates/store/src/usage` | schema, SQLite, ingest contracts |
| TM-DATA-003 | `crates/codex/src/file_identity.rs`, reader checkpoint | physical identity and reader contracts |
| TM-DATA-004 current append | `crates/store/src/usage/write.rs` | usage-ingest rollback/CAS/determinism contracts |
| TM-SEC-003 | provider/codex/store error and type boundaries | serialized/debug privacy contracts |
| TM-REL-001 / TM-REL-002 | `scripts/`, `M0_ACCEPTANCE.md` | Pester M0 script and soak-helper contracts; external receipts remain open |
| Clean-root invariant | `scripts/audit-clean-root.ps1` | audit-clean-root Pester contracts and root developer gate |
| TM-FUNC-007 / TM-DATA-007 | P0 plan only | replay lineage, disposition, and migration implementation pending |
| TM-FUNC-008 / TM-SEC-006 | architecture contract only | local Codex adapter exists; provider-neutral engine ports pending |

Staging generation promotion, scan epochs, full analytics, quota transport, all product
views, CLI, and MCP have no implementation row yet and must be added test-first.

The chosen design for replay correctness, runtime engine, immutable queries, universal
MCP/CLI automation, full UI, dynamic bars, modular presentation, localization, and
release gates is recorded in
`docs/superpowers/specs/2026-07-14-tokenmaster-product-architecture-design.md`.
This is design traceability only; it is not implementation evidence.

The approved P0 execution breakdown and its focused validators are recorded in
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-replay-correctness.md`. The plan also
preserves a provider-neutral ingest boundary: local Codex input is first, while later
allowlisted source adapters must terminate at the same bounded observation contract.
This is planning evidence, not a completed replay or adapter implementation row.
