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
  implemented under transactional contracts; P0-E is the cross-crate proof and P1
  owns runtime scan orchestration.

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
or automatic Codex runtime invokes this yet. P0-E first proves the cross-crate
transactional path with synthetic fixtures; P1 owns the automatic runtime.

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

## 2026-07-14 — scalable-manifest preflight correction

The P0-E preflight mapped real Codex enumeration semantics to the P0-D schema and found
that the 256-entry replay manifest caps JSONL files, not provider roots. That bound can
be exceeded by valid long-lived profiles, so a small synthetic pipeline test would not
have represented the product. No user file path or content was inspected or recorded;
only an aggregate local file count was used to confirm the mismatch.

Selected P0-D.1 before P0-E: schema v3 widens the checked count, SQLite creates the
all-registered-source revision without an application manifest vector, and seal walks
source states in 256-row keyset pages. The exact migration follows SQLite's safe
create-new/copy/drop/rename procedure and restores foreign-key enforcement on every
outcome. The original explicit 256-key manifest remains only a bounded test/repair API
and cannot seal a subset. At this design-only milestone, the cap was still present in
code pending the corrective TDD plan recorded next.

## 2026-07-14 — scalable replay manifest implemented

Completed P0-D.1. Strict schema v3 removes the historical upper-256 revision count;
fresh/v1 archives create v3 directly, while an exact populated v2 archive is rebuilt
non-destructively with SQLite's create-new/copy/drop/rename order. Tests preserve rows
from every replay child table and immutable legacy data. Injected faults after create,
copy, and drop all roll back to exact v2 and restore `foreign_keys=ON`; malformed v2 is
rejected before enforcement is disabled.

The product `begin_replay_revision_all_sources` path now creates one staging generation
and manifest row per registered source with set-based SQL in one immediate transaction.
Stored counts are checked `u64` values bounded by SQLite's signed integer range and are
never collection capacity. The explicit `ReplayManifest` remains capped at 256 only
for focused tests/repair and still cannot seal an omitted registered source.

Final manifest proof retains one 256-row `file_key` keyset page, validates every
checkpoint and chunk range, and compares checked aggregate/mutation counts. A
300-source contract completes, seals, promotes, and reopens across two pages. A source
registered after begin blocks seal without writes, and exact discard restores the
pre-rebuild archive. Continuation uses a cheap closed-source aggregate; final seal and
promotion always repeat full paged validation.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
cargo +1.97.0 test -p tokenmaster-accounting --test replay_classifier_contract --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
Remove-Item Env:RUSTFLAGS
cargo +1.97.0 test --workspace --locked
git diff --check
```

Focused suites passed 14 schema, 33 replay archive, 4 ingest, and 6 classifier tests.
Clean-root, formatting, strict workspace Clippy, and the full locked workspace passed.
The normal workspace run retained exactly one pre-existing explicitly ignored
one-million-row M0 scale test. No P0-E engine behavior, M0 acceptance, interactive
Windows result, package, or release is claimed. P0-E is now the next implementation
gate.

## 2026-07-14 — transactional Codex-to-archive proof implemented

Completed P0-E without adding a production scheduler or changing the normal Codex
dependency graph. A development-only integration driver now sends real synthetic
JSONL through Codex discovery, streaming enumeration, bounded reader batches,
TokenMaster accounting canonicalization, replay staging/relations/continuation,
full-prefix proof, exact seal, and atomic promotion. Independent baseline oracles prove
two eligible events, one replay observation, exact total tokens, and replay-quality
counts after reopen.

The preflight exposed two restart gaps and closed them with narrow reusable seams.
`PhysicalFileIdentity` can reconstruct only its opaque fixed 32-byte persisted digest.
Store reads return one exact unsealed staging generation and one exact chunk. The new
`prepare_replay_source` CAS accepts only a validated zero-offset incremental checkpoint
for an untouched pending staging source, preserves logical identity, binds the live
physical identity plus valid adapter resume, synchronizes work epochs, and leaves
current data untouched.

Seven pipeline contracts cover baseline replay, append, reopen after the first of
multiple batches, 300 observations, 300 JSONL files, Windows `ReplaceFileW` atomic
physical replacement, streaming enumeration cancellation, reader cancellation,
malformed relevant JSON, incomplete tail, and complete-line truncation. Batches and
pages cap at 256; expected chunks are fetched one at a time. Truncation and replacement
are classification, not deletion authority: an overlay omitting prior visible evidence
cannot promote, exact discard removes staging, and the old 2-event/26-token page remains
current. P1 must add explicit bounded carry-forward/retention policy.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-platform --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --locked
cargo +1.97.0 test -p tokenmaster-accounting --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
cargo +1.97.0 test --workspace --locked
```

