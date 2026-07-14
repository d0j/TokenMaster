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
- P0-D archive foundation: strict SQLite schema v2, transactional exact-v1 migration,
  immutable legacy snapshot, explicit archive modes, fixed/version-owned replay
  manifests, invisible staging generations, transactional classified replay append,
  deterministic eligible selection, epoch CAS, and fail-closed persisted-version
  validation. Late explicit relations invalidate old selections atomically and use
  restart-safe ordinal/child keyset work capped at 32 ancestry links and 256 direct
  descendants per transaction. Conflicts/cycles are permanent; bound exhaustion is
  durable pending work without epoch spin. Staging rows never affect current event
  pages or source metadata.

## Next implementation slice

The product architecture, universal automation connector, complete UI, dynamic quota
bars, skins, layouts, density, and localization are approved in
`docs/superpowers/specs/2026-07-14-tokenmaster-product-architecture-design.md`. Its
source-adapter seam keeps the current local Codex reader replaceable by future
sandboxed bounded provider plugins without coupling storage, analytics, automation,
or UI to Codex JSONL. The selected future format is a `.tmplugin` WebAssembly
Component executed in an isolated on-demand host; Codex remains compiled in and pays
no plugin runtime cost. No plugin implementation is claimed by that design.

The repeated critical audit is recorded in `docs/AUDIT_AND_MASTER_PLAN.md`. P0-A and
the P0-B Codex-lineage surface are implemented under the completed executable TDD plan
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-authority-boundary.md`.

P0-D is in progress. Its next slice is seal validation and atomic promotion. Current
archive totals remain not replay-safe until that gate and P0-E pipeline proof
complete.
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
