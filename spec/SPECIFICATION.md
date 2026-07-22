# TokenMaster product specification

This is the primary normative product contract. MUST and MUST NOT are binding.
TokenMaster is Windows-first, portable, local-first, and implemented as one Rust
workspace.

## Global reminder settings synchronization

Portable settings are the desired-state authority for the single global reminder
profile. A committed settings generation `N` projects to global profile revision
`N + 1`; startup of first-install, current, migrated, reconstructed, and restored
archives, explicit Save, and confirmed config import use that same synchronizer.
Pending is visible before durable settings mutation and Synchronized follows only a
successful atomic global-profile store commit; archive Busy/unavailable leaves the
durable desired state retryable as Pending. The fixed Settings editor supports
enable/disable, five recommended leads, and at most eight normalized custom leads.
Per-scope editing, snooze, quiet hours, reminder OS/tray delivery, usage alerts, activation,
P4/P5/P6, M0 acceptance, package/signing/soak, and release remain incomplete.

P3-E.2 compact quota mode, P3-E.3 production tray lifecycle, and P3-E.4 current-session
activation are implemented as developer evidence around the sole production window.
Compact reuses the current bounded quota projection and one reversible geometry slot.
One isolated Windows tray adapter emits only Show, Hide, OpenCompact, OpenDashboard,
and Quit; the application owns their consequences, and Quit returns to the existing
joined shutdown/clean-mark path. One fixed current-session auto-reset event arbitrates
the primary process before renderer/data startup; a secondary can only signal Show and
exit. One joined message-driven owner registers fixed `Ctrl+Alt+T`, while the app keeps
one pending activation bit and one scheduled UI task. None of these slices adds query,
snapshot, unbounded queue, timer, cache, data, or provider authority. Current-user
startup and interactive Windows/Explorer/focus/hotkey/ACL/sleep/DPI/screen-reader/
resource acceptance remain incomplete.

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

The History route MUST expose one shared usage-range control with exactly 1, 7, and
30 rolling civil-day presets. The initial and restart default MUST be 30 days, and
30 days is both the maximum requested and displayed range. The selected preset MUST
drive one exact half-open analytics envelope consumed identically by History, Models,
and Projects; all three MUST render its range and timezone context. The request MUST
produce daily newest-first rows, overview tokens/cost/events, and evidence
freshness/quality. Empty days MUST preserve unavailable token components and
legitimate-zero cost semantics rather than fabricate complete zero usage. Selecting a
preset MUST replace the shared envelope through the same snapshot section; route
selection MUST NOT query or retain prior ranges in the frontend. Dashboard MUST retain
its today request, and Projects MUST retain its separately labelled UTC-today Git
range.

The default Sessions route MUST show the newest all-time session page with at most 64
rows and explicit continuation availability. Each row MUST preserve first/last time,
event count, every available token bucket, total tokens, cost, freshness, and quality;
the UI MUST NOT claim that the first page is the complete archive when more rows exist.
Opaque query keys and cursors MUST remain behind the desktop controller boundary. Exact
session detail is a separate generation-bound follow-up: a selection MUST resolve
against the viewed product generation and MUST NOT publish a late result for another
selection or dataset. Backend/controller replacement MUST add an independent monotonic
epoch because product generations can restart. Slint MUST submit only a visible ordinal;
the controller MUST resolve the opaque key inside the existing worker. Ready detail MUST
retain at most 32 model and 32 approved path-free project-alias rows, show exact summary
and evidence facts, and expose explicit loading, missing, unavailable, and truncation
states without retaining another selection's payload.

All data-bearing routes MUST derive from one bounded immutable product snapshot. The
snapshot MUST distinguish refresh-attempt order from durable source revisions, retain
the last compatible successful section when a later refresh fails, reject an older
asynchronous result, and invalidate a payload whose durable identity no longer matches
the joined status. Route readiness MUST be explicit; an aggregate rebuild MUST keep
activity and data-health truth available while aggregate-dependent history, session,
model, and project views remain unavailable rather than showing fabricated zero data.

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

### TM-FUNC-012 — Reliable settings, backup, and recovery

