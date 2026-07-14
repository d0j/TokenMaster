# TokenMaster project history

## 2026-07-14 — clean TokenMaster foundation

The repository was established as a single-root Rust project. It retains the active
TokenMaster source, tests, resource gates, external-reference provenance, and a
fresh product documentation set. The product is independently implemented; external
references are used for requirement analysis only.

The root audit, Pester contracts, strict Rust linting, complete locked workspace test,
release build, and M0 developer stress gate were revalidated after the root transition.
Interactive and long-run acceptance evidence remains open.

## 2026-07-14 — product architecture and automation design

A complete Codex-first product design was selected for written review. It retains the
Rust/Slint/SQLite architecture, adds replay-correct canonical accounting before
analytics, defines immutable shared query snapshots, and isolates a universal local
MCP stdio connector in a separate on-demand process. Hermes, Codex, Claude Code,
Gemini CLI, and OpenCode consume the same bounded typed tools and advisory automation
decisions; no daemon or HTTP listener is part of 1.0.

The same design specifies the six-section desktop board, provider-defined dynamic
quota bars, independent skin/layout/density/color-scheme/locale axes, bounded
declarative skin inheritance, complete English/Russian localization, performance,
privacy, conformance, and delivery gates. This milestone is design-only and does not
claim those surfaces are implemented.

