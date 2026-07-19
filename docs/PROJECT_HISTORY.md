# TokenMaster project history

## 2026-07-19 — global reminder settings developer closure

Portable settings became the desired-state authority for the single global reminder
profile. Generation `N` maps to revision `N + 1`; startup, explicit Save, and confirmed
config import reuse one Pending-first synchronizer. The store preserves scope overrides,
deliveries, acknowledgements, and provider evidence while the fixed editor supports five
recommended and eight normalized custom leads. This is developer closure only: per-scope
editing, snooze, quiet hours, OS/tray delivery, usage alerts, activation, P4/P5/P6, M0,
package/signing/soak, and release are not accepted.

Independent review passes found and closed five acceptance blockers: same-command
coalescing could discard a newer settings payload; Pending
was scheduled but not visibly acknowledged before settings mutation; aggregate due
count conversion could fail after SQLite commit; and current planning text still
pointed back to the completed slice. A final rereview also found startup archive
contention overwriting retryable Pending with an incompatible Unavailable projection.
The repaired path keeps one latest-wins pending payload, waits for bounded visible
Pending acknowledgement, validates aggregate results before commit, covers 65,536
overridden-scope due rows across retry and reopen, and separates optional runtime
StoreUnavailable health from the exact durable Pending policy.

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

## 2026-07-15 — P1-D.1 replay facts made transaction-atomic

The runtime preflight found a second exact-handle hazard: P0-E committed a replay event
batch, then committed every late session relation separately. A fault between commits
could advance SQLite's evidence epoch while the engine still held the prior handle.
`ReplayAppendBatch` now carries independently bounded collections of at most 256
canonical events and 256 `SessionRelationDraft` values. One immediate transaction
applies observations, replay overlay/session state, relation reconciliation, selection
invalidation, continuation work, chunks, checkpoint/source state, and one evidence
epoch advance. Debug exposes only relation count.

Two injected boundaries, after event-overlay work and after relation work, compare the
full pre/post state and prove rollback of observations, overlays, selections, sessions,
work, chunks, checkpoint, and epoch. The success contract applies two relations yet
advances epoch exactly once and leaves required continuation visible; a 257-relation
batch fails with the exact capacity limit. The real synthetic Codex pipeline now
submits `ReadBatch` events and relations together and removes its per-relation commit
loop while all seven JSONL pipeline contracts remain green.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-store -p tokenmaster-codex --all-targets --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

All gates passed. Store evidence is 17 unit tests and 47 replay contracts; the Codex
pipeline retains seven passing contracts. The workspace has exactly the pre-existing
explicitly ignored one-million-row M0 scale test. P1-D.2 bootstrap runtime composition,
incremental tail refresh, OS lease, watcher, sleep/resume, P1-E, M0 acceptance,
packaging, signing, and release remain unclaimed.

## 2026-07-15 — P1-D.2 production bootstrap composition added

The test-only P0-E driver is no longer the only real Codex-to-store proof. A separate
`tokenmaster-runtime` crate now implements the engine adapter/archive/clock bridges
without adding Codex, SQLite, filesystem, or platform dependencies to the engine.
The Codex adapter retains only its bounded discovery snapshot and lends one temporary
descriptor-bound reader per callback. `StoreArchive` maps scan/revision/epoch handles
through checked zero-based/nonzero conversions and sends only normalized identities,
canonical facts, chunks, and checkpoints to SQLite.

`initialize_source_checkpoint` performs safe open/metadata probe without reading or
discarding the first event batch. `CodexCheckpointV1` is a manual path-free binary
envelope capped at 32 KiB total; decode rejects oversize, unknown versions/flags,
logical identity mismatch, truncation, and trailing bytes. Bootstrap reading begins
with a full-prefix proof over the empty covered prefix, while exact store preparation
receives its required independent incremental zero-offset staging checkpoint.

Focused contracts cover strict codec round-trip/privacy/adversarial decode and seven
real runtime paths: baseline plus SQLite reopen, 300 logical JSONL files sharing one
provider source ID, authoritative zero-source, missing-profile partial retention,
append rebuild, Windows atomic physical replacement, truncation carry-forward,
pre-start cancellation, and exact discard after a deadline immediately following
replay begin. The latter proves no staging state or canonical mutation remains.

Verification:

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test checkpoint_codec_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test bootstrap_contract --locked
cargo +1.97.0 tree -p tokenmaster-runtime --edges normal
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-codex -p tokenmaster-runtime --all-targets --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

All focused and root gates passed. The workspace retains exactly the pre-existing
explicitly ignored one-million-row M0 scale test.

P1-D.2 is bootstrap/full rebuild only. Replay-aware incremental tail refresh, the real
OS writer lease, watcher/scheduler, lifecycle/sleep recovery, P1-E, M0 acceptance,
packaging, signing, and release remain unclaimed.

## 2026-07-15 — P1-D.3 replay-aware incremental archive added

The steady-state Codex path no longer requires a full history rebuild. Strict schema
v6 adds one singleton publication record with current revision, latest exact complete
scan set, checked archive generation, and explicit complete/partial/recovery state.
The exact v5 migration starts at generation zero and either preserves scan-backed
complete authority or fails into recovery-pending; injected create/seed failures
restore the exact v5 schema and rows.

An exact complete scan now advances freshness for the current revision, provisionally
registers new path-private sources, admits empty sources directly, and keeps non-empty
sources partial until their content is read. Missing sources remain historical rather
than destructive authority. Runtime preflights every present source before writing,
then pulls only from persisted checkpoints. Unchanged refresh reads zero JSONL payload
bytes; one-line and 300-event tails commit exact bounded batches; a deadline after the
first batch resumes without duplicate events. Physical replacement, rewrite,
truncation, or anchor mismatch advances only the publication into durable
`recovery_pending` and preserves prior canonical truth for full rebuild. A changed
profile scope is classified the same way, and full rebuild can safely replace an
unadmitted provisional generation left by that interrupted admission path. More than
256 new sources in one incremental pass also requests rebuild before retained admission
state can exceed its fixed bound.

Current append compares revision epoch, archive generation, source generation,
identity, offsets, and chunk proof, atomically updates replay facts plus only affected
fingerprints, and disables the older canonical-only bypass. Four injected boundaries
prove rollback of projection, relations, work, chunks, checkpoint, source state, both
CAS tokens, and publication quality. Focused evidence is 20 store unit tests, seven
incremental store contracts, and eleven real runtime incremental contracts. P1-D.4 OS
writer lease, watcher/scheduler, lifecycle assembly, P1-E, M0 acceptance, packaging,
signing, and release remain unclaimed.

## 2026-07-15 — P1-D.4 portable process-owned writer lease added

Added `ExclusiveFileLease` in the platform boundary and `RuntimeWriterLease` as the
provider-neutral engine bridge. One canonical controlled local archive parent derives
one persistent sidecar. It is opened read/write without truncation, must remain a
regular zero-byte file, and is never removed on unlock. Rust 1.97 `File::try_lock`
provides non-blocking exclusive ownership; only typed contention maps to engine
`busy`. One erased guard retains one file handle, so guard drop, normal process exit,
and forced process termination release ownership without PID/timestamp state, a
heartbeat, cleanup thread, or stale-owner heuristic.

Eight platform integration contracts prove independent same-process handles, an
independent child process, normal and forced child exit, reacquisition, canonical `.`
parent aliasing, UNC/device rejection before I/O, payload rejection, persistent
emptiness, and redacted Debug/error surfaces. One Windows unit contract rejects mapped
remote, unknown, missing, and read-only optical drive types while accepting local
fixed/removable/RAM-disk media. The eighth integration contract runs 4,096 acquire/drop
cycles and proves that the Windows process handle count does not grow. Two runtime
contracts prove stable `busy`/invalid-data mapping and guard-drop reacquisition through
the engine port.
Focused strict Clippy and all platform/runtime targets pass. P1-D.5 watcher/scheduler,
P1-D.6 lifecycle assembly, P1-E, M0 acceptance, packaging, signing, and release remain
unclaimed.

## 2026-07-15 — P1-D.5 bounded scheduler and filesystem hints added

Pinned `notify = 8.2.0` as the only new direct runtime dependency and isolated it
inside `tokenmaster-runtime`. `RefreshHintSink` reduces filesystem activity, rescan,
watcher health, and forced reconciliation to one atomic flag word, latest monotonic
tick, fixed health/lifecycle bytes, checked counters, and a capacity-one non-blocking
wake. It retains no event, path, source, request, timer node, backend error, or history.
`RefreshScheduler` owns one named thread and enforces immediate startup recovery, a
250 ms quiet window, 15 minute healthy and 60 second degraded reconciliation, checked
clock rollback, stable pause/resume/stop/fault phases, redacted panic output, and joined
shutdown.

`BoundedFilesystemWatcher` canonicalizes at most 64 existing roots, rejects duplicate,
oversized, relative, unsupported-namespace, symlink, and reparse ambiguity, creates no
backend for missing roots, and publishes root replacement as one recovery hint. Each
callback first checks its generation and then inspects only the rescan flag before the
complete event/error object is dropped. The backend is not source authority; mandatory
periodic exact discovery repairs missed hints.

Five scheduler contracts prove exact timing, 10,000-hint fixed-state collapse, one
real `RefreshWorker` follow-up, clock rollback, pause/resume, submission fault, and
joined shutdown. Five watcher contracts prove real create/append/rename reduction,
root bounds, missing-root degradation, latest-generation replacement, and return of
Windows process handles/threads to baseline after 32 replacements. P1-D.6 live
archive/lease/worker/scheduler/watcher recovery and lifecycle assembly, P1-E, M0
acceptance, packaging, signing, and release remain unclaimed.

## 2026-07-15 — P1-D.6 live runtime and restart recovery assembled

Added `LiveRuntime` as the production composition root. Startup derives and acquires
the persistent archive sidecar guard before opening or migrating SQLite. Under that
guard it closes the one bounded orphan running scan set as failed, validates the exact
staging status/accounting versions/scan binding/revision/epoch, resumes complete
staging through bounded continuation and promotion, or exact-discards only an invalid
unpublished revision. Lease contention fails before SQLite creation; ambiguous or
unavailable state is preserved rather than deleted.

The worker execution object owns the path-private Codex adapter, SQLite archive bridge,
and reusable lease. Replay-verified complete/partial publications use the paired-CAS
incremental path; other truth uses full rebuild, and typed `rebuild_required` hands the
already-held guard to the one-shot executor without a second lease race. Successful
refresh updates only a bounded root vector and current watcher generation. Public
snapshots expose fixed phase, counters, refresh kind/outcome, and stable error code;
Debug contains no source or archive path.

One admission mutex orders scheduler submission against pause and shutdown. Pause
closes admission, pauses the scheduler, and cancels the exact active permit. Resume
invalidates watcher assumptions and forces one recovery reconciliation. Shutdown
drops watcher ownership, joins the scheduler and its worker reference, then cancels
and joins the worker; faulted state still attempts cleanup. Combined Windows evidence
returns handles and threads to baseline after two complete live generations.

TDD also exposed a store seam: after an exact scan admitted a new pending source, a
valid tail for an existing complete source was rejected as pending continuation. The
guard now requires exact current-replay membership rather than pending state while
preserving revision, archive generation, source generation, offset, identity, and
chunk-proof CAS. Direct store and real runtime regressions cover the combined append
plus new-source publication.

Focused evidence is 20 store unit tests, seven incremental store contracts, twelve
runtime incremental contracts, four startup recovery contracts, and three combined
live contracts. Clean-root audit, formatting, strict workspace Clippy, all store and
runtime targets, and the full locked workspace pass. The normal workspace run retains
the one explicitly ignored one-million-row M0 scale gate. P1-E immutable query
snapshots, sleep/race integration, M0 acceptance, packaging, signing, and release
remain unclaimed.

## 2026-07-16 — Architecture and release plan closed

Repeated the critical audit across the normative contracts, current dependency and
target configuration, implemented P0-P1-D.6 boundaries, and the exact external
reference pins. The previous broad feature matrix was replaced with a row-level
behavioral ledger covering quota/reset, dashboard, session/model/activity/code-output,
settings/widget/notifications, daily/weekly/monthly/session analytics, filters,
pricing, projects, live/statusline, JSON/offline/compact behavior, and explicit
security rejections. A parity claim now blocks on every row becoming implemented or
normatively rejected with a surviving regression gate.

ADR-024 freezes the remaining product decisions: P3 complete UI, P4 presentation and
localization, P5 read-only automation, canonical P6
`x86_64-pc-windows-msvc` signed portable ZIP, explicit GNU/MSVC comparison, Slint
Royalty-free License 2.0 attribution, no updater/installer in 1.0, release-pinned
pricing, permitted credential-free local or documented official quota sources only,
and advisory/source/license/secret/SBOM/action/attestation/package/clean-room evidence.
The closure review found no remaining known planning contradiction or unfrozen 1.0
release decision. This is architecture evidence only; P1-E, P2-P6 implementation,
M0 receipts, packaging, signing, parity, and release remain unclaimed.

## 2026-07-16 — P1-E.1 immutable engine publication added

Added one startup-seeded, fixed `EnginePublicationState` to the production live
composition. The public `EngineSnapshot` is a small copied value containing a checked
in-process generation, persisted archive generation, optional replay revision and
latest complete scan set, that set's exact completion timestamp, explicit archive
quality, and fixed checked diagnostics. It owns no SQLite connection/transaction,
query page, history, path, source, checkpoint, watcher, or UI handle. Consumers have a
strict newer-generation predicate.

Publication reads current SQLite truth after refresh work and writer-guard release.
Only a strictly newer archive generation replaces the retained snapshot. Equal and
older candidates increment fixed counters; busy/cancelled/deadline/failed work cannot
manufacture a newer archive snapshot. Ten thousand equal candidates retain one state,
and generation/counter overflow sets a fail-closed flag without wrapping. Startup
seeds current archive truth before worker admission, so an existing archive does not
depend on a new scan to become visible.

The store now exposes one indexed `scan_set_snapshot` lookup used to derive truthful
`data_through_ms`; a referenced set must be complete and have an exact completion time
or publication fails unavailable. RED proved the missing store method and missing live
engine field. Focused GREEN evidence is 12 scan contracts, three publication unit
contracts, two live publication contracts, the complete runtime target suite, and
strict store/runtime Clippy. The next P1-E gate remains the expanded race/recovery
matrix, Windows power-event suspend/resume binding, and final resource/CPU evidence.
M0 acceptance, P2 query snapshots, packaging, signing, and release remain unclaimed.

## 2026-07-16 — P1-E.2 publication race and recovery matrix closed

Added integration contracts for repeated no-change refresh, pause/resume reconciliation,
and restart ordering. They prove that in-process generation is process-local while
persisted archive generation/revision/scan freshness remains the durable authority.
Existing 10,000-hint, stale-cancellation, busy, older-candidate, and consumer-ordering
contracts complete the bounded race matrix without adding retained history.

The malformed-truncation RED contract exposed that the Codex adapter counted a blocking
diagnostic but still returned a complete batch. The reader now maps malformed,
incomplete, or oversized relevant input to fixed `invalid_data` before checkpoint or
batch commit. The failed rebuild publishes newer `recovery_pending`, leaves the prior
two canonical events readable, and a later valid rebuild publishes `complete` with a
new revision. Focused strict runtime Clippy and the complete runtime test suite pass.
Windows power-event binding and final resource/CPU evidence remain; P1, M0, packaging,
signing, and release are not yet accepted.

## 2026-07-16 — P1-E.3 Windows power binding completed

Added the Windows 8+ callback form of `RegisterSuspendResumeNotification` behind
`tokenmaster-platform`. A process-wide static capacity-one atomic signal stores only
the latest suspend/resume event and checked counters. It owns no callback heap context,
helper thread, hidden window, USER/GDI object, runtime reference, SQLite handle, path,
timestamp, or event history. Unknown codes are counted and ignored; every documented
resume form maps to one fixed event.

`LiveRuntime::apply_power_event` keeps suspend idempotent and makes every resume reset
watcher assumptions and force authoritative reconciliation, including resume-before-
suspend and coalesced/missed suspend. Registration is a singleton with explicit stable
shutdown errors and private Debug. RED contracts first proved the missing platform and
runtime APIs. Focused GREEN covers last-wins/duplicates/counter overflow, all resume
codes, actual OS register/unregister/reuse, runtime pause/resume/shutdown behavior, and
4,096 register/unregister cycles under a 1-MiB private-memory ceiling with no sustained
handle, thread, USER, or GDI growth. The first resource run exposed a two-handle observer
warm-up in ToolHelp measurement; after warming the measurement itself, the unchanged
strict plateau gate passed three consecutive runs.

P1 is now implemented and P2 immutable indexed query snapshots are next. This does not
accept M0, replace a frozen-candidate real hibernation/interactive receipt, run the final
soak, package, sign, or release the product. An explicit MSVC check stopped before
TokenMaster code because this machine has the Rust MSVC target but no `cl.exe`, Visual
Studio installation, or `vswhere`; canonical MSVC setup/comparison remains P6 rather
than a claimed validation result.

## 2026-07-16 — P2-A immutable query foundation approved

Audited the existing store indexes, publication state, product query contract, UI/CLI/
MCP bounds, and million-row latency target before starting P2. The approved design adds
a separate synchronous `tokenmaster-query` facade and dedicated SQLite read-only/
query-only store. Each response is captured in one short read transaction and returned
as owned bounded data with stable errors, progress-handler deadline, injected clock,
and no writer/UI/source handle.

The key planning correction is two-dimensional identity: archive publication generation
orders freshness/quality updates, while `empty`, immutable legacy, or replay-revision
dataset identity binds keyset cursors. A no-change scan may refresh `dataThrough`
without resetting scroll; a changed revision rejects the old cursor rather than mixing
pages. P2-A starts with indexed latest activity and proves `pageSize + 1` lookahead,
privacy, deadlines, and resources. P2-B will add schema-v8 transactional materialized
aggregates; UI code is never allowed to full-scan/group the event table. This is an
approved executable plan, not P2 implementation evidence.

## 2026-07-16 — P2-A bounded query values implemented

Added the root-workspace `tokenmaster-query` crate using RED/GREEN contracts. Its first
slice owns schema-v1 immutable headers and envelopes, checked process-local and
persisted generations, separate empty/legacy/replay dataset identity, one injected
wall/monotonic clock sample, stable path-free errors, bounded scope/warning collections,
and latest-activity pages capped at 256 owned items. Cursor and activity Debug redact
the canonical fingerprint, and invalid nanoseconds, generations, revisions, page sizes,
capacities, or continuation shape fail closed.

All six focused value contracts and strict package clippy pass. This is Task 1 only:
there is still no query SQLite connection, exact transaction capture, deadline handler,
service mapping, frontend worker, CLI/MCP surface, aggregate, pricing, or quota claim.

## 2026-07-16 — P2-A exact query-only store implemented

Added `UsageReadStore` as a separate exact schema-v7 reader. It uses SQLite read-only/
no-mutex plus query-only, foreign-key, defensive, QPSG and no-checkpoint-on-close
controls; trusted schema and DQS are disabled, mmap is zero, busy timeout is 250 ms and
the cache is fixed at 4 MiB. It validates but never migrates or exposes a write method.

One deferred transaction captures archive generation, empty/legacy/replay dataset
identity, complete scan time/manifest and the current or immutable-legacy event page.
Composite keyset SQL fetches only `pageSize + 1`, continuation without exact dataset
identity fails, optional token components stay absent, and activity/cursor fingerprints
remain redacted. Contracts prove unchanged archive bytes, missing/old/new/malformed
failure, revision zero compatibility, legacy ownership after store drop, stale identity,
concurrent-commit snapshot isolation, expected index/no offset/temp sort, deterministic
deadline interruption and handler cleanup. Store and query strict gates pass. Query
service/freshness mapping, P2-B aggregates, UI, automation and release remain unclaimed.