The product MUST persist typed versioned settings through redundant atomic records and
MUST provide bounded `.tmconfig` import/export plus verified `.tmbackup` export,
verification, retention, and restore. A live backup MUST use a SQLite-consistent
snapshot and MUST NOT copy only the main file while WAL may contain committed state.
Every published backup and every restore candidate MUST pass container hashes,
bounded decompression, SQLite integrity, foreign-key, exact schema, and application
semantic checks.

Optional manual export protection MUST wrap only an already complete, verified
`.tmbackup` in a standard age v1 passphrase envelope. Export MUST use scrypt
`log_n = 16`; import MUST reject any larger work factor before derivation. New
passphrases MUST be confirmed exactly, contain 12 through 128 Unicode scalar values,
and be neither trimmed nor normalized. Automatic backups MUST remain unencrypted and
MUST NOT store a recovery secret.

Automatic backups MUST be coalesced off the UI thread, bounded by count and bytes, and
preserve at least the newest protected verified restore points. Restore MUST stop all
archive users, hold the stable writer lease, quarantine the complete prior main/WAL/
SHM set, and use a durable idempotent journal. Interrupted restore MUST resume before
any SQLite open. Busy, access-denied, disk-full, transient-I/O, unsupported-location,
and schema-too-new failures MUST NOT authorize replacement.

Automatic maintenance MUST own exactly one worker and one scheduler. It MUST retain
only one active request, one urgency-merged follow-up, one latest general completion,
and one latest mandatory-guard completion. Priority is mandatory safety point, manual,
source retry, then periodic. Automatic work MUST remain disabled until the first
healthy publication. A startup state already proved `Healthy` MAY seed that truth at
the current monotonic tick; `HealthyUnpublished` MUST NOT. Automatic work MUST run only
after the configured quiet window and ordinary minimum interval and coalesce resume or
clock-discontinuity catch-up to one request. The periodic-disable setting MUST affect
only quiet/interval work and MUST discard an already merged periodic-origin follow-up.
Source retry is internal urgency, never a caller-submit purpose.

Cancellation MUST propagate into the bounded SQLite backup control before final
publication. Final publication and the immediately following proof/retention update
MUST be non-cancellable. A `Published` completion without first entering that boundary,
or a `Cancelled` completion after it, MUST fail as an internal invariant. A failed
candidate MAY trigger one fresh retry while retaining
the original backup purpose and guarded request identity; two integrity/semantic
failures for the same unchanged source identity MUST mark the source suspect and stop
new backup mutation admission.

The automatic-backup namespace MUST contain at most 32 fixed private slots and MUST
reject unexpected names, links, reparse points, hard links, and ambiguous physical
identities. Its catalog MUST be disposable and rebuildable from bounded package
headers without treating header validity as complete verification. Retention MUST
perform a no-delete admission before publication, then fully revalidate the complete
current verified set and the exact selected old point before deleting at most one
verified unprotected file. Every deletion MUST be followed by a catalog rebuild and
replan; corrupt, unchecked, or stale points MUST NOT become deletion authority.

Disabling scheduled periodic backup MUST NOT disable mandatory pre-migration,
pre-restore, or pre-destructive-maintenance safety points for a healthy non-empty
archive. If such a point cannot be created and reverified, its mutation MUST be
blocked. Manual full restore MUST explicitly choose data only or data plus portable
settings; automatic recovery MUST be data only, and device-local settings MUST never
be restored. Database and selected portable-settings publication MUST complete
idempotently together or restore the prior database/settings truth.

Definitive corruption MAY automatically select the newest previously verified backup
only after revalidation. When no backup is usable, the corrupt set MUST remain in
quarantine and reconstructible usage MAY rebuild from authoritative local sources;
lost non-reconstructible domains MUST remain explicitly unavailable. Automatic
salvage of corrupt rows is forbidden. Data Health and safe mode MUST remain usable
without starting archive/query/runtime owners.

A no-backup rebuild MUST create and fully verify a fresh normal-schema archive before
atomically replacing the definitively corrupt set. The replacement MUST NOT become
healthy, seed ordinary backup scheduling, or present reconstructed zero values until a
bounded recovery-urgency refresh has completed from authoritative local sources. Its
durable path-free recovery receipt MUST distinguish verified-backup restoration from
authoritative-source reconstruction and MUST explicitly report non-reconstructible
quota, reset-credit, reminder, and Git history as unavailable. A completed source-
reconstruction journal MUST preserve this reconciliation obligation across process
death; restart or retry MUST complete reconciliation without attempting to reconstruct
the already promoted healthy-schema archive again.

