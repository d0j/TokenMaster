# Changelog

All notable changes are recorded here.

## Unreleased

### Added

- Single-root TokenMaster workspace and clean-history product boundary.
- Root-only Rust M0 scripts and CI.
- Clean-root audit for product-tree invariants.
- TokenMaster-only contracts, handoff, roadmap, feature matrix, and provenance.
- Critical architecture audit with a single approved delivery rail.
- Architecture/release closure review with a blocking row-level reference parity
  ledger, UI-before-automation phase order, permitted Codex quota-source policy,
  release-pinned pricing, canonical MSVC signed portable package, Slint attribution,
  no-updater 1.0 boundary, and explicit supply-chain evidence gates.
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
  canonical batches, temporary source readers, stable coded errors, and object-safe
  synchronous adapter/archive/clock/writer-lease ports with compile-fail privacy gates.
- Provider-neutral one-shot refresh execution with lease-first admission, streamed
  scope-exact discovery, all-complete replay, core canonicalization, exact replay-handle
  continuity, bounded continuation, phase-complete cancellation/deadline handling, and
  explicit last-confirmed staging cleanup.
- Deterministic provider-neutral refresh worker with one owned thread, capacity-one
  wake/latest-result channels, non-blocking checked supersession, constant-state
  10,000-hint coalescing, cooperative shutdown/Drop join, stale-ID safety, fixed
  completion/snapshot values, and redacted panic/fault containment.
- Exact per-logical-file engine identity plus a descriptor-private two-pass rebuild
  seam that lends one temporary source reader at a time. Contracts cover shared
  provider source IDs, extra/duplicate/omitted or mismatched second-pass input,
  incomplete quality, and repeated 300-file promotion with one maximum live reader.
- Bounded atomic replay fact batches containing up to 256 canonical events and 256
  late relations with one revision/epoch advance and full event/relation/selection/
  work/chunk/checkpoint rollback at two injected transaction boundaries.
- Production `tokenmaster-runtime` bootstrap composition with the built-in Codex
  adapter, checked SQLite archive bridge, strict path-free 32-KiB checkpoint envelope,
  300-file/reopen/zero/missing-profile/Windows-replacement/truncation contracts, and
  exact post-begin staging cleanup.
- Replay-aware incremental archive and runtime: strict schema v6 publication
  generations, exact complete-scan freshness and source admission, paired revision/
  archive CAS, targeted fingerprint materialization, zero-payload unchanged refresh,
  persisted-offset multi-batch tails, bounded partial restart, multiple new/empty
  sources, missing-history retention, and durable non-destructive rebuild state.
- Portable process-owned writer lease using one persistent empty sidecar and Rust 1.97
  `File::try_lock`, with same-process/cross-process contention, normal/forced process
  release, canonical parent alias, unsupported namespace/mapped remote drive, privacy,
  and runtime bridge contracts.
- Bounded pathless filesystem scheduling with exact `notify = 8.2.0`, one fixed atomic
  hint aggregate, capacity-one wake, one scheduler thread, 250 ms quiet coalescing,
  15 minute healthy/60 second degraded reconciliation, checked clock rollback,
  64-root generation bounds, and Windows handle/thread return-to-baseline evidence.
- Lease-first live runtime composition with exact startup scan/staging recovery,
  incremental/rebuild selection, one worker-owned Codex/archive/lease state,
  admission-safe pause/resume, ordered joined shutdown, stable path-free snapshots,
  partial/reopen recovery, and combined Windows handle/thread return evidence.
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
- Rejected cross-scope adapter discovery and non-progressing checkpoints before they
  can loop or mutate the wrong archive scope.
- Removed the archive replay-page/cursor descriptor-recovery assumption, which aliased
  real Codex files sharing one source ID and could not recover a live path-private
  descriptor without unbounded caching or repeated enumeration.
- Disabled canonical-only append after replay promotion and removed false `complete`
  windows during new-source admission; current append, checkpoint, replay projection,
  publication quality, epoch, and archive generation now roll back together.
- Allowed a valid existing-source tail to commit while an exact scan has also admitted
  a new pending source; current replay membership remains required and all paired CAS
  checks remain unchanged.
- Classified profile-scope changes as durable rebuild requirements and made full
  rebuild safely recover an unadmitted provisional source instead of leaving the
  archive blocked after interrupted incremental admission.
- Converted changed provisional identity and over-bound new-source admission into a
  typed non-destructive rebuild path instead of a database conflict or retry loop.
- Removed the synthetic Codex pipeline's per-relation transaction loop; reader events
  and relations now reach the store as one exact batch, preventing stale engine handles
  after a partial fact commit.
- Reserved terminal `busy` for writer-lease admission so later port faults cannot be
  mislabeled as harmless backpressure.
- Prevented external Clock or execution callbacks from running under the worker state
  mutex, and made stopped/faulted admission reject before consulting the Clock.
- Made callback panic dominate concurrent cancellation as fixed `failed`/`panicked`,
  abandon the one follow-up, suppress worker-only panic payload output, clear runtime
  state on other worker-port panics, preserve faulted join ownership, and reject
  incompatible `panic=abort` engine builds.