## 2026-07-16 — P2-A immutable activity query completed

Composed the exact read store into the synchronous `QueryService`. Successful captures
receive checked process-local generations; failed stale continuations consume no
generation. The facade maps empty/current/partial/recovery/legacy and clock rollback
without fabricated values, applies the documented 20-minute/2-hour usage freshness
policy, and marks current revisions built with obsolete accounting versions `unknown`
plus `accounting_version_stale`. A publication-only generation advance retains dataset
identity and cursor validity; a changed dataset fails with stable `stale_snapshot`.

Added a one-envelope consumer slot that rejects older candidates, coalesces equal
generations, and retained exactly one payload across 10,000 replacements. Owned result
and privacy contracts survive service drop and exclude archive paths, source IDs, raw
fingerprints, SQLite text, prompts, responses, commands, and reasoning content from
public Debug/error surfaces. The exact composite-index plan remains offset/count-free.

A generated 100,000-event schema-v7 archive measured 35.65 ms for a new read connection
plus first 256-row page and 1.10 ms for the warm cursor page, below the 1 s/250 ms gates.
A 256-cycle Windows open/query/drop test returned handles, threads, USER/GDI objects,
and private memory to the strict plateau. Focused store/query tests and strict Clippy
pass. The first full workspace gate exposed one missing store-to-runtime mapping for
the new query deadline code; a focused contract now preserves it as
`DeadlineExceeded`, and the repeated full gate passes clean-root, formatting, strict
workspace Clippy, normal locked workspace tests, doctests, and diff-check. The one
pre-existing explicitly ignored million-row M0 scale test remains outside that normal
gate. P2-A does not claim P2-B aggregates/pricing/quota, P3 UI, P5 automation, M0
acceptance, packaging, signing, or release; P2-B transactional materialized aggregates
are next.

## 2026-07-16 — P2-A current-tail cursor identity corrected

The P2-B write-path audit found that a current tail append can mutate `usage_event`
without promoting a new replay revision. Revision ID alone therefore could not bind a
keyset cursor safely. The deeper audit then proved replay evidence epoch is unsuitable
because it can advance during a complete no-change scan. Current dataset identity now
includes replay revision ID plus schema-v7 `dataset_generation`, which advances after
every canonical event insert/update/delete in the same transaction and stays fixed for
scan/publication-only work. Exact v6 migration fault rollback, overflow, real no-change
scan, append, store, and facade contracts prove the corrected boundary.

## 2026-07-16 — P2-B aggregate storage and bounded publication implemented

Schema v8 now makes the current canonical projection provider-self-contained and
creates exact generation-qualified UTC minute/hour and session rollups. Exact v7
migration preserves every canonical value and dataset generation, rejects source/
profile ambiguity, uses explicit `unknown` only for an old orphan, and rolls back at
every provider and aggregate boundary. Current insert/update/delete paths maintain
known-count/sum availability algebra, activity and context counters, event counts,
dataset generation, and rollups in one SQLite transaction. Missing singleton state,
overflow, or a missing expected published row fails the complete source mutation
closed.

Non-empty archives enter `rebuild_required` instead of blocking startup. The store
rebuilds current and immutable legacy facts through persisted fingerprint-keyset pages
of at most 256 events into disk-backed unpublished rows. Cleanup is bounded, reopen
resumes, event mutation restarts against the new dataset generation, faults roll back
the exact page/state, and publication is one checked active-generation update. No
history-sized Rust map or long-lived read transaction is retained.

The first release-mode reference measurements are 1.814 ms p95 for one event,
19.888 ms for 32, and 230.620 ms for 256 with aggregate maintenance ready; all remain
inside corrected absolute gates and 1.5-times matching-baseline limits. The old single
25 ms expectation for a maximum 256-event catch-up was rejected because the measured
aggregate-disabled baseline itself is 159.787 ms. Database amplification measurement
now includes the main SQLite file, WAL, and SHM. Aggregate/session reads, private
calendar composition, immutable public values, and million-row/resource acceptance
remain open; no complete P2-B or release claim is made.

## 2026-07-16 — P2-B exact overview read implemented

Added the first fixed aggregate consumer to the separate query-only store. One short
deferred transaction captures publication/dataset identity, requires a matching
`ready` aggregate generation, and returns checked owned metrics from only
`usage_time_rollup`. A request carries at most 32 unique typed scopes and one to three
ordered adjacent UTC segments aligned to minute or hour rollups. This lets the future
private calendar layer compose a boundary-minute prefix, full-hour middle, and
boundary-minute suffix without embedding timezone types in the store.

Contracts cover exact token known-count/sum values, current and empty scopes, stale
dataset and rebuild-required state, deterministic cancellation cleanup, concurrent
state mutation after snapshot acquisition, gaps, overlaps, misalignment, capacity,
and events exactly on width-transition boundaries. The query plan contains neither
raw event table nor `OFFSET`. Series, independent breakdowns, session keyset reads,
calendar/Jiff mapping, public facade values, and million-row/resource evidence remain
open; no P2-B completion or release claim is made.

## 2026-07-16 — P2-B exact series and breakdown snapshot implemented

Extended the fixed aggregate reader with a combined analytics capture. Overview, up
to 400 ordered series points, and any unique subset of model, project, provider, and
provider-qualified profile breakdowns now bind to one publication, dataset identity,
ready aggregate generation, and deferred transaction. Non-empty series points must
partition the overview exactly; a minute-aligned zero-duration point preserves a civil
date skipped by timezone history without inventing usage.

Each breakdown is a separate fixed rollup query rather than a caller-defined cube. It
orders by known total, event count, and stable identity, reads only 257 groups, retains
256, and reports truncation. Unassociated project is typed explicitly. Exact fixtures
cover mixed widths, boundary events, two scopes, all four breakdowns, scope filtering,
zero points, incoherent partitions, duplicate/capacity rejection, 257-model truncation,
real `EXPLAIN`, concurrent aggregate-state mutation, and cancellation cleanup. Focused
store tests and strict Clippy pass. Session reads, private calendar mapping, immutable
facade values, and million-row/resource evidence remain open.

## 2026-07-16 — P2-B opaque keyset session reads implemented

Added all-time session first/cursor pages and exact detail to the isolated read-only
store. Pages order by last UTC instant descending and provider/profile/private-session
identity ascending, mirror that mixed direction in the continuation predicate, bind
opaque keys and cursors to the exact dataset, accept at most 32 scopes, retain 256 of
257 rows, and expose no raw session getter or Debug value. Detail returns `None` for a
missing exact key or one summary plus independently capped model/project rollups;
unassociated project remains typed. Period analytics remain separate so whole-session
totals cannot be mislabeled as period-clipped values.

TDD fixtures cover equal timestamps across identities, two-page continuity without
duplicates, stale and unbound cursors, exact scopes, current and rebuilt legacy data,
missing detail, 257 sessions, 257 models, opaque Debug, concurrent aggregate-state
change, forced cancellation cleanup, and real `EXPLAIN QUERY PLAN`. Fixed SQL uses
`usage_session_rollup`, the composite page index, no `usage_event`, and no `OFFSET`.
The complete store suite and strict store Clippy pass. Private Jiff calendar mapping,
immutable aggregate facade values, million-row/resource evidence, UI, automation, and
release remain open. The post-documentation root gate then passed clean-root audit,
format check, strict locked workspace Clippy, all locked workspace tests/doctests, and
diff-check in 79.8 seconds.

## 2026-07-16 — P2-B exact calendar and immutable facade implemented

Pinned Jiff 0.2.32 inside `tokenmaster-query` and kept its timezone objects outside
public values and the archive contract. Explicit IANA or positively resolved system
zones now convert day, configurable week, month, and bounded custom half-open ranges
into at most three exact UTC minute/hour segments. Compatible gap/fold handling,
zero-duration Pacific/Apia skipped date, UTC, Asia/Jerusalem, America/New_York,
Australia/Lord_Howe, Asia/Kathmandu, leap/year edges, all seven week starts, and the
Africa/Monrovia historical sub-minute rejection pass. The locked platform chain is
`jiff-tzdb-platform` 0.1.3 and `jiff-tzdb` 0.1.8 / IANA tzdb 2026c.

`QueryService` now maps one short aggregate capture into owned immutable public
analytics with canonical zone/date/UTC boundaries, optional daily series capped at
400, exact unavailable/known/partial token algebra, activity facts, and independently
capped model/project/provider/profile breakdowns. All-time session first/continuation
and exact detail are mapped into opaque public keys/cursors with raw session identity
redacted. A closure audit found and fixed filter drift: continuation now retains the
canonical scope set and rejects a changed filter before SQLite so keyset rows cannot
be skipped. Missing detail remains typed `None`; changed datasets are stale; aggregate
rebuild is unavailable without raw fallback; failed calls consume no snapshot
generation. Focused strict query Clippy and the complete locked query suite pass.

Task 8 million-row latency, database amplification, Debug/privacy, rebuild-cycle, and
Windows resource evidence remains open; no P2-B completion, UI, automation, package,
or release claim is made.

## 2026-07-16 — P2-B scale, storage, privacy, and resource gate closed

Added deterministic release-mode current and immutable-legacy fixtures with one
million canonical events each. The first current red run exposed a real design defect:
the 256-event rebuild cap reached only 912,128 events after 346.44 seconds wall,
approximately 2,850 events/s, even though process private memory remained near 14 MiB.
The hard cap was raised to 2,048 events while preserving one persisted fingerprint
cursor, one expected dataset generation, disk-backed inactive rows, crash/reopen
resume, and at most 18,432 derived/cleanup rows per call. Existing fault, stale-
generation, bounds, reopen, and resource contracts remained green.

The repeated current/legacy million gate then passed in 174.35 seconds total. Current
rebuild took 75.528 seconds at 13,240 events/s with 246.558 ms page p95; legacy took
81.142 seconds at 12,324 events/s with 268.305 ms page p95. Both used 490 resumable
calls. Main+WAL+SHM amplification was 1.483x/1.568x. Cold overview was
174.318/178.241 ms; cached overview p95 0.543/0.365 ms; full 400-point/four-breakdown
p95 151.043/141.192 ms; all-32-scope full analytics 165.120/139.040 ms; session first/
cursor p95 stayed below 0.75 ms. Repeated bounded analytics/session snapshots and
cooperative rebuild reopen cycles retained private-memory, handle, thread, USER, and
GDI plateaus. Existing public Debug/error contracts exclude archive paths, source and
session identities, fingerprints, SQLite text, prompts, responses, commands, and
reasoning; future serialized CLI/MCP values still require their own wire privacy gate.

This closes P2-B Task 8 but does not claim pricing, quota/reset inventory, Git output,
complete UI, automation, M0 acceptance, packaging, signing, or release. P2-C pinned
pricing and validated overrides are the next product-data slice. After project-truth
synchronization, the complete baseline gate passed clean-root audit, formatting,
strict locked workspace Clippy, every locked workspace test/doctest, and diff-check in
79.326 seconds; P2-B Tasks 1-9 are complete.

## 2026-07-16 — P2-C deterministic pricing and cost facade closed

Added the pure `tokenmaster-pricing` engine with an embedded reviewed catalog, checked
integer microdollar calculation, exact aliases, bounded immutable overrides, and
`auto`/`calculated`/`reported` selection. Unknown models or tiers, incomplete basis,
overflow, capped missing keys, and source/catalog conflicts remain explicit rather
than becoming plausible zeroes. Runtime pricing remains local: the production closure,
source, and release-artifact string audit reports zero forbidden HTTP dependencies or
fetch signatures.

Schema v9 now retains only derived price-basis facts beside existing time/session
rollups. Live insert/replay/delete/replace and legacy rebuild update them in the same
transaction and generation; public reads bind exact dataset identity and batch the
overview, up to 400 series points, four capped breakdowns, 32 scopes, and session
page/detail targets without raw-event or one-query-per-item fallback. The immutable
query facade exposes cost availability, amount, mode, provenance, catalog and override
identity, conflict, counters, and bounded missing evidence.

The first release-scale run found scoped range pricing above the one-second contract.
Query-phase evidence isolated the regression to SQLite's tuple membership plan. A
bounded scopes CTE plus the composite price-time index restored indexed seeks; a real
32-scope `EXPLAIN QUERY PLAN` assertion prevents recurrence. The final current/legacy
million-event receipts passed at 8,737/8,129 rebuild events/s, 376.824/406.604 ms
rebuild-page p95, 1.862x/2.010x SQLite amplification, 2.040/2.065 ms cached-overview
p95, 148.168/156.080 ms full 400-point/four-breakdown p95, 158.588/162.504 ms
all-32-scope analytics, below 14 ms session-page p95, and below 1 ms detail p95.
Catalog/override/mode query switching retained private-memory, handle, thread, USER,
and GDI plateaus. The original two-point Windows `PrivateUsage` assertion was first
replaced by repeated warmup/measurement samples. A later complete workspace gate
proved that the default Rust test harness itself could terminate worker threads during
process-wide sampling and that a single allocator high-water sample could exceed the
budget even when later samples returned below the earlier floor. The resource contract
now runs as a Cargo `harness = false` single-thread process. It preserves per-sample
handle/thread/USER/GDI checks and the original 1/2 MiB budgets. A later full gate
showed that a fixed warm-up could still finish before process topology and allocator
phase converged. Warm-up is now bounded to 64 rounds and begins measurement only
after two eight-round windows share one structural topology and converged retained
floors; their higher return floor is the conservative reference. Deterministic
fixtures prove topology/allocator phase shifts and transient spikes do not masquerade
as leaks, while sustained growth and incomplete windows fail. Two fresh focused
processes, clean-root audit, formatting, strict locked workspace Clippy, every locked
workspace test/doctest, privacy checks, and diff review pass.

This completes P2-C only. P2-D quota/reset history and expiring reset inventory is the
next approved slice; Git output, joined snapshots, UI, automation, M0 acceptance,
packaging, signing, and release remain open.

## 2026-07-16 — P2-D exact quota domain implemented

Replaced the provisional `QuotaTarget` and floating-point used ratio with exact
provider-neutral quota values. Account/workspace/window/unit/provider-epoch IDs use a
bounded ASCII alphabet, observations use redacted exact 32-byte identities, ratios
use integer parts per million, and absolute provider units remain optional. Fixed
windows may carry provider-defined post-reset thresholds without any built-in
five-hour, weekly, zero-used, or full-remaining rule.

Definitions and samples now validate positive revisions/durations, ordered
observation/freshness/staleness times, optional capacity coherence, nonempty quota
facts, explicit reset evidence, exact reset occurrence bounds, and absence as typed
`None`. Custom deserialization repeats constructor validation and rejects unknown
nested scope/window fields. The new contract was observed RED before implementation;
eleven focused quota tests, the complete domain suite, strict domain Clippy, and
diff-check pass. The subsequent complete baseline also passes clean-root audit,
formatting, strict locked workspace Clippy, every locked workspace test/doctest, and
the corrected isolated query resource contract.

This closes only P2-D Task 1. The pure detector and deterministic identities are next;
schema v10, quota persistence/retention/reads/query, provider transport, banked-reset
inventory/reminders, UI, automation, M0 acceptance, packaging, signing, and release
remain open.

## 2026-07-16 — P2-D pure quota detector implemented

Added the I/O-free `tokenmaster-quota` crate with only `tokenmaster-domain` and
release-pinned `sha2` as direct production dependencies. Versioned, domain-separated
SHA-256 scope, epoch, and transition identities use normalized length-framed fields,
big-endian integers, fixed 32-byte values, and redacted `Debug`; fixed independent
vectors cover scope, epoch, and transition identity.

The constant-state evaluator now starts and advances epochs, retains comparable
maximum use, rejects conflicting duplicates and incoherent window/state continuity,
ignores stale observations, and detects provider-epoch, explicit provider/local,
manual/banked, and provider-threshold resets. Scheduled, early, manual/banked, and
lower-confidence unknown kinds remain distinct. Allowance changes are orthogonal and
may accompany a reset. Rolling recovery, ratio drops alone, and untrusted threshold
inference cannot manufacture resets. Per-window sequences are exact, monotonic, and
checked against overflow. Maximum ratio and maximum comparable units retain separate
observation identities in epoch state and reset transitions, allowing later retention
to protect the exact evidence rows when those maxima occur in different samples.

Critical self-review found that applying a newer window-definition revision would
have preserved the old epoch ID but overwritten the revision used to validate it,
making a later restore reject valid state. The state now retains the opening
definition revision separately from the latest applied revision; the regression test
proves revision advance, restart restore, and fail-closed revision rollback.

The package-absent RED, 11 focused identity/detector tests, strict warnings-as-errors
Clippy, direct-dependency audit, forbidden-capability scan, formatting, and diff-check
pass. The complete workspace gate also passes after the resource measurement
correction above. This closes P2-D Task 2 only. Task 3 strict schema v10 and exact v9
migration is next; writes, retention, reads, query, transport, inventory/reminders,
UI, automation, M0 acceptance, packaging, signing, and release remain open.

## 2026-07-16 — P2-D strict quota schema v10 implemented

Added a quota-owned schema-v10 plane inside the existing bundled SQLite archive:
singleton revision/count state, immutable definition revisions and samples, current
and closed epoch projections, immutable reset/allowance transitions, and one exact
current window projection. All seven tables are `STRICT`; identifiers, enums, times,
ratios, units, sequence values, and freshness relationships are checked. Fixed
indexes support definition lookup, sample/epoch retention, transition sequence, and
current-scope reads.

Critical review found two integrity gaps before commit. Global evidence IDs alone
could bind a current window to another window's sample or epoch, and SQLite `NULL`
check semantics could accept incomplete allowance-change units. Composite
scope/window/revision foreign keys now bind current and retained evidence exactly.
Allowance facts require both unit IDs and capacities, and increased/decreased/unit-
changed kinds must match their unit/capacity relationship. Published definitions,
samples, closed epochs, and transitions reject `UPDATE`; future bounded maintenance
retains authority to delete only whole unreferenced rows.

The exact v9 migration validates the source archive, creates and seeds only quota
objects inside one immediate transaction, advances the schema version, validates the
complete v10 contract, and commits without rewriting or reclassifying usage,
aggregate, dataset-generation, or price facts. An injected fault after quota creation
rolls back to exact v9 with no quota residue. Malformed current quota SQL fails closed
on reopen.

Five focused schema contracts, 20 migration unit contracts, 49 complete store unit
tests, every locked workspace test and doctest, strict warnings-as-errors workspace
Clippy, formatting, clean-root, and diff checks pass. The normal workspace run keeps
the two explicitly ignored reference/scale gates skipped. This closes P2-D Task 3
only. Task 4 transactional quota observation application is next; retention,
reads/query, transport, banked-reset inventory/reminders, UI, automation, M0
acceptance, packaging, signing, and release remain open.

## 2026-07-16 — P2-D transactional quota publication implemented

Added the public `UsageStore::apply_quota_observation` boundary and exact result values
for started, duplicate, stale, advanced, allowance-changed, and reset outcomes. One
immediate transaction loads only the target window, calls the pure quota evaluator,
and publishes immutable definition/sample/history/transition facts plus exact current
epoch/window state. Duplicate and stale observations are complete no-ops. Every visible
result advances the independent quota revision exactly once; reset plus allowance
change remains one complete transition.

Observation identity is global and content-stable, definition content is immutable per
revision, revision regression fails stale, and all revision/count/transition values are
checked against SQLite capacity. Critical review found that a missing current-window
projection could otherwise be silently recreated from the epoch. Writable use and
reopen now require the current epoch, current projection, and exact last sample to
agree on definition, identities, times, evidence metadata, and sequence; corruption
fails closed without repair.

