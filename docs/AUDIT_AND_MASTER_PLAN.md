# TokenMaster Critical Audit and Approved Master Plan

Status: architecture and release plan approved after closure review; P0 through P1,
P2-A, P2-B, P2-C pinned pricing, and the P2-D quota history core are implemented;
the permitted built-in Codex quota normalizer/transport, exact-native executable
discovery, and dedicated quota refresh/store publication are also implemented.

Global reminder settings synchronization is developer-closed: mutation-resistant
application/Desktop receipts guard the sealed worker payload, settings-first
generation-bound global projection, fixed eight-row editor, dirty draft, accessibility,
startup contention retention of retryable Pending with independent optional-runtime
health, and no new owner surfaces. This does not claim per-scope editing, snooze, quiet
hours, OS/tray delivery, usage alerts, activation, P4/P5/P6, M0, package/signing/soak,
or release acceptance.
Typed banked-reset domain/core/Codex normalization, strict schema-v12 store
foundation, immutable FEFO current/revision-bound history queries, and combined Codex
runtime publication with separate quota/benefit truth are implemented. Reminder
runtime and in-app presentation are implemented; OS delivery, automation, and
activation remain later.
Date: 2026-07-16.

## 1. Executive decision

TokenMaster remains an original Rust 1.97, Slint 1.17, and bundled SQLite product.
The current workspace is worth continuing. A rewrite in Electron, Tauri, React, Go,
or another GUI stack would discard verified bounded-reader, parser, SQLite, Slint,
tray, and resource-gate work without fixing the actual accounting-contract defects.

The approved implementation boundary is:

```text
Codex bytes -> Codex decoder -> ObservationDraft
             -> TokenMaster accounting canonicalizer
             -> replay classifier -> transactional SQLite
             -> immutable query snapshots -> UI / CLI / MCP
```

Providers may normalize source facts. Only TokenMaster accounting code may derive
fingerprints, replay signatures/evidence, public event IDs, replay dispositions, or
canonical contributions. The Codex adapter remains the compiled-in fast path. The
external Wasm provider host remains deferred until the native engine, query, and quota
contracts are stable.

## 2. Alternatives considered

| Approach | Result | Decision |
| --- | --- | --- |
| Patch replay fields into the existing Codex-owned canonical event | Short diff, but preserves an authority bypass and unsafe identity rules | Rejected |
| Rebase P0 around provider-neutral drafts and a core canonicalizer | Slightly larger migration, but makes one enforceable accounting authority | Approved |
| Rewrite the complete application | Maximum churn, loses proven native/resource work, does not inherently solve replay correctness | Rejected |

## 3. Audit findings and binding resolutions

### A. Accounting authority

1. `CanonicalUsageEvent::new(parts, fingerprint)` is public, so adapters and tests can
   bypass ADR-009. Resolution: move the canonical event and its constructor into a new
   `tokenmaster-accounting` crate; expose read-only accessors and a canonicalizer only.
2. Codex currently computes fingerprints and emits canonical events. Resolution:
   Codex emits bounded `ObservationDraft` values. The reader carries drafts, never
   canonical identities.
3. Replay types were added to `tokenmaster-domain` before the plugin boundary was
   finalized. Resolution: drafts retain only parent, ordinal, conflict marker, delta,
   and optional cumulative facts. Replay signature/evidence move to accounting output.

### B. Identity and replay correctness

4. Fingerprint v1 hashes timestamp, model, profile, and token values but omits provider,
   session, and ordinal. Equal events in different sessions can collide. Resolution:
   fingerprint v2 hashes a domain tag plus provider/profile/session/ordinal, normalized
   model, and explicit delta availability/values. It excludes timestamp, source, path,
   display metadata, and activity so active/archive observations of one logical event
   can deduplicate without collapsing distinct session ordinals.
5. Replay identity remains separate. Replay signature v1 hashes normalized model,
   delta, and optional cumulative snapshot. Evidence is strong only when cumulative
   total is available; otherwise it is weak and cannot suppress a pre-divergence event.
6. Codex lineage has multiple evolving shapes. Resolution: maintain a pinned
   compatibility matrix for `forked_from_id`, `parent_thread_id`, and structured
   subagent spawn ancestry, including precedence, conflicts, late metadata, invalid
   types, and unknown-shape fail-soft behavior. Session relation state is separate from
   individual event drafts so late ancestry can reclassify earlier observations.
7. Exhausting depth or fanout is not proof of a contradictory lineage. Resolution:
   cycles, self-parenting, and two different explicit parents are `conflict`; bounded
   work exhaustion is `pending_bound` with a durable continuation queue.

