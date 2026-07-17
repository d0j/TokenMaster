# TokenMaster product specification

This is the primary normative product contract. MUST and MUST NOT are binding.
TokenMaster is Windows-first, portable, local-first, and implemented as one Rust
workspace.

## Product goal

TokenMaster MUST provide a fast, responsive, stable usage monitor with the complete
WhereMyTokens-class information architecture and ccusage-class usage analysis while
improving bounded memory, long-run stability, privacy, and native desktop behavior.

## Functional requirements

### TM-FUNC-001 — Codex source discovery

The product MUST discover bounded configured Codex roots and distinguish active,
archived, and direct sources without following reparse points. Source identities MUST
remain profile-scoped and path-private outside the reader boundary.

### TM-FUNC-002 — Incremental history archive

The product MUST read complete JSONL lines from durable checkpoints, reread incomplete
tails, and classify append, truncate, rewrite, and replacement without double-counting.
The fast append path MUST NOT rescan an entire history.

### TM-FUNC-003 — Usage and cost semantics

The product MUST expose explicit available/unavailable token components, cumulative
deltas, model normalization, sessions, projects, activity, service tier, and estimated
API-equivalent cost. Missing values MUST remain explicit and never become fabricated
zeroes. Session timeline and detail MUST use all-time materialized session facts;
period analytics MUST NOT relabel whole-session totals as period-clipped values.
Session lookup keys MUST bind to one exact dataset and MUST NOT expose raw source
session identity.
Estimated cost MUST use release-pinned fixed-point rates or explicit source-reported
values with mode, availability, provenance, catalog/override identity, conflicts, and
bounded missing reasons. Unknown pricing, tier, context, token basis, or truncated
detail MUST NOT become zero. Overview, series, breakdown, session page, and session
detail cost MUST use indexed bounded facts from the same exact dataset as their token
metrics and MUST NOT issue one raw-history or per-visible-item query.

### TM-FUNC-004 — Complete desktop product

The product MUST provide the six-section quota-first board and supporting history,
sessions, models, projects, activity, data health, notifications, settings, agent
help, command palette, and compact-widget views. Users MUST be able to reorder, hide,
and collapse board sections without data loss.

### TM-FUNC-009 — Quota reset history

Provider quota windows MUST be versioned as immutable epochs. A detected full weekly
reset MUST preserve the last trustworthy state before reset, the first state after,
maximum use in the closed epoch, old/new reset times, evidence source, confidence, and
scheduled/early/unknown semantics. Repeated resets MUST create distinct transitions
instead of overwriting history. Unavailable absolute limits MUST remain unavailable;
local token totals MUST NOT be presented as provider quota capacity.

### TM-FUNC-010 — Banked reset inventory and expiry safety

Provider-granted banked rate-limit resets MUST be represented separately from normal
quota resets, credits, and temporary usage. Separate expirations MUST remain separate
lots with explicit precision, source, freshness, and state. TokenMaster MUST initialize
reminders at 7 days, 24 hours, 12 hours, 6 hours, and 1 hour, while allowing users to
select any subset or replace it with up to eight unique bounded custom lead times.
Reminder profiles MUST remain bounded, deduplicated, and independently configurable.
TokenMaster MUST expose truthful notification coverage and linked activation receipts.
Automatic activation MUST default off and MUST be unavailable without an
official narrow idempotent provider capability, fresh high-confidence evidence,
explicit local policy, compare-and-swap admission, durable intent, and post-action
reconciliation. Manual inventory and read-only automation MUST NOT authorize it.

### TM-FUNC-011 — Bounded Git output analytics

The product MUST expose bounded local Git output facts for the active author:
additions, deletions, net lines, commits, merge commits, and versioned product-code
categories over exact daily ranges. Repository discovery MUST begin only from a
validated transient provider activity hint or an explicit local user selection.
Absolute paths, author email, refs, commit IDs/messages, file names/content, and raw
Git output MUST NOT enter the archive or public snapshots.

Git inspection MUST use an exact native executable and fixed read-only commands with
shells, hooks, paging, editors, credentials, network access, external diff, textconv,
and repository mutation disabled. Merge, rename, binary, gitlink, worktree, mailmap,
shallow-history, history-change, partial, and unavailable semantics MUST remain
explicit. Missing or ambiguous evidence MUST NOT become zero or complete quality.
Persistent projections and queries MUST remain bounded to 32 repositories and 400
daily points per snapshot.
Git daily buckets are UTC calendar days and every public Git range MUST label that
boundary explicitly; the facade MUST NOT relabel a UTC bucket as a local civil day.

