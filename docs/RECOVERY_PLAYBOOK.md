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
   evidence epoch. Run bounded continuation until no actionable work remains, then
   seal only after the complete SQLite-owned all-registered-source manifest has been
   proven. Product rebuilds use `begin_replay_revision_all_sources`; the explicit
   256-key manifest is test/repair-only and cannot authorize a subset.
3. A sealed revision with pending quality evidence is intentionally not promotable.
   Preserve it for bounded `replay_quality()` inspection or explicitly call
   `discard_replay_revision` with its exact revision ID and latest epoch.
4. Discard is the only supported retry reset. It removes the staging revision and all
   staging generations in one immediate transaction, validates foreign keys, and
   leaves current events, current source pointers, and the immutable legacy snapshot
   unchanged. A stale epoch or integrity fault writes nothing.
5. Never delete SQLite rows, generations, the legacy snapshot, or the archive file to
   recover a rebuild. P0-E must expose this store operation through bounded runtime
   policy; no user-facing recovery command exists yet.

## Schema recovery

- Opening an exact schema-v2 archive performs the non-destructive v3 revision-table
  migration automatically. Preserve the original archive and reproduce any failure
  against a synthetic copy; do not edit `sqlite_schema`, rename tables manually, or
  disable foreign keys in an operator workflow.
- Migration validates exact v2 before mutation, restores `foreign_keys=ON` after each
  tested success/failure boundary, and rolls back create/copy/drop faults. A migration
  error is fail-closed; never delete the database to bypass it.

Generated `target/`, `reports/`, and `dist/` content is disposable developer output.
Do not use it as a release claim. M0 acceptance requires the exact external receipts
listed in `M0_ACCEPTANCE.md`.
