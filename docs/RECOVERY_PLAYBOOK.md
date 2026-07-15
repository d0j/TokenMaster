# TokenMaster recovery playbook

1. Confirm the current branch and clean worktree with `git status --short`.
2. Read `AGENTS.md`, the contracts in `spec/`, and `docs/HANDOFF.md`.
3. Run `pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path`.
4. Run the focused test for the affected crate before changing behavior.
5. If source data or SQLite state is involved, preserve user data and reproduce with a
   synthetic fixture; never persist or attach private JSONL content.
6. Run the workspace gate before updating handoff documents.

## Replay staging recovery

1. Read `archive_state()` first. Canonical reads continue from the current compatible
   or stale replay revision, immutable legacy snapshot, or empty state; they never
   read the staging overlay.
2. For an unsealed staging revision, resume only with its exact revision ID and
   evidence epoch. Before its first append, prepare each untouched pending source with
   its validated zero-offset adapter checkpoint; this binds the live path-private
   physical identity and a valid bounded resume payload without touching current.
   After any append/reopen, recover only through `replay_generation_snapshot` and
   fetch full-prefix proofs one chunk at a time through `source_chunk`. Run bounded
   continuation until no actionable work remains, then seal only after the persisted
   complete scan-set ID and exact present membership are revalidated. Product rebuilds
   use `begin_replay_revision_for_scan_set`; `begin_replay_revision_all_sources` and
   the explicit 256-key manifest remain bounded composition/test/repair compatibility.
   If a later scan changes membership, discard the stale revision and restart from
   the newer complete set. A zero-source bound revision has no source checkpoint work
   and may seal/promote retention-only without changing missing-source generations.
3. A sealed revision with pending quality evidence is intentionally not promotable.
   Preserve it for bounded `replay_quality()` inspection or explicitly call
   `discard_replay_revision` with its exact revision ID and latest epoch.
4. Discard is the only supported retry reset. It removes the staging revision and all
   staging generations in one immediate transaction, validates foreign keys, and
   leaves current events, current source pointers, and the immutable legacy snapshot
   unchanged. A stale epoch or integrity fault writes nothing.
5. Never treat `Truncated`, `IdentityChanged`, missing, or rewrite detection as
   permission to erase prior visible usage. A complete sealed revision may promote
   through P1-A: eligible replaces, replay-only suppresses, and absent/conflict-only
   replay-verified events carry with older origin provenance. Partial, cancelled,
   pending, stale, or invalid staging must still be discarded or resumed by exact
   epoch; it cannot invoke carry-forward.
6. Never delete SQLite rows, generations, the legacy snapshot, or the archive file to
   recover a rebuild. P0-E proves exact discard through a test-only driver; no
   user-facing recovery command exists yet.

## Scan-set recovery

1. Read `running_scan_set()` after reopen. Resume only that exact set and its exact
   provider/profile child scans; never synthesize replacement IDs or treat append
   activity as observation.
2. A running child may accept idempotent observations only for registered sources in
   its own scope. Finish it with the truthful explicit outcome. Only `complete`
   finalizes presence; every other outcome preserves the prior `missing` values.
3. Close the parent only after every child is terminal. A mixed parent truthfully
   aggregates to failed, timed out, cancelled, or partial and cannot authorize the
   production replay path. Do not edit scan or source rows manually.
4. Parent close automatically performs one reference-safe pruning batch. For an older
   backlog, call the internal `prune_scan_history_batch()` repeatedly until it returns
   zero. Each call removes at most 64 whole closed sets only when every child scope has
   32 newer closed sets. Running sets and source/replay references are preserved. A
   failure writes nothing; never replace this operation with ad hoc SQL.

## Schema recovery

- Opening an exact schema-v1, v2, v3, or v4 archive performs the non-destructive
  schema-v5 migration automatically. Preserve the original archive and reproduce any failure
  against a synthetic copy; do not edit `sqlite_schema`, rename tables manually, or
  disable foreign keys in an operator workflow.
- Migration validates the exact source schema before mutation. V2 revision migration
  restores `foreign_keys=ON` after every outcome; v3 canonical projection and v4
  scan-set migrations run in immediate transactions. Create/copy/drop faults roll
  back to the exact prior schema and logical rows. Ambiguous migrated scan ownership
  or incoherent terminal state fails closed. Never delete the database to bypass a
  migration error.

Generated `target/`, `reports/`, and `dist/` content is disposable developer output.
Do not use it as a release claim. M0 acceptance requires the exact external receipts
listed in `M0_ACCEPTANCE.md`.
