# TokenMaster Critical Audit and Approved Master Plan

Status: architecture approved; P0-A through P1-A implemented; P1-B is the active gate.
Date: 2026-07-14.

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
    credential-free local rate-limit events when available plus opt-in direct HTTPS for
    fresher usage/reset-credit data. Secrets are never returned or stored; official
    origins are allowlisted; custom origins never receive credentials; requests have
    strict time/body bounds, backoff, jitter, auth-change invalidation, and stale
    fallback. Quota windows are provider data, never hard-coded `5h`/`1w` UI fields.
    Every full weekly reset, including an early or repeated reset, closes an immutable
    quota epoch and preserves exact available before/after ratios or units, maximum
    pre-reset use, old/new reset time, evidence/confidence, and allowance changes.
    The approved executable plan is
    `docs/superpowers/plans/2026-07-15-tokenmaster-quota-reset-history.md`.
12. Code Output needs its own bounded Git metrics subsystem. It must not expose a shell
    or retain file contents. It owns repository association, author filtering,
    worktree/submodule/rename/merge semantics, incremental cache, and explicit
    unavailable quality states.
13. Pricing uses an embedded pinned catalog with source/version provenance, explicit
    unknown-model state, and validated local overrides. No automatic network request
    runs in the hot path.
14. `docs/FEATURE_PARITY.md` is too broad to prove the requested parity. It must become
    a row-level matrix for WhereMyTokens and ccusage: pinned source, exact capability,
    implement/adapt/reject decision, TokenMaster improvement, requirement ID, plan,
    test, and status.

### E. UI, quality, and documentation

15. Slint software rendering does not support drop shadows. Production elevation is
    expressed through border tone, surface contrast, spacing, and optional solid
    offset layers; skin tokens may not require unsupported effects. Paint latency is
    measured at visible present, not at callback completion.
16. Traceability and status documents have drift. Every normative `TM-*` requirement
    gets a row even when planned; every ADR reference must exist; current-state,
    roadmap, README, changelog, and plan status must agree. A machine validator becomes
    a release gate.

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
8. **P3 — automation:** strict JSON CLI, separate stdio MCP process, capabilities,
   bounded queries, idempotent refresh, and declarative advisory policy for Hermes and
   other clients.
9. **P4 — complete desktop UI:** six-section board and all history/session/model/
   project/activity/health/settings/help/widget routes from immutable snapshots.
10. **P5 — presentation:** independent skin/layout/density/scheme/locale axes, dynamic
    quota bars, en/ru/pseudo, keyboard/accessibility, DPI, and visible-paint gates.
11. **P6 — release:** Windows integration, portability, dependency/security audit,
    SBOM, secret scan, package identity, interactive receipt, and soak receipt.
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

## 6. Current execution rail

P0-A through P0-E are complete under their executable plans in `docs/superpowers`.
P1-A is complete under
`docs/superpowers/plans/2026-07-14-tokenmaster-p1-retained-projection.md`; it adds
strict schema-v4 provenance and explicit bounded carry-forward. The immediate
P1-B.1 gate is implemented as strict schema v5 plus scoped complete-only scan
authority. The immediate implementation gate is P1-B.2 scan-bound replay, followed
by P1-B.3 bounded history/recovery, under
`docs/superpowers/plans/2026-07-15-tokenmaster-p1-b-scan-authority.md`. The older replay
plan remains historical evidence for completed Tasks 1-2, but its Codex-owned Tasks
3+ are superseded and must not be executed.