The durable projection MUST use immutable aggregate generations and one independent
monotonic Git publication revision. Rebuild, proven same-process append, unchanged
refresh, and rebuild-required invalidation MUST publish transactionally; any stale
authority or failed write MUST preserve the prior generation. All-time totals remain
exact when the latest 400 daily points omit older days, but the range MUST then be
marked partial with an explicit retention boundary. Repository and project selection
MUST use salted opaque identities; conflicting or missing project associations MUST
disable the efficiency join instead of selecting one silently. Reads MUST use a hard
caller deadline of at most two seconds and one-row lookahead when a repository limit
can omit results.

Git population MUST run on one dedicated constant-state scheduler/worker pair with at
most 32 latest in-memory repository candidates, one active scan, and one aggregate
follow-up. Every native Git child MUST exit and be reaped before a non-waiting writer
lease and SQLite open. A superseded scan MUST NOT publish; a failed known repository
MUST publish explicit unavailable or rebuild-required truth without erasing its last
trustworthy generation. Pause MUST close admission, cancel/reap the exact active child,
and discard raw object-ID frontiers; resume and power recovery MUST force rediscovery.
Shutdown and `Drop` MUST join all owned work.

### TM-FUNC-005 — Native interaction

The product MUST provide single-instance tray behavior, dashboard/compact access,
global hotkey, current-user startup, and headless degradation. It MUST support instant
modular layout, skin, density, and English/Russian locale switching.

### TM-FUNC-006 — Safe local interfaces

Future CLI and MCP surfaces MUST read the same indexed state as the UI, return strict
bounded results, and expose no arbitrary SQL, shell, HTTP, filesystem, credential, or
transcript operation.

### TM-FUNC-007 — Replay-safe canonical accounting

Forked and subagent histories can repeat an ancestor's usage prefix under different
timestamps and source identities. TokenMaster MUST retain each bounded observation
but MUST admit only observations classified `eligible` by explicit session-lineage
evidence to canonical totals. Strong prefix matches are replay, the first proved
mismatch locks divergence, and missing parent tails, weak pre-divergence matches,
cycles, conflicting parents, or exhausted bounds remain pending or conflict rather
than being counted twice.

### TM-FUNC-008 — Provider-neutral ingestion boundary

The 1.0 product MUST implement local Codex ingestion through bounded source catalog,
sequential reader, and provider decoder contracts. Engine, archive, query, automation,
and UI code MUST depend on provider-neutral observations and snapshots rather than
Codex paths or JSONL wire shapes. Codex MUST remain a compiled-in native adapter.
Refresh coordination MUST use checked monotonic request IDs, cooperative cancellation,
monotonic deadlines, and one bounded active/follow-up aggregate rather than retaining
one queued item per filesystem or caller hint.
Filesystem events MUST be treated only as pathless lossy hints. The runtime MUST retain
one fixed aggregate and capacity-one wake, apply a bounded quiet window, reconcile
periodically even while the watcher is healthy, degrade to a shorter poll after watcher
failure, and cap configured watch roots. Event paths and backend errors MUST NOT enter
engine, archive, diagnostics, query, UI, CLI, or MCP state.

Startup recovery MUST acquire the process-owned writer lease before SQLite open,
migration, scan closure, or staging repair. The live runtime MUST keep the adapter,
archive writer, worker, scheduler, and watcher under one explicit lifecycle. Pause
MUST close new admissions before cancelling the exact active request; resume MUST
force authoritative reconciliation; shutdown and `Drop` MUST stop watcher admission
and join every owned thread. A fault MUST NOT bypass cleanup.

The future external-provider surface MUST accept versioned WebAssembly Component
packages through an isolated on-demand host implementing the same source contract.
Adding a valid provider package MUST NOT require rebuilding TokenMaster or changing
downstream accounting/presentation contracts. External packages MUST NOT execute in
the GUI process or supply canonical identities, SQL, UI code, commands, or ambient OS
access.

## UX requirements

### TM-UI-001 — Reference-quality information design

The UI MUST be quota-first, technically dense but readable, keyboard accessible,
responsive, and explicit about loading, stale, partial, unavailable, and failure
states. The dark default SHOULD preserve the useful visual hierarchy of the UI
reference without copying its implementation.

