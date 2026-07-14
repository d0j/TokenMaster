# TokenMaster data contract

## TM-DATA-001 — Private content prohibition

The archive, settings, diagnostics, UI snapshots, CLI, MCP, backups, logs, reports,
and release artifacts MUST NOT persist or expose prompts, responses, reasoning,
commands, command output, file contents, OAuth material, API keys, or raw incomplete
JSONL tails.

## TM-DATA-002 — Canonical usage event

A canonical event contains only bounded profile/session/source identities, UTC time,
bounded model metadata, explicit token availability and values, service tier, bounded
project/activity metadata, and a full deterministic fingerprint. The shortened public
event ID is not a uniqueness key.

## TM-DATA-003 — Source state and checkpoints

Source keys, physical/logical identity, fingerprints, anchors, and chunk hashes are
fixed-size path-private values. Checkpoints record complete-line committed and numeric
scan offsets. A partial replacement requires exact prior length and digest proof.

## TM-DATA-004 — Current and staging generations

Current-generation append writes observations, canonical selections, chunk coverage,
checkpoint, and source metadata in one transaction. A stale generation, identity,
offset, scan position, or partial proof MUST write nothing.

Future staging generations MUST remain invisible to canonical reads. Promotion MUST
verify complete staged input, replace the affected canonical selections atomically, and
leave the previous current generation intact on failure.

## TM-DATA-005 — SQLite policy

The usage archive has a strict versioned schema. File-backed connections MUST use WAL,
FULL synchronous writes, foreign keys, a bounded busy timeout, bounded journal/cache
policy, and disabled mmap. Collections are keyset-paged at no more than 256 rows.

## TM-DATA-006 — Bounds

Reader lines are limited to 16 MiB. Resume metadata is capped at 32 KiB. General
display metadata is UTF-8 bounded; tool names, collection counts, profile roots,
source directories, and UI snapshots have explicit contract limits.