All commands passed. Focused evidence is 2 platform identity, 38 replay archive, and
7 cross-crate pipeline tests, plus the complete Codex and accounting suites. Clean-root
returned `TM-CLEAN-PASS`; formatting and strict workspace Clippy passed. The full
workspace retained exactly one pre-existing explicitly ignored one-million-row M0
scale test. Normal dependency-tree inspection found no store/accounting dependency in
the Codex production graph. No M0 acceptance, interactive Windows product result,
package, signing, or release is claimed.

## 2026-07-15 — P1-A retained canonical projection implemented

Completed the first runtime prerequisite without adding a scheduler. Strict schema v4
makes `usage_event` a self-contained indexed projection with publishing revision,
last-direct origin revision, and retained state. Exact v1/v2/v3 archives migrate
non-destructively through validated create/copy/drop/rename steps. Populated current
and legacy fixtures preserve every logical event field; injected faults after event
table create, copy, and drop restore the exact prior schema/version/rows. The existing
v2 foreign-key restoration boundaries remain intact.

Promotion no longer needs to keep an obsolete source generation or invent a new
observation when a complete replacement omits old accounted usage. In one immediate
transaction it removes replay-only prior contributions, carries absent/conflict-only
replay-verified rows with their older origin provenance, installs eligible selections
as direct rows, swaps generations/revision, and validates the result. Unrebuilt legacy
rows remain only in the immutable legacy snapshot because their older identity cannot
safely enter replay-verified totals. Invalid publishing ownership fails closed.
Existing promotion fault points now prove rollback of values and provenance as well as
row counts, generations, and revision status.