The design was approved with execution ordered by correctness first: replay-safe
canonical accounting, then staging/runtime engine, with automation connector and
presentation work later. A provider-neutral source-adapter seam was added so local
Codex JSONL is the first bounded adapter rather than a dependency of the store,
queries, automation, or UI. The detailed P0 TDD plan is recorded in
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-replay-correctness.md`; this planning
milestone changes no runtime behavior.

The approved replay and provider-neutral source boundaries were normalized into the
authoritative product, data, security, decision, traceability, and roadmap documents.
Local Codex remains the only implemented reader. Canonical replay disposition,
source-adapter engine ports, and complete-scan tail finalization remain implementation
work; this contract step changes no executable behavior.

Contract verification:

```powershell
pwsh -NoLogo -NoProfile -File scripts\audit-clean-root.ps1
git diff --check
```

The clean-root audit returned `TM-CLEAN-PASS`, the diff check exited zero, and the
changed contracts contained no unfinished drafting markers.

## 2026-07-14 — replay lineage domain contract

Added provider-neutral fixed-size replay signatures, strong/weak evidence, bounded
parent session identity, zero-based ordinal, and an explicit declared-conflict bit.
The replay signature has redacted debug output and rejects non-32-byte deserialization;
self-parenting is accepted only when already marked conflict. Canonical events do not
carry the new value yet, so no runtime accounting claim is made.

Verification:

```powershell
cargo test -p tokenmaster-domain --test usage_contract replay_lineage_is_bounded_serializable_and_private
cargo fmt --all -- --check
cargo test -p tokenmaster-domain
git diff --check
```

The focused test first failed on missing replay types, then passed after the minimal
implementation. All domain tests, formatting, and the diff check passed.

## 2026-07-14 — sandboxed provider plugin architecture

Selected a future language-neutral provider extension system: Codex stays compiled in
as the zero-overhead default adapter, while external `.tmplugin` WebAssembly
Components run one package per isolated on-demand host process. Native DLL/executable
plugins and plugin-provided UI are excluded. Packages use a versioned WIT API, strict
manifest/signature/permission rules, scoped host capabilities, transactional hot
replacement, quarantine, and explicit process/guest resource gates.

Providers return bounded observation drafts. A shared TokenMaster canonicalizer owns
fingerprints, replay signatures/evidence, event IDs, and canonical-event construction,
so neither built-in nor external providers can bypass accounting rules. The design is
recorded in
`docs/superpowers/specs/2026-07-14-tokenmaster-provider-plugin-system-design.md`.
This milestone changes no plugin runtime behavior; the current P0 plan requires a
written revision before Codex signature implementation continues.

Design verification:

```powershell
pwsh -NoLogo -NoProfile -File scripts\audit-clean-root.ps1
git diff --check
```

The clean-root audit returned `TM-CLEAN-PASS`; the diff check, unfinished-marker scan,
obsolete-contract scan, and sensitive-marker scan all passed.

## 2026-07-14 — critical audit and P0 authority replan

Re-audited every normative contract, architecture design, active P0 plan, current
implementation boundary, and pinned WhereMyTokens/ccusage feature surface. The Rust,
Slint, and SQLite stack remains approved; a complete rewrite was rejected. Sixteen
contract/product/documentation gaps were resolved in `docs/AUDIT_AND_MASTER_PLAN.md`.

The binding correction is P0-A: Codex and future adapters emit bounded
`ObservationDraft` values, while a new provider-neutral accounting crate exclusively
constructs fingerprint v2, replay signature/evidence, event IDs, lineage, and
canonical events. The old replay plan's Tasks 3+ are superseded. SQLite migration is
deferred until the authority, Codex ancestry, and pure classifier gates pass. This
milestone changes documents only; runtime authority is not yet fixed.

## Architecture milestones

- M0 selected and proved Rust 1.97, Slint 1.17, bundled SQLite, the software renderer,
bounded models, native tray lifecycle, modular skins, layouts, and localization.
- M1 established bounded Codex discovery, streaming parse/revalidation, strict
path-private SQLite storage, checkpoint CAS, and atomic current-generation ingest.
- M1 staging-generation promotion and scan reconciliation remain deliberately
  unimplemented until their transactional contract tests are written.

## 2026-07-14 — exclusive accounting authority and Codex lineage

Implemented the audited P0-A authority correction and the P0-B Codex-lineage input
surface. `tokenmaster-domain` now owns only bounded provider-neutral observation and
session-relation drafts. The new `tokenmaster-accounting` crate exclusively creates
fingerprint v2, replay signature v1/evidence, event IDs, lineage, and opaque canonical
events. Public domain constructors and Codex-owned fingerprinting were removed, and
the store accepts accounting output only. Store append also verifies that canonical
provider, profile, and source identities match the registered source before writing.

Codex now recognizes bounded ancestry from top-level/payload `forked_from_id`,
top-level/payload `parent_thread_id`, and structured subagent spawn metadata without
precedence. Multiple distinct valid parents become an explicit conflict. Parser resume
v2 retains parent/conflict/next ordinal and cumulative facts; ancestry arriving after
usage is emitted as a separate bounded relation. Resume v1 fails closed because its
ordinal cannot be reconstructed safely.

Verification included focused RED/GREEN contracts, deterministic hash vectors,
compile-fail authority proofs, the complete locked Codex/store/domain/accounting
suites, full locked workspace tests, strict workspace Clippy, clean-root,
documentation-consistency, privacy, format, and diff gates. P0-C replay
At that checkpoint P0-C classification remained next; no replay schema migration or
replay-safe totals were claimed.

## 2026-07-14 — pure bounded replay classifier

Implemented P0-C inside `tokenmaster-accounting`, keeping replay semantics independent
of SQLite and provider adapters. The allocation-free transition validates provider,
profile, declared parent session, and ordinal before comparing strong signatures.
Equal strong prefixes are replay; strong mismatches and completed parent tails lock
divergence; weak evidence stays pending without blocking a later strong divergence.
Cycles, contradictory facts, and corrupt state combinations fail closed as conflict.
Depth 33 or fanout 257 is pending work, not false conflict.

The public contract covers root, replay, divergence, weak/missing evidence,
irreversibility, scope/session/ordinal mismatch, cycle/conflict, and both work bounds.
SQLite persistence, durable continuation, descendant reclassification, and replay-safe
totals remain P0-D/P0-E work and are not claimed here.