Eight focused integration contracts cover start/advance/duplicate/stale, unpublished
future definitions, allowance and reset-plus-allowance, repeated resets, account
isolation, reopen continuity, observation/definition conflicts, and missing projection
tamper. Internal tests inject failure after sample, epoch, transition, projection, and
revision and prove exact rollback plus successful deterministic retry. The complete
store suite now has 51 unit tests and passes with strict Clippy.

The complete workspace gate initially exposed a repeatable false failure in the
isolated Windows resource harness: one transient low allocator sample became the
warm-up baseline and made the later stable plateau look like retained growth despite
unchanged handles, threads, USER, and GDI counts. A deterministic RED vector now
separates one low trough from a sustained phase change. Warm-up selection uses a
second-lowest retained floor, while measured windows retain their original minimum
return and sustained-growth rules. Two fresh isolated processes and the final
clean-root, formatting, strict locked workspace Clippy, and complete locked workspace
test/doctest gate pass; the explicitly ignored reference/scale tests remain skipped.

This closes P2-D Task 4 only. Task 5 bounded retention, restart, and maintenance fault
evidence is next; quota reads/query, permitted Codex transport, banked-reset inventory/
reminders, UI, automation, M0 acceptance, packaging, signing, and release remain open.

## 2026-07-16 — P2-D bounded quota retention implemented

Added fixed public retention contracts: 512 samples and 256 closed epochs/transitions
per window as soft defaults, 2,048 samples and 1,024 closed epochs/transitions as hard
caps, and maintenance pages from 1 through 256 candidates. The write transaction now
recognizes a consecutive same-definition sample whose normalized quota facts are
equivalent, moves the exact current epoch/window projection first, and removes only
the prior sample when no current, epoch, maximum-use, or transition reference protects
it. Ten thousand identical polls therefore retain only the protected first and newest
samples while quota revision still records every visible observation.

Added `UsageStore::maintain_quota_history_page`. One immediate transaction selects
only old unprotected samples in the requested scope/window that have a newer
same-definition normalized equivalent, deletes at most the fixed page, and adjusts
only the retained-sample count. It returns examined/deleted/remaining counts without
exposing IDs or rows and does not advance semantic quota revision. Meaningful history
may remain over the soft default; transitions and closed epochs are never merged or
deleted in this task. First/current/last, independent ratio/unit maxima, and every
transition pre/post/max sample remain protected.

Critical review added persisted hard-cap validation on reopen rather than trusting
only the global singleton counts. An applying observation that would create sample,
closed-epoch, or transition count above a hard cap rolls the complete transaction
back. A manually altered archive with one extra valid sample and a correspondingly
altered global count now fails closed. Maintenance fault injection after deletion and
after state-count update proves exact rollback and deterministic retry.

Seven focused integration contracts cover the 10,000-poll plateau, 600-row paged
backlog, window isolation, protected first/max/current evidence, 513 meaningful
samples, 1,024 reset transitions with complete pre/post evidence, page bounds, hard
caps, tampered reopen, boundary restarts, and revision/sequence overflow. The store
suite has 52 unit tests including both maintenance rollback boundaries; focused store
tests, clean-root, formatting, strict locked workspace Clippy, and the complete locked
workspace test/doctest suite pass. The normal run keeps the explicitly ignored
reference/scale gates skipped.

This closes P2-D Task 5 only. Task 6 defensive quota read snapshots and bounded keyset
transition history is next; public query mapping, permitted Codex transport,
banked-reset inventory/reminders, UI, automation, M0 acceptance, packaging, signing,
and release remain open.

## 2026-07-16 — P2-D defensive quota store snapshots implemented

Added fixed `UsageReadStore` quota captures. Current capture accepts zero through 32
unique exact window keys and returns one independent quota revision plus owned
available definitions, current samples, current epoch state and first samples, and
optional exact last transitions. Missing requested windows remain absent. Transition
capture accepts one exact window, optional expected revision, optional opaque
revision/filter-bound cursor, a page from 1 through 256, and a deadline no greater than
two seconds. It returns newest-first immutable transitions, owned pre/post samples,
and a continuation only after a fixed one-row lookahead.

The SQL surface is fixed and quota-only: no caller SQL, sort, projection, `OFFSET`,
usage table, or price table is accepted. Exact primary/composite index plans are
asserted for current, first transition, and cursor pages without temporary sorting.
Each capture owns one deferred snapshot. Stale revisions and changed cursor filters
fail before returning values; deadline interruption is cleared before the next query.
Critical review found that SQLite VM interruption alone was not a strict total
deadline across multiple short statements, so a completed late capture is also
rejected.

Read-side values are restored through domain/quota authority rather than unchecked
DTO construction. `QuotaTransition` now retains its definition revision and exposes a
validated restore/parts round trip that recomputes deterministic identity and checks
kind/epoch, allowance, maximum-use, reset-time, and detection shapes. The store also
cross-checks current epoch provider/reset projection and transition source, old/new
reset times, allowance boundary units, detection interval, observation ordering, and
reset-current epoch identity against joined samples. Long-lived-reader tests prove
that missing last transitions and post-open epoch/transition projection drift fail
`InvalidStoredValue` rather than becoming plausible UI state.

Six focused quota query contracts, 56 store unit tests, the complete store and quota
suites, strict quota/store Clippy, formatting, diff checks, clean-root, and the
complete locked workspace test/doctest and warnings-as-errors gate pass. The normal
workspace run keeps the explicitly ignored reference/scale tests skipped.

This closes P2-D Task 6 only. Task 7 immutable public quota query values/service is
next; permitted Codex transport, banked-reset inventory/reminders, UI, automation, M0
acceptance, packaging, signing, and release remain open.

## 2026-07-16 — P2-D immutable public quota facade implemented

Added query-owned quota values and two fixed `QueryService` methods. Current requests
accept zero through 32 unique exact windows, preserve request order, and return one
explicit available or unavailable result per filter. Transition requests return
newest-first immutable pages with an opaque continuation bound to the exact window and
quota revision. `QuotaQueryHeader` is independent from usage `DatasetIdentity` and
carries checked process-local generation, exact quota revision, generated/data-
through time, provider-defined aggregate freshness, worst truthful selected quality,
exact bounded filters, and stable warnings.

Public values preserve definitions, ratios, optional units, samples, current epochs,
last transitions, reset/allowance kind, evidence, confidence, and exact-or-interval
detection time without leaking store DTOs. Filter, label, account/window,
provider-epoch, and cursor identity are redacted from public Debug. A stale revision,
changed filter, unavailable window, clock rollback, and partial/conflicting evidence
remain explicit. Generation advances only after store capture, mapping, and header
construction succeed; a stale continuation therefore does not consume consumer
ordering.

Four focused facade contracts cover scheduled, early, unknown, manual/banked,
allowance, ratio-only, unit-bearing, repeated sequence, first/continuation paging,
changed filters, stale revisions, freshness boundaries, quality aggregation, missing
windows, clock discontinuity, bounds, stable warnings, and Debug privacy. The complete
locked query suite, resource contract, formatting, diff check, and strict query
Clippy pass.

This closes P2-D Task 7 only. Task 8 scale, resource, privacy, offline authority, and
project-truth closure is next; transport, banked inventory/reminders, UI, automation,
M0 acceptance, packaging, signing, and release remain open.

## 2026-07-16 — P2-D quota history core acceptance closed

Added a pure adversarial detector matrix proving that rolling/unknown windows and
low-quality or low-confidence fixed-window recoveries cannot infer automatic resets.
Explicit manual/banked evidence remains typed even when automatic evidence is
conflicting or unknown, while opaque scope/epoch/transition identities remain
redacted.

Added an ignored release-scale quota gate covering 32 windows, 1,000 immutable
scheduled/early/manual repeated transitions, 10,000 duplicate polls, writer restart,
reader reopen, request-ordered current snapshots, complete 256-row keyset paging,
bounded maintenance, and coexistence with both current and migrated immutable-legacy
usage. The reference run completed in 1.72 seconds. Maximum measured calls were
3.429 ms for a visible write, 0.228 ms for a duplicate poll, 2.774 ms for the
32-window snapshot, and 1.256 ms for a 256-row history page.

Extended the isolated Windows resource binary with repeated quota current/history/
switch/reopen cycles under the existing topology-stable private-memory,
handle/thread/USER/GDI plateau rules. Added `scripts/audit-quota-network.ps1`; its
release closure covered 76 production dependency packages, 43 production files, and
the three current quota/store/query release libraries with zero forbidden network,
browser, cookie, shell, socket, or async-client matches.

This closes P2-D Tasks 1-8 and the provider-neutral quota history core only. The next
honest blocker is a permitted credential-free Codex quota transport. Banked-reset
inventory/reminders, notification delivery, UI, CLI/MCP, M0 acceptance, packaging,
signing, and release remain separate and unclaimed.

## 2026-07-16 — official Codex quota connector implemented

The source decision was revalidated against the installed Codex `0.144.1` generated
non-experimental JSON Schemas and the official app-server manual. The accepted
boundary is one caller-resolved native executable and one short-lived
`app-server --stdio` session. Session JSONL inference, dashboard/slash-command
scraping, browser cookies, private endpoint replay, local-token allowance estimates,
persistent children, and shared sockets remain rejected.

Added strict private account and rate-limit wire values plus
`CodexQuotaNormalizer`. ChatGPT email is bounded, normalized, hashed through a
domain-separated SHA-256 identity, and dropped; no raw email, Codex home, reset-credit
ID, response frame, or provider error can enter public values/errors/Debug. A non-empty
multi-bucket map supersedes its legacy duplicate. Primary and secondary windows expand
to at most 32 provider-neutral fixed-point observations. Integer percentage/time/
duration conversion, deterministic observation IDs, provider-official evidence,
provider-supplied reset thresholds, 20-minute fresh and two-hour stale boundaries,
legacy fallback, account switching, invalid/unknown/oversized input, clock overflow,
and privacy surfaces have focused contracts. Reset-credit detail is validated up to 64
rows only and is not yet stored or exposed.

Added `CodexAppServerCommand` and `CodexQuotaTransport`. The descriptor accepts only an
absolute regular non-reparse native file; Windows requires `.exe`. One poll executes
only fixed `app-server --stdio` arguments, discards stderr, hides the Windows child,
uses one helper thread and one complete monotonic deadline, and performs stable
initialize/initialized, account read, and rate-limit read without enabling
`experimentalApi`. Initialization opts out of `account/rateLimits/updated` and the
observed unsolicited `remoteControl/status/changed` notification. The parser enforces
fixed request IDs, strict unknown-field rejection, a 256-KiB frame, 1-MiB complete
stdout, 64 frames, exact supported version, and stable redacted failure codes.
Success, malformed input, RPC error, EOF, and timeout all terminate/reap the child and
join the helper before return. The caller-provided poll-start wall clock is a
conservative observation lower bound, so process duration may age evidence slightly
but cannot overstate freshness.

Repeated real authenticated live smokes returned two normalized observations in
0.70-0.94 seconds. The deterministic fixture covers success, stderr, unsupported
version, RPC failure, malformed/unknown/blank/oversized envelopes, unsolicited
notification, both-result-and-error, missing result, wrong/duplicate/out-of-order/
negative IDs, early EOF, and hang. A separate `harness = false` Windows process ran
16 warm-up plus 64 measured rounds, each containing success, RPC failure, and forced
timeout. Parent resources retained a stable approximately 1.4-MiB private-memory,
four-thread, USER=1, GDI=0 plateau. Focused/full-workspace runs observed 131-135 stable
handles within their respective process topology, and no task-owned fixture process
remained.

Added `scripts/audit-codex-quota-transport.ps1`. It traverses 72 production dependency
packages, scans 22 non-test Codex library source files, proves one fixed command and
argument construction, builds the release library, and rejects network/browser/async,
cookie, private-endpoint, credential-file, shell, socket, experimental, logging, and
raw persistence authority. The current release audit reports zero forbidden matches.
Focused normalization/transport/adversarial/resource tests, strict Codex Clippy, the
complete Codex suite, and the authenticated live smoke pass.

This implements and verifies the built-in Codex quota source boundary only.
Executable discovery, dedicated quota refresh scheduling, writer coordination,
transactional store publication, health projection, benefit inventory/reminders,
UI, CLI/MCP, M0 acceptance, packaging, signing, and release remain unclaimed. The next
implementation slice must finish app-server I/O before acquiring the existing writer
lease and may pass only the owned normalized snapshot into quota publication.

## 2026-07-16 — dedicated Codex quota runtime implemented

Added path-private executable selection in `tokenmaster-runtime`. Explicit native
configuration is authoritative and invalid configuration cannot fall back. Automatic
selection captures the current process `PATH` on each poll, rejects more than 64 KiB
or 128 entries, ignores relative entries, and validates only the exact platform
native `codex.exe`/`codex` filename through `CodexAppServerCommand`. It never resolves
shell aliases, `PATHEXT`, `.cmd`, `.ps1`, JavaScript/package-manager wrappers, browser
state, or credentials. Deterministic contracts cover directory order, exact-name
selection, invalid explicit configuration, bounds, missing candidates, and Debug/error
redaction.

Added `CodexQuotaRuntime` as a composition independent from the usage `LiveRuntime`.
It reuses distinct instances of the constant-state scheduler and worker, performs one
startup recovery refresh, coalesces manual/resume bursts, uses a 15-minute normal
period and a 60-second period only for bounded transient process/lease failures, and
owns pause/resume/power/shutdown/Drop lifecycle. Quota phase, schedule, worker, latest
attempt, stable failure stage/code, counts, elapsed/observation time, and last-success
time are exposed separately from usage-engine health without paths, identities,
labels, quota values, raw provider payloads, or inner platform/store errors.

The execution captures its wall-clock lower bound and completes discovery plus the
short-lived app-server poll before trying the shared process writer lease. It rechecks
cancellation/deadline, acquires without waiting, opens SQLite only under the guard,
and applies at most 32 normalized observations in deterministic order. The guard spans
the complete loop while each window retains the existing independent idempotent
transaction. A later failure may keep an exact committed prefix and reports its
counts; writer contention opens no SQLite store. Cancellation after source I/O
publishes nothing. Store and guard are dropped before health publication.

Focused discovery, execution, lifecycle, public fail-closed, concurrent
usage-runtime/quota-worker fault-isolation, and full runtime tests pass with strict
Clippy. The isolated Windows resource harness ran success, RPC failure, forced
timeout, writer contention, and pause/resume in every round. It
established a stable plateau after 16 warm-up rounds and passed 48 measured rounds
with a 3,149,824-byte retained private floor, 5,615,616-byte sampled high, 131
handles, four threads, USER=1, GDI=0, and no remaining task-owned fixture child.
`scripts/audit-codex-quota-runtime.ps1` traverses 114 production dependency packages,
checks the production portions of six quota-runtime source files and the release
library, proves exact-native discovery plus source-before-publisher/lease-before-store
ordering, and reports zero
forbidden network/browser/cookie/private-endpoint/credential-file/shell/socket/
direct-SQL/foreign-runtime matches. The current machine contains npm Codex
`.ps1`/`.cmd` wrappers, while the exact-native search resolves the installed Windows
app `codex.exe`.

This closes executable discovery, quota refresh scheduling, writer coordination,
transactional publication, separate health, and runtime resource/security evidence.
Typed reset-credit benefit inventory, expiration reconciliation, reminders,
notification delivery, activation, UI, CLI/MCP, M0 acceptance, packaging, signing,
and release remain unclaimed. The next implementation slice is the independently
approved benefit inventory/reminder contour; inventory read must not authorize
activation.

## 2026-07-16 — benefit inventory foundation through strict schema v11

Implemented the first four tasks of the approved benefit contour. The domain now owns
bounded provider-neutral lots, typed expiry precision, opaque identities, inventory
observations, notification channels, and versioned profiles. The new pure
`tokenmaster-benefits` crate reconciles awarded/changed/missing/ambiguous/reappeared/
terminal facts and computes deterministic reminder keys without I/O, clock, SQLite,
thread, UI, or provider authority. The Codex normalizer hashes detailed reset-credit
IDs by pseudonymous account, discards title/description, preserves separate lots, and
emits one aggregate unknown-expiry remainder only when the official available count
exceeds detailed available rows.

Schema v11 adds strict benefit state/scope/material revision/current/change/profile/
threshold/due/delivery objects and exact transactional v10 migration. One observation
atomically publishes current facts, immutable history, freshness, and due rows.
Duplicate polls append nothing; freshness-only input advances publication without lot
history. Terminal disappearance preserves one bounded latest-change cursor, allowing
the same opaque lot to reappear after restart with monotonic revision rather than
identity reuse.

Retention uses 512 changes and 256 deliveries as soft defaults, 2,048/1,024 hard
limits, and one total 256-row page. It protects current/latest terminal evidence,
removes only orphan material revisions and noncurrent receipts, advances the benefit
publication when visible history changes, and never scans usage events. Fresh schema,
exact v10 migration, weakened-schema rejection, write/restart/profile/terminal
reappearance, 600-change plateau, delivery retention, and injected schema/write/
maintenance rollback pass. Strict store Clippy, the complete store suite, dependency
inspection, `git diff --check`, and `TM-CLEAN-PASS` are green. Immutable benefit query
snapshots are next; reminder runtime, UI, automation, activation, packaging, and
release remain unclaimed.

The full workspace gate also exposed a scheduler startup race: a paused scheduler
could inherit the constructor recovery flag and receive another from `resume()` if
its thread started in the intervening window. Paused construction now starts with no
pending flags, so resume is the sole startup-recovery authority; the focused quota
runtime test and resource contract pass.

## 2026-07-16 — immutable benefit query snapshots

Completed Task 5 of the approved benefit inventory contour. `UsageReadStore` now owns
benefit-only current and history captures that read the independent benefit revision
and scope facts in one deferred transaction. Current rows restore immutable material
revisions, reject disagreement with redundant projection columns, and return at most
64 lots in conservative FEFO order. History uses descending `(sequence, change_id)`
keyset paging with 256+1 lookahead and a redacted cursor bound to the exact scope and
global benefit revision.

`tokenmaster-query` now exposes independent immutable benefit envelopes for current
inventory and change history. They preserve absent, fresh/aging/stale,
complete/quantity-partial/partial, unknown-expiry, unknown-evidence, nearest
expiry/due, and inherited/override profile truth. Configured OS scheduling is not
claimed; coverage is only `in_app_only` when the supported channel is configured.
Failed/stale/corrupt requests consume no public snapshot generation, and inventory
read grants no notification or activation authority.

Focused acceptance covers restart, exact concurrent read snapshots, deadline-handler
cleanup, live projection corruption, no usage-dataset scan, 64 current lots, 2,048
changes, and eight 256-row history pages. On this machine the current read measured
0.842 ms and the slowest history page 4.904 ms. After 32 open/query/drop cycles the
process returned at 4,517,888 private bytes, 116 handles, five threads, USER=2, and
GDI=0. Strict query Clippy, complete store/query suites, and diff checks pass. Task 6,
separate benefit publication through the existing Codex quota runtime, is next;
reminder delivery, UI, automation, activation, M0 acceptance, packaging, signing, and
release remain unclaimed.

## 2026-07-16 — Codex runtime publishes quota and benefit inventory separately

Completed Task 6 of the benefit contour. One short-lived Codex app-server poll now
produces one owned snapshot that reaches writer admission only after provider I/O.
The runtime tries the shared process lease once, opens `UsageStore` once, and holds the
same non-interleaving guard while applying at most 32 quota windows and one optional
benefit observation. Each quota window and the benefit inventory retain independent
transactions and revisions: quota success survives benefit failure, benefit success
remains visible after quota failure or duplicate input, and no cross-domain atomic
success or rollback is claimed.

