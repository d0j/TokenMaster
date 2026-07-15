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
zeroes.

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

### TM-PERF-002 — Long-run stability

The default renderer and lifecycle MUST meet documented private-memory, CPU, handle,
thread, USER-object, GDI-object, and sampling-gap gates during the acceptance soak.

### TM-PERF-003 — Responsive archive reads

Archive reads MUST be keyset-paged and use indexes that seek from the cursor. UI
snapshots MUST be immutable, bounded, and independent of writer lock duration.

## Release requirements

### TM-REL-001 — Evidence identity

Packages and acceptance receipts MUST bind to one clean commit and executable SHA-256.
Missing or mismatched identity fields fail closed.

### TM-REL-002 — Interactive evidence

Developer tests do not prove interactive Windows behavior. M0 acceptance requires the
independent Windows/DPI/accessibility and uninterrupted 24-hour-soak receipts listed
in `M0_ACCEPTANCE.md`.
