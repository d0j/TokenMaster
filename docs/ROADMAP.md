# TokenMaster roadmap

## M0 — native architecture proof

Implemented: Rust/Slint/SQLite baseline, tray lifecycle, layouts, skins,
English/Russian localization, accessibility-aware presentation contracts, resource
gates, and developer stress harness.

Open: interactive Windows 10/11, keyboard/screen-reader, 100/150/200% DPI,
mixed-monitor, Explorer restart, sleep/resume, measured input-to-paint, and a clean
uninterrupted 24-hour software soak. These block M0 acceptance.

## M1 — bounded Codex archive

Completed: provider discovery, safe streaming enumeration, parser and cumulative
accounting, physical/logical identities, reader framing/checkpoints/revalidation,
strict SQLite archive, keyset reads, and atomic current-generation append.

Completed: P0-A provider-neutral observation drafts and exclusive accounting
authority; P0-B Codex ancestry compatibility, late relation emission, resume v2,
ordinals, and cumulative facts; P0-C allocation-free bounded replay classification.

Completed: P0-D strict v2 replay archive, immutable v1 fallback, invisible fixed
staging, bounded restart-safe reconciliation, exact seal, rollback-safe atomic
promotion, and staging recovery.

Completed: P0-D.1 removes the historical 256-file replay-manifest blocker with exact
schema-v2-to-v3 migration, disk-backed all-source staging, checked 64-bit counts, and
256-row keyset-paged validation.

Completed: P0-E exercises discovery, enumeration, reader, accounting, archive epochs,
continuation/seal/promotion, restart, cancellation, append, truncation, and Windows
atomic replacement beyond 256 files/events without introducing a live engine. It also
adds only bounded path-private restart reads and exact-epoch preparation of untouched
staging.

Completed: P1-A upgrades the canonical projection to strict schema v4 with exact
v1/v2/v3 migration and explicit prior-evidence carry-forward. Complete promotion
selects eligible events, suppresses replay-only contributions, retains absent/conflict
history, removes obsolete generations, and rolls back provenance atomically.

Completed: P1-B.1 adds strict schema v5, provider-qualified scan sets, exact
complete-only presence finalization, late-registration safety, and transactional
rollback proofs.

Completed: P1-B.2 persists exact scan-set provenance, stages only present members,
revalidates membership at continuation/seal/promotion, preserves missing generations,
supports zero-source retention-only publication after reopen, and moves the real
synthetic Codex pipeline onto this path.

Completed: P1-B.3 keeps the newest 32 closed sets per scope, prunes at most 64 whole
unreferenced sets per transaction, preserves running/source/replay references, recovers
older backlogs in bounded passes, and fails atomically on pruning or ID exhaustion.
P1-C is the completed provider-neutral runtime engine core. P1-D.0 has corrected its
real multi-file boundary with fixed logical-file identity and two linear streaming
passes with one temporary descriptor-bound reader. P1-D.1 atomically binds events,
late relations, checkpoint, replay work, and one epoch advance. P1-D.2 adds the real
built-in Codex bootstrap/store composition and strict path-free checkpoint codec.
P1-D.3+ and P1-E next add incremental live composition, the real writer lease,
sleep/resume, continuous recovery, and
immutable publication before indexed
analytics, pricing, quota, Git output, automation, and complete UI work.

Completed P1-C.1: a no-async, constant-state coordinator with checked monotonic IDs,
deadline/cancellation semantics, one active refresh, and one aggregate follow-up.
Completed P1-C.2: sealed bounded provider-neutral values, scope-exact adapter/canonical
batches, and object-safe adapter/archive/clock/lease ports with compile-fail privacy
boundaries. Completed P1-C.3: one truthful lease-first execution streams discovery,
closes scan outcomes, replays/canonicalizes bounded batches, validates exact handle
progress, and seals/promotes or discards the unpublished revision under full phase
cancellation/deadline coverage. Completed P1-C.4: one owned deterministic worker uses
capacity-one wake/latest-result channels, constant-state burst coalescing, checked
supersession, cooperative cancel/wake/join shutdown and `Drop`, stale-ID safety,
pre-execution deadline/cancellation, and redacted panic/fault containment. Completed
P1-D.0: source identity is exact per logical file, archive page/cursor descriptor
recovery is removed, and 300 same-root files rebuild with a maximum of one live reader
and no engine descriptor collection. Completed P1-D.1: a 256+256 bounded replay fact
batch applies events and relations in one immediate transaction, rolls every fact and
checkpoint/epoch back at both injected boundaries, and advances epoch once. Completed
P1-D.2: the production runtime composes the Codex adapter and store for bounded
bootstrap/full rebuild with a strict 32-KiB checkpoint, checked ID bridge, real
300-file/reopen/replacement/cleanup evidence. The current gate is P1-D.3 replay-aware
incremental archive, then the real portable writer lease,
incremental tail path, watcher/periodic hints, and lifecycle cancellation without
adding provider/platform/UI dependencies to the engine core.

Approved P2 quota gate: provider-defined current windows plus immutable full-reset
epochs. The weekly view preserves last-before/first-after state, maximum use before
reset, old/new reset time, early/repeated reset markers, confidence, and simultaneous
allowance changes. Ratios remain exact when absolute capacity is unavailable. See
`docs/superpowers/plans/2026-07-15-tokenmaster-quota-reset-history.md`.

The same P2 gate keeps banked rate-limit reset benefits separate from quota epochs and
credits. It adds independently expiring inventory lots, an initial
7-day/24-hour/12-hour/6-hour/1-hour reminder profile that users can subset or replace
with bounded custom thresholds, truthful notification coverage, immutable activation
receipts, and an
official-capability-only path to future automatic activation. Manual inventory may
ship first and never authorizes mutation. See
`docs/superpowers/plans/2026-07-15-tokenmaster-banked-reset-inventory.md`.

## 0.9 — complete desktop product

Complete the six-section board and supporting exploration views, tray/hotkey/startup,
compact widget, notifications, settings, modular skins, en/ru, and migration tools.

## 1.0 — release candidate and stable

Require privacy/dependency review, deterministic package rehearsal, complete evidence,
performance reference runs, 72-hour soak, signed-build rehearsal, and the full
interactive Windows matrix before a stable claim.

## 1.1 — provider plugin ecosystem

After the observation, engine, query, quota, and quality contracts are stable, deliver
the versioned WIT API, deterministic `.tmplugin` package, isolated on-demand Wasmtime
host, capability grants, hot installation/update/rollback, quarantine, signatures,
Rust/TypeScript SDK templates, conformance kit, and plugin-specific resource/security
gates. Codex remains built in and pays no plugin runtime cost.