The public runtime snapshot now reports separate quota and benefit observed, processed,
exact status, failure, and last-success facts, plus benefit material-change and
pending-due counts. Common lease/open/control failure remains distinct from quota or
benefit transaction failure. Count/status arithmetic and expected domain cardinality
are validated before publication; inconsistent internal reports fail closed as
`invalid_data` and cannot advance any success timestamp.

Focused execution tests cover source-before-publication, cancellation, writer
contention without store creation, quota success plus benefit failure, benefit success
plus quota failure, benefit contention retry, quota/benefit duplicate publication
across publisher restart, and report corruption. The Windows harness uses a real
reset-credit response in every success round and passed 16 warm-up plus 48 measured
success/RPC/timeout/busy/pause-resume rounds at a 3,432,448-byte private floor,
6,139,904-byte sampled high, 131 handles, four threads, USER=1, GDI=0, with no
task-owned child remaining. The refreshed release-authority audit covers 115
production dependency packages, six production quota-runtime source files, and one
release library with zero forbidden matches. Reminder delivery, UI, automation,
activation, M0 acceptance, packaging, signing, and release remain unclaimed.

## 2026-07-16 — durable one-timer benefit reminder runtime implemented

Completed Task 7 of the approved benefit contour and corrected one omission in the
original file plan: runtime could not safely own reminder SQL. The new narrow
`UsageStore` operation is therefore the sole due-queue mutation boundary. One
immediate transaction reads at most 256 indexed in-app rows, drops expired entries,
collapses overdue thresholds to the smallest useful lead per lot revision/channel,
records the immutable receipt before deleting the examined rows, updates exact global
counts, and returns only provider-neutral delivery values plus the nearest due time.
A selected urgent receipt suppresses equal and less-urgent missed thresholds after
restart or profile/inventory rebuild while preserving future more-urgent thresholds.

The new `BenefitReminderRuntime` is isolated from usage and quota execution. It owns
one dedicated scheduler, one existing bounded worker, one nearest wall-clock deadline,
one coalesced urgency, one latest count-only health snapshot, and one pending owned
batch capped at 256. Startup and resume force recovery; inventory/profile/clock hints
coalesce; transient writer/store failure uses one 60-second retry. An unconsumed batch
backpressures later store commits rather than overwriting or accumulating events.
Pause/suspend close admission, resume/hibernation recovers, shutdown/`Drop` join both
threads, and scheduler panic output is thread-locally redacted.

The final implementation corrects a crash gap found during critical review. Schema
v12 adds a separate immutable acknowledgement relation: the delivery row is a durable
outbox item, not proof that P3 displayed it. Taking a batch leases it; release makes a
failed presentation retryable; only explicit post-presentation acknowledgement ends
restart replay. Exact v11 migration marks legacy receipts acknowledged, retention
protects unacknowledged rows, and acknowledgement contention preserves the leased
batch.

Five store reminder contracts, four schema-migration contracts, and five public
runtime contracts cover the exact 256-row split, overdue collapse, future one-hour
preservation, expired drain, disabled profiles, outbox-before-event, pre-ack restart
replay, post-ack deduplication, release/retry, acknowledgement contention, 10,000
mixed hints, pause/resume/clock recovery, contention-before-SQLite, and reminder-fault
isolation from a live usage runtime.
Private scheduler tests cover the exact nearest-due wait, notification backpressure,
burst coalescing, and one accelerated retry deadline. The Windows harness passed 16
warm-up plus 48 measured delivery/acknowledgement/reconcile/lifecycle/contention rounds
at a 3,440,640-byte private floor, 5,799,936-byte sampled high, 117 handles, four
threads, USER=1, and GDI=0. The new four-package release audit covers 125 production
dependencies, four reminder source files, and four release libraries with zero
forbidden dependency/source/binary matches.

This publishes durable typed in-app events only; it does not claim that unfinished P3
rendered a notification. OS/tray scheduling, snooze, quiet hours, activation, CLI/MCP,
M0 acceptance, packaging, signing, and release remain unclaimed.

## 2026-07-16 — benefit inventory/reminder authority contour closed

Completed Task 8 and closed the approved read-only benefit inventory/reminder
foundation without expanding provider authority. The final audit covers four
production packages, 125 dependency packages, four reminder production source files,
and four release libraries. It confirms lease-before-store, outbox-before-publication,
post-presentation acknowledgement, the fixed 256-row due-page boundary, durable
less-urgent suppression, no direct runtime SQL, no foreign runtime, and zero
forbidden dependency, source, or binary-string matches.

The complete clean-root audit, formatting check, warnings-as-errors locked workspace
Clippy, locked workspace tests and doctests, specialized benefit authority audit,
complete-diff check, dependency/language review, and task-owned process-return check
all pass. This closes only the read-only inventory, history, publication, and durable
typed in-app event foundation. P2-E Git output is next. Visible P3 notifications/UI,
OS/tray delivery, snooze, quiet hours, CLI/MCP, activation, M0 acceptance, packaging,
signing, and release remain unclaimed.

## 2026-07-16 — Git output foundation and private repository hints

Completed P2-E Tasks 1-4. New strict domain values cover opaque installation-scoped
repository/activity identity, bounded day/category metrics, commits/merges, quality,
warnings, unavailable reasons, freshness, and omission truth. The isolated
`tokenmaster-git` core incrementally parses capped NUL-framed native Git output,
classifies versioned product-code paths, and emits at most 256 aggregates without
retaining commit or file history.

The exact native backend discovers and validates one platform executable without a
shell, runs fixed read-only version/repository/config/ref/log commands with paging,
hooks, credentials, network helpers, external diff, textconv, and mutation paths
disabled, reads stdout/stderr concurrently under caps, and owns kill/wait/join cleanup
on every cancellation, deadline, parse, and drop path. Synthetic fixtures cover root,
ordinary, branch deduplication, merge/octopus, rename, binary, gitlink, worktree,
mailmap, empty, missing-author, shallow, and history-change behavior.

Codex now advertises `RepositoryActivity` and emits one latest transient hint beside a
source batch. The hint preserves exact provider/profile/source/session/time and safe
project alias while its canonical local path remains sealed, non-serializable, and
redacted. Shared platform policy rejects relative, traversal, network/device/mapped-
remote, symlink, and reparse ancestry. Explicit invalid `cwd` clears old transient
association; untimed context may use the next timed usage line. Parser resume,
checkpoints, observations, canonical batches, SQLite, diagnostics, errors, and Debug
remain path-free. The Git backend repeats the same local-directory validation for the
executable parent, candidate, common directory, and worktree root before command use.
Schema-v13 Git projection, query, runtime scheduling/resource gates, joined status,
and UI remain unclaimed.

## 2026-07-16 — Private incremental Git projection added

Completed P2-E Task 5. Schema v13 adds a random installation salt, independent
monotonic Git publication revision, at most 32 opaque repositories, 4,096 opaque
activity associations, immutable daily/day-category/category/warning generations,
eight fixed categories, 16 warnings, and no repository path, executable path, author
email/name, ref, commit/object identity, file path/content, command, stdout, or stderr.
Exact v12 migration is transactional and its injected post-schema failure restores the
literal prior schema and all usage, pricing, quota, benefit, reminder, and
acknowledgement facts.

Authoritative rebuild and same-process CAS-proven append switch one generation
atomically. Unchanged refresh changes no aggregate; changed or incompatible authority
marks the prior projection rebuild-required; stale CAS and injected repository/state
faults write nothing. Unavailable results have no fabricated cache identity or zero
series. All-time totals remain exact while daily retention keeps only the latest 400
days; any older-day loss forces partial `daily_history_truncated`, exposes the oldest
retained day, and marks older requested ranges incomplete.

The defensive read store now returns owned bounded all-time/range totals, eight
all-time/range categories, daily points, warnings, quality, omission counters, and
32+1 repository lookahead under a hard maximum two-second deadline. Completed-late
reads fail, SQLite interruption is mapped explicitly, and the progress handler is
cleared before reuse. A missing project key clears prior association state; multiple
associations expose a key only when every row agrees, otherwise the capture becomes
partial with `association_incomplete`. Focused Git schema/projection/incremental/query,
domain retention, and strict store Clippy validators pass. The clean-root audit,
formatting check, warnings-as-errors locked workspace Clippy, complete locked
workspace tests/doctests, and diff check also pass. Public query envelopes, the
cost-efficiency join, runtime publication/resource evidence, final authority audit,
joined status, and UI remain unclaimed.

## 2026-07-16 — Immutable Git query and exact efficiency facade added

Completed P2-E Task 6. `QueryService::git_output` now returns a schema-v1 owned
envelope with checked process-local generation, independent Git publication revision,
an explicitly labelled UTC half-open range, all-time and requested totals/categories,
retained days, freshness, quality, warnings, omission/retention truth, and 32+1
repository lookahead. Old snapshots remain independent across later publication and
service restart, and failed calls do not consume a generation.

Project attribution no longer requires query-layer access to the installation salt.
The exact safe `ProjectAlias` from the transient activity hint is domain-separated and
installation-salted; one fixed store matcher compares at most 32 opaque keys with 256
materialized usage project candidates and returns only candidate indices. Public Git
values expose a matched safe alias but no salt, path, or opaque project key.

The cost-per-100-added-product-code-lines join uses round-half-up fixed-point
arithmetic and one shared maximum two-second budget. It reads only materialized
usage/project/price aggregates and produces a value only for exact UTC range,
association, complete Git evidence, compatible non-stale usage, exact non-conflicting
cost, and a nonzero denominator. Ambiguity, retention, stale/unavailable/corrupt
evidence, deadline, unknown cost, and zero lines are typed absence. A usage-side
failure degrades only efficiency and cannot hide independent Git facts.

Focused acceptance covers privacy/UTC boundaries, aggregate-only reads with the raw
usage table unavailable, 32 repositories by 400 days under the service deadline,
one-row lookahead, restart and concurrent publication isolation, failed-generation
neutrality, corruption rejection, repeated transaction release, and Windows handle
stability. The clean-root audit, formatting, warnings-as-errors locked workspace
Clippy, complete locked workspace tests/doctests, and diff check pass. Runtime
discovery/scan/publication, its lifecycle/resource/authority gates, joined status, P3
UI, CLI/MCP, M0 acceptance, packaging, signing, and release remain unclaimed.

## 2026-07-17 — Bounded Git runtime and P2-E authority closure

Completed P2-E Tasks 7-8. `tokenmaster-git` now retains one compatible in-process
frontier and selects unchanged, ancestry-proven append, or authoritative rebuild
without persisting commit IDs. `GitRuntime` owns one constant-state scheduler/worker,
one active scan, one aggregate follow-up, and at most 32 latest transient repository
candidates. `LiveRuntime` routes the Codex reader side channel into it without changing
usage accounting.

All Git discovery, scanning, bounded parsing, and exact child cleanup finish before
one non-waiting writer-lease attempt and one SQLite open. Publication rechecks the
candidate sequence, so superseded work cannot commit. Known scan failures now publish
durable unavailable truth or mark an existing trustworthy generation rebuild-required
instead of writing zero. Pause closes admission, invalidates raw object-ID frontiers,
cancels and reaps the exact child, and retains only bounded process-memory candidates;
resume forces rediscovery. Shutdown and `Drop` clear candidates and join owned work.

Focused contracts cover unchanged/append/rewrite, 32-candidate eviction, sibling
fault isolation, contention after Git I/O, stale-result follow-up, missing-author
durable failure, live Codex routing, pause/resume recovery, and child cleanup. The
Windows 16-warm-up/48-measured runtime gate passed at a 3,293,184-byte private floor,
6,422,528-byte sampled high, 118 handles, four threads, USER=1, and GDI=0. The Git
authority audit passed across 126 production dependencies, 19 production boundary
files, and four release libraries with zero forbidden dependency, foreign-language,
network/browser/credential/shell/direct-SQL/mutation, vendored-upstream, or private
binary-string matches.

The clean-root, formatting, warnings-as-errors locked workspace Clippy, complete
locked workspace tests/doctests, specialized Git audit, diff check, dependency/
language review, and task-owned process-return gates pass. This closes P2-E only.
P2-F joined product status, P3 UI, P5 CLI/MCP, M0 acceptance, packaging, signing, and
release remain unclaimed.

## 2026-07-17 — Exact joined product status and immutable product reducer

Completed P2-F under
`docs/superpowers/plans/2026-07-17-tokenmaster-p2f-product-status.md`. Schema v13 now
supports one defensive scalar status transaction binding usage publication/dataset/
aggregate progress with independent quota, benefit, and Git state. The query facade
maps the capture into one bounded schema-v1 `ProductDataStatusEnvelope` and consumes no
public generation on capture or mapping failure. Fixed statements and the authority
audit prove the status path does not scan event, rollup, quota-sample, benefit-change,
or Git-day history.

Added the leaf `tokenmaster-product` crate. Its reducer retains one current immutable
snapshot and no history; checked attempt generation is independent from durable source
generation and runtime health uses another checked generation. Stale async work is
rejected, compatible failures retain last-good payloads plus stable path-free codes,
and incompatible durable identities invalidate only affected sections. Usage, quota/
benefit, reminder, and Git runtime owners remain outside the product layer; only
bounded count/lifecycle/retry/failure projections are copied.

Eleven fixed routes derive `ready`, `degraded`, or `unavailable` from one `u16` reason
set. Aggregate rebuild degrades Dashboard section by section, leaves Activity and Data
Health reachable, and disables only History, Sessions, Models, and Projects. Settings
and Help/About remain archive-independent. Real pause/resume, reminder contention,
quota transport failure, and sibling-fault isolation pass.

The deterministic 100,000-event status fixture measured 0.125 ms p95 over 40 samples
against the 25 ms gate. Ten thousand reducer replacements retain one current payload.
The isolated Windows gate completed 1,152 open/capture/drop cycles with 111 stable
handles, four threads, USER=1, GDI=0, and private memory returning below the original
+2 MiB budget after bounded topology/convergence warm-up. The product audit reports
one leaf package, six production files, zero dynamic state collections/runtime owners,
no direct filesystem/network/process/SQL/UI authority, 11 fixed routes, and zero
vendored-source/release-string matches. P3 visible UI, P5 automation, M0 acceptance,
packaging, signing, and release remain unclaimed.

After project-truth synchronization, the clean-root audit, formatting check,
warnings-as-errors locked workspace Clippy, complete locked workspace tests/doctests,
and diff check pass.

## 2026-07-17 — P3-A production desktop shell

Approved and implemented the first complete vertical UI contour under
`docs/superpowers/plans/2026-07-17-tokenmaster-p3a-desktop-shell.md`. The production
frontend is a new `tokenmaster-desktop` package; `tokenmaster-m0` remains a separate
evidence artifact. Workspace Slint defaults now enable only the software renderer,
while the M0 package opts into FemtoVG explicitly for its diagnostic fallback.

The desktop projection maps `ProductRoute::ALL` into exactly 11 owned fixed rows with
stable route/label/state/reason codes, retains one generation and selection, rejects
equal or older snapshots, and has no history. The original compiled Slint shell uses
one window, header, route navigation, and route-state panel. Startup consumes the real
initial `ProductReducer` snapshot; no mock quota/session/chart data or probe module is
present.

Focused projection and compiled-UI contracts pass. Six adversarial Pester tests prove
the audit rejects probe dependencies, seeded data, FemtoVG, route drift, direct
store/runtime authority, and filesystem/network/process/SQL/browser/credential
surfaces. The production release audit builds `TokenMaster.exe` and reports five Rust
files, five Slint files, one retained route model, 11 routes/reasons maximum, and zero
forbidden dependency/source/probe/renderer/private-canary matches. A broad `PRIVATE_`
binary check was deliberately replaced by exact project canaries after evidence showed
it matched SQLite's legitimate `SQLITE_OPEN_PRIVATE_CACHE` constant.

This milestone does not claim live product-controller publication, dashboard payloads,
visible reminder acknowledgement, compact widget lifecycle, P4 presentation gates,
M0 acceptance, packaging, signing, or release. P3-B bounded controller publication is
next.

## 2026-07-17 — P3-B.1 bounded desktop controller

Approved the controller design and executable TDD plan under
`docs/superpowers/specs/2026-07-17-tokenmaster-p3b-controller-design.md` and
`docs/superpowers/plans/2026-07-17-tokenmaster-p3b-controller.md`. The review exposed
two contracts that must not be guessed: identity-free product status cannot supply the
exact benefit scope required by the query facade, and the repository has no approved
installed/portable production data-root policy. The work was therefore split into
controller core, Slint delivery, and application composition.

P3-B.1 adds one typed desktop query plan/source, one reused engine `RefreshWorker`, one
worker-confined `ProductReducer`, and one replaceable latest immutable product
snapshot. Status is reduced before sibling sections; a section query error remains
local while healthy reads continue. Cancellation or the fixed monotonic attempt
deadline discards partial visible publication. Started attempt IDs are distinct from
coalesced intent receipts because the real follow-up attempt is allocated only after
the active attempt finishes. Shutdown joins the worker and rejects later admission
with a stable path-free code.

Focused tests prove one attempt generation across sections, exact status-first order,
1,000 hints to one follow-up, latest-only retention, cancellation/deadline discard,
real empty schema-v13 reads, injected analytics fault isolation, path redaction, and
post-close rejection. The expanded eight-case Pester audit rejects a second controller
worker and UI-query calls in addition to the P3-A authority gates. The release audit
reports six Rust files, five Slint files, five approved production dependencies, one
worker, one retained snapshot slot, and zero forbidden source, renderer, probe,
private-canary, or direct store/provider/runtime/network/shell/SQL matches.

This milestone does not wire queries to the visible window or select an archive root.
P3-B.2 capacity-one Slint event-loop delivery and P3-B.3 approved data-root/live-
runtime composition are next. Dashboard payloads, benefit scope discovery, remaining
P3 routes, P4 presentation, P5 automation, M0 acceptance, packaging, signing, and
release remain unclaimed.

After project-truth synchronization, the clean-root audit, formatting check,
warnings-as-errors locked workspace Clippy, and complete locked workspace tests/
doctests pass. Process inspection found no task-owned Cargo, compiler, test, GUI, or
temporary server process.

## 2026-07-17 — P3-B.2 capacity-one Slint event bridge

Approved and implemented the event-delivery design and TDD plan under
`docs/superpowers/specs/2026-07-17-tokenmaster-p3b2-event-bridge-design.md` and
`docs/superpowers/plans/2026-07-17-tokenmaster-p3b2-event-bridge.md`. The P3-B.1
mailbox remains the sole retained snapshot slot. One notifier attaches only while the
controller is running and idle, wakes an already populated mailbox, and holds only a
weak bridge reference.

One atomic scheduled flag coalesces publications into at most one
`invoke_from_event_loop` closure. The closure takes the newest snapshot, upgrades a
weak Slint window, applies only a newer generation, clears scheduling state, and
rechecks once for a racing publication. The bridge owns no timer, polling thread,
second result queue, query/store/runtime authority, or strong window cycle; fixed
saturating counters and stable codes report delivery health.

Six deterministic bridge tests cover 10,000-to-one coalescing, newest-only
delivery, the drain race, schedule retry, window/drop lifecycle, and `Send + Sync`
handles. Eight controller contracts include idle-post-publication wakeup. A real
headless Slint event loop applies the controller snapshot to the generated window.
The 12-case adversarial audit rejects a second slot/event site, bridge polling, and
strong window retention in addition to all earlier desktop authority drift.

This milestone does not select a production archive root or compose the live runtime.
Those remain P3-B.3, followed by visible route payloads and P4 presentation/resource
acceptance. M0 acceptance, automation, packaging, signing, and release remain
unclaimed.

