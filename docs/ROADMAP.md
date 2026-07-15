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

Current gate: P1-B adds scan epochs and complete-source-set finalization. P1-C through
P1-E then add the provider-neutral runtime engine, coalescing, cancellation, writer
lease, sleep/resume, continuous recovery, and immutable publication before indexed
analytics, pricing, quota, Git output, automation, and complete UI work.

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
