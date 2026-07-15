# TokenMaster current state

## Product identity

TokenMaster is the sole product. It is an original Rust/Slint/SQLite implementation
in one root workspace. WhereMyTokens is the UI/product reference and ccusage is the
usage-analysis reference; both remain external, MIT-pinned provenance only.

## Implemented

- M0 native architecture proof: one process, software-rendered Slint UI, tray
  lifecycle, three layouts, three skins, English/Russian/pseudo localization,
  bounded chart/session models, and explicit resource-gate contracts.
- M1 usage foundation: bounded provider roots, path-private source discovery,
  reparse-safe streaming enumeration, typed bounded JSONL parser, cumulative token
  state, physical/logical source identity, byte framing, revalidation, strict SQLite
  usage schema, keyset reads, and atomic current-generation append.
- P0 authority and Codex-lineage boundary: provider-neutral bounded observation and
  session-relation drafts, parser resume v2 with ordinals/cumulative snapshots,
  exclusive `tokenmaster-accounting` canonicalization, fingerprint v2, replay
  signature v1/evidence, opaque canonical events, and store-only canonical input.
- P0-C pure replay classifier: root/matching/diverged/pending/conflict transitions,
  strong/weak proof rules, provider/profile/parent/ordinal validation, irreversible
  divergence, and pending (not conflict) depth/fanout exhaustion.
- P0-D/P0-D.1 replay archive: strict SQLite replay overlay, transactional exact-v1
  migration, non-destructive exact-v2 migration with fault-tested foreign-key policy
  restoration,
  immutable legacy snapshot, explicit archive modes, fixed/version-owned replay
  manifests, invisible staging generations, transactional classified replay append,
  deterministic eligible selection, epoch CAS, and fail-closed persisted-version
  validation. Late explicit relations invalidate old selections atomically and use
  restart-safe ordinal/child keyset work capped at 32 ancestry links and 256 direct
  descendants per transaction. Conflicts/cycles are permanent; bound exhaustion is
  durable pending work without epoch spin. Staging rows never affect current event
  pages or source metadata. Product begin snapshots every registered source with
  set-based SQLite operations and checked 64-bit counts without a history-sized Rust
  manifest. The explicit 256-key manifest remains bounded test/repair input and cannot
  seal a subset. Exact all-source seal validates 256-row `file_key` keyset pages,
  full-prefix checkpoints,
  chunk and overlay coverage, exhausted work, and foreign keys. Zero-pending promotion
  atomically materializes eligible rows and swaps generations with injected-failure
  rollback; incomplete replacements cannot silently omit prior visible evidence.
  Replay reclassification may intentionally change which accounted events are
  canonical. Exact-epoch
  discard removes only unpublished staging and leaves current/legacy state unchanged.
- P0-E transactional composition proof: real synthetic Codex JSONL discovery and
  streaming enumeration feed bounded reader batches through the accounting authority
  into the replay archive. The proof includes exact replay/eligible totals and quality,
  staging invisibility, append rebuild, reopen after the first of multiple batches,
  300 observations, 300 files, one-chunk-at-a-time full-prefix verification, Windows
  atomic physical replacement, cancellation, malformed JSON, incomplete tail, and
  complete-line truncation. A constrained exact-epoch `prepare_replay_source` supplies
  a valid adapter-owned empty resume and live physical identity only to untouched
  staging; two bounded reads recover its checkpoint and one chunk after restart.
  P0-E originally proved omitted prior evidence failed closed before retention
  authority existed.
- P1-A retained canonical projection: strict schema v4 migrates exact v1/v2/v3
  archives through validated create/copy/drop/rename steps and three injected
  rollback boundaries. The indexed canonical page is self-contained and records its
  publishing revision, last direct origin revision, and retained state, so obsolete
  source generations can be removed without false provenance or unbounded history
  retention. Atomic promotion installs eligible selections directly, suppresses
  replay-only prior contributions, and carries absent/conflict-only replay-verified
  events. Legacy-unverified rows remain only in their immutable snapshot. Store
  contracts prove the complete truth table, exact provenance after reopen, invalid
  owner rejection, and rollback of values/provenance/generations/revision. The real
  Codex JSONL truncation fixture now promotes while preserving the 2-event/26-token
  canonical result; cancellation, malformed data, incomplete tails, and pending work
  remain non-promotable.
- P1-B.1 scoped scan authority: strict schema v5 adds one bounded global scan set
  with one provider/profile-qualified child per scope, coherent terminal states,
  exact last-seen references, and exclusive running indexes. Exact v1-v4 migrations
  preserve populated source/scan/replay state and fault-test every v4 create/copy/drop
  boundary. Store-owned lifecycle contracts prove multi-provider scope isolation,
  idempotent observation, complete-only missing finalization, later restoration,
  reopen, late registration, and atomic rollback after parent creation or presence
  mutation. Ordinary append no longer creates or clears scan authority.