### TM-UI-002 — Reactive presentation boundary

Skin, layout, locale, selection, and range changes MUST update bounded presentation
state without mutating the archive or initiating an unbounded source scan. An older
asynchronous result MUST NOT overwrite a newer UI generation.

## Performance requirements

### TM-PERF-001 — Bounded hot paths

Input lines, retained parser metadata, reader batches, checkpoint data, SQLite pages,
chart points, UI lists, and external request bodies MUST have explicit limits. No
production path may allocate solely from an untrusted declared size.
An active refresh may retain at most one aggregate follow-up. Burst size MUST NOT
increase retained coordinator memory or create a worker, timer, or queue node per hint.
A filesystem burst MUST NOT retain event/path history; watcher generation replacement
and shutdown MUST return backend threads and handles rather than accumulating them.

### TM-PERF-002 — Long-run stability

The default renderer and lifecycle MUST meet documented private-memory, CPU, handle,
thread, USER-object, GDI-object, and sampling-gap gates during the acceptance soak.

### TM-PERF-003 — Responsive archive reads

Archive reads MUST be keyset-paged and use indexes that seek from the cursor. UI
snapshots MUST be immutable, bounded, and independent of writer lock duration.
Dashboard totals, series, breakdowns, and session summaries MUST read transactional
materialized rollups rather than grouping the complete event archive at view time.
Session pages MUST use indexed mixed-order keyset continuation with one lookahead row,
and exact detail MUST read only bounded model/project session rollups. Raw session IDs
MUST remain private to the store query key and MUST NOT enter Debug or wire values.
Calendar ranges MUST resolve an explicit IANA or positively identified system zone,
use exact half-open local boundaries, and never silently fall back to UTC or round a
historical sub-minute offset. Public analytics MUST preserve known, partial, and
unavailable token facts and cap a requested daily series at 400 owned points. A
session continuation MUST remain bound to both its exact dataset and canonical scope
filter set; changing either starts a new first page.
Quota current reads MUST accept at most 32 exact windows. Reset history MUST use a
quota-revision-bound keyset cursor, return at most 256 transitions plus one internal
lookahead, and apply each sample's provider-defined freshness boundaries rather than
the usage TTL. On the reference machine, one quota write, duplicate poll, 32-window
current snapshot, and 256-row history page MUST each complete below one second.
On the reference machine, aggregate-ready append p95 MUST remain below 25 ms for the
normal one-event path, 50 ms for 32-event catch-up, and 250 ms for the maximum
256-event catch-up, and MUST NOT exceed 1.5 times the matching aggregate-unavailable
baseline.
One-million-event current and immutable-legacy fixtures MUST also meet the following
reference-machine gates: rebuild throughput at least 5,000 events/s, rebuild-page p95
below 500 ms, cold open plus overview below one second, cached overview p95 below
250 ms, a 400-point/four-breakdown analytics snapshot with all 32 scopes below one
second, and first/cursor session-page p95 below 100 ms. SQLite footprint measurement
MUST include the main file, WAL, and SHM and MUST remain at or below 3.0 times the
matching pre-aggregate fixture. These developer gates do not replace package-bound
release performance or soak receipts.

## Release requirements

### TM-REL-001 — Evidence identity

Packages and acceptance receipts MUST bind to one clean commit and executable SHA-256.
Missing or mismatched identity fields fail closed.

### TM-REL-002 — Interactive evidence

Developer tests do not prove interactive Windows behavior. M0 acceptance requires the
independent Windows/DPI/accessibility and uninterrupted 24-hour-soak receipts listed
in `M0_ACCEPTANCE.md`.

### TM-REL-003 — Reproducible native release

The canonical Windows 1.0 artifact MUST target `x86_64-pc-windows-msvc` and ship as a
signed portable ZIP bound to one clean commit, executable SHA-256, and package-content
manifest. The package MUST include applicable license/attribution notices and an SBOM.
Release evidence MUST include dependency advisory/source/license policy, secret scan,
immutable CI action references, artifact provenance, deterministic package audit, and
clean-room launch. The Slint desktop distribution MUST follow the selected
Royalty-free License 2.0 attribution route unless a separately reviewed license route
replaces it. GNU developer/M0 evidence MUST NOT be represented as MSVC release
evidence. Automatic update and installer behavior MUST remain unavailable until a
separate signed-manifest, rollback, downgrade, and interrupted-update contract passes.