## 2026-07-17 — P3-B.3 deterministic data root and live application composition

Approved and implemented the composition design and TDD plan under
`docs/superpowers/specs/2026-07-17-tokenmaster-p3b3-application-composition-design.md`
and `docs/superpowers/plans/2026-07-17-tokenmaster-p3b3-application-composition.md`.
The new `tokenmaster-app` package owns the sole production `TokenMaster.exe` while
`tokenmaster-desktop` is library-only and retains its no-runtime/no-filesystem
authority boundary.

An exact zero-byte `tokenmaster.portable` marker selects the validated adjacent
`data` child; absence selects validated `%LOCALAPPDATA%\TokenMaster`. Invalid marker
or location fails closed without fallback, CWD, or path-bearing errors. The app
composes mandatory usage/nested-Git plus independently degradable quota/reminder
runtimes, one query controller, one bridge, and ordered no-lock-across-join shutdown.

Engine workers now support an optional lossy completion notifier after receipt
publication. The same weak notifier observes all four runtime workers, copies fixed
product health under a checked generation, and replaces one desktop observation slot.
Existing controller/event coalescing handles bursts; no timer, polling thread,
dispatcher, queue, duplicate ingestion, or strong ownership cycle was added.

Focused notifier, runtime, product, desktop, data-root, real-bundle, and shutdown
contracts pass. Twenty-one adversarial Pester cases and both release audits prove one
binary/runtime/controller/bridge composition, exact dependencies, software rendering,
zero arbitrary-root/polling/old-project/private-string drift, and a successful release
build. Visible P3-C routes, safe benefit-scope discovery, P4-P6, activation, M0
acceptance, packaging, signing, and release remain unclaimed.

The complete post-milestone workspace gate exposed a Windows scheduling race in the
Git process-test oracle, not in the reaping implementation: a 100 ms deadline could
reap the fixture before its first receipt write. A delayed-start regression now
reproduces that state deterministically and verifies no process remains by exact
executable path; receipt PIDs remain an additional check when available. The renewed
clean-root, release audits, 21-case Pester suite, format, strict Clippy, workspace
tests, and doctests all pass.

## 2026-07-17 — P3-C quota-first Dashboard

Implemented the approved P3-C design and executable plan under
`docs/superpowers/specs/2026-07-17-tokenmaster-p3c-dashboard-design.md` and
`docs/superpowers/plans/2026-07-17-tokenmaster-p3c-dashboard.md`. Separate store/query
overview APIs now discover all current quota windows and benefit scopes without
changing exact-empty filter semantics. One transaction binds each overview to its
revision, with 32-window, 32-scope, and 256-lot plus-one rejection and identity-free
public mapping.

The controller publishes quota and benefit overview envelopes through the existing
single worker/reducer/snapshot path. A pure `DesktopDashboardProjection` maps one
immutable product snapshot into Plan Usage, Code Output, Usage and Cost Trend,
Sessions, Activity, and Model Usage. It retains at most 32 quota rows, 32 benefit
summaries, 240 trend points, 12 sessions, eight fixed activity categories, 12 model
rows, and checked aggregate Git facts from at most 32 repositories. Compatible sibling
failure remains local and visibly degraded; missing values are never fabricated zero.

The production Slint shell now renders the responsive six-section Dashboard from real
models. Dynamic quota ratios/units/reset times, distinct banked resets and credit
kinds, today metrics, Git efficiency, trend, recent sessions, activity, and model
usage are visible. Semantic components/tokens and stable label keys preserve the P4
skin/locale boundary. Narrow/wide switching and route navigation reuse `MainWindow`;
route-only selection no longer rebuilds Dashboard models. There is no UI query, SQL,
runtime, timer, animation, polling thread, or private opaque ID.

Focused projection/UI/event-loop tests prove real fixture values, unknown truth,
32 dynamic quota rows, reset separation, section-local bounds, checked multi-project
Git sums, and 10,000 old-model releases. The desktop adversarial suite passes 20 cases;
its source receipt reports seven Rust files, nine Slint files, six Dashboard sections,
seven bounded list replacements, one Dashboard application path, one worker, one
snapshot slot, one event-loop site, and zero polling/private-ID surfaces.

P3-D.0 Reliable State, the remaining P3-D supporting routes, P3-E desktop integration,
P4 skins/locales/accessibility/paint/resource evidence, P5 automation, activation, M0
acceptance, packaging, signing, and release remain unclaimed.

## 2026-07-17 — P3-D.0 reliable-state architecture approved

Re-audited whole-file/configuration failure handling against the implemented WAL,
schema migration, fixed archive identity, process writer lease, live application
lifecycle, privacy rules, Windows replacement semantics, and long-run resource goals.
The approved design and 18-task executable rail are recorded in
`docs/superpowers/specs/2026-07-17-tokenmaster-reliable-state-design.md` and
`docs/superpowers/plans/2026-07-17-tokenmaster-reliable-state.md`.

The review rejected main-only ZIP copies, a continuously mirrored database, and live
database generation paths. The selected contour keeps `tokenmaster.sqlite3` and its
existing writer sidecar fixed, creates verified Online Backup snapshots, uses strict
streaming `.tmconfig`/`.tmbackup` packages, bounds automatic retention, and restores
through redundant records, complete main/WAL/SHM quarantine, Windows atomic
replacement, revalidation, and an idempotent crash-resumable journal. Automatic
replacement is limited to definitive corruption; busy, access, disk, transient I/O,
and newer-schema failures preserve current truth.

The closure review expanded the journal to six exact states so full restore can commit
the chosen data-only or data-plus-portable-settings result without partial state.
Automatic recovery always preserves current settings; device-local settings are never
restored. It also distinguishes existing-main atomic replacement, missing-damaged-main
same-volume promotion, and brand-new schema creation; binds every package byte with a
footer digest; freezes the 256 MiB-through-64 GiB retention range; and keeps mandatory
safety points active when ordinary periodic backup is disabled.

The design also freezes safe defaults/fallback settings, optional bounded standard
age protection for manual exports, no-secret automatic recovery, no automatic SQLite
salvage, three-set quarantine stop, safe mode, explicit no-backup rebuild/data-loss
truth, one capacity-one maintenance worker, and memory/latency/fault-injection gates.
Traceability remains `planned`: no reliable-state source, backup, restore, settings,
safe mode, encryption, UI, M0 acceptance, package, signature, or release was produced
by this planning milestone.

## 2026-07-17 — P3-D.0 Task 1 reliable-state boundary

Added `tokenmaster-state` as the single library-only reliable-state workspace package.
It currently contains nine stable serialized/path-private error categories and one
bounded byte/item limit value whose private wrappers reject limit excess and arithmetic
overflow. It stores no source error text and exposes no filesystem-path constructor.

Added a deterministic Pester/workspace authority audit plus 29 mutation cases. The
receipt records exactly five direct production dependencies (`serde`, `serde_json`,
`sha2`, `thiserror`, and `tokenmaster-platform`), one exact workspace member, two Rust
source files, no binary or build target, no filesystem/process/network/shell/SQL/Slint/
archive/external-source authority, no public arbitrary-path constructor, and no
forbidden transitive dependency. Rust contracts cover all
nine stable codes, serialization, redacted diagnostics, inclusive limits, excess, and
integer overflow.

Focused evidence passes 2/2 Rust contracts, 29/29 Pester mutation cases, and the full
workspace authority receipt. Clean-root, formatting, strict warnings-as-errors
workspace Clippy, and the complete locked workspace test/doctest suite also pass.
Independent review added red/green regressions for direct and grouped filesystem/path
authority, standard-library aliases, platform re-exports, public aliases/traits,
declarative macros, external source inclusion, commented workspace entries, excluded
path dependencies, and forbidden transitive authority.

The plan review also removed a future `src/bin` recovery fixture that contradicted the
library-only invariant; Task 10 will reinvoke its integration-test executable through a
test-support module instead. Persistent records, settings, durable file operations,
packages, backup, recovery, runtime, and UI remain unimplemented. Task 2 controlled
durable file primitives are next.

## 2026-07-17 — P3-D.0 Task 2 controlled durable files

Added a sealed platform publication boundary without exposing arbitrary filesystem
paths. `DurableFileTarget` accepts only one validated local directory and restricted
exact child; `DurableStagedFile` provides 32 create-new candidates, a 64 GiB plus 2 MiB
ceiling, 256 KiB call chunks, partial-write accounting, poisoned I/O failure state,
flush/close/reopen, and bounded exact length/SHA-256 receipts. Paths, handles, OS
messages, and digests remain absent from public diagnostics.

Windows new-target publication uses `MoveFileExW(MOVEFILE_WRITE_THROUGH)` without
`MOVEFILE_COPY_ALLOWED`; existing-target publication uses `ReplaceFileW` with zero
unsupported flags and an independently captured/reverified exact old-target backup.
The documented displaced-target error is rolled back write-through when possible.
Every ambiguous rollback and every hook/sync/verification failure after successful OS
publication is `RecoveryRequired`, preserving discoverable recovery artifacts instead
of authorizing a blind retry. Unix uses no-overwrite hard links, atomic rename, and
file/parent synchronization while explicitly not claiming the Windows guarantee.

Focused evidence passes strict all-target platform Clippy, 9 library tests, and 11
durable integration tests. The process fixture performs 20 deterministic kills before
the replace call, 20 after verified publication, and 20 immediate replacement-entry
race kills; every round retains a complete old or new target and validates any backup
as the exact old bytes. Independent Sol High review closed partial-write, artifact-
preservation, Unix race/rollback, backup-proof, crash-evidence, and post-publication
error-contract findings and reports no remaining Critical or Important issue. This
does not implement records, settings, backup packages, restore, safe mode, UI, M0
acceptance, packaging, signing, or release. Task 3 redundant bounded records is next.

## 2026-07-17 — P3-D.0 Task 3 redundant bounded records

Added the crate-private record core used by future typed settings, run-state, and
recovery-journal stores. It constructs only `settings-{a,b}.tms`, `run-{a,b}.tms`, and
`recovery-{a,b}.tms`. The version-1 envelope uses an exact 64-byte `TMREC001` header,
strict JSON payload capped at 1 MiB, and 40-byte `TMEND001` footer. Checked nonzero
generation, exact actual/declared length, zero flags, payload SHA-256, whole-record
SHA-256, UTF-8/typed JSON, and absence of trailing bytes are all required before a
slot is valid. Highest generation wins; one corrupt slot is a typed fallback; equal
generations require an equal payload digest; conflicting or two-invalid slots write
nothing.

Save now performs a no-buffer measurement/hash pass and streams the second
serialization directly into sealed platform staging in at most 256 KiB calls. A
length or digest change between passes is integrity failure before publication. The
inactive slot is replaced without a third backup, both slots are reread, and every
failure or ambiguity after publication is `RecoveryRequired`. Pre-save decoded values
are dropped before serialization so post-publication validation does not retain extra
typed copies.

The platform boundary gained caller-bounded exact-child reads plus Windows
`MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`/Unix atomic rename
for an inactive redundant slot. It deliberately creates no third backup because the
other independently valid slot is the fallback. Deterministic hooks cover both sides
of the OS replacement boundary; a dedicated Windows fixture passes 40 pre/post
process kills and 20 replacement-entry race kills. State process tests seed
generations 1 and 2, then kill generation 3 during a partial JSON write, after seal
before publish, and after publish before state reread. The observed state is always
the complete prior or new generation.

Independent high-risk review initially found four Important issues: weak post-publish
error mapping, equal-generation ambiguity, public generic authority, and approved-
alias audit bypasses. All were corrected with red/green regressions. The final review
reports no Critical or Important finding. The authority suite now passes 33 mutation
cases and fixes the production surface to one bounded writer import, three permitted
`io` members, one exact platform import, and six literal child constructors. A future
defensive no-follow/open-handle identity check could narrow a same-user path-replacement
TOCTOU; this is outside the documented threat boundary and is not a Task 3 blocker.

Focused formatting, strict state/platform all-target Clippy, 10 platform unit tests,
14 durable-file integration tests plus the remaining platform suites, 13 record unit
tests, two public state authority contracts, the workspace authority receipt, and
33/33 Pester mutations pass. Typed settings, snapshot/package generation, retention,
maintenance, recovery, app safe mode, UI, M0 acceptance, packaging, signing, and
release remain unimplemented. Task 4 typed settings/schema/import preview is next.

## 2026-07-17 — P3-D.0 Task 4 typed settings and portable preview

Added the fixed-purpose public settings API over the private A/B record core. Schema
version 1 stores only current product-owned values: one canonical in-app reminder
default capped at eight unique validated leads, automatic-backup periodic/quiet/
interval/retention policy, and the device-local last route. The default schedule has
a five-minute quiet window and no more than one ordinary periodic point per six hours
under a 2 GiB budget; configurable bounds are 300..3,600 seconds quiet,
21,600..604,800 seconds interval with quiet below interval, and 256 MiB..64 GiB.
Presentation skins/locales, OS delivery, pricing/provider values, source paths,
credentials, prompts, responses, commands, and source content are deliberately absent
until their owners exist or forever forbidden.

Load distinguishes healthy current state (including one intentional first slot),
corrupt-peer fallback, and two-invalid safe defaults. Defaults do not rewrite evidence;
an explicit validated save may replace only one invalid slot and preserves the peer.
Record payload decoding now retains a valid-envelope unsupported settings version as
`UnsupportedVersion`, so an older binary cannot downgrade newer state to generic
corruption or overwrite it. Portable candidate decode is capped at 1 MiB, uses an
eight-element sequence visitor, probes version before strict full decode, rejects
unsupported/unknown/duplicate/malformed/range/relationship input, and never accepts
device state.

Preview exposes only ordered change categories/counts. Commit binds confirmation to
the base generation and record digest, preserves the current device route, is a no-
write success when already current, rereads publication, and returns a reconstructible
nonzero generation plus portable SHA-256 target for later journal verification.
Focused evidence passes ten settings integration contracts, 13 record unit contracts,
two public authority contracts, strict locked all-target state Clippy, formatting, the
workspace state-authority receipt, and 34/34 Pester mutations. The audit permits one
exact bounded record/platform import plus one exact typed-store directory-capability
import, allows exactly four capability type uses and the exact constructor signature,
and forbids `.as_path`; generic records, caller-selected children, arbitrary paths,
and forbidden transitive authority remain rejected.

Independent high-risk review first found newer-schema downgrade/overwrite, schedule
floors below the approved operating gates, an approved-directory path leak, and
unbounded rejected-list allocation. RED/GREEN fixes introduced typed payload decode,
valid-envelope newer-version write protection, exact policy floors, a bounded visitor,
and the exact constructor/capability audit. A second pass found the whitespace form
`directory . as_path()`; the regex and mutation were hardened. The final review
reports Critical 0, Important 0, Minor 0, and `Ready: Yes`.

The final repository baseline passes `TM-CLEAN-PASS`, formatting, strict locked
warnings-as-errors workspace Clippy, and every locked workspace test/doctest in 427
seconds. This is development evidence only; it does not claim P3-D.0 acceptance,
interactive acceptance, packaging, signing, or release.

SQLite snapshots, fixed `.tmconfig`/`.tmbackup` packages, encryption, catalog/
retention, recovery journal, safe mode, UI integration, M0 acceptance, packaging,
signing, and release remain unimplemented. Task 5 verified SQLite snapshots and
candidates is next.

## 2026-07-18 — P3-D.0 Task 5 verified SQLite snapshots

Added the store-owned fixed-capability snapshot boundary. `BackupSource` resolves only
the implemented `tokenmaster.sqlite3`; `BackupStaging` allocates only 32 exact
create-new snapshot or compact names. SQLite Online Backup copies 64 pages per step,
includes committed WAL truth, bounds busy/locked retry, and honors cooperative
cancellation and deadlines. Invalid source headers and non-transient failures stop
without publishing a candidate. A deterministic barrier test pauses after an actual
`StepResult::More`, commits a writer transaction, and releases the next backup step.

Candidate verification is standalone and non-mutating. It applies defensive and
query-only policy, trusted schema and both DQS modes off, `cell_size_check` on, zero
mmap, fixed cache/busy policy, exact bundled SQLite 3.53.2 identity, and explicit
16 MiB value, 256 KiB SQL, and 256-column limits. Progress handling bounds integrity,
foreign-key, exact schema/index, stored count/generation, and semantic validation.
Schema enumeration retains at most the expected table count and compares borrowed,
bounded names/SQL. Supported old schemas are inspected without migration and newer
schemas remain typed and untouched. `VACUUM INTO` accepts only an isolated verified
snapshot, clears its progress handler, and re-verifies a smaller/equal result.

Verification now binds physical file identity, length, and streaming SHA-256 before
and after proof and every compaction consumer, so path replacement fails as
`StaleBackupCandidate`. Busy, I/O, cancellation, deadline, corruption, and semantic
categories are not collapsed. Candidate `discard` reports cleanup failure; Drop
records a bounded health counter; an explicit recovery pass scans only 64 fixed names
and resets health only after complete success. Store/query compatibility, five backup
contracts, ten adversarial contracts, the page-step barrier, strict store Clippy, and
the complete store suite pass. Independent high-risk review iterated to `Ready: Yes`
with Critical 0, Important 0, Minor 0.

The repository baseline then passed `TM-CLEAN-PASS`, formatting, strict locked
warnings-as-errors workspace Clippy, and the complete locked workspace test/doctest
suite in 470.7 seconds.

This closes Task 5 only. Fixed `.tmconfig`/`.tmbackup` containers, encryption,
catalog/retention, maintenance runtime, recovery journal, safe mode, UI integration,
M0 acceptance, packaging, signing, and release remain unimplemented. Task 6 fixed
bounded package containers is next.

## 2026-07-18 — P3-D.0 Task 6 bounded typed backup packages

Added the fixed deterministic v1 `.tmconfig`/`.tmbackup` format in
`tokenmaster-state`. The wire order is a 32-byte `TMPKG001` header, one 40-byte
`TMMNF001` self-describing manifest, ordered `TMENTR01` settings/database entries
with 64-byte descriptors and 24-byte `TMENEND1` suffixes, then descriptor binding,
`TMEND001`, and SHA-256 of every preceding package byte. The controlled complete file
is independently SHA-256 sealed. Config carries portable settings only. Backup also
carries database schema, creation UTC milliseconds, compression profile, and one of
periodic/manual/pre-migration/post-migration/pre-restore.

Pinned `zstd` 0.13.3 with `default-features = false`; the resolved feature tree adds
only `zstd-safe/std`. Each entry is exactly one checksummed, content-sized frame at
level 6, 12, or 19, without multithreading, training, legacy, or experimental modes.
The codec fixes window log 23, 64 KiB buffers, exact expanded counts/hashes, 1 MiB
settings, 64 GiB database, and checked 64 GiB-plus-2-MiB ceilings. Reader validation
rejects unknown/reserved fields, duplicate/wrong order, overflow, concatenation,
trailing data, missing end, checksum/digest/length/content-size mismatch, oversized
windows, and expanded output beyond its independent counter.

Independent review found that merely leaving a failed borrowed stage unsealed was
insufficient because the platform seal API remained public. The final design adds
irreversible `DurableStagedFile::discard`: it clears receipt, closes/removes the
unpublished file, and poisons the handle before any codec error returns. Cleanup
uncertainty is `RecoveryRequired`. Late-footer corruption after full database
extraction, partial writer output, a crafted 300-to-256 content-size bomb, and the
platform contract now prove subsequent write/seal/publish all fail `InvalidState`.
Public codec methods accept only `DurableFileReader`/`DurableStagedFile`; raw generic
stream helpers remain private, and a new authority mutation rejects future public
`Read`/`Write` methods.

