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
- Strict SQLite schema v5 with exact v1-v4 migration, bounded provider-qualified
  scan sets, complete-only source presence, coherent terminal state, and lifecycle
  rollback contracts.
- Exact scan-bound replay with persisted provenance, bidirectional membership
  revalidation, multi-provider scopes, atomic begin faults, missing-generation
  preservation, and zero-source retention-only promotion after reopen.
- Real synthetic Codex pipeline composition over complete/partial scan authority
  without adding a store dependency to the production Codex adapter.
- Reference-safe scan-history retention: 32 closed sets per scope, at most 64 whole
  unreferenced sets pruned per transaction, running/source/replay protection, bounded
  backlog recovery, checked ID exhaustion, and injected rollback proof.
- Provider-neutral constant-state refresh coordinator with monotonic checked IDs and
  deadlines, cooperative cancellation, explicit admission/terminal outcomes, one
  active permit, and one aggregate follow-up across 10,000-hint bursts.
- Bounded provider-neutral engine runtime contracts: sealed scope/source identities,
  32-KiB opaque checkpoints, 18 chunk-proof updates, scope-exact 256-item adapter and
  canonical batches, 256-record replay pages, stable coded errors, and object-safe
  synchronous adapter/archive/clock/writer-lease ports with compile-fail privacy gates.
- Provider-neutral one-shot refresh execution with lease-first admission, streamed
  scope-exact discovery, all-complete replay, core canonicalization, exact replay-handle
  continuity, bounded continuation, phase-complete cancellation/deadline handling, and
  explicit last-confirmed staging cleanup.
- Deterministic provider-neutral refresh worker with one owned thread, capacity-one
  wake/latest-result channels, non-blocking checked supersession, constant-state
  10,000-hint coalescing, cooperative shutdown/Drop join, stale-ID safety, fixed
  completion/snapshot values, and redacted panic/fault containment.
- Approved a provider-neutral weekly quota reset history: immutable pre/post epochs,
  scheduled/early/repeated reset transitions, allowance-change separation, bounded
  retention, and shared UI/CLI/MCP semantics for P2.
- Approved separate banked reset inventory and expiry safety: independently expiring
  lots, a selectable 7d/24h/12h/6h/1h default profile plus bounded custom reminders,
  truthful notification coverage, assisted activation, and official-capability-only
  crash-safe automatic policy for P2.

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
- Removed scan authority from ordinary append and made post-scan source registration
  remain missing until a later complete matching-scope scan observes it.
- Rejected cross-scope adapter discovery and non-progressing checkpoints/cursors before
  they can loop or mutate the wrong archive scope.
- Reserved terminal `busy` for writer-lease admission so later port faults cannot be
  mislabeled as harmless backpressure.
- Prevented external Clock or execution callbacks from running under the worker state
  mutex, and made stopped/faulted admission reject before consulting the Clock.
- Made callback panic dominate concurrent cancellation as fixed `failed`/`panicked`,
  abandon the one follow-up, suppress worker-only panic payload output, clear runtime
  state on other worker-port panics, preserve faulted join ownership, and reject
  incompatible `panic=abort` engine builds.