Startup MUST durably publish and reread an unclean-run marker before any writable
SQLite open. A missing active main with prior durable TokenMaster artifacts MUST be
treated as damaged state, while a root with no prior durable artifacts MAY use normal
first-install schema creation.

### TM-FUNC-005 — Native interaction

The product MUST provide single-instance tray behavior, dashboard/compact access,
global hotkey, current-user startup, and headless degradation. It MUST support instant
modular layout, skin, density, and English/Russian locale switching.

Compact access MUST remain a presentation mode of the sole production window and MUST
reuse the current immutable quota projection. It MUST show every published provider-
defined quota window up to the existing bound, represent an unknown ratio explicitly,
provide one checked return-to-dashboard action, retain at most one device-local normal
window-size restore value, and MUST NOT query or retain a second product snapshot.

Production startup MUST claim the fixed non-inheritable auto-reset event
`Local\TokenMaster.CurrentSession.Activation.v1` before renderer, data-root, SQLite, or
runtime work. The existing-event path MUST only signal activation and exit; claim or
signal failure MUST fail closed as `current_session_unavailable`. The primary MUST own
one joined message-driven thread, one unnamed shutdown event, and fixed
`Ctrl+Alt+T`/`MOD_NOREPEAT` registration. Hotkey conflict MUST degrade explicitly
without hiding or stopping the main application. Secondary and hotkey activation MUST
reuse Show/restore/focus, retain at most one pending bit and one scheduled UI task, and
MUST join/unregister before clean-run publication.

Current-user startup MUST be explicit, device-local, and non-fatal. The sole source of
truth MUST be the fixed `REG_SZ` value `TokenMaster` under
`HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`; it MUST NOT add a
portable or reliable-state desired flag. Inspection MUST be read-only. Enable, stale-
relocation repair, and disable/removal MUST be separate typed actions, and conflict
MUST never be overwritten. A successful enable/repair MUST reread the exact quoted,
argument-free current executable command, enforce the Windows Run limit of 260 UTF-16
code units excluding the terminating NUL, and prove the running file's physical
identity; disable MUST reread absence. Access denial, malformed/foreign values,
unverified executable state, and unsupported platforms MUST degrade only this control
through path-free status. UNC, device/verbatim, mapped-remote, and unknown-volume paths
MUST be rejected before filesystem access; an alternate same-basename local path MUST
be reported stale without opening it. No HKLM, shell, process, elevation, service, scheduled task,
retry, timer, polling loop, retained path, or arbitrary registry input is permitted.

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

The Dashboard MUST render exactly six semantic sections in this order: Plan Usage,
Code Output, Usage and Cost Trend, Sessions, Activity, and Model Usage. Provider quota
rows MUST be discovered dynamically and MUST NOT assume five-hour or weekly windows.
Missing token, cost, quota, reset, or benefit evidence MUST render as unavailable,
never as a fabricated zero. Banked resets MUST remain visually and semantically
separate from provider quota bars, credits, temporary usage, and unavailable lots.
Unknown reliable-state backup counts and byte totals MUST also render as unavailable;
legitimate known zero values remain distinct.

History MUST render a bounded daily trend plus responsive daily details. The wide
layout shows input, cached, output, reasoning, total, cost, and event evidence; the
narrow layout may reduce visible columns but MUST retain the same owned bounded model
and accessible row meaning.

Sessions MUST render one bounded newest-first list with explicit page completeness.
The wide layout shows last activity, duration, events, input, cached, output,
reasoning, total, and cost; the narrow layout may reduce visible columns but MUST
retain the same model and accessible row meaning. No display row may expose a provider,
profile, source, workspace, project, private session key, or continuation cursor.

Models MUST render the Model breakdown from the same exact selected shared analytics
envelope as History. The wide layout shows canonical model key, events, input, cached,
output, reasoning, total, cost, and relative total-token distribution. The narrow
layout MAY rearrange those facts but MUST keep every component in the same owned row
model and accessible row meaning. Backend or presentation truncation MUST be visible.
No Models row may expose provider, profile, source, account, workspace, project,
session, opaque key/cursor, path, content, command, credential, or query authority.

