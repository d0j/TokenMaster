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
- M1 P0-D staging, bounded replay reconciliation, and atomic promotion are now
  implemented under transactional contracts; P0-E runtime scan orchestration remains.

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
documentation-consistency, privacy, format, and diff gates. At that checkpoint P0-C
classification remained next; no replay schema migration or
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

## 2026-07-14 — replay archive v2 and classified staging append

Implemented the first P0-D archive slices under the approved replay-archive design and
executable plan. SQLite schema v2 adds an immutable exact-v1 legacy snapshot, private
version-owned replay revisions, fixed source manifests, staging generations, persisted
observations/sessions/selections, and durable bounded work rows. Exact v1 migration is
transactional and fails closed on altered schema; archive reads expose explicit empty,
legacy-unverified, replay-verified, stale-version, and rebuild-staging state without
using staging data as current product truth.

Replay append now shares the existing identity/chunk/checkpoint proof path but writes
only to staging. It validates provider/source/accounting scope, persists replay facts,
classifies through `tokenmaster-accounting`, records deterministic eligible selection,
advances a store-owned evidence epoch, and rolls the whole transaction back on stale
CAS, mismatched duplicate content, invalid persisted accounting versions, or foreign
key failure. It cannot change current event pages or visible source metadata.

Verification:

```powershell
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

All commands passed. The store replay contract has 11 passing tests. The workspace
run retains the explicitly ignored one-million-row M0 scale gate; it was not rerun for
this archive slice. Late ancestry continuation, sealing, promotion, and P0-E pipeline
integration remain unimplemented and are not claimed.

## 2026-07-14 — durable late ancestry reconciliation

Added a bounded `ReplayRelation` adapter derived from validated provider-neutral
`SessionRelationDraft` values plus `apply_replay_relation` and `continue_replay` store
transactions. A late explicit relation records the lexicographically first
source-key/offset identity, permanently records parent disagreement or a confirmed
cycle as conflict, invalidates the affected staging selection in the same transaction,
and advances the store-owned evidence epoch. Stale API or persisted work epochs write
nothing.

Continuation reclassifies one session ordinal page at a time and scans direct child
sessions with a deterministic keyset cursor. One transaction retains at most 256
observations/children and traverses at most 32 ancestry links. A 257-child scan persists
its cursor across reopen without duplicate or skipped child work; depth 33 remains a
non-spinning durable pending item. Nested descendants are reconsidered in stable
session/ordinal order, and already-conflicted cycles converge instead of recursively
re-enqueuing each other.

Verification:

```powershell
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 clippy -p tokenmaster-store --all-targets --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

The focused restart, stale epoch, conflict, cycle, deterministic identity, nested
ordinal, depth, and fanout contracts passed. Seal, promotion, and P0-E remain outside
this milestone and are not claimed.

## 2026-07-14 — exact seal, atomic promotion, and staging recovery

Completed the P0-D store lifecycle. A seal now requires every registered source in
the fixed manifest, exact full-prefix checkpoint/chunk coverage, no durable work,
complete replay-overlay/selection coverage, compiled accounting versions, and clean
foreign keys. Once the complete manifest exists, bounded continuation converts an
outgrown missing parent from open pending evidence to deterministic divergence before
seal. Weak unresolved evidence may remain sealed for quality inspection, but
zero-pending remains mandatory for promotion.

Promotion rematerializes only deterministic eligible selections and swaps revision,
generation, and source-pointer state in one immediate transaction. It rejects a
replacement that does not account for every prior visible fingerprint in the new
evidence overlay or immutable legacy snapshot. Test-only faults after materialization,
generation swap, and revision-state mutation prove full rollback both for first
promotion and replacement of an existing current replay revision. The legacy snapshot
remains immutable across successful promotion.

Added exact revision/epoch staging discard for cancelled, obsolete, or quality-only
rebuilds. It removes only unpublished replay/staging state, validates foreign keys,
preserves the current canonical page and revision, and permits a clean retry. No CLI
or automatic Codex runtime invokes this yet; that is the P0-E boundary.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-accounting --test replay_classifier_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
cargo +1.97.0 test --workspace --locked
git diff --check
```

All commands passed. The focused replay archive contract has 29 passing tests; the
store unit suite also covers five internal rollback/read/write boundaries. The full
workspace run passed with only the pre-existing one-million-row M0 scale test
explicitly ignored. Changed-Rust forbidden-storage scans, secret-value pattern scans,
absolute-user-path scans, and tracked legacy-language extension checks returned no
findings. No M0 acceptance, interactive Windows result, package, or release is claimed.
