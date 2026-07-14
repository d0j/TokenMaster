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

## Next implementation slice

The product architecture, universal automation connector, complete UI, dynamic quota
bars, skins, layouts, density, and localization are approved in
`docs/superpowers/specs/2026-07-14-tokenmaster-product-architecture-design.md`. Its
source-adapter seam keeps the current local Codex reader replaceable by future
sandboxed bounded provider plugins without coupling storage, analytics, automation,
or UI to Codex JSONL. The selected future format is a `.tmplugin` WebAssembly
Component executed in an isolated on-demand host; Codex remains compiled in and pays
no plugin runtime cost. No plugin implementation is claimed by that design.

The repeated critical audit is recorded in `docs/AUDIT_AND_MASTER_PLAN.md`. It closes
the remaining architectural ambiguities and approves one sequence. P0-A first adds a
provider-neutral `ObservationDraft` and an exclusive `tokenmaster-accounting`
canonicalizer, then moves Codex and store APIs onto that boundary. Its executable TDD
plan is `docs/superpowers/plans/2026-07-14-tokenmaster-p0-authority-boundary.md`.

The authoritative product, data, security, decision, traceability, and roadmap
contracts now define this sequence. Runtime replay classification and engine source
ports are still unimplemented.

The provider-neutral replay value prototype is implemented and tested, but its
placement is superseded by the approved authority boundary. Current Codex code still
computes fingerprints and the store still accepts publicly constructed canonical
events. P0-A is therefore not yet implemented and current canonical totals remain not
replay-safe. No SQLite replay migration is allowed before P0-A through P0-C pass.

## Release truth

M0 is not accepted. The required interactive Windows/DPI/accessibility receipt and an
uninterrupted 24-hour software-soak receipt are absent. No package, signing, or
product-release claim is authorized by the current developer evidence.

The clean-root audit, all three Pester contract files, root format check, strict
Clippy with `RUSTFLAGS=-Dwarnings`, full locked Rust workspace tests, release build,
and M0 developer stress verification pass from the root workspace. The exact commands
are recorded in `docs/HANDOFF.md` and the M0 script; this does not replace external
acceptance evidence.