Focused evidence passes a frozen 405-byte config golden and complete-file SHA-256,
all three compression profiles times the then-five purposes, a 24 MiB controlled streaming
round trip, 5 package contracts, 10 adversarial package contracts, and 17 durable-file
contracts. Strict platform/state warnings-as-errors Clippy, the reliable-state
workspace audit, exact Zstd feature-tree inspection, 36/36 Pester mutations, and
`git diff --check` pass. Final independent review reports Critical 0, Important 0,
Minor 0 and `Ready: Yes`.

The final repository baseline passes `TM-CLEAN-PASS`, formatting, strict locked
warnings-as-errors workspace Clippy, and the complete locked workspace test/doctest
suite in 548.2 seconds total; Clippy takes 22.5 seconds. The authenticated live Codex
transport contract remains the one intentional opt-in ignore.

The implementation plan was hardened before continuing: Task 8 now requires a sealed
platform backup-directory capability for bounded enumeration/publication/deletion,
and Task 9 requires a store-owned identity-bound verified-candidate reader plus exact
state interop. Neither may add paths or public generic streams. Task 7 optional manual
age protection is next. Retention, maintenance, recovery, safe mode, UI integration,
M0 acceptance, packaging, signing, and release remain unimplemented.

## 2026-07-18 — P3-D.0 Task 7 bounded manual age protection

Pinned `age = 0.12.1` with default features disabled and added only the standard
binary age v1 scrypt envelope for manual backup export/import. No CLI, plugin, SSH,
armor, async, unstable, or web age feature is enabled. Export fixes
`log_n = 16`; import constructs the identity with maximum 16 before stanza unwrap, so
an attacker-selected higher factor is rejected before derivation. Automatic encryption
is explicitly rejected and automatic recovery keeps no secret.

Encryption requires an opaque `VerifiedBackupPackage` and rechecks its exact source
length plus complete-file SHA-256 during the same streaming pass. A changed file,
appended byte, or same-length substitution poisons ciphertext output. New passphrases
require exact 12-through-128 Unicode-scalar confirmation without trim or normalization.
New and existing constructors immediately take caller-owned buffers into a
non-cloneable redacted zeroizing `SecretString` and clear every supplied field on all
outcomes.

Independent review found that the first decrypt API authenticated age but could seal
arbitrary plaintext before verifying the inner TokenMaster package. The final design
removes that generic extraction surface: the authenticated age reader feeds the
existing private typed `BackupPackage` parser directly and only its verified database
stage can be sealed. The same correction preserves `InvalidData`/`UnexpectedEof` as
integrity failures without exposing source text. Authenticated non-package plaintext,
wrong password, header/MAC/body/final-tag corruption, truncation, trailing data,
ciphertext/database capacity, failed cleanup, and inner package failure all poison
output; cleanup uncertainty is `RecoveryRequired`.

Seven grouped encryption contracts pass the complete matrix, including typed
round-trip, malicious work-factor early rejection, same-length substitution,
authenticated non-package plaintext, destination bounds, cleanup failure, exact
passphrase boundaries, and privacy canaries. The state authority audit now pins seven
direct dependencies and rejects age version/feature drift; 37/37 Pester mutations
pass. The pinned age tree currently emits an upstream future-incompatibility warning
from `proc-macro-error2 2.0.1` through mandatory `i18n-embed-fl`; Rust 1.97 builds it
successfully, and TokenMaster does not carry an unaudited local cryptographic patch.
The final authority gate additionally caught that the first shared inner parser was
crate-visible as `R: Read`. The gate was preserved: the generic parser is now fully
private, while the age module alone can construct a package-private typed
authenticated-payload proof for direct inner verification. Independent security
rereview then reported Critical 0, Important 0, Minor 0 and `Ready: Yes`.

The final unchanged-source component baseline passes: clean-root 15.9 seconds,
formatting 1.4 seconds, strict locked full-workspace Clippy 35.6 seconds, and the
complete locked workspace test/doctest suite 491.2 seconds (544.1 seconds combined).
One initial full-suite attempt stopped in `quota_transport_contract` without a retained
failure body; the exact target then passed 11 consecutive runs and the complete
workspace rerun passed without an intervening source edit. This remains recorded as a
transient test event rather than hidden or claimed as a product regression.

Maintenance, recovery, safe mode, UI integration, M0 acceptance, packaging, signing,
and release remain unimplemented; Task 8 bounded catalog/retention followed.

## 2026-07-18 — P3-D.0 Task 8 sealed catalog and protected retention

Added the platform-owned canonical local `backups` directory with exactly 32 private
`point-00.tmbackup` through `point-31.tmbackup` slots. Public directory entries bind
scope, ordinal, length, and physical identity without exposing names or paths. Scans
reject unexpected names/types, symlinks, reparse points, hard links, duplicate
physical identities, over-capacity, and exact controlled stage/tombstone remnants.
Stages remain unpublished and expose only bounded write/seal/discard plus a path-free
reader after seal; only the owning directory publishes. Deletion is a write-through
rename to an exact tombstone followed by removal, so before-move interruption leaves
the point unchanged and post-move uncertainty is explicit recovery state.

Added a disposable process-local `BackupCatalog`. Rebuild streams every complete file
with one 64 KiB buffer, validates the fixed header/manifest, records the complete-file
SHA-256 privately, rejects duplicate content, and reports cold rows as `HeaderValid`
or `Corrupt`, never verified. A prior verified state carries only across unchanged
physical token, length, full SHA-256, and typed metadata; explicit current package
proof must bind the exact catalog generation/ordinal. Public points expose only UTC
time, size, purpose, schema/compression, health, and checked selection.

Closed a production-composition gap found by independent review. The final chain is
typed package write into the sealed unpublished slot, complete package parsing through
its path-free reader, pure no-delete retention admission, directory publication with
seal recheck, catalog rebuild/proof bind, and exact confirmation. This avoids both raw
stage escape and publish-before-verification. Mixed source/destination failures retain
the existing source-first precedence and discarded-stage reuse is an internal
invariant rather than transient I/O.

Retention now protects the admitted candidate, newest two verified points, and newest
pre-migration point until later verified post-migration evidence, then applies shared
four-newest, seven distinct UTC-day, and four distinct ISO-week tiers under the
15-point cap. The compressed-byte budget defaults to 2 GiB and accepts only 256 MiB
through 64 GiB. Unchecked/corrupt bytes count but are never deletion-eligible.
Admission requires a free slot and deletes nothing; confirmation requires exactly one
new verified candidate and preservation of all prior package identities.

Independent review also found that checking only the candidate and selected target
could still plan from another stale protected point. The final deletion path therefore
fully rehashes every current `Verified` fact before planning, rechecks the exact target
and directory generation, deletes at most one oldest unprotected file, and requires a
catalog rebuild/replan. Regressions prove same-length corruption of the candidate,
target, or a different protected point causes zero deletion and correct tier promotion
after rebuild.

Focused evidence passes four catalog contracts, two retention contracts, five
backup-directory contracts, the injected deletion-boundary unit, mixed-error unit,
strict source authority audit, and 42/42 mutation cases. The independent third review
closed both Important code findings and the Minor error-semantics finding; its only
later documentation wording Minor was corrected. Task 9 capacity-one maintenance is
next; recovery, safe mode, UI, acceptance, packaging, signing, and release remain
unclaimed. The final review is Critical 0, Important 0, Minor 0 and `Ready: Yes`.
The unchanged Rust source passes clean-root in 17.4 seconds, formatting in 1.3
seconds, strict locked full-workspace Clippy in 13.3 seconds, and the complete locked
workspace test/doctest gate in 566.3 seconds total.

## 2026-07-18 — P3-D.0 Task 9 capacity-one backup maintenance

Added a constant-state maintenance coordinator with checked request IDs, one active
permit, and one urgency-merged follow-up. Mandatory safety points outrank manual,
source-retry, and periodic work; a second unresolved mandatory guard is rejected busy.
A source retry gets a fresh attempt ID and lower scheduling urgency while preserving
the root request and original backup purpose. This closes a design bug found during
implementation where a failed pre-migration attempt could otherwise have become a
periodic-labeled retry. Two failures against the same opaque identity enter `Suspect`.

Added the native `BackupMaintenanceRuntime` with exactly one worker, one scheduler,
capacity-one wake channels, one shared timeout, joined pause/resume/shutdown/Drop, and
fixed latest completion/counters plus a separate mandatory-guard completion. Automatic
work seeds a new monotonic interval only from exact `Healthy` restart truth;
`HealthyUnpublished` remains closed. It requires both quiet and ordinary minimum
intervals and emits one resume/clock-rollback catch-up. Disabling periodic work removes
a pending periodic-origin follow-up without removing internal retry or mandatory
guards. Source retry remains internal urgency, never a synthetic caller purpose.
Worker panic payloads use the thread-local redaction boundary.

Linked each permit to a typed store `BackupControl` so cancellation reaches page-stepped
SQLite work without exposing a raw atomic. A compare-exchange makes final publication
non-cancellable, and impossible completion/state combinations fail closed. The store
now owns a bounded path-free reader for one verified SQLite
candidate; it rechecks physical identity, exact length, and full SHA-256 before open,
during complete consumption, and after EOF. State adds the sole typed bridge from that
reader into an unpublished sealed package stage. Replacement, truncation, append, or
source/output failure poisons and removes the stage. Backup purpose value 6 records
pre-destructive maintenance without changing prior v1 values.

Focused evidence passes 17 maintenance contracts, the strengthened Windows 12-warm-up/
24-measured resource contract, seven store backup contracts, six catalog contracts,
strict state Clippy, the workspace reliable-state authority audit, and 47 mutation
tests. The first independent review reported Critical 0, Important 4, Minor 1; all five
findings now have focused regressions and fixes. Post-fix rereview reports Critical 0,
Important 0, Minor 0 and `Ready`. The final baseline passes clean-root in 14.940
seconds, formatting in 1.256 seconds, strict locked workspace Clippy in 12.340 seconds,
and the complete locked workspace test/doctest suite in 507.6 seconds. The one
live-auth Codex transport test remains explicitly environment-gated and ignored.
Task 10 durable restore journal/quarantine is next; startup recovery, application
composition, safe mode, UI, acceptance, packaging, signing, and release remain
unclaimed.

## 2026-07-18 — P3-D.0 Task 10 durable restore and quarantine

Implemented restore as an acyclic sealed platform/store/state composition rather than
a state-owned filesystem shortcut. `ArchiveRecoveryScope` is bound to the exact
writer-lease archive and owns only fixed main/WAL/SHM, staging, and quarantine names.
It generates opaque operation IDs, rejects wrong leases, links/reparse points,
multiple links and unexpected entries, retains at most three complete quarantine
sets, and uses `ReplaceFileW` for an existing main or write-through same-volume move
for a missing damaged main. Rollback preserves the failed main and restores exact old
main/WAL/SHM facts. Unjournaled staging is globally capped at three exact artifacts,
preflights actual free space for `max(2B, B+A) + 8 MiB`, and
is discarded only after an absent or completed journal; unknown evidence is retained
and blocks.

Package expansion now ends in a platform-owned sealed recovery candidate. Store copies
its bounded path-free reader into the fixed recovery candidate namespace and applies
the complete defensive SQLite, integrity, foreign-key, exact-schema/index, retained-
count/generation, and semantic verifier. Promotion rechecks length/SHA-256 against the
same sealed stage, and the active main is reopened through the same store verifier.
No path, SQL connection, generic stream, or caller-selected name crosses state.

State persists exactly `prepared`, `sidecars_quarantined`, `main_replaced`,
`reopened_verified`, `settings_published`, and `complete` in the existing recovery A/B
slots. The journal records opaque package/candidate/operation identities, fixed prior
main/WAL/SHM facts, mode, attempt, and an optional prepared settings generation/digest.
Automatic recovery is always corruption-only data-only with two attempts; manual
restore is data-only or data-plus-portable-settings. Device-local settings never move.
Settings publication is idempotent and a restart after durable commit but before
journal advance verifies the exact target instead of creating another generation.

Crash fixtures reinvoke the exact integration-test executable and force process death
at all six durable phases plus the pre-journal sidecar, main-promotion, and settings-
commit boundaries. A review found and the tests closed a real gap where `ReplaceFileW`
had consumed the staged candidate while journal still said `sidecars_quarantined`;
resume now proves either the unchanged stage or the already-promoted fully verified
active main and advances idempotently. Focused recovery tests, strict Clippy, the
workspace reliable-state audit, and 52/52 authority mutations pass. Tasks 11-18,
safe-mode/application/UI integration, acceptance, packaging, signing, and release
remain unclaimed.

Independent review then found nine boundary defects/evidence gaps. The corrected
implementation cleans store-owned verifier remnants before platform staging, allows a
completed journal to start the next operation generation, persists the fixed physical
backup slot, rereads settings after publication ambiguity, treats a verified backup as
prior-install evidence for a missing main, and binds recovery authority to the physical
locked sidecar. A caller can no longer assert corruption: the coordinator itself runs
the complete store verifier and rejects busy/I/O/schema-newer as corruption authority.
Create-new reservation markers close the operation-directory race; native replacement
errors restore sidecars only when exact facts prove replacement never began, and an
actual-free-space preflight replaces the former large theoretical staging allowance.
New regressions cover repeated restore generations, catalog rebuild/resume, settings
post-publication errors and kills, store-verifier kills before journal, first journal-
slot death, missing-main recovery, healthy-main rejection, lock-file substitution,
reservation collision, rollback continuation, and ambiguous replacement facts.

A second independent review found two remaining pre-journal containment defects. A
wrong archive guard could reach store-owned cleanup before platform rejected it, and
the first verified candidate could remain live while corruption verification created
a fourth staging artifact. Recovery now authorizes the physical guard before either
cleanup path, drops the first verifier proof before corruption verification, and
enforces the same three-entry cap inside the store allocator. Disk admission observes
the active main and requires `max(2B, B+A) + 8 MiB`; an identity change after that
observation fails before corruption authority or journal publication. Focused tests
prove wrong-guard evidence preservation, exact three-artifact peaks at both verifier
boundaries, cap rejection, capacity arithmetic/overflow, and the full 18-case restore
contract.
Final independent rereview reports Critical 0, Important 0, Minor 0 and `Ready`.
The final Task 10 baseline passes clean-root in 14.899 seconds, formatting in 1.396
seconds, strict locked workspace Clippy in 9.169 seconds, and the complete locked
workspace test/doctest suite in 545.3 seconds. The reliable-state audit, 52/52
authority mutations, and the changed platform MSVC target check also pass. This
accepts the library milestone only; startup/application/UI recovery and release gates
remain later tasks.

## 2026-07-18 — P3-D.0 Task 11A pre-open bootstrap and guard handoff

Added strict typed run-state A/B records and `StateBootstrap`. Startup validates that
all data/reliable-state capabilities share one root, observes prior owned evidence,
then durably publishes and rereads `unclean` before catalog, package, or SQLite access.
A prior clean run uses bounded normal read-only inspection; unclean, missing, or invalid
truth adds `quick_check(100)`. The store inspector never creates or migrates an archive:
legacy returns migration-required, newer returns upgrade-required, and non-corruption
failures preserve the active set.

Bootstrap resumes any pending recovery journal before ordinary inspection. Definitive
corruption or missing-main damage with prior backup evidence selects candidates newest
first, fully reverifies every package, skips corrupt newer points, and automatically
restores data only. No usable point returns recovery-required with the corrupt set
preserved. A recovered candidate is tied to its operation generation and exact identity;
two unclean launches are allowed and the third enters safe mode. Later clean acceptance
prevents a completed historical journal from creating a false retry loop or blocking a
new independent recovery generation.

The runtime now consumes an already-held platform guard through archive open and startup
recovery, then releases that startup guard; later mutations reacquire the same fixed
lease per operation. Legacy constructors retain their behavior. An integration contract
separately proves guarded first-install startup, joined shutdown, and only then clean
publication through the retained generation/digest-bound `RunSession`. Root
capabilities are reauthorized before every recovery/bootstrap operation, unknown
pre-journal staging is preserved, and exact zero-length WAL/SHM facts are supported
without accepting an empty main.

Focused platform archive-recovery 13/13, writer-lease 9/9, store startup 5/5, state
bootstrap 12/12, automatic recovery 7/7, restore 20/20, and the complete runtime suite
pass. Independent rereview found one remaining strict-Clippy blocker: redundant
`must_use` attributes on `Result` constructors. They were removed, the exact locked
workspace Clippy gate, reliable-state audit, and 55/55 authority mutations pass, and a
direct persisted-facts regression now proves empty WAL/SHM acceptance versus empty-main
rejection. The complete locked workspace test/doctest suite passes in 571.4 seconds.
The changed platform capability also passes an explicit `x86_64-pc-windows-msvc`
warnings-as-errors target check. Task 12 owns migration safety points,
authoritative no-backup reconstruction, all-owner restart/safe mode, and final clean
publication; those claims remain open.

## 2026-07-18 — P3-D.0 Task 12A application backup and migration lifecycle

Added the application-owned reliable-state boundary. `ApplicationStateOwner` prepares
startup before live/query/controller construction, and failure leaves the existing
Slint shell in safe mode without an archive, runtime, query, or maintenance owner. A
healthy bundle owns exactly one capacity-one maintenance runtime and one concrete
backup operation: SQLite Online Backup, full candidate verification, typed package
staging and re-verification, sealed publication, exact verified-package catalog
binding, and bounded one-at-a-time retention. Terminal mandatory receipts use a single
deadline-bounded condition variable, not polling or another worker. Submission and wait
atomically reserve the exact root, so a newer completion cannot overwrite its receipt.
Cold catalog verification runs on the worker, unchanged package proofs survive every
rebuild, and the operation retains the final projection for the next backup cycle.

Supported legacy startup now requires one completely verified pre-migration package
before any writable old-schema open. Redundant run-state schema v2, with strict legacy-
v1 acceptance, then records the pending source/target pair. One verified post-migration
package clears it before the bundle becomes live. Disabling periodic backups suppresses
neither safety point. Failure before writable open preserves the old archive; failure
after migration commit preserves the migrated archive and pending obligation in safe
mode. Restart completes the post point before live, and clean rejects a pending pair.
Healthy shutdown pauses and joins
maintenance, then controller/quota/reminder/live owners, and only then marks the exact
run generation clean.

A real first-install backup exposed a SQLite WAL detail: before the first checkpoint,
the main header may report schema-format byte zero while the schema is committed in the
WAL. The live-source precheck now accepts that form only with a regular WAL present;
every snapshot candidate and package still requires strict format four and the complete
database verifier. Task 12B command/restart and no-backup reconstruction, Tasks 13-18,
product acceptance, packaging, signing, and release remain unclaimed.

Focused store backup 8/8 and adversarial 10/10, catalog 6/6, maintenance 19/19, state
bootstrap 13/13, app 6 unit plus 7 integration, application authority 17/17, and
reliable-state authority 55/55 contracts pass. The app lifecycle test executes 19 real
sequential backups and keeps the retained catalog within the default 15-point bound.
The mandatory clean-root, formatting, strict locked workspace
Clippy, and complete locked workspace test/doctest gate exits zero in 617.2 seconds;
the one live-auth Codex transport test remains intentionally environment-gated. A
release application composition audit also proves one production binary/artifact, one
owner for every declared state/runtime component, both migration gates, and zero
polling, arbitrary-root, renderer-fallback, probe, or forbidden-binary-string surface.

## 2026-07-18 — P3-D.0 Task 12B.1 bounded commands and generation-safe restart

