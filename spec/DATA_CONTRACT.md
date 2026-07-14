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

A provider observation draft carries a zero-based session ordinal, optional bounded
parent session identity, explicit lineage-conflict marker, delta usage, and optional
cumulative usage. The TokenMaster canonicalizer, not the provider, derives the
fixed-size replay signature, evidence level, event fingerprint, and public event ID.
Replay identity is distinct from the event fingerprint and excludes timestamp, source
identity, display metadata, and activity.

## TM-DATA-003 — Source state and checkpoints

Source keys, physical/logical identity, fingerprints, anchors, and chunk hashes are
fixed-size path-private values. Checkpoints record complete-line committed and numeric
scan offsets. A partial replacement requires exact prior length and digest proof.

Adapter checkpoints are versioned, opaque outside their provider adapter, and capped
by the common checkpoint bound. They MUST NOT contain source content, credentials, or
unbounded transport state.

Codex resume v2 carries bounded lineage state and the next zero-based usage ordinal.
Resume v1 MUST fail closed and be rebuilt through a new non-destructive generation;
the ordinal MUST NOT be guessed. Late ancestry is a separate bounded session-relation
draft so it can reconcile prior observations without retaining source content.

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

## TM-DATA-007 — Replay classification

Every current observation has one replay disposition: `eligible`, `replay`, `pending`,
or `conflict`. Canonical selection uses only `eligible`. All observations remain
available for bounded reconciliation and quality counts. Session ancestry traversal
is capped at 32 levels and one transaction re-evaluates at most 256 direct children.

Only explicit provider ancestry identifies a parent. A strong signature covers the
normalized model, emitted delta, and provider cumulative snapshot. A weak signature
covers model and delta only and cannot suppress a pre-divergence event by itself.
Once a child diverges from a fixed parent relation, later events remain eligible.

The pure classifier validates matching provider/profile scope, declared parent
session, and equal child/parent ordinal before comparing signatures. Depth or direct
fanout exhaustion is `pending` and requires continuation; it is not evidence of a
cycle or contradictory relation.

If a child's ordinal is beyond the observed tail of a parent that has not been proved
complete, the child remains `pending`. Only completed scan/session evidence from the
staging runtime may prove that the child outgrew its parent.