The store truth-table contract covers eligible, replay-only, conflict-only, absent,
pending, reopen, obsolete-generation removal, owner tamper, and every promotion fault.
The real synthetic Codex JSONL complete-line truncation now promotes while preserving
the prior 2-event/26-token result. Cancellation, malformed relevant JSON, incomplete
tail, partial enumeration, and pending replay work remain non-promotable. No
history-sized Rust collection or new dependency was added; projection work is
set-based SQL inside the existing promotion transaction.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-store --lib migration --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-store -p tokenmaster-codex --all-targets --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
cargo +1.97.0 test --workspace --locked
git diff --check
```

Focused schema, replay archive, and pipeline suites pass 14, 40, and 7 tests. The
workspace gate passes with exactly the pre-existing explicitly ignored one-million-row
M0 scale test. Clean-root returns `TM-CLEAN-PASS`; formatting and strict workspace
Clippy pass. The Codex normal dependency graph contains neither store nor accounting;
tracked Go/JavaScript/TypeScript/Python source, new forbidden storage identifiers, and
secret-value patterns are absent. An environment-root scan found no actual private
path; synthetic `Example`/`private` path fixtures remain intentionally tracked and
were not misreported as leaks. This completes P1-A only. P1-B scan epochs/source-set
finalization and the P1-C+ live engine remain. No M0 acceptance, interactive Windows
product result,
package, signing, or release is claimed.

## 2026-07-15 — P1-B.1 scoped scan authority implemented

Added strict schema v5 and exact non-destructive v1-v4 migration. A bounded global
scan set owns one provider/profile-qualified child per scope; typed IDs, outcomes,
counters, and immutable snapshots reject invalid or oversized values. Running-set
and running-scope indexes prevent competing authority. Populated v4 scans derive a
provider only from exact referenced sources and otherwise migrate as
`legacy-unverified`; ambiguous ownership or incoherent terminal state fails closed.
Replay revisions preserve migrated state with nullable scan-set provenance.

The store now begins and recovers a scan set, pages children, records exact
scope-matching observations idempotently, finishes each child, and truthfully
aggregates the parent. Only a complete child updates `missing`; partial, cancelled,
failed, and timed-out children preserve prior presence. Ordinary append no longer
sets last-seen authority or clears missing state. A source registered after complete
scope authority starts missing until a later complete scan observes it. Injected
failures after parent creation and after presence mutation roll back every row and
terminal state. The v4 migration has create/copy/drop rollback proofs with foreign-key
restoration. Schema v5 permits a future zero-source replay revision while existing
compatibility begin paths still reject empty manifests.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-store --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-store -p tokenmaster-codex --all-targets --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

All commands passed. Store evidence includes 14 unit tests, 40 replay contracts, 5
scan contracts, 5 ingest contracts, 14 schema contracts, and the compile-fail API
contract. Seven real synthetic Codex pipeline contracts pass. The full workspace has
exactly one pre-existing explicitly ignored one-million-row M0 scale test. Clean-root
returns `TM-CLEAN-PASS`; formatting and strict Clippy pass. This completes P1-B.1
only. Scan-bound replay, zero-source retention promotion, bounded scan-history
pruning, and live scheduling remain P1-B.2/P1-B.3/P1-C. No M0 acceptance,
interactive product result, package, signing, or release is claimed.

## 2026-07-15 — P1-B.2 scan-bound replay implemented

Added the production `begin_replay_revision_for_scan_set` path. It accepts only one
coherent complete scan set, stores its typed ID, and creates staging generations with
set-based SQL only for exact present members. Same-profile scopes from different
providers remain distinct. The compatibility all-source and explicit-manifest paths
remain unbound for bounded composition/test/repair use and still reject empty input.

Continuation, seal, and promotion load the persisted binding and revalidate parent and
child completion times, exact scope membership in both directions, staging counts,
and foreign keys. A later complete scan invalidates stale replay authority. Injected
failures after revision creation and after generation creation roll back revision,
generation, and manifest rows. A complete set with zero present sources survives
reopen, creates no staging generation, seals, and publishes a retention-only revision:
the missing source keeps its current generation and canonical events keep their
original origin revision while receiving the new publishing provenance.

The real synthetic Codex pipeline now builds a bounded scope manifest, records each
registered file against the exact child scan, closes complete enumeration before
replay, and uses the scan-bound path. Cancelled enumeration closes the set partial and
leaves neither running scan authority nor staging projection. The production Codex
crate dependency direction remains unchanged because this composition stays in its
test-only driver.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-store --locked
cargo +1.97.0 test -p tokenmaster-codex --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

All commands passed. Store evidence includes 15 unit tests, 45 replay contracts, 5
scan contracts, 5 ingest contracts, 14 schema contracts, and its compile-fail API
contract. All seven real synthetic Codex pipeline contracts pass on the scan-bound
path. The full workspace retains exactly one pre-existing explicitly ignored
one-million-row M0 scale test. Clean-root returns `TM-CLEAN-PASS`; formatting and
strict Clippy pass. This completes P1-B.2 only. Reference-safe scan-history pruning,
repeated-scan/ID-exhaustion recovery, and the live scheduler remain P1-B.3/P1-C. No
M0 acceptance, interactive product result, package, signing, or release is claimed.

## 2026-07-15 — P1-B.3 bounded scan history implemented

Completed P1-B with reference-safe scan-history retention. Closing a parent now
selects only whole closed sets for which every child scope has 32 newer closed sets.
Any running state, source `last_seen_scan_id`, or replay `scan_set_id` reference keeps
the complete set. One immediate transaction prunes at most 64 sets through a SQLite
temporary candidate table and checks only the three scan-related foreign-key tables;
it does not collect history in Rust or scan the canonical usage-event archive. The
same bounded operation is public to store-owned recovery and can be repeated until an
older backlog returns zero candidates.

Contracts prove a repeated single-scope plateau, whole-set safety across shared
Codex/Hermes scopes, survival of replay-referenced and running sets, 64+remainder
backlog recovery, checked parent and child ID exhaustion, stale lookup after removal,
reopen, and rollback of parent close, deleted rows, and temporary schema after an
injected post-prune fault. The seven real synthetic Codex contracts remain unchanged.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-store --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

All commands passed. Store evidence includes 16 unit tests, 45 replay contracts, 11
scan contracts, 5 ingest contracts, 14 schema contracts, and the compile-fail API
contract. All seven scan-bound synthetic Codex pipeline contracts pass. The full
workspace retains exactly one pre-existing explicitly ignored one-million-row M0
scale test. Clean-root returns `TM-CLEAN-PASS`; formatting and strict Clippy pass.
The normal Codex dependency graph contains neither store nor accounting; tracked
Go/JavaScript/TypeScript/Python source, secret-value patterns, actual user-profile
paths, and new forbidden storage identifiers are absent. This completes P1-B only;
P1-C provider-neutral engine core is next. No M0 acceptance, interactive product
result, package, signing, or release is claimed.

## 2026-07-15 — P1-C.1 constant-state refresh coordinator implemented

Added the root-workspace `tokenmaster-engine` crate with no Codex, platform, Slint, or
async-runtime dependency. Refresh admission is distinct from terminal execution:
started/coalesced/deadline-exceeded admissions and completed/busy/cancelled/deadline-
exceeded/failed terminal outcomes cannot be conflated. Request IDs are non-zero,
checked, monotonic `u64`; deadlines use caller-supplied monotonic milliseconds only.

The coordinator retains one active permit with an `Arc<AtomicBool>` cancellation token
and at most one pending aggregate. Ten thousand active-time hints collapse to one
highest-urgency follow-up; deadlines merge so work remains live while any coalesced
request remains live. Completion starts at most one new permit. Stale completion or
cancellation cannot mutate a newer request. Active cancellation/deadline overrides a
nominal success, and direct or follow-up ID exhaustion never wraps or reopens a slot.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-engine --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-engine --all-targets --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

All commands passed. Engine evidence is 2 unit and 10 public coordinator contracts;
the full workspace retains exactly one pre-existing explicitly ignored million-row M0
scale test. Clean-root returns `TM-CLEAN-PASS`; formatting and strict Clippy pass.
This completes P1-C Task 1 only. Bounded adapter/archive/clock/writer-lease ports,
one-shot orchestration, worker shell, Codex integration, and the OS lease remain. No
M0 acceptance, interactive product result, package, signing, or release is claimed.

## 2026-07-15 — banked reset expiry and activation architecture approved

Separated provider-granted banked rate-limit resets from automatic quota epochs,
credits, and temporary usage. The P2 plan keeps different expirations as different
typed lots, preserves immutable award/quantity/activation/expiry/revocation change
points, links confirmed consumption to `manual_or_banked_reset` quota transitions,
and never invents provider capacity from local token usage.

Expiry safety starts with selectable 7-day, 24-hour, 12-hour, 6-hour, and 1-hour lead
times. Users may choose any subset or replace it with up to eight unique custom values,
including `3 hours only` or `6 hours + 3 hours`; duplicates normalize and an empty
profile explicitly disables reminders. One durable indexed due queue, one nearest-due
runtime timer, settings revisions, persistent delivery deduplication, bounded snooze,
quiet hours, and explicit in-app/tray/OS notification coverage prevent per-lot resource
growth. Restart, sleep/hibernation, clock/time-zone change, date-only expiry, and
multiple-account behavior are acceptance fixtures rather than platform assumptions.

Activation has four modes: off, remind-only, confirm-each, and automatic policy.
Automatic policy defaults off and is impossible for manual data, external plugins,
or read-only CLI/MCP/LLM access. It requires an official host-owned idempotent/status
capability, explicit versioned scope policy, fresh high-confidence inventory/quota,
known effect and adequate expiry precision, CAS, durable intent before mutation, one
in-flight action, and bounded post-action reconciliation. Ambiguous results never
blindly retry or become false success. Browser scraping, synthetic clicks, cookie/
session reuse, private endpoint replay, and raw provider payload retention are
forbidden.

The contracts, product architecture, provider-plugin ABI, feature matrix, roadmap,
current state, handoff, audit, changelog, and traceability now point to the approved
plan. This is design-only P2 scope; no current account discovery, reminder delivery,
or activation implementation is claimed. P1-C.3 remains the immediate code gate.

Verification:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

The clean-root audit returned `TM-CLEAN-PASS`; formatting, strict workspace Clippy,
and the full locked workspace passed. The workspace retained exactly one explicitly
ignored one-million-row M0 scale gate. Requirement/traceability and ADR consistency
checks found 33 traced specification/security/data IDs and 16 unique ADR headings.
No M0 acceptance, interactive product result, package, signing, or release is claimed.

## 2026-07-15 — P1-C.2 bounded provider-neutral ports implemented

Extended `tokenmaster-engine` without adding Codex, platform, Slint, Tokio, Wasmtime,
filesystem, or UI dependencies. Sealed scope/source identities and provider-owned
opaque checkpoints prevent paths and descriptors from becoming engine or archive
state. Checkpoints cap at 32 KiB, chunk-proof updates at 18, adapter observations and
relations independently at 256, and archive replay-source pages at 256. Counters are
checked against SQLite `i64`; diagnostic categories are fixed and count-only.

Synchronous object-safe `Adapter`, `Archive`, monotonic `Clock`, and `WriterLease`
ports now separate provider I/O from storage authority. Adapter discovery streams
owned normalized values through callbacks and batch pulls. The archive receives only
normalized discovery state, completion summaries, opaque checkpoints, and scope-exact
canonical accounting batches; it has no provider descriptor, path, or raw-source API.
Replay handles carry exact revision/epoch identity and pages are bounded before any
provider I/O. Stable port errors contain enumerated codes only. Compile-fail doctests
reject private identity construction, filesystem-path substitution, and raw byte
archive writes.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-engine --locked
cargo +1.97.0 test -p tokenmaster-engine --doc --locked
cargo +1.97.0 tree -p tokenmaster-engine --edges normal
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

Engine evidence is 2 unit tests, 22 public value/batch/port/coordinator contracts, and
3 compile-fail doctests. The normal dependency graph contains only the approved
domain/accounting contracts and their transitive libraries; forbidden runtime/UI/
provider dependencies are absent. This completes P1-C Task 2 only. One-shot
execution, the bounded worker, live Codex composition, the OS writer lease, and
sleep/resume remain. No M0 acceptance, interactive product result, package, signing,
or release is claimed.

## 2026-07-15 — P1-C.3 provider-neutral one-shot executor implemented

Added the synchronous `OneShotExecutor` without adding Codex, platform, filesystem,
Slint, async-runtime, or UI dependencies to `tokenmaster-engine`. It acquires the
writer lease before provider/archive work, collects only the bounded scope manifest,
streams discovered sources directly into one exact scan set, and begins replay only
after every scope and the parent set close complete. Adapter observations are
canonicalized under `tokenmaster-accounting` authority one bounded batch at a time;
the executor retains only opaque checkpoints, one replay page/batch, fixed counters,
and the latest exact replay handle.

The TDD failure matrix found and fixed four boundary problems: a cancellation/deadline
between discovery and parent close could leave a scan running; unchanged checkpoints,
repeated cursors, and unbounded continuation could make no progress; an archive could
return a different revision identity; and non-lease `busy` could be mislabeled as safe
admission backpressure. The final implementation closes the scan at every cooperative
boundary, rejects cross-scope discovery before archive mutation, validates same-revision
non-regressing epochs, caps continuation calls at 4,096, and discards only the last
confirmed unpublished handle. Cleanup failure is explicit and never masks the original
stable error code.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test one_shot_executor_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-engine --all-targets --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

Engine evidence is 2 unit tests, 40 public coordinator/value/batch/port/executor
contracts, and 3 compile-fail doctests. The eighteen executor contracts cover complete
and zero-source publication, partial/fault closure, cross-scope rejection, lease-only
busy, every deadline boundary, every cancellable phase interval, stale/foreign handles,
non-progress, continuation capacity, and successful/failed exact cleanup. Existing
store contracts remain the evidence that staging and fault paths preserve prior
canonical readability. This completes P1-C Task 3 only. The bounded deterministic
worker, live Codex composition, OS writer lease, sleep/resume, immutable product
snapshot, M0 acceptance, packaging, signing, and release remain unclaimed.

## 2026-07-15 — P1-C.4 bounded deterministic worker implemented

Completed the provider-neutral engine core with one optional `RefreshWorker`. It owns
one named thread, a capacity-one wake token, a capacity-one latest-only completion,
one shared constant-state coordinator, and one `JoinHandle`. Submission updates the
coordinator directly, so a blocked refresh plus 10,000 hints retains one aggregate and
executes exactly one follow-up. Unread completion replacement never blocks, retains no
history, and increments one checked fixed supersession counter.

Focused TDD exposed and corrected the lifecycle edges before integration: shutdown
must cancel an allocated follow-up without invoking its callback; stale cancellation
must not affect a newer request; a callback mutex must not block cancellation; panic
must dominate concurrent shutdown as fixed `failed`/`panicked`; and external Clock
calls must not run under worker state lock or execute on a stopped worker. Explicit
shutdown and `Drop` now cancel/wake/join without detach. Ordinary `failed` remains
recoverable; callback panic abandons the one follow-up and closes admission as
`faulted`. An outer boundary also cancels and clears fixed coordinator state if another
worker port panics.

Rust invokes its panic hook before `catch_unwind`, so first worker creation installs
one fixed process hook wrapper. A thread-local flag suppresses payload/location output
only for TokenMaster's marked worker and delegates all other panics to the prior hook.
Worker completions, snapshots, errors, and debug values contain only typed IDs, enums,
flags, and counters. Application crash hooks must be composed before first worker
creation and not replaced during its lifetime. A compile-time guard rejects
`panic=abort`, which cannot provide the promised contained fault transition.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test worker_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-engine --all-targets --locked
cargo +1.97.0 tree -p tokenmaster-engine --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

Engine evidence is 2 unit tests, 50 public coordinator/value/batch/port/executor/worker
contracts, and 3 compile-fail doctests. Ten worker contracts cover burst/backpressure,
ordinary-failure follow-up, latest-only replacement, cooperative shutdown, stale IDs,
idempotent close, `Drop` join, pending deadline, callback and port panic, concurrent
panic/shutdown, and external-callback lock order. The normal engine graph remains
domain/accounting/thiserror only; Codex, platform, filesystem, Slint, async-runtime,
Wasmtime, and UI dependencies remain absent. P1-C is complete. Live Codex composition,
the real OS writer lease, watcher scheduling, sleep/resume, immutable publication, M0
acceptance, packaging, signing, and release remain unclaimed; P1-D is next.

## 2026-07-15 — P1-D.0 real multi-file engine seam repaired

The P1-D preflight found that P1-C's provider/profile/source identity was not unique
per real Codex JSONL file and that archive-page replay could not recover a path-private
live descriptor without a history-sized cache or repeated enumeration. `SourceIdentity`
now includes a fixed redacted 32-byte logical-file key. Full rebuild performs two
linear adapter passes: discovery streams directly into the scan set, then each exact
scope lends one temporary descriptor-bound `SourceBatchReader` while the adapter still
owns the path. The engine receives no descriptor, path, file handle, or raw bytes and
retains no replay-source list.

The executor validates scope and complete logical identity before append, rejects
unchanged non-terminal checkpoints, requires complete second-pass quality, and keeps
only the latest exact replay handle. Store preparation remains exact membership and
duplicate authority; store seal remains the disk-backed omission proof. Contracts
cover two files sharing one provider source ID, cross-logical batch substitution,
extra and omitted second-pass files, partial/cancelled/failed replay quality, every
cancellation/deadline boundary, and repeated 300-file rebuilds with exactly one
maximum live reader and zero remaining after each run. The historical engine
page/cursor seam is explicitly superseded.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-engine --test one_shot_executor_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-engine --all-targets --locked
cargo +1.97.0 tree -p tokenmaster-engine --edges normal
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

All gates passed. The engine executor suite has 23 passing contracts; the repeated
300-file case performs 300 archive appends with one maximum live temporary reader.
The engine normal dependency tree remains domain/accounting/thiserror only. The full
workspace retains exactly one pre-existing explicitly ignored one-million-row M0 scale
test. P1-D.1 atomic event/relation replay append, the runtime crate, live Codex,
incremental tail refresh, OS lease, watcher, sleep/resume, P1-E, M0 acceptance,
packaging, signing, and release remain unclaimed.