Added one application-owned, path-free command admission core for config export/import,
backup, verification, data-only or portable-settings restore, and rebuild intents. It
keeps one active request plus one distinct follow-up, coalesces 10,000 repeated hints,
rejects a third command busy, supports exact active and queued cancellation, preserves
one retryable failure, and rejects cancellation after an explicit irreversible
boundary. Closing or pausing admission retains the active receipt and removes only the
queued request; restart may resume admission while final shutdown remains closed.

Refactored live-bundle creation into one guarded start/finalization path and added a
controlled current-bundle restart. It keeps the compiled Slint window, joins the old
maintenance/controller/reminder/quota/live owners, acquires a fresh fixed archive
lease, and installs one higher bundle generation. Runtime completion notifiers now
carry that generation and compare it while holding the same bundle mutex; an obsolete
notifier returns without allocating a product observation or touching the new bundle.
The real lifecycle regression exercises this across the existing 19-backup retention,
safe-mode, migration, and crash-window scenario. The Task 12B.2 operation worker,
actual command bindings, selected restore, no-backup reconstruction, and UI remain
unclaimed.

Focused verification passes 7/7 command contracts, 14 application unit plus 7
integration contracts, strict app Clippy, and 23/23 application authority contracts,
including a clean-composition control and grouped process-import rejection.
The required clean-root, formatting, and warnings-as-errors locked workspace Clippy
gates pass. Two earlier full validation attempts ended at unrelated, non-repeating
environmental points: one MinGW link returned exit 1 without diagnostics and one
parallel runtime-library invocation returned exit 101; each exact target immediately
passed focused reruns, including 15/15 Codex enumeration and serial plus parallel 22/22
runtime tests. The subsequent complete locked workspace test/doctest gate passes in
481.6 seconds. The release application composition audit passes with one production
binary/artifact and exact owner counts, the reliable-state workspace audit and 55/55
authority mutations pass, and independent follow-up review reports
Critical/Important/Minor 0/0/0.

## 2026-07-18 — P3-D.0 Task 12B.2a identity-pinned selected restore

Added the internal selected-restore lifecycle without exposing filesystem authority.
A generation/ordinal choice is checked against the current complete backup directory,
sealed into one opaque identity, and held by an RAII process-local pin. Every retention
deletion shares the same gate and consults a late pin even when its cycle crossed
publication before the restore command. This closes the independent-review race where
old maintenance could otherwise delete the actively selected point while shutdown
joined. The pin survives old-owner join and the protected mandatory `PreRestore`
publication, then clears before journaled archive replacement.

The old bundle is fully joined before one fresh fixed guard enters recovery. A complete
recovery receipt is immediately attached to the existing run session. Current archives
start through the existing guarded bundle path; restored supported legacy archives
publish a new old-schema `PreMigration`, record the exact pending source/target pair,
migrate under the guard, publish current-schema `PostMigration`, and clear the pair
before becoming live. A later clean joined shutdown accepts the recovery generation,
so cold startup does not replay the completed manual restore as a new candidate.

The shared catalog was changed from a long-held mutable projection to immutable bounded
`Arc` snapshots. Snapshot/compression/verification/recovery work occurs outside its
mutex, while only current-projection replacement is locked. Deterministic contracts
prove directory-drift rejection, generation/ordinal reorder survival, byte-replacement
rejection, an admitted-before-pin retention cycle preserving the selected point,
current and v12 restore, stale reuse, safe shutdown, and clean recovery acceptance.
Final evidence passes catalog 7/7, retention 3/3, app 14 unit plus 7 integration,
application policy 31/31, reliable-state authority 55/55, clean-root, formatting,
warnings-as-errors locked workspace Clippy, the complete locked workspace test/doctest
gate in 507.5 seconds, and release composition audits. Independent final review reports
Critical/Important/Minor 0/0/0. The live-auth Codex executable contract remains
intentionally ignored without its explicit environment binding; no product or release
acceptance is claimed.
Task 12B.2b worker/UI/native-file bindings, config operations, verify/rebuild,
cancellation propagation, no-backup reconstruction, and release gates remain open.

## 2026-07-18 — P3-D.0 Task 12B.2b.1 joined operation worker and config core

Replaced the production application's bare command coordinator with one joined
`ApplicationOperationWorker`. The worker owns the sole coordinator, one standard-
library thread, one capacity-one wake, one active permit plus one follow-up, and one
latest-only completion. Execution is outside the mutex. Exact cancellation is
normalized under the coordinator lock immediately before completion, irreversible
state rejects late cancel, retry uses only
the last failed typed command, a caught executor panic publishes fixed internal failure
and closes admission, and explicit shutdown plus `Drop` cancel/wake/join without a
detached thread or result history.

Bound the first real production command: manual backup now executes off the Slint thread
through the existing maintenance runtime's atomic exact-root receipt wait. It crosses
irreversible state before maintenance can mutate and holds the bundle slot stable, so
restart, restore, or shutdown cannot replace owners during publication. Application
shutdown joins the operation worker before the backend bundle; clean run state requires
both joins.

Added the sealed config operation core without claiming native dialogs. `.tmconfig` has
a separate 2 MiB encoded ceiling checked by writer and before reader parsing. Export
accepts only an already controlled create-new durable target, writes portable settings,
publishes after the command irreversible boundary, then reopens and fully verifies the
exact package. Import fully verifies an already open bounded reader and retains one
typed base-identity preview with at most three categories and scalar counts. Confirm
consumes that preview through the existing atomic settings store and preserves device-
local settings. No UI value receives a target, reader, path, filename, raw bytes, or
digest.

Final developer evidence passes nine worker contracts, two application config contracts, all
six package contracts including fail-fast encoded size, 25 application unit plus seven
integration tests, strict state/app Clippy, the application source audit, and 38/38
policy mutations. Clean-root, formatting/diff, warnings-as-errors locked workspace
Clippy, the complete locked workspace test/doctest suite in 502.1 seconds, the release
composition audit, and reliable-state 55/55 mutations also pass. Independent final
review reports Critical/Important/Minor 0/0/0 and `Ready`; the authenticated live Codex
contract remains intentionally ignored without explicit environment binding. Native-
file/UI config preview-confirm, verify/selected-restore/rebuild
execution, full cancellation propagation, no-backup reconstruction, final resource/
product release gates, and product or release acceptance remain open.

## 2026-07-18 — P3-D.0 Task 14 sealed native file selection

Added the platform-only file selection boundary without a new dependency or process.
The existing pinned Windows bindings now compose the Common Item Dialog with balanced
STA COM lifetime, exact `.tmconfig`, `.tmbackup`, and `.tmbackup.age` filters/defaults,
filesystem/path/no-link/no-working-directory-change flags, strict non-mutating Save
selection, required active-window ownership, and explicit `ERROR_CANCELLED`. The native
selector is thread-affine instead of a `Send`/`Sync` worker service.
The returned COM path is copied only into transient platform memory and freed before
the selection call returns.

The public selector returns only selected, cancelled, or a fixed error. Selected input
is already open with final-component no-follow semantics, bounded, regular, and single-
link. Selected output retains the parent identity and target absence/opaque identity,
rechecks them before stage/publication, and on Windows pins one adjacent create-new
bounded stage with an exact delete-capable cleanup handle. Existing replace captures the
displaced
target, checks its physical identity after the syscall boundary, rolls back a raced
replacement, reverifies the new identity/bytes, and deletes old bytes last. An existing
export remains byte-for-byte untouched until publication; changed identity,
wrong extension, remote/device/mapped-remote parent, directory, reparse/symlink,
hard-link, invalid/bound-exceeding Unicode child, and oversize input all fail closed.
Capabilities and `Debug` expose no path, filename, raw file, shell, or process.

The deterministic controlled selector provides the same capability contract for tests
and unsupported hosts without a path callback or queue. Focused evidence passes file-
dialog 11/11, 19 platform unit tests including five deterministic replacement/rollback/
displaced-evidence/cleanup-namespace races, every platform integration/resource/
process-death contract, strict platform Clippy, and formatting. The first independent
review found 0 Critical / 3 Important / 1 Minor and correctly blocked the slice; those
findings plus its later post-mutation and one-stage observations are implemented.
Independent final rereview reports Critical/Important/Minor 0/0/0 and `Ready`.

The strengthened handle cleanup made the prior encryption cleanup fault fixture no
longer fail: a renamed open stage was now deleted by its physical handle as intended.
The fixture was corrected to add a second hard link to the same stage, restoring genuine
cleanup ambiguity and the required `RecoveryRequired` result. Focused encryption 7/7
then passes. One initial full workspace attempt encountered a transient parallel GNU
linker failure; both named test targets passed immediately when run sequentially. The
next complete locked workspace test/doctest suite passes in approximately 473 seconds.
Clean-root, formatting/diff, warnings-as-errors locked workspace Clippy, MSVC all-target
platform check, release application composition, application 38/38 and reliable-state
55/55 authority mutations also pass. Application/UI binding and interactive Windows
evidence remain Task 15 work; no product or release acceptance is claimed.

## 2026-07-18 — P3-D.0 Task 12B.2b and Task 15 reliable recovery UI

Completed the reliable-state application/Desktop contour without granting the UI file,
store, runtime, provider, or recovery authority. Native dialog selection stays on the
owning Slint/STA thread; only sealed input/output capabilities enter the existing single
joined worker. Config preview/confirm/cancel, normal/compact/encrypted backup, verify,
confirmed selected restore, rebuild, retry/cancel, and backup-policy updates are fixed
path-free intents. The worker retains one active request, one follow-up, and one latest
completion. All durable operations now publish the non-cancellable `AtomicPromotion`
phase at their exact one-way boundary.

Added the Data & Recovery and Settings views over one latest-only bounded Desktop
projection. It carries scalar health/policy/receipt facts, at most fifteen generation/
ordinal restore choices, one config preview, and one operation. Restore has an explicit
second review and data-only or data-plus-portable-settings choice. Passphrases are
redacted and cleared after admission. The views include accessibility, high-contrast,
reduced-motion, and narrow/wide hooks and add no timer, animation, progress queue, or
history-sized model. Reliable state remains separate from the archive-backed product
snapshot so safe mode can render it without starting query/controller/runtime owners.

Implemented the previously missing no-backup reconstruction. Proven definitive active
corruption plus absence of a usable fully reverified point authorizes a fresh archive
created through the ordinary store schema. State fully verifies it, stages and
reverifies it, writes an explicit reconstruction journal with no backup identity,
preserves main/WAL/SHM in bounded quarantine, atomically promotes, and fully verifies
the active result. Application then starts one guarded live runtime, forces
`RefreshUrgency::Recovery`, and waits through the bounded worker completion path until
authoritative local Codex reconciliation completes and no refresh remains active or
pending. Only then does backup maintenance start healthy. The durable UI receipt marks
quota, reset-credit, reminder, and Git history unavailable rather than creating false
zeros.

Focused evidence passes application tests, the new engine completion-wait contract, the
complete engine/runtime/store/state/desktop package set including Windows resource
contracts, source audits, and application 42/42, reliable-state 56/56, and desktop 26/26
policy mutations. Clean-root, formatting, warnings-as-errors locked workspace Clippy,
and the complete locked workspace test/doctest suite in 494.3 seconds also pass. One
initial parallel GNU link of the query value contract returned no diagnostic; its exact
target passed 7/7 sequentially and the unchanged full command then passed. The
authenticated live Codex contract remains the sole expected environment-gated ignored
test. Task 16 adversarial matrix, Task 17 release-mode resource/UI latency receipts,
interactive Windows evidence, M0 acceptance, packaging, signing, and release remain
unclaimed.

Independent review then identified four Important UI/operation truth defects and the
root audit found a fifth restart/retry lifecycle gap. The corrected contour pins restore
confirmation to the exact reviewed selection, publishes follow-up `Running` only at
actual execution start, keeps manual backup cancellable until its real irreversible
boundary, and represents unknown counts/bytes as unavailable. Source-reconstruction
reconciliation is now a preflight obligation derived from durable journal/run-state
evidence: it survives cold restart, failed same-process retry, and two interrupted
launches into Safe Mode, while explicit retry reconciles the promoted archive without
repeating reconstruction. Application 46/46, desktop 28/28, and reliable-state 56/56
mutations, clean-root, formatting, strict workspace Clippy, and the complete locked
workspace test/doctest suite in 540.8 seconds pass. Independent rereview reports
Critical/Important/Minor 0/0/0 and `Ready`; Tasks 16-18 and release acceptance remain
open.

## 2026-07-18 — P3-D.0 Task 16 adversarial and privacy closure

Added the dedicated state fault matrix, application recovery coverage gate, and backup-
package audit rail. The package matrix rejects every proper prefix and every one-bit
mutation of deterministic config and backup packages, while retaining the earlier Zstd
bomb/window, age work-factor/password, duplicate/trailing/version, SQLite corruption,
six-phase crash, migration, rollback, data-only, and mandatory-safety evidence as named
independent contracts rather than duplicating weaker implementations.

The WAL/SHM matrix found a real recovery bug: a pre-existing SHM mismatch could be
detected only after the expected WAL had already moved to quarantine. Recovery now
preflights main plus active/quarantine WAL and SHM as one coherent layout before the
first new sidecar move; the existing per-move checks still catch later races. The
regression covers adding, removing, and changing either sidecar, conflicting operation
targets, and exact partially moved resume.

The new PowerShell audit pins the seven-file package codec, twenty-three attack/
compatibility/execution anchors, SHA-256 identities for all 196 resolved name/version/
license and enabled-feature records, both immutable MIT upstream references/notices,
247 production-source files, a synthetic exported archive, and the release binary. It
rejects process, network, shell, generic extraction, plugin, UI, and SQL authority inside
the package codec. Focused state 4/4, app aggregate 57/57, platform archive-recovery
13/13, package Pester 14/14, and the combined package/state/application 120/120 Pester
suite pass. The pre-review locked workspace suite passed in 476.7 seconds.

The first full-suite attempt also converted a previously unexplained intermittent event
into a deterministic regression fix. A 100 ms Codex transport deadline can expire and
reap the child before the fixture writes its PID receipt; the test had incorrectly
required that file. The timeout-only assertion now accepts the absent receipt and checks
the exact copied executable path for a surviving process. The focused target passes
eleven consecutive runs. The first independent review then found four Important gaps:
exact partially moved sidecars could not resume, target collision could move WAL first,
the app gate was source-only, and dependency/feature/export privacy rails were not exact.
New RED/GREEN contracts close each issue. The post-review locked workspace test/doctest
suite passes in 604.1 seconds, strict
warnings-as-errors workspace Clippy passes, and independent rereview reports Critical/
Important/Minor 0/0/0 with `READY`. Task 16 closes; Task 17-18 and release acceptance
remain open.

## 2026-07-18 — P3-D.0 Tasks 17-18 performance and acceptance closure

Task 17 added three release-mode evidence targets and a separate P3-D.0 acceptance
contract. The state performance target runs real Online Backup, optional compact
snapshot, typed package write, and full verification for automatic, normal, and compact
profiles against deterministic schema-13 freelist fixtures. The initial fixture hash
was not reproducible because schema v13 deliberately seeds the Git installation salt
with `randomblob(32)`; the test now normalizes only that test salt. Two consecutive runs
produce the same 8 MiB and 96 MiB fixture hashes. An initial small-versus-large sampled
peak comparison also proved timing-sensitive: constant Zstd allocator high water could
be missed on the short small run. The final stronger gate uses a fixed 64 MiB growth
ceiling and more than 16 MiB headroom to the 101,519,360-byte database. Latest large
automatic/normal/compact throughput measured 70.96/65.03/0.612 MiB/s.
The test-only direct `rusqlite`, `serde_json`, and Slint software-testing edges reuse
already locked packages; no resolved package/version/license or production dependency
was added, so the existing notices/SBOM inputs do not change.

The Windows lifecycle target warms every contour, then executes 256 real backup/package/
verify/import-inspect-cancel/retention cycles. The first independent review found that
the forced cancel happened before resource acquisition, encryption reset its tolerance,
restore was absent, and disk could grow by one package. The corrected contract cancels
after the recovery source reader and candidate exist, performs 16 complete isolated
data-only restores through the real coordinator/journal/promotion path, compares final
encryption resources with the original baseline, and requires exact filled-tier disk
equality on every cycle. The accepted run retained 15 points at a constant 340,155
bytes; private memory returned 4,194,304 to 8,261,632 bytes, handles 151 to 152, threads
5 to 3, USER 2 to 2, GDI 0 to 0, and child processes stayed zero.

The UI target performs real software rendering, not a callback-only proxy. The first
review showed that a backup could finish only during join. The final test warms one
cycle, pins cycle two, and asserts that exact 96 MiB automatic backup remains in progress
before, between, and after all loaded query and paint samples. Cached Dashboard query
p95 increased 0.0326 ms and software-paint p95 did not increase against 10 ms limits.
Corrected independent rereview reports Critical/Important/Minor 0/0/0 and `Ready for
Task 17 integration`.

Task 18 updates specification, data/API/security boundaries, traceability, ADR-064,
architecture, roadmap, recovery operations, current state, handoff, changelog, and this
history. `P3D0_ACCEPTANCE.md` binds eleven gates to one clean commit, application SHA-256,
exact versions, fixture identities, commands, durations, resource/latency/disk metrics,
and `dirty=false`. The ignored receipt is developer output only. Interactive Windows,
M0, soak, MSVC packaging, signing, and product release remain separate and unaccepted.
The first full debug workspace run also exposed that the non-ignored UI target attempted
its 96 MiB release measurement under unoptimized code and could not finish warm-up inside
the ten-second release deadline. The debug target now compiles and reports an explicit
release-only measurement skip; the exact mandatory `--release` gate remains unchanged.
Fresh clean-root, reliable-state, package, application-composition, and Desktop audits,
formatting, strict warnings-as-errors workspace Clippy, the complete locked workspace
test plus doctest gate in 665.4 seconds, and the GNU release application build pass on
the closure tree. The known upstream `proc-macro-error2 2.0.1` future-incompatibility
warning remains non-fatal and unchanged.

## 2026-07-19 — P3-D.1 bounded History route

After a product-priority audit, development returned from infrastructure-heavy work to
the first visible supporting data route. The selected slice deliberately did not reuse
the today-only Dashboard payload and did not introduce the final mutable range scheduler.
Instead, `UsageRange::recent_days` resolves an exact 1-through-400-day civil range;
the production plan requests the latest 30 days with daily points and no breakdowns.

The product reducer now publishes History independently from Dashboard analytics and
invalidates it with dataset identity. The existing capacity-one controller executes the
second request sequentially, preserving complete-attempt cancellation/deadline rules.
Desktop copies one overview plus at most 30 newest-first rows, exact range/timezone,
freshness, quality, and stable reasons. The compiled responsive Slint view renders a
header, overview metrics, daily trend, and wide/narrow detail table without query-time
route behavior, another thread, timer, cache, connection, dependency, or private ID.

TDD first proved the missing recent-range, product section, second query, projection,
and UI contracts. Focused query/product/controller/desktop/UI tests pass. The desktop
audit was extended from its prior fixed file count to exact 9 Rust/15 Slint boundaries,
one History model/application path, 30-day cap, and zero added polling/private-ID/
authority surfaces; all 30 mutation cases and the release audit/build pass. Clean-root,
formatting, strict warnings-as-errors workspace Clippy, and the complete locked workspace
test/doctest suite pass in 710.7 seconds. Interactive arbitrary ranges, Sessions/detail,
remaining supporting routes, P4 presentation, release evidence, packaging, signing, and
product release remain open.

## 2026-07-19 — P3-D.2a bounded Sessions list

The Sessions milestone was split after reviewing the existing dataset-bound page/detail
contracts. The first slice delivers the independently useful list; exact detail remains
separate because a row callback alone cannot safely match rapid selection against a
newer product/dataset generation without either exposing the opaque session key to Slint
or risking stale detail. ADR-066 records the follow-up generation/ordinal boundary.

