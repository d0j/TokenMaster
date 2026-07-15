# Changelog

All notable changes are recorded here.

## Unreleased

### Added

- Single-root TokenMaster workspace and clean-history product boundary.
- Root-only Rust M0 scripts and CI.
- Clean-root audit for product-tree invariants.
- TokenMaster-only contracts, handoff, roadmap, feature matrix, and provenance.
- Critical architecture audit with a single approved delivery rail.
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

### Fixed

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