- P1-B.2 scan-bound replay: production begin stores one exact complete `scan_set_id`
  and stages only its present members with set-based SQL. Continuation, seal, and
  promotion revalidate parent/child completion plus bidirectional membership, so a
  later scan invalidates stale staging. Same-profile Codex/Hermes scopes compose
  without collision. A zero-present-source set survives reopen, creates no staging
  generation, preserves missing sources/current generations, and atomically publishes
  retained canonical truth. Faults after revision or generation creation roll back
  all staging state. The real synthetic Codex pipeline now creates/observes/finalizes
  a scan set before replay; partial enumeration closes partial and cannot authorize
  replay. All seven pipeline contracts remain green without a production dependency
  from the Codex adapter to the store.
- P1-B.3 bounded scan history: parent close prunes only whole closed scan sets when
  every child scope has 32 newer closed sets. A source `last_seen_scan_id`, replay
  `scan_set_id`, or running state protects the entire set. Each transaction removes at
  most 64 candidates through a SQLite temporary table and scan-related foreign-key
  checks, without scanning canonical usage events or collecting history in Rust.
  Contracts prove steady-state plateau, repeated bounded backlog recovery, preservation
  of running/replay-referenced sets, checked parent/child ID exhaustion, reopen, and
  rollback of both close and pruning after an injected fault.
- P1-C.1 constant-state refresh coordinator: the new provider-neutral
  `tokenmaster-engine` crate separates admission from terminal execution, uses checked
  monotonic request IDs/deadlines and cooperative atomic cancellation, and retains one
  active permit plus at most one aggregate follow-up. Ten thousand hints collapse to
  one highest-urgency follow-up without path/source/request history. Contracts cover
  immediate and active deadline expiry, cancellation precedence, busy, stale IDs,
  exactly-one follow-up, and both direct and follow-up ID exhaustion without wrap.
- P1-C.2 bounded runtime ports: sealed provider/profile/source identities, opaque
  checkpoints capped at 32 KiB, fixed diagnostic counters, 18-update chunk-proof
  batches, scope-exact 256-observation/256-relation adapter and canonical batches,
  and 256-record replay-source pages. Object-safe synchronous `Adapter`, `Archive`,
  monotonic `Clock`, and `WriterLease` contracts keep provider I/O, archive authority,
  raw bytes, paths, Slint, OS handles, and async runtimes structurally separate.
  Compile-fail contracts prove sealed identity, path rejection, and canonical-only
  archive writes; debug/errors expose stable codes/counts only.

## Next implementation slice

The product architecture, universal automation connector, complete UI, dynamic quota
bars, skins, layouts, density, and localization are approved in
`docs/superpowers/specs/2026-07-14-tokenmaster-product-architecture-design.md`. Its
weekly quota contract now keeps immutable before/after epochs for scheduled, early,
and repeated full resets under the P2 plan
`docs/superpowers/plans/2026-07-15-tokenmaster-quota-reset-history.md`.
The separately approved P2 banked-reset plan models multiple quantities/expirations,
bounded reminders, notification coverage, immutable activation receipts, and a future
official-capability-only automatic policy:
`docs/superpowers/plans/2026-07-15-tokenmaster-banked-reset-inventory.md`. It is design
only; current reset discovery, notifications, assisted activation, and automatic
activation are not implemented.
The source-adapter seam keeps the current local Codex reader replaceable by future
sandboxed bounded provider plugins without coupling storage, analytics, automation,
or UI to Codex JSONL. The selected future format is a `.tmplugin` WebAssembly
Component executed in an isolated on-demand host; Codex remains compiled in and pays
no plugin runtime cost. No plugin implementation is claimed by that design.

The repeated critical audit is recorded in `docs/AUDIT_AND_MASTER_PLAN.md`. P0-A and
the P0-B Codex-lineage surface are implemented under the completed executable TDD plan
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-authority-boundary.md`.

P0-D.1 is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-scalable-replay-manifest.md`. Its
300-source contract crosses two manifest pages, promotes, reopens, and preserves
late-source fail-closed behavior. P0-E is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-e-pipeline-proof.md`; it is a
transactional cross-crate proof, not the production scheduler. P1-A is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p1-retained-projection.md`. P1-B.1 and
P1-B.2 now own strict scan-set presence and exact replay binding, including a
zero-present-source retention-only revision. P1-B.3 completes reference-safe 32/64
scan-history retention, ID exhaustion, and recovery. P1-C.1 supplies the
constant-state coordinator and P1-C.2 supplies bounded adapter/archive/clock/
writer-lease ports. P1-C.3 one-shot orchestration is next; P1-C.4 adds the bounded
worker shell. P1-D/P1-E later add Codex live
integration, the real writer
lease, sleep/resume, immutable publication, and continuous runtime recovery.
Parser resume v1 still fails closed because its event ordinal cannot be inferred
safely; legacy data remains immutable and must be rebuilt, never reinterpreted.

## Release truth

M0 is not accepted. The required interactive Windows/DPI/accessibility receipt and an
uninterrupted 24-hour software-soak receipt are absent. No package, signing, or
product-release claim is authorized by the current developer evidence.

The clean-root audit, all three Pester contract files, root format check, strict
Clippy with `RUSTFLAGS=-Dwarnings`, full locked Rust workspace tests, release build,
and M0 developer stress verification pass from the root workspace. The exact commands
are recorded in `docs/HANDOFF.md` and the M0 script; this does not replace external
acceptance evidence.