### C. Archive and migration safety

8. Deleting all v1 canonical rows during migration can leave an empty product when
   original sources are missing. Resolution: preserve the v1 archive as a read-only
   `legacy_unverified` snapshot, build replay-safe v2 state in invisible staging, and
   atomically promote only after a complete validated rebuild. Failure leaves the
   prior archive readable and explicitly marked stale/unverified.
9. Replay schema keys must include provider scope and version fields. Every replay
   row records `canonicalizer_version`, `fingerprint_version`, and
   `replay_signature_version`. Descendant indexes include provider, profile, parent,
   ordinal, and disposition.
10. Fingerprints are immutable. Reconciliation refreshes canonical selection for the
    affected logical key; it never rewrites an observation fingerprint in place.

### D. Product completeness

11. Live Codex Plan Usage needs a separate quota adapter. The approved 1.0 design uses
    only a credential-free versioned local format or a documented stable official
    machine interface. A dashboard, slash command, browser page, cookie, or observed
    private endpoint is not an API contract. When neither permitted source exists,
    quota/reset discovery is explicitly unavailable or stale; local token totals never
    become provider capacity. Quota windows are provider data, never hard-coded
    `5h`/`1w` UI fields.
    Every full weekly reset, including an early or repeated reset, closes an immutable
    quota epoch and preserves exact available before/after ratios or units, maximum
    pre-reset use, old/new reset time, evidence/confidence, and allowance changes.
    The approved executable plan is
    `docs/superpowers/plans/2026-07-15-tokenmaster-quota-reset-history.md`.
    Banked rate-limit resets are a separate P2 inventory, not a quota window or credit:
    different expirations remain separate lots, one bounded due queue drives truthful
    reminders, and confirmed consumption links to the quota transition. Automatic
    activation defaults off and exists only behind an official idempotent/status
    capability, explicit policy, fresh evidence, CAS, durable intent, and reconciliation;
    browser/session automation is forbidden. The approved plan is
    `docs/superpowers/plans/2026-07-15-tokenmaster-banked-reset-inventory.md`.
12. Code Output needs its own bounded Git metrics subsystem. It must not expose a shell
    or retain file contents. It owns repository association, author filtering,
    worktree/submodule/rename/merge semantics, incremental cache, and explicit
    unavailable quality states.
13. Pricing uses an embedded pinned catalog with source/version provenance, explicit
    unknown-model state, and validated local overrides. No automatic network request
    runs in the hot path.
14. `docs/FEATURE_PARITY.md` is now the row-level behavioral ledger for the pinned
    references: exact capability, implement/adapt/reject decision, TokenMaster
    improvement, requirement owner, delivery gate, validator, and status. A 1.0 parity
    claim is blocked until every row is implemented or deliberately rejected under a
    surviving normative rationale and regression gate.
15. Product composition is one leaf crate over immutable query/runtime projections.
    One exact scalar status transaction prevents mixed-revision UI stitching; separate
    checked attempt/source/runtime generations prevent stale async overwrite; one
    current snapshot and fixed route/reason topology bound retained memory. P3 may add
    a worker and presentation mapping but no SQLite or runtime ownership to Slint.

### E. UI, quality, and documentation

16. Slint software rendering does not support drop shadows. Production elevation is
    expressed through border tone, surface contrast, spacing, and optional solid
    offset layers; skin tokens may not require unsupported effects. Paint latency is
    measured at visible present, not at callback completion.
17. Traceability and status documents have drift. Every normative `TM-*` requirement
    gets a row even when planned; every ADR reference must exist; current-state,
    roadmap, README, changelog, and plan status must agree. A machine validator becomes
    a release gate.

### F. Release and supply-chain closure

18. The workspace-global GNU target is a development constraint, not a signed-release
    decision. The canonical 1.0 artifact is `x86_64-pc-windows-msvc`; P6 removes the
    forced global target and uses an explicit GNU/MSVC comparison before release.
19. The 1.0 package is a signed portable ZIP. Automatic update and installer behavior
    are deferred until signed-manifest, interrupted-update, rollback, and downgrade
    contracts exist.
20. The Slint Royalty-free License 2.0 route is selected with attribution in Help/About
    and on the public download page. Dependency notices, license policy, and SBOM are
    package gates.
21. Pricing is a release-pinned embedded catalog plus bounded validated overrides; no
    hot-path network update is allowed and unknown model cost stays unknown.
22. P6 requires advisory, dependency/source/license, secret, SBOM, immutable-action,
    artifact-attestation, deterministic-content, and clean-room-launch evidence.