Projects MUST render the Project breakdown from that same selected shared analytics
envelope and MAY enrich named rows only with exact safe-alias matches from the existing
Git envelope. `Unassociated` usage never matches Git, and Git-only aliases MUST NOT be
fabricated as zero-usage rows. Recent usage range/timezone/evidence and UTC-today Git
range/evidence MUST remain separately labelled; the UI MUST NOT combine or relabel
them as one period. Rows retain events, every token component, total, typed cost and
provenance, relative usage, and optional commits/added/removed/net/efficiency. Backend,
frontend, and repository truncation plus unmatched/partial/unavailable state MUST
remain visible. No Projects row may expose repository/association/dataset/private IDs,
provider/profile/account/source/session identity, path, key/cursor, content, command,
credential, or query authority.

Activity MUST render one bounded newest-first page from the already-published latest
activity envelope. Each row contains only an exact UTC timestamp, canonical model key,
and typed input, cached, output, reasoning, and total token facts. Page completeness,
freshness, quality, empty, retained-failure, unavailable, and truncation truth MUST
remain explicit. No Activity row may expose scope, provider/profile/account, source,
session/project, event/dataset identity, key/cursor/fingerprint, path, content, prompt,
response, command, credential, or query authority. This recent-event page MUST NOT be
labelled as a rhythm, heatmap, hourly, or day-of-week aggregate; the separate bounded
rhythm contract supplies 24 hourly and seven Monday-Sunday buckets from the same
rollup transaction. It is capped at 30 civil days, 768 occurrences, and 2,304
segments, preserves metrics/exposure/occurrence metadata plus DST/fractional/skipped-
date truth, and never retains raw events, paths, prompts, or cost authority.

Notifications MUST render the already-published all-current benefit overview as a
bounded expiry-safety center. It MUST preserve separate current lots, exact/bounded/
provider-local/provider-date/unknown expiry precision, effective inherited or override
profiles, disabled or in-app-only coverage, freshness, quality, warnings, and explicit
truncation. The frontend MUST retain at most 32 profile rows, 256 lot rows, and eight
lead times per profile. Provider/account/workspace/scope/lot/delivery identity and
activation authority MUST remain behind the query/runtime boundary. Merely selecting
or rendering this route MUST NOT take, acknowledge, release, schedule, or otherwise
mutate a reminder delivery. Visible delivery requires a separate app-owned presentation
receipt that acknowledges only after successful presentation and releases on failure.
The implemented in-app presenter MUST retain exactly one leased batch of 1 through 256
identity-free rows, schedule one weak-window checked-epoch callback, and make the panel
visible before emitting `Presented`. One app-owned receipt worker MUST perform durable
acknowledgement outside the UI thread. Only `Busy` and `StoreUnavailable` may retry,
after exactly 60 seconds, as acknowledgement failures. A successfully released failed
presentation MUST be re-pumped by the same bounded worker without requiring unrelated
runtime activity. A non-retryable acknowledgement error MUST release the lease without
automatic re-presentation. Runtime panic MUST restore a releasable lease; `Err` or `false`
release MUST retain local backpressure. Scheduling, callback, stale-epoch, closed-window,
terminal, and shutdown paths MUST release the lease. UI code MUST own no acknowledgement, timer,
polling loop, auto-dismiss, runtime, store, or private delivery identity.

Help/About MUST remain ready without an archive or live runtime. It MUST present one
responsive fixed six-section guide covering navigation, data-source truth, privacy,
health/recovery, current automation availability, and licenses. The package version
MUST come from the compile-time Cargo package version and MUST be applied exactly once
during window construction. The route MUST mount exactly one standard pinned Slint
`AboutSlint` attribution widget. It MUST NOT own a list model, query, diagnostic probe,
callback, filesystem/network/process/browser/SQL surface, worker, timer, queue, cache,
polling loop, provider mutation, or dynamic release claim. P4 owns unified locale and
presentation switching; P5 owns CLI/MCP; generated notices, SBOM, MSVC package,
signing, public-download attribution, and release identity remain P6 receipt truth.

### TM-UI-002 — Reactive presentation boundary

