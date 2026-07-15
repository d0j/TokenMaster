# TokenMaster P1-B Scan Authority Implementation Plan

**Status:** Implemented and fully verified on 2026-07-15. Tasks 1-6 pass. This plan
does not claim a live scheduler, M0 acceptance, packaging, or a product release.

> **Execution mode:** root-only, test-first, one writer, current feature branch. The
> available task-name-only child surface cannot prove requested model routing, so
> implementation and verification remain with root and `MODEL_ROUTING_DRIFT` is
> reported explicitly.

## Goal

Add the store-owned source-discovery authority required by the live engine: exact
provider/profile scan scopes, complete-only missing-source finalization, bounded scan
history, and replay revisions bound to one complete scan set. Preserve all P0 and
P1-A archive truth, privacy, rollback, and memory bounds.

## Progress evidence

- Tasks 1-2: strict schema v5, exact v1-v4 migration, populated scan/source/replay
  preservation, and every v4 create/copy/drop rollback boundary pass.
- Task 3: provider-qualified lifecycle, idempotent observation, complete-only
  presence, partial preservation, reopen, post-scan registration safety, and injected
  parent/finalization rollback pass.
- Task 4: multi-provider exact membership, partial/stale rejection, persisted binding,
  continuation/seal/promotion revalidation, two begin fault boundaries, zero-source
  reopen/retention promotion, and all prior promotion faults pass.
- Task 5 composition: all seven real synthetic Codex contracts use complete scan-bound
  replay; cancelled enumeration closes partial and leaves no running authority.
- Task 5 retention: closing a set keeps 32 closed sets per child scope and prunes at
  most 64 whole unreferenced sets per transaction. Repeated scans plateau; bounded
  backlog passes preserve running/source/replay references; parent/child ID exhaustion
  and a post-prune fault roll back without partial state.
- Task 6: contracts, decisions, traceability, state, roadmap, recovery, handoff,
  changelog, and history are synchronized; the clean-root, format, strict Clippy, and
  full locked workspace gates pass.

## Corrected architecture

The earlier single `profile_id`/single-scan design is insufficient. Profile IDs are
not globally unique across future providers, while a replay revision is archive-wide
and may contain several provider/profile scopes. P1-B therefore uses:

- `ScanScope = (provider_id, profile_id)`;
- one `ScanSetId` for a fixed duplicate-free bounded scope manifest;
- one typed `ScanId` child per scope;
- `usage_source.last_seen_scan_id` as the bounded exact seen set;
- an optional `scan_set_id` on migrated/unbound historical replay revisions and a
  mandatory exact binding for the production scan-bound begin path.

No `(scan, source)` history table is retained. A source stores one last-seen pointer;
unreferenced closed scan rows are pruned to a fixed recent window. This avoids memory
or database growth proportional to refresh count.

## Non-goals

- no scheduler, filesystem watcher, async runtime, OS writer lease, UI, CLI, or MCP;
- no external provider execution or plugin ABI change;
- no path, prompt, response, command, source content, raw line, or credential storage;
- no deletion of canonical usage because a source is absent;
- no M0/release/package claim.

## Invariants

1. IDs and counters fit SQLite's non-negative signed range; arithmetic is checked.
2. Scope IDs use the existing bounded ASCII provider/profile grammar.
3. At most one scan set is running, and at most one child scan is running per scope.
4. Observation is idempotent only for the exact running scan and matching source
   scope; closed, foreign, unknown, or stale IDs fail without mutation.
5. The store derives distinct `sources_seen`; callers provide only files/bytes/events/
   diagnostic counters.
6. Only a complete child scan updates `missing`: seen -> present, unseen -> missing.
   Other outcomes preserve the prior missing state exactly.
7. A scan set is complete only if every child is complete. Only a complete set may
   start a scan-bound replay revision.
8. A bound replay manifest is the exact present source set owned by that scan set and
   is revalidated at seal/promotion. Missing sources keep their old current generation
   and their prior events are handled only by P1-A retention.
9. A complete scan set with zero present sources is valid and can publish a
   retention-only revision.
10. Migration, finalization, replay begin/seal, pruning, and injected faults are
    transactional and leave the prior readable state unchanged on failure.

## Task 1 — contract-first types and schema-v5 RED

Add failing contracts for `ScanScope`, `ScanSetManifest`, `ScanSetId`, `ScanId`,
`ScanOutcome`, `ScanCounters`, child/set snapshots, strict schema objects, indexes,
and exact v4-to-v5 migration. Prove duplicate/empty/oversized scopes, private Debug,
counter overflow, newer/malformed schema rejection, populated scan/source preservation,
and rollback at each migration mutation boundary.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
```

## Task 2 — schema v5 and non-destructive migration GREEN

Add `usage_scan_set`; rebuild `usage_scan` with `scan_set_id`, `provider_id`, terminal
state coherence, and strict foreign keys; add the exact running-set/running-scope and
scope-missing indexes; add nullable `scan_set_id` provenance to historical replay
revisions. Migrate exact v1-v4 archives to v5 without changing event, generation,
legacy, replay, or last-seen logical data. Restore foreign-key policy on every path.

Validator: Task 1 plus migration unit fault tests.

## Task 3 — scan lifecycle TDD

Add `begin_scan_set`, bounded child lookup/page access, `observe_scan_source`,
`finish_scan`, `finish_scan_set`, running-set recovery lookup, and immutable snapshots.
Remove scan authority from `AppendBatchParts`; ordinary append may update checkpoint
and diagnostics only.

Contracts cover:

- multiple providers sharing the same profile ID;
- exact scope mismatch/unknown/closed/stale rejection;
- duplicate observation idempotence and store-derived counts;
- complete seen/unseen finalization and later restoration;
- partial/cancelled/failed/timed-out non-finalization;
- reopen with a running set and exact completion;
- transaction rollback after seen-state and missing-state mutations.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test scan_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
```

## Task 4 — scan-bound replay TDD

Add a production `begin_replay_revision_for_scan_set` path. It accepts only one exact
complete set, stages exactly its present sources with set-based SQL, stores the set ID,
and supports zero sources. Seal and promotion revalidate set ownership, completion,
membership, counts, and foreign keys. Keep unbound manifest/all-source APIs only as
bounded composition/test compatibility until P1-C moves all production callers.

Contracts cover multi-scope membership, omitted/extra/stale sources, partial set
rejection, source disappearing between begin/seal, missing-generation preservation,
zero-source retention-only promotion, reopen, and every existing promotion fault.

Validator:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
```

## Task 5 — bounded history, recovery, and integration

Prune only closed unreferenced child/set rows beyond the fixed per-scope window.
Prove row bounds across repeated complete/partial scans, retained last-seen references,
running-set recovery after reopen, checked ID exhaustion, no history-sized Rust
collection, and unchanged current canonical truth under all failures. Update the real
synthetic Codex driver to create and consume an exact scan set without adding a
production store dependency to the Codex adapter crate.

## Task 6 — documentation and acceptance

Update data/security/API contracts, decisions, traceability, current state, roadmap,
recovery, handoff, changelog, and project history. Run focused tests first, then:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Perform dependency-direction, forbidden-storage, secret, actual-private-path, tracked
legacy-language, and task-owned process audits. Commit and push coherent reviewable
milestones; do not claim P1-B complete until every task and gate passes.