23. The complete closure rationale and self-review are recorded in
    `docs/superpowers/specs/2026-07-16-tokenmaster-plan-closure-design.md`.

## 4. Approved delivery order

1. **P0-A — authority boundary:** `ObservationDraft`, provider-scoped identities,
   `tokenmaster-accounting`, fingerprint v2, replay signature v1, opaque canonical
   output, and removal of public construction bypasses.
2. **P0-B — Codex lineage:** compatibility fixtures, late ancestry, parser resume v2,
   ordinal/cumulative emission, and path-private diagnostics.
3. **P0-C — pure replay classifier:** root/matching/diverged/pending/conflict states,
   bounded continuation, and nested ancestry tests.
4. **P0-D — SQLite replay archive:** provider/versioned schema, non-destructive legacy
   snapshot, invisible v2 staging, rollback, indexes, and canonical selection.
5. **P0-E — transactional pipeline proof:** append/reconcile/restart/truncate fixtures,
   exact totals, quality counts, privacy scan, and bounded-memory evidence.
6. **P1 — runtime engine:** scan epochs, source finalization, staging promotion,
   coalescing, cancellation, sleep/resume, writer lease, recovery, and immutable
   snapshot revisions.
7. **P2 — product data:** built-in Codex quota source and runtime publication,
   immutable weekly reset epochs and before/after history, pricing catalog/overrides,
   bounded Git output metrics, indexed analytics, and data-quality semantics.
8. **P3 — complete desktop UI:** six-section board; P3-D.0 reliable settings,
   verified import/export/backups, bounded retention, corruption recovery and safe
   mode; then all history/session/model/project/activity/health/settings/help/widget
   routes from immutable snapshots.
9. **P4 — presentation:** independent skin/layout/density/scheme/locale axes, dynamic
    quota bars, en/ru/pseudo, keyboard/accessibility, DPI, and visible-paint gates.
10. **P5 — automation:** strict JSON CLI, separate stdio MCP process, capabilities,
    bounded queries, idempotent refresh, and declarative advisory policy for Hermes and
    other clients. This remains read-only/advisory and follows the complete UI.
11. **P6 — release:** Windows integration, portability, dependency/security audit,
    canonical MSVC signed portable ZIP, Slint attribution, SBOM, secret scan,
    immutable CI actions, attestation, package identity, interactive receipt, and soak
    receipt.
12. **1.1 — provider ecosystem:** isolated Wasm Component host, WIT SDK, package trust,
    permissions, hot install/replace, and conformance suite. No Wasmtime dependency in
    GUI or Codex-only operation.

## 5. Global acceptance invariants

- No whole-history vector or unbounded queue on a production path.
- No prompt, response, reasoning, command, output, file content, credential, raw tail,
  or absolute user path in persistence, diagnostics, snapshots, CLI, MCP, or reports.
- Missing data remains unavailable/partial/stale/conflict, never fabricated zero.
- Older async results cannot overwrite newer generations.
- UI, CLI, and MCP read the same immutable indexed truth.
- Skin/layout/locale changes do not scan sources or mutate the archive.
- No M0 acceptance, package, or release claim without the exact interactive and soak
  receipts bound to one clean commit and executable SHA-256.
- No 1.0 parity claim while `docs/FEATURE_PARITY.md` contains `planned` or `partial`;
  rejected rows require a surviving normative rationale and regression gate.

## 6. Current execution rail

