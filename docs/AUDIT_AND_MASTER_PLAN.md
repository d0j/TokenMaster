# TokenMaster Critical Audit and Approved Master Plan

Status: architecture and release plan approved after closure review; P0 through P1-D
implemented; P1-E is active.
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

### E. UI, quality, and documentation

15. Slint software rendering does not support drop shadows. Production elevation is
    expressed through border tone, surface contrast, spacing, and optional solid
    offset layers; skin tokens may not require unsupported effects. Paint latency is
    measured at visible present, not at callback completion.
16. Traceability and status documents have drift. Every normative `TM-*` requirement
    gets a row even when planned; every ADR reference must exist; current-state,
    roadmap, README, changelog, and plan status must agree. A machine validator becomes
    a release gate.

### F. Release and supply-chain closure

17. The workspace-global GNU target is a development constraint, not a signed-release
    decision. The canonical 1.0 artifact is `x86_64-pc-windows-msvc`; P6 removes the
    forced global target and uses an explicit GNU/MSVC comparison before release.
18. The 1.0 package is a signed portable ZIP. Automatic update and installer behavior
    are deferred until signed-manifest, interrupted-update, rollback, and downgrade
    contracts exist.
19. The Slint Royalty-free License 2.0 route is selected with attribution in Help/About
    and on the public download page. Dependency notices, license policy, and SBOM are
    package gates.
20. Pricing is a release-pinned embedded catalog plus bounded validated overrides; no
    hot-path network update is allowed and unknown model cost stays unknown.
21. P6 requires advisory, dependency/source/license, secret, SBOM, immutable-action,
    artifact-attestation, deterministic-content, and clean-room-launch evidence.
22. The complete closure rationale and self-review are recorded in
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
7. **P2 — product data:** Codex quota transport, immutable weekly reset epochs and
   before/after history, pricing catalog/overrides, bounded Git output metrics,
   indexed analytics, and data-quality semantics.
8. **P3 — complete desktop UI:** six-section board and all history/session/model/
   project/activity/health/settings/help/widget routes from immutable snapshots.
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
pause/resume, and ordered joined shutdown. The immediate next gate is P1-E immutable
query/publication snapshots plus sleep/resume and race-generation integration.
The older replay plan remains historical evidence for
completed Tasks 1-2, but its Codex-owned Tasks 3+ are superseded and must not be
executed.