The existing desktop plan now requests one all-time newest-first session page capped at
64 on the same capacity-one query worker. Dashboard continues to copy its first 12 rows.
The independent product and Desktop Sessions projections retain explicit `has_more` and
only aggregate first/last UTC instants, event count, input/cached/output/reasoning/total
tokens, cost, freshness, quality, and stable reasons. One responsive Slint model renders
wide and narrow layouts without keys, cursors, paths, IDs, detail cache, route-time
query, model rebuild, timer, worker, archive handle, or new dependency.

TDD first proved the missing 64-row controller request and independent projection/UI
contracts. Focused controller, projection, desktop package, and real Slint UI tests pass.
The desktop audit now contains 33 mutation cases and the release receipt pins 10 Rust/
16 Slint files, one worker/slot, a 64-row Sessions maximum, one model/application path,
and zero polling/private-ID/direct-authority surfaces. Clean-root, formatting, strict
warnings-as-errors workspace Clippy, and the complete locked workspace test/doctest
suite pass; the full suite completed in 725.2 seconds. P3-D.2b exact generation-bound
detail, later-page navigation, remaining routes, presentation, automation, acceptance,
packaging, signing, and product release remain open.

## 2026-07-19 — P3-D.2b exact Sessions detail

The detail slice began with the stale-identity problem rather than the card layout.
Product generations restart when an application bundle replaces its controller, so the
implementation adds a checked monotonic `DesktopSnapshotEpoch` beside the viewed product
generation and a nonzero per-click selection generation/ordinal. A higher epoch accepts
the restarted backend generation, rejects the obsolete backend, and clears selection.
Product correlation retains only generation/ordinal and never an opaque session key.

One typed UI intent is admitted only through the current application bundle. The existing
controller worker owns one latest-only pending selection slot shared with refresh work;
only that worker may resolve the visible ordinal to `UsageSessionKey`. Exact result
publication is selection-latest-only. Rapid clicks, stale product/epoch, missing row or
detail, query failure, cancellation, dataset invalidation, safe mode, and bundle shutdown
all fail closed without retaining another selection's payload, creating a queue, or
adding a worker/cache/timer/snapshot slot.

Desktop and the compiled Slint view add synchronous highlight/loading feedback and one
explicit idle/loading/ready/missing/unavailable detail projection. Ready state renders
exact summary and envelope freshness/quality plus a combined cap of 32 model and 32
approved path-free project-alias aggregate rows, with explicit truncation. Rows support
pointer hover/click, explicit Tab navigation, Enter/Space, and the accessibility default
action. The runtime headless contract sends real pointer, Enter, and Space events; the
source mutation rail separately pins the Tab-focus binding. Opaque
keys, cursors, provider/profile/source/session identities, raw paths, prompts, responses,
reasoning content, commands, credentials, SQL, and query authority remain outside Slint.

TDD contracts cover epoch replacement, product correlation, latest-only worker behavior,
refresh/detail coalescing, cancellation, missing/failure states, 32+32 truncation/privacy,
current-bundle rejection, and real headless Slint interaction. Independent review then
closed UI-thread mutex waiting, nanosecond duration borrowing, missing narrow reasoning/
component fields, and callback-only interaction evidence. Product, Desktop, and app
focused tests plus strict package Clippy pass. Eight new Desktop audit cases and four new
application mutations bring their combined audit suite to 93/93; product/Desktop/application release
audits pass and report one controller worker, one snapshot slot, one detail model, zero
retained product/UI session keys, and no new authority/dependency/polling surface. Models,
Projects, Activity, later-page Sessions navigation, presentation, automation, interactive
acceptance, packaging, signing, and product release remain open.

Independent follow-up review returned READY with Critical/Important/Minor 0/0/0. Its
initial sole Minor noted that the lexical queue rail was broader than the session-detail
boundary; the pattern was scoped to session/detail lines, a false-positive acceptance
case was added, and all 41 Desktop audit cases passed. The final clean-root, formatting,
strict workspace Clippy, and complete locked workspace test/doctest baseline passed in
820.7 seconds overall (18.845, 1.611, and 22.080 seconds for the first three stages). One
credential-dependent live Codex contract remains explicitly ignored. No M0, package,
signing, or product-release acceptance is inferred.

## 2026-07-19 — P3-D.3 bounded Models route

The Models slice began by auditing query duplication rather than enlarging the existing
Dashboard card. Dashboard already held a today-only top-12 Model breakdown while
History held the exact recent 30-day series without breakdowns. A separate Models query
would have repeated the same aggregate and pricing work and created another range state.
ADR-068 therefore adds Model and Project breakdowns to the existing recent request.
The controller still performs exactly two analytics calls: today and recent 30 days.
History consumes the series, Models consumes Model, and Projects can consume the already
captured Project breakdown.

TDD first proved that Models incorrectly followed today analytics and that the recent
request lacked breakdowns. The product route now follows recent-section readiness.
`DesktopModelsProjection` retains at most 64 canonical model rows from the query's
256+lookahead boundary, preserves input/cached/output/reasoning/total, event, cost,
range/timezone/freshness/quality truth, and surfaces both backend and desktop truncation.
Missing breakdown, retained failure, empty, partial, and unavailable states remain
typed. Provider/profile/source/account/workspace/project/session identity, keys,
cursors, paths, content, and authority do not cross the projection.

The first independent review found that the numeric cost label hid partial availability
and actual provenance, and that ready-empty/partial/mismatched-identity acceptance was
under-tested. TDD fixtures now cover all three. `DesktopCostValue` retains availability,
selection mode, and actual calculated/reported/mixed composition; the compiled Slint
header and rows render partial/composition evidence visibly and in accessible labels.
The relative-to-largest bar is now named `Relative`, not the mathematically incorrect
`Share`.

The compiled Slint route replaces the placeholder with one responsive header and ranked
table. Wide and narrow layouts use the same model and accessible row meaning; narrow
rows retain all token components rather than showing only totals. In-place route
switching neither rebuilds the model nor recreates the window. Focused product/Desktop
and real headless Slint tests pass, including 257-model backend lookahead and a 64-row UI
fixture. The production audit now passes 47 mutation cases covering the shared request,
no third query, exact cap/truncation, one mapping/application site, complete responsive
token mix, privacy, and zero route-time work. Independent re-review returned READY with
Critical/Important/Minor 0/0/0. Clean-root, formatting, strict warnings-as-errors
workspace Clippy, and the complete locked workspace test/doctest baseline pass; the
full suite completed in 790 seconds. P3-D.3 is closed. Projects, Activity, interactive
ranges, presentation, automation, packaging, signing, and release remain open.

## 2026-07-19 — P3-D.4 bounded Projects route

The Projects slice began by resolving an evidence-window conflict rather than hiding
it in presentation. Recent usage already covered 30 local civil days, while the one
existing Git request intentionally covered UTC today for Dashboard. Replacing that
request would mislabel Dashboard; adding another Git query would add work and mutable
range ownership. ADR-069 therefore keeps both immutable envelopes and requires visibly
separate `Recent usage` and `Today code` ranges, timezones, evidence, and completeness.

TDD added `DesktopProjectsProjection` with at most 32 usage-centric rows over the
existing 256+lookahead Project breakdown. Only safe `ProjectAlias` or explicit
`Unassociated` crosses the projection. Named rows match at most 32 Git repositories by
exact alias equality; unassociated usage never matches and Git-only aliases never
become fabricated zero-usage rows. Rows preserve input/cached/output/reasoning/total,
events, typed cost provenance, relative usage, and optional commits/added/removed/net/
efficiency. Multiple same-alias repositories use checked sums. Compatible product-code
lines are summed but one project usage cost is used once, preventing repository-count
cost multiplication. Mismatch, absence, partial evidence, retained failure, overflow,
and query/frontend truncation remain explicit.

The compiled Slint route uses one bounded model for wide and narrow layouts. Its
accessible labels name both periods and retain every usage and code fact. Route-only
switching neither queries nor rebuilds the model/window. Tests cover ready, empty,
partial cost, unassociated, unmatched, Git-only, retained Git failure, same-alias
aggregation, 256-item backend lookahead, 32-store/16-query Git lookahead, mismatched
identity/cost, zero divisor, checked line/ratio/totals overflow, privacy, Git-unavailable
non-zero fabrication, and 10,000 projection replacements. The complete Desktop package,
strict package Clippy, source/release audits, and 57/57 mutation audit pass. Independent
review first found four Important and one Minor truth/evidence gaps; red/green fixes made
partial cost independently degrade, removed fabricated Git zeroes, exposed completeness/
reasons without color dependence, labelled product-code efficiency exactly, separated
non-UTC usage from UTC Git, and proved mismatch/zero/overflow/not-linked fail-closed
paths. Final re-review returned Critical/Important/Minor 0/0/0. Clean-root, formatting,
strict warnings-as-errors workspace Clippy, and the complete locked workspace test/
doctest suite pass; the full suite completed in 807 seconds with serialized Windows GNU
linking after one isolated concurrent blank-stderr MinGW linker exit. P3-D.4 is closed;
Activity and later interactive/detail/presentation/automation/release work remain open.

## 2026-07-19 — P3-D.5 bounded Recent activity route

The Activity slice began by separating two capabilities that the reference UI presents
together: a useful latest-event list and a true time-distribution/rhythm aggregate.
TokenMaster already prefetched one `LatestActivityRequest::first(12)` for Dashboard,
but had no bounded hourly/day-of-week dataset. ADR-070 therefore reuses the existing
page for a truthfully named Recent activity route and leaves rhythm/heatmap work behind
a future timezone/DST-aware aggregate contract. No sample-derived parity claim was made.

TDD added `DesktopActivityProjection` with at most 12 newest-first rows. Each row copies
only timestamp seconds/nanoseconds, canonical model, and typed input/cached/output/
reasoning/total tokens. Freshness, quality, optional `has_more`, authoritative empty,
unavailable, retained failure, backend lookahead, and frontend truncation remain
distinct. Scope, provider/profile/account, event/dataset/source/session/project
identity, cursor/fingerprint/key, paths, content, prompts, responses, commands,
credentials, and authority never enter the projection. Ten thousand replacements
release the old row list, and aggregate rebuild leaves the Activity route ready.

The compiled Slint route mounts one responsive header/table and one replace-only model.
Wide and narrow layouts retain every token component and the same full accessible row
meaning; the UI labels UTC context and incomplete-page truth explicitly. Switching
routes changes visibility only. Focused projection tests pass 9/9, the real headless UI
contract passes, the complete Desktop package and strict package Clippy pass, and the
source/release audit reports 13 Rust plus 19 Slint production files, one worker, one
snapshot slot, one Activity query/application/model site, and zero polling. The audit's
67 mutation cases cover caps, exact fractional UTC formatting, duplicate query/model/
application sites, responsive mount/token/accessibility semantics, private identity,
and false rhythm claims.

Independent review found two Important state intersections hidden between otherwise
passing cases. An empty partial/stale page skipped evidence degradation because the
shared helper was told that zero rows meant no payload. A retained authoritative-empty
page then rendered `Recent activity evidence unavailable` despite still owning a
complete empty page. Red/green contracts now treat an empty envelope as available
evidence, degrade its non-authoritative header truth, and pass one safe
`page-available` boolean so retained empty and unavailable remain distinct. The same
closeout preserves exact fractional UTC nanoseconds and fails closed on invalid values.
Re-review returned Critical/Important/Minor 0/0/0. Clean-root, formatting, strict
warnings-as-errors workspace Clippy, release composition, and the complete locked
workspace test/doctest gate pass; the full baseline completed in 1,035 seconds.
P3-D.5 is closed. Notifications/Help, full rhythm aggregation, later pagination/ranges,
presentation, automation, packaging, signing, M0, and release remain open.

## 2026-07-19 — P3-D.6 bounded Notifications expiry-safety route

The Notifications slice began with a boundary audit of the already-implemented benefit
inventory/query/reminder stack. The all-current benefit overview already carried
separate current lots, effective profile inheritance/override, coverage, due time,
expiry precision, freshness, quality, and warnings. The durable reminder runtime also
had crash-safe leased take/release/ack behavior, but the application had no event-loop
presentation receipt. ADR-071 therefore separates a useful read-only expiry center
from visible delivery: route projection/navigation cannot lease or acknowledge an
event, while a later app-owned bridge must acknowledge only after successful visible
presentation and release every failed/cancelled batch.

TDD added `DesktopNotificationsProjection` over the existing benefit snapshot. It
retains at most 32 identity-free effective profile rows, 256 separate current-lot rows,
and eight leads per profile. Exact UTC, bounded UTC, provider-local, provider-date, and
unknown expiry remain distinct; exact/bounded UTC UI labels preserve milliseconds.
Policy source/coverage, revisions, completeness, evidence, warning codes, nearest
expiry/due, provider-neutral lot kind/quantity/state/label, optional grant time, and
evidence source/confidence/detail remain explicit. Provider/account/workspace/scope/
lot/delivery/window IDs, target, path, content, credential, receipt, and activation
authority remain outside Desktop/Slint. Ten thousand populated snapshot replacements
release both old arrays.

The compiled Slint route mounts one responsive expiry header, one effective-profile
model, and one separate-lot model. Waiting, unavailable, retained/degraded, empty,
warning, and truncated states remain distinct. Wide and narrow profiles both show
completeness plus evidence; rows expose complete accessible policy/expiry/evidence
meaning. One accepted product generation replaces each model once, and route-only
selection changes visibility without querying, rebuilding, polling, scheduling, or
mutating reminder state.

The deterministic source audit reports 14 Rust plus 20 Slint production files, one
existing benefit query, one Notifications projection application, one replacement site
per bounded model, and computed zero delivery authority, owner/control, and polling
counts. Its 82 mutation cases reject cap drift, duplicate query/model/application,
missing route/precision/responsive meaning, private identity, delivery authority,
query/database ownership, worker/thread, queue/cache, timer/polling, activation
callback, and missing wide completeness. Focused projection 7/7, exact-time unit,
compiled UI, complete Desktop package, strict Desktop Clippy, and source audit pass.
The first independent review found three Important and one Minor closure gaps: exact/
bounded UTC presentation lost seconds/milliseconds, waiting resembled unavailable,
owner/control receipts were under-proved, and wide profiles omitted completeness. Red/
green fixes close all four and strengthen the resource proof with populated old arrays.
Re-review returned Critical/Important/Minor 0/0/0 and READY. Clean-root, formatting,
strict warnings-as-errors workspace Clippy, release composition, and the complete
locked workspace test/doctest gate pass in 1,216.4 seconds overall
(25.8/1.7/76.0 seconds for clean-root/fmt/Clippy). P3-D.6 is closed.
Help/About, app-owned presentation receipts, settings synchronization/editing, snooze,
quiet hours, OS delivery, usage alerts, activation, presentation/automation/release
work, packaging, signing, M0, and release acceptance remain open.

## 2026-07-19 — P3-D.7 bounded Help/About route

The Help/About slice began by proving that `ProductRoute::HelpAbout` was already ready
without an archive while the compiled shell still rendered its generic placeholder.
The boundary audit rejected a dynamic diagnostics or release-status owner: the useful
truth is fixed product navigation, source/evidence semantics, privacy, Data Health and
Settings ownership, current automation availability, and license attribution. ADR-072
therefore selects one archive-independent static view, the compile-time Cargo package
version, and the pinned standard `AboutSlint` widget. P4 still owns unified en/ru/pseudo
localization, P5 owns bounded read-only CLI/MCP, and P6 owns generated notices/SBOM,
MSVC/package/signing/public-download attribution and release identity.

TDD replaced the placeholder with one always-mounted `HelpAboutView`. Its five guide
cards plus one attribution card are instantiated once and only visibility changes on
route selection. Content-width layout truth flows back from the child, so a 900-pixel
window correctly uses the narrow layout after the fixed navigation rail. The standard
attribution widget has a 112-pixel safe height inside a 232-pixel card, and the external
MIT reference line remains at readable body-text size. The route owns no projection,
list model, query, dynamic diagnostics, callback, URL, browser/session surface, worker,
timer, queue, cache, connection, polling loop, provider mutation, or release receipt.

The real headless Slint contract proves ready-without-archive state, compile-time
version, each of the six section region labels exactly once, one standard attribution
label, 700/900 narrow and 1120 wide layout, truthful privacy/automation text, and
in-place `MainWindow` identity.
The deterministic audit parses five guide plus one attribution instance, requires the
single always-mounted child, child-owned layout/count truth, safe attribution geometry,
one version setter, and computed zero model/authority/polling counts. Its 104 mutation
cases reject duplicate/removed sections, conditional reconstruction, full-window
breakpoint drift, hardcoded outer counts, clipped/small attribution, list models,
callbacks/open-URL authority, wrong recovery ownership, hidden privacy, and false
release/automation/all-provider claims. The complete Desktop package, strict package
Clippy, source/release audits, and focused formatting/diff checks pass. Independent
final review returned Critical/Important/Minor 0/0/0.

The first exact workspace baseline stopped at the existing product resource contract:
its warm-up accepted a bimodal 3.5-6.1 MiB process profile at a 3,723,264-byte floor,
then the measured 5.12-6.06 MiB minima missed the unchanged 2 MiB return tolerance.
A deterministic regression preserves the exact 16 samples and fails the old selector.
The corrected selector additionally rejects a retained ceiling more than the same 2 MiB
above its floor, so it continues warming rather than loosening the resource gate. Three
new processes pass 1,088 captures each, strict focused Clippy passes, and independent
review returned 0/0/0. The subsequent uninterrupted `TM-CLEAN-PASS`, formatting,
strict workspace Clippy, and complete locked workspace test/doctest baseline passes in
879.3 seconds. P3-D.7 is closed as a developer slice; P4/P5/P6, M0, packaging, signing,
and product-release acceptance remain unclaimed.

## 2026-07-19 — app-owned visible expiry presentation

ADR-073 closes the false-delivery gap between the durable reminder lease and Slint.
Desktop now accepts only a provider-neutral batch capped at 256 rows, schedules one
independently checked weak-window event, replaces one transient model/count/visible
state, and emits `Presented` only after verifying the applied row count. It owns no
runtime, store, delivery identity, timer, polling loop, queue, or auto-dismiss authority.

The application now wraps the optional reminder runtime in one shared locked owner and
creates one presentation port plus one condition-variable receipt worker when that
runtime starts. The worker retains no batch, acknowledges outside the UI thread,
retries acknowledgement only for Busy and StoreUnavailable after exactly 60 seconds,
and re-pumps a confirmed released failed presentation on that same worker. A terminal
acknowledgement error releases without automatic re-presentation. Deterministic RED
tests found and closed release and wake races: local backpressure remains set after
`Err` or `false`, and a concurrent external retry wakes receipt processing immediately.
Runtime acknowledgement panics are redacted and restore `Acknowledging` to `Leased`;
the narrow release fallback can recover outer-mutex poison. Desktop clears its busy bit
before calling the receipt and accessibility includes both visible labels.

Desktop tests, app tests including a real SQLite reminder lease/ack path, the reminder
runtime replay suite, computed source receipts, and 177/177
Desktop/application mutation cases pass. Settings editing, snooze, quiet hours,
OS/tray delivery, usage alerts, activation, P4/P5/P6, M0, packaging, signing, soak, and
release acceptance remain unclaimed.

Repeated independent lifecycle review closed at Critical 0, Important 0, Minor 0. The
exact clean-root, formatting, workspace Clippy with warnings denied, and complete
workspace-test developer baseline also passed. These checks do not substitute for M0,
interactive Windows, soak, package, signing, or release acceptance.