P0-A through P0-E are complete under their executable plans in `docs/superpowers`.
P1-A is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p1-retained-projection.md`; it adds
strict schema-v4 provenance and explicit bounded carry-forward. The immediate
P1-B.1 gate is implemented as strict schema v5 plus scoped complete-only scan
authority. P1-B.2 exact scan-bound replay and zero-source retention are also
implemented. P1-B.3 completes reference-safe scan-history retention with a 32-set
per-scope window, 64-set transaction batch, reference/running preservation, recovery,
ID-exhaustion, and rollback proofs under
`docs/superpowers/plans/2026-07-15-tokenmaster-p1-b-scan-authority.md`. The immediate
completed P1-C provider-neutral engine core is recorded under
`docs/superpowers/plans/2026-07-15-tokenmaster-p1-c-engine-core.md`. P1-C.1 constant-
state admission/coalescing/deadline/cancellation and P1-C.2 bounded
adapter/archive/clock/writer-lease ports are implemented. P1-C.3 now adds lease-first,
scope-exact streamed discovery, all-complete replay, bounded canonical batches, exact
revision/epoch continuity, all-phase cancellation/deadline handling, and last-confirmed
unpublished cleanup. P1-C.4 now completes the engine core with one owned worker,
capacity-one wake/latest-result channels, constant-state coalescing, checked
supersession, cancel/wake/join ownership, stale/deadline safety, and redacted
panic/fault containment. P1-D.0 through P1-D.5 now provide exact per-file streaming,
atomic replay facts, production Codex bootstrap, and replay-aware tail-only refresh
with schema-v6 publication/recovery truth, a real portable writer lease, and bounded
pathless watcher/periodic scheduling. P1-D.6 now composes lease-first restart recovery,
incremental/rebuild selection, worker/scheduler/watcher ownership, admission-safe
pause/resume, and ordered joined shutdown. P1-E.1 now publishes one startup-seeded,
strictly generation-ordered immutable engine snapshot with exact archive revision,
scan-set/data-through truth, fixed checked diagnostics, and busy/older-result rejection.
P1-E.2 now closes no-change, pause/resume, restart, malformed-truncation recovery,
canonical-retention, and successful-repair publication semantics. P1-E.3 adds a static
capacity-one Windows power callback, idempotent runtime lifecycle
command, forced resume reconciliation, and 4,096-cycle private-memory/handle/thread/
USER/GDI bounds. P1 is implemented; P2-A now supplies the first immutable indexed
query snapshot. M0 interactive/soak evidence remains separate.
P2-A now has an executable approved design in
`docs/superpowers/specs/2026-07-16-tokenmaster-p2-query-design.md`: publication
generation is independent from dataset identity; current identity includes replay
revision plus schema-v7 dataset generation; every result is transaction-exact and
owned, frontends receive no SQLite access, and latest activity uses the existing
composite keyset index. P2-B schema-v8 provider identity, transactional materialized
rollups, and bounded resumable publication are now implemented. Fixed overview,
partitioned series, independently capped breakdown reads, and opaque mixed-order
keyset session page/detail reads also pass in exact snapshots. Private exact IANA/DST
calendar composition and immutable analytics/session facade values now pass as well,
including optional daily series, explicit token availability, and scope-bound opaque
session continuation. Deterministic current/legacy million-event latency, rebuild,
storage amplification, privacy, and repeated resource evidence now pass. P2-C
schema-v9 price facts, fixed-point engine, immutable overrides, batched cost facade,
million-event, resource, and offline-network gates pass. P2-D quota history Tasks 1-8
also pass: exact domain/evaluator/schema/write/retention/read/query contracts,
adversarial inference, 32-window/1,000-transition/10,000-poll scale, Windows resource
return, and offline authority audit. The strict official Codex app-server normalizer/
transport, live smoke, adversarial/resource gates, and release authority audit also
pass. Exact-native discovery plus dedicated refresh/store publication now pass their
focused, resource, and release-authority gates. Banked-reset inventory Tasks 1-8 now
also pass through combined runtime publication, the isolated durable reminder event
runtime, complete project-truth synchronization, and the full workspace plus
specialized authority gates. P2-E Git output Tasks 1-8 now pass: domain/parser/native
backend/hint/schema-v13 projection, bounded store capture, immutable public UTC/
efficiency facade, bounded runtime publication/lifecycle, Windows resource plateau,
and final authority audit. P2-F joined product status is complete under
`docs/superpowers/plans/2026-07-17-tokenmaster-p2f-product-status.md`: exact scalar
schema-v13 capture, schema-v1 joined envelope, one-current-snapshot reducer, separate
attempt/source/runtime generations, 11 fixed route states, runtime fault isolation,
100,000-event latency, replacement/resource plateau, and authority audit pass. P3
complete desktop UI is the active critical path. Before the remaining P3-D views,
P3-D.0 Reliable State is approved under
`docs/superpowers/specs/2026-07-17-tokenmaster-reliable-state-design.md` and
`docs/superpowers/plans/2026-07-17-tokenmaster-reliable-state.md`: it keeps the fixed
archive/lease identity and adds redundant settings, consistent verified snapshots,
strict compressed packages, bounded retention, durable restore/quarantine, automatic
corruption-only recovery, safe mode, and resource/privacy gates. Task 1 is implemented:
the state package is library-only and the sealed platform/store/state/app authority
chain is now implemented through Task 15. This includes bounded durable files and A/B
records, typed settings/packages, WAL-consistent verified snapshots, optional manual age
protection, fixed-slot catalog/retention, capacity-one maintenance, journaled restore/
quarantine, pre-open recovery, migration safety, identity-pinned selected restore, one
joined application worker, sealed native selection, complete path-free command/UI
binding, exact irreversible operation phases, and bounded Data Health/Settings views.
No-backup rebuild creates and fully verifies a normal fresh archive, preserves the
corrupt set, and completes mandatory authoritative-source reconciliation before healthy
publication, including restart/retry after promotion, while reporting unknown metrics
and non-reconstructible domains unavailable. Restore confirmation remains bound to its
exact reviewed generation/ordinal and queued operation state changes at actual worker
execution. Tasks 16-18 fault, resource-return/UI-latency, documentation, and P3-D.0
acceptance closure are complete. P3-D.1 History, P3-D.2 Sessions list/detail, P3-D.3
Models, P3-D.4 Projects, P3-D.5 Recent activity, and the read-only P3-D.6 Notifications
expiry center now extend the same bounded snapshot/controller boundary. Notifications
reuses the existing benefit overview with 32 profile/256 lot/eight-lead caps and zero
delivery acknowledgement or new runtime owner. P3-D.7 Help/About now closes its P3-D
archive-independent route placeholder with six static accessible sections, one compile-
time version setter, one standard pinned Slint attribution widget, and zero model/query/
runtime/callback/polling owner. The separate app-owned in-app presentation contour is
now implemented with a 256-row cap, one checked weak-window epoch, one transient model,
visible-before-ack receipt ordering, one condition-variable worker, exact 60-second
Busy/StoreUnavailable acknowledgement retry, same-worker failed-presentation re-pump,
terminal-acknowledgement release without re-presentation, panic-safe runtime rollback,
confirmed failure/shutdown release, and release-before-
local-clear backpressure. Global notification settings synchronization/editing is
implemented with one latest-wins pending payload and visible Pending before durable
settings mutation. P3-E.1 palette, P3-E.2 compact mode, and P3-E.3 production tray/
close lifecycle are developer-closed; current-session activation/global hotkey and
current-user startup are next. Per-scope editing, snooze, quiet hours, reminder OS/tray
delivery, usage alerts, and activation remain later independent slices.
P3-D.6 independent re-review is Critical/Important/Minor 0/0/0; its 82 mutation audit,
release composition, strict workspace Clippy, and complete locked workspace test/
doctest gate pass in one 1,216.4-second baseline.
P3-D.7 independent final review is also Critical/Important/Minor 0/0/0; its real Slint
contract, release audit, 104 mutation cases, and exact clean-root/fmt/strict workspace
Clippy/locked test-doctest baseline pass in 879.3 seconds. The baseline-discovered
product resource warm-up false plateau has an exact deterministic regression, three
independent live passes, unchanged 2 MiB return tolerance, and separate 0/0/0 review.

The final planning pass fixes a six-state database/settings transaction, explicit
manual data-only or data-plus-portable-settings choice, data-only automatic recovery,
distinct missing-damaged-main and first-install paths, mandatory safety points even
when periodic scheduling is disabled, and a separate P3-D.0 acceptance receipt rather
than modifying M0 evidence.
View-time full scans and period-labeled
whole-session totals remain forbidden.
P2-A now implements the bounded public values, separate defensive read-only/query-only
schema-v8 store, synchronous `QueryService`, and one-result consumer ordering. Current/
legacy activity, exact publication and scan truth, stale accounting downgrade, stale
identity, indexed lookahead, deadline cleanup, concurrent-commit isolation, 100K
latency, Debug/privacy, 10,000-candidate retention, and 256-cycle Windows resource
contracts pass. No frontend owns SQLite. P2-B transactional storage, rebuild,
overview, series, breakdown, session store reads, calendar composition, immutable
aggregate/session facade values, and Task 8 scale/resource evidence are implemented.
P2-C pinned pricing and its release gates are implemented; the P2-D quota history
core and its acceptance gates are implemented. The permitted built-in Codex quota
normalizer/transport, exact-native discovery, and dedicated refresh/store publication
are implemented for the pinned version. Typed banked-reset values, pure
reconciliation/reminder planning, Codex normalization, strict schema-v12 storage,
immutable FEFO current/revision-bound history queries, Codex runtime benefit
publication, and one-timer crash-safe outbox/ack reminder delivery are implemented.
P2-E Git output Tasks 1-8 are implemented: bounded domain/parser/native inspection,
transient path-private activity hints, schema-v13 immutable projection/store capture,
and schema-v1 explicit-UTC public envelopes with aggregate-only exact project/cost
joining and typed graceful degradation, plus constant-state runtime publication,
pause/resume recovery, durable unavailable truth, resource evidence, and authority
closure.
Visible P3 notification rendering, OS/tray scheduling, snooze/quiet hours, and
activation remain separate.
The older replay plan remains historical evidence for
completed Tasks 1-2, but its Codex-owned Tasks 3+ are superseded and must not be
executed.
