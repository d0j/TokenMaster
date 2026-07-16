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
P1-D.3 adds strict schema-v6 publication generations, exact scan freshness and new
source admission, paired-CAS replay-aware tail append, targeted materialization,
bounded partial restart, and durable rebuild-required state. P1-D.4 adds the persistent
empty-sidecar `File::try_lock` writer lease with process-death release. P1-D.5 adds the
fixed pathless hint aggregate, capacity-one scheduler, exact quiet/periodic policy, and
bounded watcher generations. P1-D.6 completes lease-first startup recovery and live
worker/scheduler/watcher lifecycle assembly. P1-E.1 now adds startup-seeded immutable
publication with strict in-process/archive generation ordering, exact revision/scan/
data-through truth, fixed diagnostics, and busy/older-result rejection. Remaining P1-E
work now closes with a Windows 8+ static capacity-one power callback, forced resume
reconciliation, and 4,096-cycle private-memory/handle/thread/USER/GDI bounds. P2
indexed analytics, pricing, and the provider-neutral quota history core are complete.
The permitted built-in Codex quota normalizer and short-lived official app-server
transport, exact-native executable discovery, and dedicated quota refresh
scheduling/writer coordination are complete for the pinned supported version.
Banked-reset inventory/reminders are complete through their read-only authority and
project-truth gates. Git output is next, followed by complete UI and automation work.

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
300-file/reopen/replacement/cleanup evidence. Completed P1-D.3: schema v6 and the
runtime now publish exact freshness, admit new sources, preflight identity, append only
tail bytes with paired CAS, resume bounded partial work, and preserve old truth behind
durable `recovery_pending` on replacement, truncation, or profile-scope changes; a
full rebuild safely recovers provisional admission state. P1-D.4 adds the persistent
empty-sidecar writer lease with same-process/cross-process contention and death-release
proof. P1-D.5 adds exact `notify = 8.2.0`, a fixed atomic pathless aggregate, one
scheduler thread, deterministic quiet/healthy/degraded policy, bounded root
generations, and shutdown resource evidence. P1-D.6 adds exact startup recovery,
incremental/rebuild selection, admission-safe pause/resume, ordered joined shutdown,
partial/reopen evidence, and combined Windows resource return without adding provider/
platform/UI dependencies to the engine core. P1-E.1 immutable engine publication,
P1-E.2 race/recovery/restart closure, and P1-E.3 isolated Windows power binding plus
resource evidence are complete. The current gate is P2 immutable indexed query
snapshots; interactive hibernation/soak stay in the frozen-candidate M0 gate.

## Approved implementation rail

- **P1 — runtime publication (complete):** immutable generation-ordered snapshots,
  suspend/resume integration, race/failure recovery, and bounded-resource evidence.
- **P2 — product data:** indexed query snapshots, analytics, pinned pricing and
  overrides, dynamic quotas, full-reset epochs, banked-reset inventory/reminders, and
  bounded Git output metrics.
- **P3 — complete desktop UI:** quota-first board, history, sessions, models, projects,
  activity, health, settings, help, command palette, tray, and compact widget.
- **P4 — presentation:** modular skins/layouts/density/scheme/locale, en/ru/pseudo,
  accessibility, DPI, reduced motion, and visible-paint/resource gates.
- **P5 — automation:** strict bounded JSON CLI and a separate stdio MCP process for
  Hermes and other clients after the complete UI; read-only/advisory by default with
  no provider-mutation authority.
- **P6 — release:** Windows integration, explicit GNU/MSVC comparison, canonical
  signed MSVC portable package, license/supply-chain evidence, interactive matrix, and
  final soak receipts.
- **1.1 — providers:** isolated signed WebAssembly Component packages after 1.0
  observation/query/quota contracts freeze.