Skin, color-scheme, layout, locale, selection, and range changes MUST update bounded presentation
state without mutating the archive or initiating an unbounded source scan. An older
asynchronous result MUST NOT overwrite a newer UI generation.
Restore confirmation MUST authorize the exact generation/ordinal identity reviewed by
the user even if a newer reliable-state projection arrives before confirmation.
The presentation owner MUST retain only the current immutable product snapshot and
MUST copy bounded runtime health rather than retaining runtime owners, callbacks,
guards, paths, or database handles.

The production `TokenMaster.exe` MUST be owned by a composition package separate from
the desktop frontend. The composition owner MUST select exactly one validated archive
root before starting live data: an empty regular `tokenmaster.portable` marker beside
the executable selects the adjacent `data` directory; absence selects
`%LOCALAPPDATA%\TokenMaster`. An invalid marker or unavailable/unsupported selected
location MUST fail closed without fallback, current-working-directory use, or path
disclosure.

Runtime-to-presentation refresh MUST be driven by bounded lossy completion hints from
the existing workers, not a UI timer or polling thread. The application may retain one
latest fixed runtime-health observation and one checked generation only. It MUST NOT
duplicate ingestion, runtime ownership, result history, or the desktop snapshot slot.

The current Dashboard projection MUST retain at most 32 quota rows, 32 benefit-scope
summaries, 240 trend points, 12 sessions, eight fixed activity categories, 12 model
rows, and one checked aggregate over at most 32 repositories. An accepted product
generation MAY replace each bounded list model once. Route-only selection MUST NOT
rebuild Dashboard models, recreate the window, query SQLite, or schedule background
work. The production Dashboard MUST contain no idle animation or presentation timer.
The current History projection MUST retain at most 30 daily rows in one model and no
prior range, query service, timer, worker, or archive handle.
The current Models projection MUST retain at most 64 model rows in one model and no
prior range, filter, sort state, query service, timer, worker, archive handle, or
private identity. History, Models, and Projects MUST share one
bounded recent-usage envelope rather than duplicate equivalent analytics queries.
Model token and cost availability MUST remain typed through Slint. Cost mode and
calculated/reported/mixed composition MUST remain typed through Desktop, while the
visible and accessible UI MUST distinguish partial cost and actual composition.
The current Projects projection MUST retain at most 32 usage-centric rows and inspect
at most 32 existing Git repository projections per row. It owns no prior range,
filter/sort state, query service, timer, worker, archive handle, or private identity.
Named aliases match Git only by exact bounded `ProjectAlias`. Same-alias repository
metrics use checked sums; project usage cost is counted once when recomputing combined
efficiency. Route-only selection MUST NOT rebuild the Projects model or issue work.
The current Activity projection MUST retain at most 12 newest-first rows in one model
and no prior page, query service, timer, worker, archive handle, private identity, or
raw event. It MUST reuse the existing first-page request and remain available while
aggregate-dependent routes rebuild. Route-only selection MUST NOT rebuild the Activity
model or issue work.
The current Sessions projection MUST retain at most 64 summary rows in one model and no
opaque key, cursor, prior page, query service, timer, worker, or archive handle. Its
route-only selection MUST NOT rebuild the model or issue a detail query.

History range admission MUST use the existing capacity-one worker and MUST retain only
the published preset, one persistent scalar range-selection high-water generation, one
active correlation, and one latest pending intent. A range intent MUST contain only the
current snapshot epoch, viewed product generation, checked newer selection generation,
and one of the fixed 1/7/30 presets. Sessions detail/page work and History range work
are mutually exclusive: either interaction MUST return `Busy` for the other without
changing its state. A full refresh supersedes active or pending range work and uses the
last successfully published preset; after refresh, only the latest eligible range
follow-up may run. Admission and publication MUST revalidate epoch, product generation,
worker correlation, and range generation; stale results MUST be discarded. The exact
arbitration is: no work + range admits; refresh + range retains latest follow-up;
range + refresh lets refresh supersede and roll back; range + range rejects UI input
and retains only the newest valid direct ingress; Sessions interaction + range and
range + Sessions interaction return `Busy`; backend epoch replacement makes old work
stale, resets the published preset to default 30 days, clears old correlations, and
rolls back the exact pending correlation. Only `ProductPublishOutcome::Accepted` may
publish a section-local snapshot. Only an accepted successful range query may update
the published preset; `Coalesced`, `RejectedOlder`, and `RejectedIncompatible` outcomes
publish nothing, leave the preset unchanged, and complete through the exact no-snapshot
rollback. An accepted query failure retains the shared envelope degraded without
changing the preset.

