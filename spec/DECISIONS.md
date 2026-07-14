# TokenMaster decisions

## ADR-001 — Single-root native workspace

Decision: TokenMaster has one root Rust workspace. Rationale: one build graph,
unambiguous ownership, no cross-project runtime dependency, and reliable verification.

## ADR-002 — Reference hierarchy

Decision: WhereMyTokens guides UI/product completeness and ccusage guides usage
analysis completeness. Rationale: requirements are taken from mature user-facing
behavior while TokenMaster keeps its own safe, bounded implementation.

## ADR-003 — Rust, Slint, and SQLite

Decision: Rust 1.97, Slint 1.17, and bundled SQLite are the product stack. Rationale:
native portable deployment, predictable ownership, declarative reactive UI, and
transactional local storage.

## ADR-004 — Presentation isolation

Decision: skins, layouts, and locales are declarative presentation state over immutable
snapshots. Rationale: instant switching without archive mutation, reparsing, or stale
asynchronous overwrite.

## ADR-005 — Incremental archive with staging

Decision: stream bounded source data into a strict SQLite archive; use invisible
staging generations for replacement/reconciliation. Rationale: fast append paths,
crash consistency, deterministic canonical selection, and safe rollback.

## ADR-006 — M0 gates remain hard

Decision: bounded M1 work may continue while M0 external evidence is open, but no M0
acceptance or package claim is permitted. Rationale: development can progress without
weakening real interactive and long-run validation.

## ADR-007 — Explicit replay lineage before analytics

Decision: canonical totals are selected from retained observations using explicit
session ancestry, versioned structural replay signatures, and fail-closed
pending/conflict states. Rationale: timestamp/fingerprint deduplication alone cannot
detect copied fork/subagent prefixes, while time or filename heuristics can suppress
legitimate equal-valued usage.

## ADR-008 — Codex-first provider-neutral source seam

Decision: local Codex discovery/reader/decoder is the only 1.0 ingestion adapter, but
engine and downstream crates consume provider-neutral bounded contracts. Adapters are
statically linked and allowlisted rather than runtime code plugins. Rationale: later
file, import, remote, or API sources can be added without rewriting the archive,
analytics, automation, or UI and without creating an arbitrary I/O surface.