P2-A query foundation is approved under
`docs/superpowers/plans/2026-07-16-tokenmaster-p2-query-foundation.md`. It first freezes
two-dimensional publication/dataset identity (with replay revision plus schema-v7
dataset generation inside current dataset identity), bounded immutable values, a dedicated
query-only SQLite connection, exact short transactions, keyset paging, and deadline/
privacy/resource gates. Materialized aggregates remain P2-B and are not replaced by
view-time full scans.
P2-A and its audited schema-v7 dataset-generation correction are complete:
`tokenmaster-query` owns schema-v1 bounded immutable values, checked
identity, truthful freshness/quality, strict consumer ordering and the synchronous
facade, while `UsageReadStore` supplies isolated defensive read-only/query-only exact
transaction capture with composite keyset/deadline evidence. The 100K activity-page
latency and repeated resource-return contracts pass. P2-B schema-v8 provider identity,
transactional materialized rollups, availability counts, and bounded resumable rebuild
are now implemented. Fixed overview/series/breakdown and opaque keyset session
page/detail reads are implemented. Calendar/timezone composition, immutable public
values, scope-bound opaque session mapping, and exact facade capture are implemented.
Deterministic current/legacy million-event cached-dashboard, rebuild, storage,
privacy, and repeated-resource evidence passes; P2-B is complete. P2-C is also
complete: schema-v9 fact-only price rollups, release-pinned fixed-point calculation,
validated immutable overrides, dataset-exact cost on overview/series/breakdown/session
surfaces, batched indexed reads, current/legacy million-event scale, resource plateaus,
  and production no-pricing-network audit pass. P2-D Tasks 1-8 exact fixed-point quota
  domain values, deterministic identities, pure reset/allowance evaluation, strict
  schema v10, exact rollback-safe v9 migration, one-transaction immutable quota
  publication, evidence-preserving bounded retention, defensive reads, and the
  independent immutable public quota facade are complete. Retention uses
  512/256 soft defaults, 2,048/1,024 hard caps, a 256-row same-window maintenance
  page, and protected first/current/max/pre/post evidence with fail-closed over-cap
  write/reopen behavior. Defensive store reads now provide exact 32-window current
  snapshots and revision-bound descending 256+1 keyset transition pages with owned
  boundary values, fixed quota-only index seeks, total deadline cleanup, and
  post-open projection-drift rejection. Current query results preserve request order,
  explicit unavailability, provider-defined freshness, worst truthful quality, opaque
  revision/filter continuation, and failed-call generation neutrality. The final gate
  covers 32 windows, 1,000 transitions, 10,000 duplicate polls, restart, 256-row
  paging, maintenance, current/legacy coexistence, Windows resource return, and a
  zero-match offline authority audit. The built-in Codex connector now adds strict
  official app-server `0.144.1` normalization/transport, exact fixed argv/protocol,
  bounded process I/O, live two-window evidence, adversarial rejection, repeated
  success/error/timeout resource return, and a zero-match release authority audit.
  Exact-native executable discovery and the separate quota scheduler/worker,
  I/O-before-lease store publication, count-only health, lifecycle, resource, and
  release-authority gates are now complete. Benefit Tasks 1-8 add provider-neutral
  lots, pure expiry/reconciliation/reminder planning, privacy-safe built-in Codex
  normalization, strict schema-v12 write/retention/outbox acknowledgement, and
  immutable FEFO current plus
  revision-bound history snapshots, plus one-poll/one-lease/one-open publication
  through the existing Codex quota runtime with separate quota/benefit transactions
  and health. Task 7 adds the store-owned 256-row due operation and one-timer durable
  in-app event runtime with outbox-before-publication, pre-ack restart replay,
  post-ack deduplication, release/retry, hibernation/clock reconciliation, bounded
  backpressure, fault isolation, and resource/authority gates. Task 8 closes
  dependency/language/privacy authority,
  project truth, and the complete workspace quality gate. P3 visible notification
  delivery, later independently
  authorized activation, P2-E Git output, and P2-F joined product status remain.
No frontend/database coupling or view-time full event grouping is accepted.

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
Every row in `docs/FEATURE_PARITY.md` must be implemented or explicitly rejected under
its normative rationale before a parity claim.

## 1.0 — release candidate and stable

Require privacy/dependency review, deterministic package rehearsal, complete evidence,
performance reference runs, 72-hour soak, and the full interactive Windows matrix
before a stable claim. The canonical artifact is a signed
`x86_64-pc-windows-msvc` portable ZIP with Slint Royalty-free License 2.0 attribution,
third-party notices, SBOM, advisory/source/license/secret scans, immutable CI action
references, artifact attestation, SHA-256 manifest, deterministic content audit, and
clean-room launch. The current GNU lane remains development/M0 evidence until the P6
dual-lane comparison passes. No updater or installer ships in 1.0.

## 1.1 — provider plugin ecosystem

After the observation, engine, query, quota, and quality contracts are stable, deliver
the versioned WIT API, deterministic `.tmplugin` package, isolated on-demand Wasmtime
host, capability grants, hot installation/update/rollback, quarantine, signatures,
Rust/TypeScript SDK templates, conformance kit, and plugin-specific resource/security
gates. Codex remains built in and pays no plugin runtime cost.