Terminal no-snapshot completion MUST use a dedicated optional History-range notifier
beside the Sessions notifier. The two fixed slots MUST NOT displace one another, and
notification MUST match the whole still-current intent before clearing UI pending state.
Snapshot publication MUST precede terminal completion observation for the same attempt,
and successful commit MUST consume current work before completion reconciliation. The
reconciliation path MUST be exact and idempotent so a committed selection cannot be
rolled back by terminal handling.
The frontend MUST retain at most 30 rows. No new range control boundary may carry free-
form dates/counts, scope/provider/profile identity, query objects, archive handles,
paths, prompts, responses, reasoning, commands, credentials, or source contents.

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
Joined product status MUST capture usage publication, aggregate progress, quota,
benefit, and Git scalar state in one short deferred transaction with a maximum
two-second deadline. It MUST NOT scan event, rollup, quota-sample, benefit-change, or
Git-day history. On the reference machine, a joined status capture over an archive
containing at least 100,000 usage events MUST have p95 below 25 ms.
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

### TM-PERF-004 — Bounded background maintenance

Settings, backup, verification, import, restore preparation, and compression MUST run
outside the Slint event thread. Maintenance MUST retain at most one active operation,
one aggregate follow-up, one latest health snapshot, fixed streaming buffers, and one
compression context. The backup contour MUST own exactly one worker thread and one
scheduler thread with one shared timer, not a timer per request. It MUST NOT allocate
a database-sized buffer, create a thread or queue node per trigger, or use
multithreaded compression.

A 10,000-trigger burst MUST remain capacity one. Repeated success, failure, cancel,
sleep/resume, and restore cycles MUST return private memory, file/process handles,
threads, USER objects, and GDI objects to the measured post-warm-up envelope. On the
reference machine, an automatic backup MUST add no more than 10 ms to cached Dashboard
query p95 or measured input-to-paint p95.

P3-D.0 developer acceptance measures automatic, normal, and compact pipelines against
deterministic 8 MiB and 96 MiB schema-13 fixtures in release mode. Private-memory growth
MUST remain within a fixed 64 MiB envelope and at least 16 MiB below the large database;
the only permitted sampled thread delta is the measurement thread. The resource gate
MUST warm every contour, then execute 256 backup/package/verify/import-cancel/retention
cycles, 16 acquired-candidate cancellation/recovery cycles, and 16 complete isolated
restore cycles while returning to the original post-warm-up envelope. Retention bytes
MUST equal the filled-tier plateau on every measured cycle. One identity-tracked
automatic backup cycle MUST span the complete loaded Dashboard-query and
route-input-to-software-paint sample windows. These measurements remain developer
evidence and do not replace physical-display/OS-input, M0, soak, or product-release
acceptance.

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

## P4-C durable built-in skins (partial)

Presentation is one complete `{ density, skin }` selection. The only skin identities are
`refined`, `graphite`, and `ember`; Rust owns immutable exact 15-role palettes and Slint
aliases one palette value. One desktop owner admits a complete request before mutation,
persists/restores both axes together, and reuses the latest-wins worker. Layout, colour
scheme, locale, typography, interactive DPI/accessibility/paint/resource, P5/P6/M0,
package/signing/soak, and release acceptance remain open.

## P4-D independent color schemes (partial)

Presentation is one complete `{ density, skin, color_scheme }` selection. The only
requested color-scheme identities are `system`, `light`, and `dark`; `system` resolves
to an observed effective Light or Dark scheme and falls back to Dark when observation
is unavailable. Fresh defaults use System, while schemas v1 through v3 migrate in
memory to Dark so an upgrade preserves the prior appearance. A system observation
MUST change only the effective palette: it MUST NOT advance the presentation revision,
persist settings, enqueue work, poll, or add a watcher thread. Rust owns the six exact
skin/scheme palettes and Slint receives one palette value. Layout, locale, remaining
typography/row-size behavior, interactive DPI/accessibility/paint/resource, P5/P6/M0,
package/signing/soak, and release acceptance remain open.
