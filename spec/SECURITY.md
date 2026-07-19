# TokenMaster security contract

## Reminder settings authority boundary

Only the application operation worker may persist and synchronize the portable global
reminder policy. Desktop forwards a checked bounded typed intent and retains one fixed
eight-row editor model; it has no store, SQL, runtime, timer, polling, queue, or
delivery authority. The global projection preserves scope overrides, deliveries,
acknowledgements, and provider evidence. Per-scope editing, snooze, quiet hours,
OS/tray delivery, usage alerts, activation, P5 automation, and P6 release controls
remain unavailable.

## TM-SEC-001 — Local-first operation

No telemetry, cloud sync, remote listener, automatic upload, analytics SDK, or
developer-controlled service is permitted. Any future local API binds loopback only;
MCP uses stdio only.

The built-in Codex quota adapter may consume only a credential-free versioned local
format or a documented stable official machine interface. A user-facing dashboard,
slash command, browser page, cached cookie, captured request, or observed private
endpoint is not an integration contract. Browser scraping, session reuse, private
endpoint replay, credential extraction, and raw response persistence are forbidden.
If no permitted source exists, live quota and banked-reset discovery remain explicitly
unavailable or stale; local usage totals cannot substitute for provider allowance.

## TM-SEC-002 — Untrusted boundaries

JSONL, configuration, archive files, CLI/MCP requests, generated reports, and future
provider output are untrusted. Each boundary MUST validate type, size, count, encoding,
path safety, timeout, and allowed values before allocation or interpretation.

Any permitted quota response is additionally bounded by an exact schema/version,
allowlisted origin when networked, maximum body size, deadline, redirect policy,
backoff, freshness, and redacted error mapping. Custom origins never receive provider
credentials. A schema change fails to unavailable/stale rather than best-effort field
guessing.

Normalized quota domain input already fails closed before detector or persistence:
identifiers use a narrow bounded ASCII alphabet, ratios use checked integer
parts-per-million, optional units and reset thresholds are relationship-validated,
sample times/evidence are ordered, unknown nested serde fields are rejected, and
observation identity `Debug` is redacted. This validation does not authorize a
provider transport or make raw provider payloads safe to retain.

The implemented quota detector adds no filesystem, environment, network, async,
SQLite, serializer, clock, timer, or mutable-global capability. Scope, epoch, and
transition identities are domain-separated SHA-256 values over normalized
length-prefixed fields and big-endian integers; their `Debug` output is redacted.
Detector errors are stable path-free codes. Low/unknown-confidence or
conflict/unknown-quality samples cannot independently trigger threshold inference,
and rolling windows never infer resets from ratio recovery.

The implemented schema-v10 quota store boundary persists only normalized bounded
facts. It has no payload, URL, header, cookie, credential, prompt, response,
reasoning, command, source-content, or path columns. Exact table/index/trigger SQL is
validated on writable and read-only reopen; weakened or extra authority fails closed.
Composite foreign keys keep sample, epoch, definition, and current-projection evidence
inside one exact scope/window. The exact v9 migration is one immediate transaction,
touches only quota objects after v9 validation, and rolls back without residue at the
injected post-create fault boundary.

The implemented quota writer accepts only already validated domain values and adds no
filesystem, network, clock, shell, HTTP, credential, or raw-payload authority. It uses
one immediate transaction, performs no write for duplicate/stale input, rejects global
observation-ID content reuse and definition mutation, and checks the exact current
epoch/window/last-sample projection before classification. Missing or mismatched
projection state is rejected on live use and reopen rather than silently reconstructed.
Injected failures at every publication boundary prove complete rollback.

The implemented quota retention path is store-owned and exposes no SQL, identifiers,
sample content, filesystem, network, clock, shell, or provider authority. Automatic
and paged deletion use fixed parameterized statements scoped to one exact window and
only remove redundant samples unreferenced by current, epoch, maximum-use, or
transition evidence. Page size and per-window hard caps are fixed exported bounds.
Maintenance does not advance semantic quota revision; injected post-delete and
post-count faults prove rollback. Reopen independently rejects over-cap persisted
state, including a tampered archive whose global retained count was made internally
consistent.

The implemented quota read boundary adds no write, migration, filesystem, network,
clock, shell, HTTP, credential, provider, or raw-payload authority. It runs only on
the existing read-only/query-only defensive connection with fixed parameterized
quota-table SQL, exact key filters, no caller-defined expressions, no `OFFSET`, at
most 32 current windows, and at most 256+1 transition rows. Opaque cursors bind the
exact window and quota revision; public capture/cursor `Debug` output redacts filters
and identities.

Read reconstruction repeats domain and deterministic transition-identity validation
and cross-checks current epoch/current-row and transition boundary facts against the
joined samples. Post-open archive drift therefore fails closed instead of becoming
plausible UI truth. A two-second maximum deadline is enforced across the complete
capture as well as inside SQLite VM execution, and the progress handler is removed
after success or failure so cancellation cannot contaminate the next query.

The public quota facade adds no SQLite, filesystem, network, shell, HTTP, browser,
cookie, credential, provider-mutation, or raw-payload authority. Its values are owned
and bounded; request/header/result/cursor `Debug` redacts exact filters and private
provider-epoch identity. The adversarial detector matrix proves that rolling/unknown
windows and low-quality or low-confidence fixed-window recovery cannot infer an
automatic reset, while explicit manual evidence remains typed.

`scripts/audit-quota-network.ps1` traverses the release dependency closure of
`tokenmaster-quota`, `tokenmaster-store`, and `tokenmaster-query`, rejects
network/browser/async client crates, scans production source for endpoint, cookie,
browser automation, shell, and socket authority, builds release libraries, and scans
their strings. The current gate covers 76 production dependency packages,
43 production files, and three current release libraries with zero forbidden
matches. This proves the quota core remains offline and separate from provider I/O.

The implemented Codex quota transport uses only an exact caller-resolved native
executable plus fixed `app-server --stdio` arguments. It performs no PATH search,
shell construction, endpoint selection, browser/session reuse, credential-file read,
environment injection, response persistence, logging, socket/listener creation, or
direct HTTP. The trusted official Codex child runs in the normal user context and owns
its own authentication/network behavior; TokenMaster reads only bounded JSONL stdout,
discards stderr, and never receives credentials. The command path, Codex home, account
email, raw frames, provider error text, reset-credit IDs, and inner OS errors are
absent from public results, errors, and `Debug`.

The stable non-experimental protocol is pinned to Codex app-server `0.144.1`.
Initialization suppresses the two observed unsolicited notifications, and strict
unknown-field/ID/message validation rejects any unrequested surface. The CLI command
remains an experimental product surface, so every version/schema change fails closed
to unavailable/stale and requires regenerated official schema review plus live
contract evidence before the version gate moves. No compatibility guess or private
fallback is permitted.

`scripts/audit-codex-quota-transport.ps1` traverses the production dependency closure,
rejects network/browser/async client crates, scans the non-test Codex library source
for browser, cookie, private endpoint, credential-file, shell, socket, and logging
authority, proves exactly one fixed command/argument construction, builds the release
library, and scans its strings. The current gate covers 72 production dependency
packages, 22 production library source files, and one release library with zero
forbidden matches. A separate isolated Windows gate repeatedly exercises success,
JSON-RPC failure, and forced timeout; private memory and handle/thread/USER/GDI
topology return to a stable plateau and no task-owned child remains.

The joined product-status boundary adds no write, migration, provider, process,
filesystem, network, shell, UI, or runtime-owner authority. Its store capture uses one
defensive read-only deferred transaction, fixed statements, checked scalar
reconstruction, and a maximum two-second deadline whose progress handler is removed on
every outcome. Public status and runtime-health values exclude paths, source/account/
window/lot/repository/project identities, raw payloads, quota or benefit values,
commands, credentials, and inner errors. Component revisions remain independent, so a
fault in quota, benefit, Git, or aggregate publication cannot manufacture or suppress
sibling truth.

The product reducer copies only bounded immutable public values and count-only runtime
health. It retains no SQLite handle, writer guard, runtime owner, callback, child,
timer, path, identifier, or history. Stale attempt/runtime generations fail closed;
durable identity mismatches invalidate affected payloads. `scripts/audit-product-status.ps1`
enforces the leaf dependency direction, fixed route/reason topology, absence of
whole-history status scans and forbidden authority, no vendored upstream source, and
release-library string privacy.

The P3 production frontend is a separate `tokenmaster-desktop` package and does not
depend on `tokenmaster-m0`. Its only product-data dependencies are the public
read-only query facade, bounded engine coordinator, and product reducer; it has no
direct store/provider/runtime/SQLite/network/browser/shell dependency. The P3-B.1
controller accepts an already selected archive path only at composition, maps open
failure to a stable path-free code, and keeps one query source plus reducer inside one
worker. At most one active and one coalesced follow-up exist, and only one completed
immutable snapshot is retained. Cancellation/deadline termination cannot publish the
partial reducer state. P3-B.2 shares that same mailbox with one receiver and one weak
notifier; one atomic gate queues at most one Slint event and retains no second
snapshot, timer, polling thread, or strong window handle. Delivery upgrades the weak
window and applies only a newer generation on the event-loop thread. Failure exposes
fixed counters/codes and no path or model contents. Slint receives one fixed 11-row
projection and no query, archive, provider, or runtime handle. Route callbacks
validate stable keys and change presentation selection only; the UI adapter contains
no query call. The production binary selects the software renderer with no diagnostic
fallback. The deterministic desktop audit rejects probe or seeded data, FemtoVG,
direct authority, route-count/controller-worker drift, a second result slot or event
site, strong window retention, bridge polling, UI-query surfaces, forbidden source
surfaces, and exact private canary strings in the release executable.

P3-C adds only an owned bounded Dashboard projection. Its public/UI structs contain
semantic keys, ordinals, values, availability, freshness, quality, and stable reasons;
they exclude account, workspace, window, lot, repository, project, session, event,
and source IDs. Dynamic quota discovery is explicit and an empty exact filter remains
empty. The audit rejects fixed five-hour/weekly UI rows, seeded metric literals,
private identity fields, UI timers/animations, Dashboard-bound drift, route-triggered
Dashboard rebuilds, and a second worker/snapshot/event site. Slint still receives no
SQL, store, query service, runtime, filesystem, network, process, browser, shell, or
credential authority.

P3-D.1 adds one independent owned History projection and one 30-row Slint model. It
contains only civil dates, aggregate counts, typed token/cost values, resolved timezone,
freshness, quality, and stable reasons. It contains no raw session/event/account/
workspace/project/source identity, absolute path, cursor, SQL, prompt, response,
reasoning content, command, or credential. Route selection remains presentation-only;
the existing query worker performs the fixed bounded request during refresh and owns
no History cache or snapshot history.

P3-D.2a adds one independent owned Sessions projection and one 64-row Slint model. It
contains only UTC instants, aggregate event/token/cost values, freshness, quality,
stable reasons, and one continuation-availability fact. It contains no provider,
profile, source, account, workspace, project, session identity, opaque key/cursor,
absolute path, SQL, prompt, response, reasoning content, command, or credential. The
existing query worker performs the bounded page request during refresh; route selection
remains presentation-only and owns no page/detail cache. Exact detail cannot be added
without controller-side generation/selection matching and must keep all opaque query
identity outside Slint.

P3-D.2b keeps the opaque `UsageSessionKey` inside the controller worker. Public/UI intent
contains only backend epoch, immutable product generation, selection generation, and
visible ordinal. Product correlation contains only selection generation and ordinal.
The application routes through a weak current-bundle reference and fails closed when no
live controller exists. It uses nonblocking bundle acquisition and rejects contention
instead of waiting behind backup/recovery ownership on the UI thread. Ready UI data
contains exact aggregate summary/evidence plus at
most 32 model and 32 path-free project-alias rows; no provider/profile/source/session key,
cursor, absolute path, prompt, response, reasoning content, command, credential, SQL,
filesystem, network, process, or browser authority crosses the frontend boundary.

P3-D.3 adds one owned Models projection and one 64-row Slint model over the existing
recent-usage envelope. It exposes only canonical bounded model keys, aggregate event/
token/cost values, non-secret cost availability/mode/composition, the half-open civil
range, timezone, freshness, quality, stable
reasons, and truncation. It contains no provider, profile, source, account, workspace,
project, session, event identity, opaque key/cursor, absolute path, SQL, prompt,
response, reasoning content, command, credential, pricing internals, filesystem,
network, process, browser, or query authority. Model labels are data, not executable
skin/plugin/config inputs. Route selection remains presentation-only.

P3-D.4 adds one 32-row Projects projection over the existing recent-usage and Git
envelopes. It exposes safe bounded project aliases or fixed `Unassociated`, aggregate
usage values, and optional checked UTC-today Git facts. Exact alias equality is the
only join; basename/path/fuzzy/case-folded guesses are forbidden. Repository ID,
association ID, dataset identity, provider/profile/account/source/session identity,
paths, Git object/author/ref/file data, keys/cursors, prompts, responses, reasoning
content, commands, credentials, SQL, and authority remain outside Desktop/Slint.
Dataset identity may be compared transiently only to validate same-alias efficiency.
The recent local-civil usage range and UTC code range remain visibly separate, so
neither evidence stream can launder the other's period or completeness.

P3-D.5 adds one 12-row Recent activity projection over the existing latest-page
envelope. It exposes only UTC timestamp, canonical model key, typed aggregate token
components, evidence, and page completeness. Scope, provider/profile/account,
event/dataset/source/session/project identity, cursor/fingerprint/key, paths, content,
prompts, responses, reasoning content, commands, credentials, SQL, raw-event export,
and authority remain outside Desktop/Slint. Activity route selection adds no query,
callback, worker, timer, queue, cache, connection, or retained row identity. The UI
does not claim rhythm/heatmap semantics without a separate bounded aggregate.

P3-D.6 adds one bounded Notifications projection over the existing all-current benefit
overview. It exposes only presentation ordinals, provider-neutral lot facts, typed
expiry precision, effective profile source/coverage/leads, bounded revisions and time
facts, freshness, quality, warnings, and truncation. Provider/account/workspace/scope/
lot/delivery/window IDs, target descriptors, paths, content, commands, credentials,
SQL, receipts, activation capability, and runtime owners remain outside Desktop/Slint.
Route projection and navigation have zero reminder take/acknowledge/release or settings
mutation authority. The separate implemented presentation bridge is app-owned and
acknowledges only after successful visible application. Desktop receives only bounded
provider-neutral display facts and a one-shot receipt; it receives no runtime/store,
delivery ID, path, provider payload, SQL, activation, or settings authority. Failed,
cancelled, stale, closed-window, terminal, and shutdown presentation paths release the
lease. Runtime panic payloads are suppressed and the lease transition is rolled back;
only the narrow fallback release recovers outer runtime-mutex poison. A false or failed
release retains local backpressure. Its one worker is condition-variable driven,
re-pumps a released failed presentation without an unrelated completion, releases a
terminal acknowledgement error without automatic re-presentation, retains no batch, and never blocks the UI
thread. Desktop clears its bridge-busy flag before receipt invocation.

P3-D.7 Help/About adds only fixed compiled English fallback text, the compile-time
Cargo package version, responsive geometry, and exactly one pinned standard
`AboutSlint` widget. It owns no archive/query/model/diagnostic/runtime state and no
TokenMaster callback, URL property, filesystem, environment, network, process,
browser/session, SQL, provider, activation, notification-delivery, or release-receipt
authority. The standard widget's fixed Slint attribution action is a narrow selected-
license surface; TokenMaster does not accept or construct an arbitrary URL. Help text
must keep prompts, responses, reasoning, commands, source contents, credentials, raw
absolute paths, private identities, and raw operating-system errors outside Slint.
Claims about CLI/MCP, MSVC, notices, SBOM, signing, packaging, or release acceptance
remain unavailable until their owning P5/P6 gates pass.

Providers emit bounded observation/session-relation drafts only. They cannot create
event fingerprints, replay signatures/evidence, event IDs, replay dispositions, or
canonical events. Those values are created only by TokenMaster accounting code. Store
append MUST reject canonical events whose provider, profile, or source identity does
not match the registered source.

## TM-SEC-003 — Path privacy

Source descriptors are path-private. Public errors, debug surfaces, serialized values,
and diagnostics use stable codes and counters, never absolute paths or wrapped OS
messages.

Repository activity uses a separate sealed transient path type. Construction accepts
only an existing canonical local directory, rejects traversal, network/device/mapped-
remote namespaces and linked/reparse ancestry, and bounds the lossless platform path
length. `RepositoryActivityHint` is capacity-one per source batch, non-serializable,
fully redacted in `Debug`, and excluded from parser resume, adapter/canonical batches,
checkpoints, SQLite, query, diagnostics, and errors. An explicit invalid candidate
clears the previous transient association. The Git backend MUST revalidate the
candidate before and after its bounded read-only scan; the hint grants neither generic
filesystem access nor repository mutation authority.

## TM-SEC-004 — Archive integrity

The frontend query path cannot receive `UsageStore`, a SQLite connection, transaction,
statement, archive path, source key, or raw fingerprint. `UsageReadStore` opens only an
existing read-only archive, enables SQLite query-only/defensive/no-checkpoint policy,
disables trusted schema and double-quoted compatibility, validates exact schema before
reads, and exposes fixed queries only. Every public error and Debug value is path-free;
activity/cursor fingerprints are redacted. Deadline interruption is cleared on every
success/error path before reuse.

Schema v13 adds only installation-salted opaque Git identities, bounded aggregate
facts, counters, stable quality codes, and immutable generations. It has no repository
or executable path, author email/name, ref/branch, commit/object ID, file path/content,
command, stdout, or stderr column. Exact v12 migration is one rollback-tested
immediate transaction. Rebuild, proven append, unchanged refresh, association update,
and rebuild-required invalidation advance a separate Git revision atomically; injected
faults after aggregate or repository mutation restore the exact prior publication.
Unavailable data stores no fabricated cache fingerprint or zero series.

Git read capture uses the same defensive read-only/query-only connection, a maximum
two-second progress handler plus final wall-clock check, fixed SQL, bounded rows, and
one-row lookahead. A missing, cleared, or conflicting opaque project association
cannot inherit an older key or authorize cost attribution. Daily retention is visible
as `daily_history_truncated` and cannot be reported as complete.

Public Git query mapping labels its durable day buckets as UTC and uses no raw-event,
filesystem, repository, or per-visible-row query. Exact alias recovery is a fixed
store-owned batch over at most 32 salted keys and 256 safe `ProjectAlias` candidates;
the installation salt and opaque project key never cross into the product snapshot.
The batch progress handler is cleared on every outcome. Usage-side deadline,
unavailability, stale evidence, or corruption disables only efficiency and cannot
erase independently captured Git facts; internal request/invariant errors still fail
the whole call.

The Git runtime stores raw candidates and object-ID frontiers only inside its bounded
process-lifetime slots. It runs fixed native commands before acquiring the shared
non-waiting writer lease, opens SQLite only while that lease is held, and publishes
through typed store methods rather than direct SQL. Pause invalidates frontiers,
cancels the exact child, and waits for cleanup; resume forces a rebuild. The
count-only health contract contains no path, repository/activity/project identity,
author, ref, output, or inner error text. `scripts/audit-git-output.ps1` verifies the
four Git production boundaries, dependency closure, fixed read-only command/lifecycle
patterns, Git-I/O/lease/store ordering, no vendored upstream source, and release
library strings.

The facade exports fixed request methods only and never arbitrary SQL, filesystem,
shell, HTTP, plugin, or provider-mutation authority. Public query results omit source
IDs and private source content. Obsolete accounting versions fail truthful quality
authority with a stable warning instead of silently blessing old interpretation rules.
The future CLI/MCP cursor wire format must repeat these bounds and redaction tests; P2-A
does not claim a serialized cursor schema.

Archive writes use explicit transactions and compare expected generation, identity,
checkpoint, and proof state. Failed writes roll back completely. Incomplete, cancelled,
or failed scans MUST NOT authorize destructive source reconciliation. Overflowed
canonical event mutations also roll back dataset generation; the generation
reveals only a monotonic mutation count and no event, path, or source content. Replay
evidence remains separate and cannot silently invalidate a cursor on a no-change scan.

Schema-v9 aggregate and price-basis rows are store-owned derived data. SQLite triggers are the final
mutation boundary: published current rollups, event counts, and dataset generation
must change in the same transaction, and missing expected published rows fail the
source mutation closed. Rebuilds retain no history-sized Rust map or long-lived read
transaction, process fixed keyset pages capped at 2,048 events under writer authority,
and publish only after the expected generation and exact total still match. The cap
expands to at most 18,432 cleanup/derived rows per call and remains subject to the
rebuild-page latency and process-resource gates. UI, CLI, MCP, plugins, and LLM
connectors receive neither aggregate write authority nor arbitrary SQL. They cannot
force a raw-history fallback while aggregates are unavailable.

The pricing engine is synchronous and has no filesystem, environment, SQLite, HTTP,
async runtime, or mutable global cache. Release-pinned catalog changes are reviewed
source changes; runtime LiteLLM/models.dev refresh is forbidden. Overrides are
immutable, bounded to 512 validated entries, use strict decimal parsing and exact
aliases, and reject the whole candidate on duplicates, alias chains/cycles, incomplete
new models, unsupported combinations, or invalid rates. Public Debug/errors reveal
counts and stable codes, never raw model lists, SQL, paths, prompts, responses,
commands, or reasoning text.

Price queries accept only typed ranges, scopes, breakdown identities, and opaque
session keys already returned by the exact token capture. All collections, SQL text,
parameters, transactions, deadlines, and returned details are bounded. The release
gate traverses the pricing/query dependency closure, rejects network/async client
crates, scans production source and release libraries for runtime pricing-network
locators, and repeats catalog/override/mode/query switches under private-memory,
handle, thread, USER, and GDI plateau checks.

Aggregate overview reads accept only validated enum widths, signed UTC boundaries,
at most three adjacent aligned segments, and at most 32 typed scopes. They bind only to
fixed SQL over the active rollup generation, use no caller expression or identifier,
never touch raw event tables, and clear progress cancellation before connection reuse.
Series and breakdown requests add no expressions: point topology and collection kinds
are typed and bounded, all four breakdown statements are host-selected fixed SQL, and
the combined capture releases its exact transaction before returning owned values.
Session requests use only fixed `usage_session_rollup` statements. Their composite
cursor and exact lookup key retain raw session identity only inside an opaque
dataset-bound store value; no public getter, Debug output, error, or frontend wire
projection may reveal it. Page scopes are typed and capped, continuation is keyset-only,
detail returns fixed model/project dimensions, and a missing exact key cannot trigger a
raw-history fallback.

The P2-B scale/privacy gate uses deterministic current and immutable-legacy fixtures,
scans public analytics/session Debug and stable errors for archive paths, source IDs,
fingerprints, SQLite text, prompts, responses, commands, and reasoning, and repeats
400-point/four-breakdown snapshot replacement plus cooperative rebuild resume. This
evidence is required in addition to type opacity; absence of a current serialization
surface is not treated as proof for future CLI/MCP wire values.

Calendar requests accept only a bounded validated IANA name or the system zone when it
resolves to an IANA identity. They accept no timezone file, POSIX rule, URL, path, or
silent UTC fallback. Historical boundaries not aligned to a UTC minute fail with
`unsupported_time_boundary`; they are never rounded or answered from raw history.
Public session cursors retain only an opaque store cursor plus the bounded canonical
scope set and reject filter changes before SQLite access. Public Debug keeps the raw
session identity redacted.

Scan authority is provider/profile qualified and store-owned. One bounded scan set
contains one child per exact scope; an observation may update only the matching
running child. Only successful completion of that child may derive presence from its
exact `last_seen_scan_id` set. Ordinary append, late registration, partial results,
and caller-supplied counters cannot clear `missing`. Parent/child creation and
complete-only presence finalization are atomic; injected failures after their first
mutation prove rollback. This presence signal still cannot delete canonical usage.

Production replay begin accepts only one complete scan set and persists its exact ID.
It creates staging generations only for sources whose scope, last-seen child, and
present state match that set. The same set completion and bidirectional source
membership are revalidated during continuation, seal, and promotion; a later scan or
tampered missing/member row invalidates the old staging revision. Zero-present-source
promotion is retention-only: it creates no generation, preserves missing sources and
their current generations, and advances projection provenance atomically. Injected
failures after revision creation and generation creation leave no staging state.

Scan-history pruning is store-owned and executes in the same immediate transaction as
parent close. It deletes at most 64 whole closed sets per call only after every child
scope has 32 newer closed sets, and only when neither source presence nor replay
provenance references the candidate. Running sets are excluded. Candidate IDs live in
a bounded SQLite temporary table, not an application collection; scan-related foreign
keys are checked before commit. An injected post-prune failure restores the parent,
children, references, and temporary schema to their prior state.

Replay rebuilds use a SQLite-owned fixed all-registered-source manifest, store-owned
accounting versions, and an evidence-epoch compare-and-swap. The product path stages
all sources with set-based SQL and retains at most one 256-row validation page; stored
source counts never size application collections. Legacy v1 rows are copied into an
immutable snapshot, exact v2 archives are migrated non-destructively through strict
v3, and exact v3 canonical rows migrate transactionally to strict v4 provenance.
Exact v4 scan/replay rows migrate transactionally to strict v5 provider-qualified
scan sets; ambiguous scope ownership, incoherent terminal state, or altered schema
fails closed.
Replay observations, classifications, selections, and checkpoints remain private
staging state until an explicit sealed promotion.
Stored parent facts from another accounting version MUST fail closed; staging MUST NOT
change current event pages, current source metadata, or externally visible totals.

Provider checkpoint preparation is allowed only for the exact unsealed revision,
evidence epoch, source key, and untouched pending staging generation. The checkpoint
must be zero-offset, empty-anchor, incremental, bounded, and match the registered
logical identity. It may carry only opaque path-private physical identity and adapter
resume bytes. Stale, touched, complete, malformed, or current-generation targets write
nothing. Preparation synchronizes durable-work epochs before committing.

Late relations are accepted only from a fixed-manifest source whose validated provider,
profile, source ID, committed range, revision, and evidence epoch match. The archive
stores only bounded identifiers plus the deterministic first source-key/offset tuple,
never source content or paths. Continuation rejects any stale durable-work epoch before
writing. Parent disagreement and confirmed cycles are irreversible conflict; ancestry
or fanout bounds remain explicit pending work and cannot be treated as proof.

Continuation may use only the transactional closed-source aggregate; it cannot
authorize promotion. Seal and promotion repeat keyset-paged exact
all-registered-source manifest completion, full-prefix checkpoint
and chunk coverage, one replay row per staged observation, eligible-only selections,
compiled accounting versions, exhausted work, and foreign-key integrity in one
immediate transaction. Promotion additionally requires zero pending rows and a valid
prior projection owner. It removes replay-only prior contributions, retains absent or
conflict-only replay-verified events with their older origin revision, installs new
eligible selections as direct rows, and swaps revision, generation, and source-pointer
state atomically. The canonical projection has no foreign key to a deletable source
generation; strict revision provenance plus promotion validation replaces that unsafe
lifetime coupling. Injected failure at every mutation phase MUST roll back values,
provenance, generations, and revision to the prior canonical page. Recovery may
discard only an exact epoch-matched staging revision and staging generations; current
and immutable legacy state remain untouched, and any integrity failure rolls the
discard back.

Reader `Truncated`, `IdentityChanged`, missing, or rewrite classifications never
authorize erasure. A complete sealed replacement may carry omitted replay-verified
events; partial/cancelled input cannot. Only replay classification may automatically
suppress a prior contribution. Unverified legacy rows remain in the separate immutable
snapshot rather than being mixed into replay-verified totals.

## TM-SEC-005 — Extensibility

Skins are declarative data only. They MUST NOT execute code, call external processes,
or introduce filesystem, network, SQL, or script expressions.

## TM-SEC-006 — Source adapter capabilities

The Codex adapter is statically linked. External provider packages are WebAssembly
Components executed one package per on-demand isolated host process. Native DLL,
shared-library, and arbitrary executable plugins are forbidden in the normal product.
The engine owns cancellation, timeouts, backpressure, package generations, and maximum
chunk/count sizes.

External components receive no ambient filesystem, network, HTTP, SSH, environment,
subprocess, SQL, archive, UI, MCP, or credential authority. Host imports expose only
user-granted read-only filesystem scopes, allowlisted bounded HTTPS requests,
host-injected credential slots, and clocks. A component requesting both local raw data
and outbound HTTPS requires a separate data-egress grant.

Raw source/provider content may exist only within one bounded adapter request and MUST
NOT enter the archive, snapshots, diagnostics, logs, crash reports, or external
interfaces. Credential bytes are attached by the host and MUST NOT cross the component
ABI. Plugin traps, timeouts, OOM, protocol violations, and crashes fail only that
provider operation and cannot commit a partial staging generation.

The engine coordinator stores no provider ID, source ID, path, descriptor, payload, or
request history. One active cancellation token uses an atomic flag; active and pending
deadlines use caller-supplied monotonic ticks, never wall clock. Stale request IDs fail
without cancelling or completing newer work, and ID exhaustion cannot wrap.

Engine runtime ports preserve the same separation structurally. `Adapter` has no
archive/store argument and can emit only sealed provider-neutral identity, bounded
drafts, opaque checkpoints, chunk proofs, counters, diagnostics, and completion
quality, plus the separately sealed transient repository hint when that capability is
declared. `Archive` has no provider descriptor/raw-input/hint operation and accepts
only normalized discovery state or scope-exact canonical batches. Adapter checkpoints are
opaque and capped at 32 KiB; observation and relation batches cap independently at
256; chunk updates cap at 18; every persisted counter fits SQLite `i64`. Full rebuild
lends exactly one temporary descriptor-bound source reader per callback. It exposes
no raw or serializable path, file handle, raw bytes, or replay-source collection; the
optional sealed candidate can only be taken synchronously and never enters archive
state. Debug and
error surfaces redact identities and checkpoint/proof bytes. Compile-fail contracts
reject private-field construction, path substitution for source identity, and raw
byte archive writes.

The one-shot executor validates scope ownership before an adapter discovery reaches
the archive and validates revision identity/epoch monotonicity before trusting every
returned replay handle. It rejects non-progressing adapter checkpoints and mismatched
replay-batch identity, caps continuation calls per execution, and never retains a source
list or history-sized batch. Exact archive preparation rejects extra/duplicate files;
exact seal rejects omissions. Cancellation and deadline are checked across the full
execution phase matrix. Only writer-lease contention is exposed as `busy`; later port
misuse of that code fails the operation. Replay failure attempts exact
last-confirmed-handle discard, and cleanup failure is reported separately without
exposing data or masking the initiating stable code.

The real writer lease resolves one controlled local parent, rejects relative,
UNC/device/mapped-remote locations, symlink and reparse ambiguity, and opens one
persistent empty sidecar without truncation. `File::try_lock` contention is the only
`busy` source. The guard owns one handle; drop or process death releases the OS lock.
The sidecar and Debug/error
surfaces contain no owner, PID, timestamp, path, OS message, payload, or history and
the sidecar is never deleted on unlock.

Store replay append binds at most 256 late relation drafts to the same source,
checkpoint, revision, and epoch as its at-most-256 canonical events. Provider/profile/
source mismatches and out-of-range relation offsets fail before commit. Events,
relations, selections, queued continuation, chunks, checkpoint, and epoch share one
transaction, preventing a fault from advancing archive authority while the engine
still holds the prior handle. Debug exposes only the bounded relation count.

The built-in runtime checkpoint codec is manual and path-free. Decode checks the
32-KiB total bound before resume allocation and rejects unknown envelope/reader/parser
versions, unknown flags, non-canonical absent fields, logical identity mismatch,
truncated payloads, and trailing bytes. The runtime never formats inner provider,
reader, or store errors into a port result. Bootstrap keeps descriptors inside
synchronous adapter callbacks, and `StoreArchive` receives only sealed identity,
canonical facts, checkpoint state, and checked handles. Bootstrap/full rebuild is not
exposed as a live watcher path.

Incremental refresh uses the same path-private callback boundary and a separate
archive-generation CAS. Exact complete scans are the only source-admission authority;
new sources are provisional until atomic publication, and non-empty admissions remain
`partial` until read. A preflight pass validates every present file's logical/physical
identity, length, modification observation, and bounded anchor before any tail write.
Tail transactions materialize only affected fingerprints and roll back projection,
relations, work, chunks, checkpoint, revision epoch, and archive generation at every
injected boundary. Replacement, rewrite, truncation, or anchor mismatch writes only a
CAS-checked `recovery_pending` marker and preserves prior canonical usage. Profile-scope
drift uses the same marker. Full rebuild can recover only a generation-zero provisional
source with no replay/observation/chunk facts, and rewrites its physical identity from
the newly opened descriptor. Provisional-admission overflow fails into the same
non-destructive recovery path before retaining an over-bound key. No path, raw line,
incomplete tail, descriptor, or checkpoint bytes enter SQLite or reports.
Malformed, incomplete, and oversized relevant input are blocking adapter diagnostics:
they become only fixed `invalid_data`, cannot advance a checkpoint, and cannot turn a
failed rebuild into a complete publication. The previous canonical publication remains
readable behind `recovery_pending` until authoritative input is valid again.

The pinned `notify = 8.2.0` backend is isolated inside `tokenmaster-runtime`. Callback
code inspects only the rescan bit, discards every event/error object immediately, and
updates one fixed atomic pathless aggregate through a capacity-one non-blocking wake.
It never logs, formats, stores, forwards, or publishes event paths, backend errors, or
event history. Root replacement is capped at 64, validates length/local namespace,
canonicalizes existing directories, rejects duplicates and reparse/symlink ancestry,
and never watches a broad ancestor for a missing root. Old callback generations fail
their atomic generation check. Watcher errors and clock rollback only force
authoritative reconciliation; they cannot mutate archive truth.

The scheduler submission callback receives only `RefreshUrgency`. Its owned thread has
a thread-local panic-output filter, checked counters/time arithmetic, fixed phase, and
joined shutdown. A failed submit faults without retry. Tests prove one aggregate and
one engine follow-up for 10,000 hints and eventual return of process handles/threads to
baseline after 32 Windows watcher replacements.

Windows suspend/resume registration uses one static callback and capacity-one atomic
signal. The callback ignores context/setting pointers, maps only documented power-event
codes, returns a stable success code, and cannot call runtime, SQLite, logging, UI, or
allocation. The OS registration handle remains private to the platform guard. Explicit
shutdown reports stable unregistration failure and keeps the guard retryable; a failed
drop leaves the singleton closed to further registrations rather than allowing repeated
handle growth or unsafe callback-context reuse.

`LiveRuntime` acquires the writer lease before SQLite open, migration, orphan-scan
closure, or staging recovery. It resumes only the exact staging revision whose status,
accounting versions, scan binding, revision, and epoch validate; ambiguous identity or
unavailable storage is preserved and fails closed rather than authorizing deletion.
The scheduler starts paused and cannot submit until worker and watcher ownership are
installed. One admission mutex orders scheduler submission against pause/shutdown.
Every refresh write obtains the OS guard, and full rebuild consumes that same
pre-acquired guard instead of racing through a second acquisition. Shutdown drops
watcher ownership, joins the scheduler closure and its worker reference, then cancels
and joins the worker. Combined Windows evidence requires handles and threads to return
to baseline; fixed snapshots and Debug expose no source/archive paths or inner errors.

The deterministic worker uses only capacity-one standard-library wake/result
channels and the constant-state coordinator. External clock and execution callbacks
run outside the worker mutex. Stale cancellation cannot affect a newer request;
shutdown and `Drop` cancel the exact active permit and join the owned thread. Caught
callback panic publishes only fixed `failed`/`panicked` state, abandons the single
follow-up, and closes admission. An outer redacted boundary also faults and clears
runtime state if another worker port panics. One thread-local-filtered process hook is
installed on first spawn: non-worker panics retain the prior application hook, while
worker panic payload/location output is suppressed. Application code MUST compose its
custom hook before worker creation and MUST NOT replace the hook while workers exist.
No panic payload, wrapped error, path, checkpoint, or provider content enters the
completion channel, snapshot, archive, or diagnostics. `tokenmaster-engine` fails
compilation under `panic=abort`; only unwind builds can provide the required contained
fault transition.

The application composition root is separate from `tokenmaster-desktop`. Only
`tokenmaster-app` may combine platform path validation, Codex discovery, live/quota/
reminder runtime owners, the read-only desktop controller, and the Slint event loop.
The frontend remains directly free of platform, Codex, provider, store, runtime,
process, network, browser, shell, and arbitrary filesystem authority.

Installed/portable selection validates the current executable and every selected
directory as local, absolute, non-link/non-reparse state. A portable marker must be a
zero-byte regular non-link file. An invalid marker never falls back to installed
storage. Only one exact child is created non-recursively and revalidated. Paths remain
private to composition and cannot enter application errors, UI/product snapshots,
worker completions, or diagnostics.

Application runtime notification is a weak, lossy wake capability only. It grants no
read/write/provider action authority, retains no runtime/window strongly, allocates no
thread/timer/queue, and recomputes only fixed product health. Shutdown removes the
shared bundle before joins, closes runtime admission, joins the query worker, then
closes reminder, quota, usage, and nested Git ownership without holding the bundle
mutex.

The Codex quota runtime is a separate composition from usage ingestion. Automatic
discovery captures only the bounded process `PATH`, visits absolute entries in order,
and validates the exact platform-native Codex filename through the path-private
command descriptor. It does not use shell resolution, `PATHEXT`, aliases, registry
commands, `.cmd`, `.ps1`, JavaScript wrappers, package managers, browser state, or
credential files. Explicit executable selection is authoritative and invalid explicit
configuration fails closed rather than selecting another binary.

Provider I/O completes before writer admission. The runtime may retain the archive
path and an unopened lease factory, but it holds no file guard, SQLite connection,
query snapshot, UI callback, or usage-engine state while the child runs. After a
successful owned normalized snapshot, it tries the existing process writer lease once,
opens the writable store only under that guard, applies at most 32 independent
idempotent quota observations plus at most one separate benefit observation, and drops
store/guard before publishing count-only health. Quota windows and benefit inventory
use separate exact transactions under the same non-interleaving guard. A benefit
failure cannot roll back committed quota; a quota failure does not authorize a false
cross-domain success and may still leave an independently successful benefit
transaction. Writer contention writes nothing and returns `busy`. Cancellation after
source I/O writes nothing; cancellation during bounded publication may leave only
exact already committed transactions and reports their domain-specific counts without
claiming rollback.

Quota-runtime health is independent from usage-engine health and excludes executable/
archive paths, pseudonymous account identity, window identity, labels, quota values,
benefit/lot values, raw frames, provider messages, email, credentials, reset-credit
IDs, and inner platform/store errors. It exposes only bounded per-domain observation,
processed, status, failure, pending-due, lot-change, and last-success facts; inconsistent
internal arithmetic fails closed. Permanent incompatibility remains on the 15-minute
cadence; only bounded transient process/lease failures may use the 60-second cadence.
The runtime owns zero child processes while idle, at most one during a poll, two
constant-state host threads while running, capacity-one scheduling/worker wakes, and
one latest snapshot. Shutdown joins owned threads and the transport reaps its child.

The benefit reminder runtime has no provider, network, browser, credential, shell,
plugin, usage-ingestion, quota-transport, or direct-SQL authority. It tries the shared
writer lease before opening SQLite and calls one bounded store-owned operation only.
The store commits an immutable delivery/outbox row before returning a notification
value; runtime cannot fabricate, edit, or delete it. A separate immutable schema-v12
acknowledgement is inserted only after the presenter confirms display. Before that
point restart replays the outbox row; after it, deduplication remains permanent.
Writer contention creates no archive and selects one bounded 60-second retry. A
dedicated scheduler retains only one nearest due/retry deadline and one coalesced
urgency; the worker and scheduler have fixed joined ownership and thread-local
panic-output redaction.

At most one owned provider-neutral batch of 256 in-app events is retained. While that
batch is pending or leased for presentation, scheduling is backpressured rather than
overwriting or accumulating events. A failed display releases the same bounded batch;
acknowledgement contention keeps it retryable. Health excludes archive paths and all
provider/account/workspace/lot/delivery identity or values. Delivery values expose
only approved presentation facts: kind,
quantity, localization key, lead/channel, due/expiry, and committed delivery time.
Their sealed acknowledgement key has no public accessor and is omitted from `Debug`.
They contain no raw provider ID/title/description, scope identity, target window,
credential, prompt, response, command, source content, or absolute path. This event
surface grants no visible-UI, OS-notification, snooze, quiet-hours, or activation
authority.

## TM-SEC-007 — Benefit activation authority

Banked-reset inventory read, official activation link, idempotent activation,
activation status, and provider lot selection are separate narrow capabilities.
Inventory read or generic allowlisted HTTPS MUST NOT imply mutation. Manual inventory,
browser page state, provider-supplied UI, CLI/MCP/LLM access, and external plugins MUST
NOT authorize automatic activation.

Automatic activation defaults off and requires explicit scoped local policy, fresh
high-confidence inventory and quota evidence, known expiry/effect, compare-and-swap
admission, a durable pre-action intent, one in-flight action per scope, official
idempotency/status semantics, and bounded reconciliation. Ambiguous outcomes MUST NOT
be retried blindly or reported as success. Browser scraping, synthetic clicks, session
cookies, private endpoint replay, raw response storage, and secret-bearing diagnostics
are forbidden.

## TM-SEC-008 — Backup and recovery containment

Configuration, backup packages, encrypted envelopes, catalogs, SQLite candidates,
run markers, and recovery journals are untrusted. Validation order is exact header and
bounds, bounded encryption work, streaming decompression count, hashes, defensive
SQLite open, full integrity and foreign-key checks, exact schema, then TokenMaster
semantic invariants. Declared sizes MUST NOT cause proportional allocation. Unknown
required versions, flags, entries, codecs, trailing data, oversized windows, excessive
password work factors, and ambiguous controlled-directory state fail closed.

The implemented Task 2 platform primitive accepts only one validated local directory
and one restricted exact child. It rejects links, reparse points, directories, device/
remote locations, traversal, reserved names, and unexpected types; retains at most 32
create-new staging candidates; caps a file at 64 GiB plus 2 MiB and each write call at
256 KiB; and verifies length plus SHA-256 by bounded streaming after flush, close, and
reopen. Public errors and `Debug` contain no path or OS message. Windows uses
`MoveFileExW(MOVEFILE_WRITE_THROUGH)` without cross-volume copy fallback and
`ReplaceFileW` with an exact independently verified old-target backup. Any uncertainty
after OS publication is `RecoveryRequired`, and ambiguous rollback preserves staged/
backup artifacts for the later journal. Unix uses no-overwrite hard links plus atomic
rename and synchronized file/directory entries, but is not Windows release evidence.

Implemented Task 3 keeps the generic redundant-record store crate-private and permits
it to construct only the six literal settings/run/recovery A/B children. The state
authority audit permits one bounded writer import, only `io::Result`, `io::Error`, and
`io::ErrorKind` uses, and one exact platform import; the expanded 34 mutation cases reject alias
reuse, caller-selected children, public generic authority, transitive authority, and
the earlier source/metadata bypass corpus. Record reads cap actual bytes at 1 MiB plus
fixed envelope overhead before decode. Writes do not retain a full encoded JSON copy:
they measure/hash once, stream a second pass in at most 256 KiB calls, and reject any
length/digest drift before publication. Equal-generation disagreement is integrity
failure, both invalid slots remain preserved, and every failed post-publication reread
is `RecoveryRequired`. Windows evidence now includes a separate inactive-slot suite
with an injected before/after replacement boundary, 40 deterministic process kills,
20 replacement-entry race kills, and state-level process deaths before partial write,
after seal/before publish, and after publish/before reread of generation 3.

Implemented Task 4 exposes only a fixed-purpose `SettingsStore` constructed from a
validated local-directory capability; it does not reexport generic record/file
authority or accept a path. The authority audit now permits exactly the original
bounded record/platform import and one exact `ValidatedLocalDirectory` import for
that typed constructor, while retaining all six fixed-child and alias-reuse gates.
Schema v1 is strict and capped at 1 MiB. Unknown, duplicate, newer/older unsupported,
invalid enum/range/relationship, and forbidden-state fields fail before publication.
Portable input cannot contain or overwrite the device-local route. Errors, `Debug`,
previews, and serialized values are regression-tested against password, credential,
absolute-path, prompt/response/command, and source-content canaries. A two-invalid-
slot load preserves both files; only an explicit validated save replaces one and
keeps the peer as evidence. Restore identity is a nonzero generation plus portable
SHA-256 digest and has a fixed reread verifier for later journal resume.

Implemented Task 5 never copies the live SQLite main file. It uses Online Backup so
committed WAL state is included, steps pages under cancellation/deadline control, and
accepts no caller SQL or output name. Candidate verification enables defensive and
query-only policy, disables trusted schema and both double-quoted string modes,
enables cell-size checks, disables mmap, fixes cache/busy policy, pins the bundled
SQLite identity, and applies explicit SQLite allocation limits. Integrity, foreign
keys, exact schema/indexes, stored counts/generations, and application semantics are
separate fail-closed gates. Only `VACUUM INTO` from an already verified isolated
snapshot may create a compact candidate, which is independently reverified.

Verification binds physical file identity, exact length, and streaming SHA-256 before
and after proof and every consumer; pathname replacement cannot inherit prior proof.
Busy, I/O, cancellation, deadline, schema-newer, and stale-identity outcomes are not
corruption authority. Candidate cleanup exposes an explicit failing discard, one
bounded failure counter, and a fixed-name recovery pass. Recovery is called only
under the single-active-maintenance-operation invariant; it is not a concurrent
candidate-deletion API. Public errors and Debug expose neither paths nor SQLite text.

Implemented Task 6 pins `zstd` 0.13.3 with default features disabled and permits only
the transitive `std` feature; multithreading, training, legacy, and experimental
features are absent. Package parsing is exact typed little-endian decoding with fixed
64 KiB buffers, an 8 MiB maximum decoder window, declared content size, frame
checksum, single-frame termination, checked expanded counters, entry SHA-256,
descriptor binding, and whole-package SHA-256. The mutation corpus covers every
structural region, unknown/duplicate fields, false/overflowing lengths, concatenation,
trailing data, a missing frame end with otherwise resealed outer digests, checksum and
digest corruption, an actual 16 MiB-advertised frame, and a 300-byte frame whose
content-size header lies that it expands to 256 bytes. Output is counted before each
sink write.

The public state codec has no generic `Read`, `Write`, archive iterator, entry name,
path, SQL, shell, or network surface. Platform readers detect early EOF and bytes
appended beyond the opened length. Writers and backup restore sinks remain unpublished
until complete verification and final file sealing. Every codec or seal failure first
irreversibly clears the stage receipt, closes/removes the unpublished file, and poisons
the handle; even cleanup failure cannot restore publication authority and is surfaced
as `RecoveryRequired`. Late-footer, partial-writer, bomb, and platform contracts prove
that later seal and publication return `InvalidState`. The reliable-state authority
suite now has 37 mutation cases, including explicit public generic-stream and
cryptographic dependency-drift bypasses.
Wire bytes, stable errors, receipts, and `Debug` exclude private archive metadata and
OS text.

Implemented Task 7 pins `age` 0.12.1 with default features disabled; TokenMaster does
not enable age CLI, plugin, SSH, armor, async, unstable, or web features and implements
no cryptographic primitive itself. Export constructs the standard scrypt recipient
and fixes `log_n = 16`; import constructs the standard identity and sets the maximum
accepted factor to 16 before attempting stanza unwrap. An attacker-selected larger
factor therefore maps to bounded capacity failure before scrypt derivation.

Encryption is manual-export-only and is bound to an opaque previously verified backup
receipt: the same pass streams once while independently checking exact source length
and complete-file SHA-256. A same-length substitution fails and poisons ciphertext
output. Decryption authenticates the complete binary age stream and passes it directly
to the private typed backup parser before sealing only its verified database stage;
authenticated non-package plaintext, wrong password, malformed/non-scrypt header,
header/body/final-tag
corruption, truncation, trailing data, or platform I/O cannot publish partial bytes.
Every failure irreversibly discards its output; cleanup uncertainty is
`RecoveryRequired`.

`BackupPassphrase` is non-cloneable and redacted. Its age `SecretString` allocation
zeroizes on drop. Constructors move caller-owned UI strings into secret storage
immediately, clear every supplied field on every outcome, count exactly 12 through
128 Unicode scalar values, compare confirmation byte-for-byte, and perform no trim or
Unicode normalization. Passphrases are absent from settings, packages, manifests,
receipts, errors, `Debug`, health, arguments, and environment. Automatic backups are
explicitly rejected by the encryption API and remain recoverable without stored
credentials.

Implemented Task 8 confines automatic packages to the canonical local `backups`
directory and 32 exact private slot names. The platform rejects unexpected children,
non-files, symlinks/reparse points, hard links, duplicate physical identities, and
stale directory/entry capabilities. Publication is available only through the owning
`BackupDirectory`; an unpublished `BackupStagedFile` exposes no raw file, path, rename,
or independent publication method. It can open only a path-free reader after seal so
the typed package parser can verify the exact unpublished candidate. Deletion first
performs a write-through rename to one exact private tombstone; an interrupted move is
explicit `RecoveryRequired` rather than an inferred successful delete.

Catalog header validity is not verification authority. Cold rebuild hashes every
complete file but marks it only `HeaderValid` or `Corrupt`; current full package proof
is required for `Verified`, and duplicate complete-file hashes fail integrity. A
retention preflight deletes nothing and consumes the proof from the exact sealed stage.
After publication, every prior package must still be present and the candidate proof
must rebind exactly. Immediately before each delete, the complete current verified set
and selected target are fully rehashed and their typed headers rechecked, followed by
a current physical directory-generation check. A same-length in-place corruption of
the candidate, target, or another protected point therefore preserves all files and
returns `RecoveryRequired`; unchecked/corrupt points are never deletion-eligible.

The state authority audit allows only one named typed `BackupPackage` writer and one
verifier over the sealed backup stage, rejects duplicate/raw backup-token methods, and
forbids direct state filesystem enumeration. This does not extend the threat model to
malicious code running under the same user token: a hostile same-user process can
still race after a completed validation. Cross-user/local-ACL containment and
evidence-preserving crash behavior are the implemented claims.

The current exact-child read checks the pathname type before opening and validates the
opened regular-file length, but does not claim hostile same-user no-follow/open-handle
identity resistance against replacement in that narrow interval. The documented
threat boundary does not treat another process under the same user token as hostile.
If that boundary changes, platform-specific no-follow open plus handle/path identity
validation is required before raising the claim; this is a recorded hardening item,
not current release evidence.

Implemented Task 9 adds no path, generic stream, SQL, shell, network, UI, or async
authority. State depends on store only for the exact `BackupControl` and
`VerifiedBackupCandidateReader` capabilities; the source audit permits those named
imports plus four exact standard-library synchronization/thread import blocks. It
still rejects arbitrary public paths/streams, direct filesystem enumeration, Slint,
Tokio, and a second archive/extractor surface.

The verified-candidate reader revalidates the complete physical identity, length, and
SHA-256 before open, binds the opened handle, counts/hashes every bounded chunk, rejects
early EOF and appended data, and repeats namespace identity verification after EOF.
The sole state/store package bridge gives source errors precedence over destination
errors and irreversibly discards the unpublished stage on replacement, truncation,
append, cancellation, codec, seal, or destination failure.

Maintenance owns one capacity-one worker and one capacity-one scheduler wake channel.
Ten thousand hints retain one active request and one merged follow-up, not attacker-
proportional queue nodes. Only one shared scheduler timeout exists. Worker panics are
contained behind the existing thread-local redaction pattern; `Debug`, completion,
and snapshot values contain fixed enums/counters only. Pause and shutdown cooperatively
cancel the linked store control and `Drop` joins both threads. A compare-exchange makes
final publication non-cancellable, preventing a late cancel from reporting that an
already published backup was discarded. The coordinator additionally rejects
`Published` before that state and `Cancelled` after it, so the executor cannot bypass
the state machine by returning an advisory enum.

Automatic scheduling is ineligible before first healthy publication and while the
source is suspect; exact `Healthy` startup truth seeds a new monotonic interval anchor,
while `HealthyUnpublished` does not. One failed source attempt may continue only as a fresh retry with
the same root request and backup purpose; two failures against the same opaque source
identity enter `Suspect`. Source retry is not a public submit purpose. Disabling
periodic scheduling also removes a pending periodic-origin follow-up but preserves an
already owned internal retry. Mandatory pre-migration, pre-restore, and pre-destructive
guards remain active. A second guard cannot overwrite
an unresolved guard, and only the separately retained matching final completion may
authorize its mutation. Empty first install and an already quarantined definitively
corrupt source are explicit typed bypasses, not inferred success.

Implemented Task 10 preserves the dependency and authority boundary. Platform alone
owns paths, exact main/WAL/SHM names, operation-derived staging/quarantine names,
native replacement/move, and rollback. Store alone owns the recovery copy and complete
defensive SQLite verifier. State receives only sealed readers/stages, fixed artifact
facts, catalog selections, verification proofs, and the matching lease guard; it has
no `std::fs`, path, SQL, shell, network, UI, or generic stream authority. The source
audit permits exact recovery imports only in their owning modules and its 52 mutation
cases reject recovery reexports, duplicate stage/control use, verifier use outside the
coordinator, and arbitrary recovery paths.

Recovery staging recognizes only operation-derived reservation/candidate/durable-stage
names, rejects links/reparse points/multiple links/unexpected entries, and has a global
three-artifact cap. It is deleted only after a valid `Absent` or `Complete` journal
result proves there is no pending restore; any unrecognized evidence is retained.
Actual available space must cover `max(2B, B+A) + 8 MiB` before staging, where `B` is
the selected database length and `A` is the observed active-main length. Platform and
store both reject a fourth live staging artifact. The exact physical guard is
authorized before store verifier cleanup or platform staging cleanup, so a wrong guard
cannot erase pre-journal evidence. Quarantine accepts at
most three exact operation directories, never auto-deletes them, and validates every
child before forward or rollback work. Candidate and active verification use fixed
64 KiB streaming buffers plus store-owned bounded SQLite limits; no database-sized
buffer or history is retained in state.

The six-phase journal is durable before each next mutation. Resume also handles the
three mutation-before-journal windows explicitly: sidecars already moved, candidate
already atomically promoted, and portable settings already committed. The promoted
case reopens and fully verifies the active main when the sealed staging name has been
consumed; it never reconstructs publication authority from journal state alone.
Invalid dual slots, wrong lease, stale package/candidate/active identity, or ambiguous
artifact combinations preserve evidence and require safe mode. Process-death tests
prove complete old-or-new main bytes and exactly-once settings generation across all
of these boundaries.

The writer guard reopens the current sidecar namespace and compares its physical file
identity to the held locked handle before every recovery action. Corruption authority
is not publicly constructible: state runs the complete store verifier over the exact
active identity, accepts only its structural/schema/FK/count/generation/semantic
corruption classes, and treats busy, I/O, cancellation, schema-newer, and policy
failure as non-corruption. Native replacement errors restore WAL/SHM only when exact
old/new/staged/quarantined facts prove replacement never began; ambiguous facts require
safe mode without further movement.

The live archive keeps one fixed identity and writer sidecar. Whole-file restore MUST
hold that guard, close every SQLite owner, preserve current main/WAL/SHM in quarantine,
and publish only a complete reverified candidate through a redundant idempotent
journal. No failed settings save, backup, retention pass, migration, import, restore,
or rollback may delete the last verified state. A corrupt backup is skipped; a corrupt
catalog is rebuilt; corrupt control slots fall back only when the alternate slot is
independently valid.

An existing active main MUST be replaced through the platform's atomic replace
primitive with the old main preserved. A missing main with prior durable TokenMaster
artifacts MUST use the separately journaled same-volume promotion path; absence of a
main alone MUST NOT authorize silent first-install creation. Manual data-plus-portable-
settings restore MUST bind the exact staged settings generation/digest to the journal.
Automatic recovery MUST preserve current settings, and no restore may import
device-local state.

Automatic restore is authorized only by definitive active-archive corruption or
repeated semantic verification failure. Busy locks, access denial, disk exhaustion,
transient I/O, unsupported media, provider unavailability, and newer schema are not
corruption authority. SQLite `.recover`, main-only copies, ad hoc row edits, lock-file
deletion, arbitrary extraction, and automatic quarantine deletion are forbidden.

Implemented Task 11A validates every data-root, reliable-state, backup, staging,
journal, settings, and run-state capability against one physical root before any
startup mutation. Cross-root composition fails without publishing the unclean marker
or cleaning evidence. A bounded presence-only staging probe is permitted before that
marker solely to distinguish prior owned artifacts; after publication, store removes
only its exact verifier names and platform performs the strict typed namespace scan.
Unknown staging names, links, or types remain preserved and force safe mode.

Startup inspection is read-only and never migrates. Missing does not create a file;
supported old schema returns migration-required; newer schema returns upgrade-required;
busy, access, disk, cancellation, transient I/O, unsupported location, and policy
failure never become corruption authority. Zero-length WAL/SHM sidecars remain exact
valid artifact facts, while a zero-length main is invalid. A recovered archive is
accepted only after a later clean shutdown, and repeated unclean launches are bounded.
Application code may mark clean only after all owners join; the state layer cannot
infer that lifecycle from a successful SQLite check.

Implemented Task 12A keeps the reliable-state owner and the only backup maintenance
runtime in `tokenmaster-app`. Safe mode constructs no archive, query, controller,
quota, reminder, or maintenance owner. The backup operation accepts only sealed
root-bound capabilities and typed values; it exposes no path, SQL connection, raw
reader, platform publication token, or caller-selected filename. Catalog publication
is bound internally to a completely verified package rather than to opaque platform
authority crossing the application boundary. Cold catalog verification runs only in
the backup worker, verifies at most the fixed directory bound, and carries proofs only
for unchanged package identities. A malformed package is marked corrupt; I/O or
directory ambiguity fails the operation instead of fabricating a proof or deleting it.

Migration is fail-closed. A verified pre-migration package must exist before any
writable old-schema open, and a verified post-migration package must exist before the
new bundle is published. Run-state schema v2 records the exact source/target pair after
the pre point and before writable open. Failure before writable open preserves the old
archive; failure after a committed migration preserves the migrated archive plus the
pending post obligation. Either path retains the pinned pre point, the unclean run, and
safe mode. Restart must complete and durably clear the post obligation before live or
clean publication. The live-source
snapshot precheck may accept SQLite schema-format byte zero only while a regular WAL
exists; every copied candidate and published package still passes the strict standalone
format-four header plus complete database verifier.

Implemented Task 12B.1 command admission stores only fixed enums, checked request IDs,
one path-free generation/ordinal restore selection, one active permit, and one pending
value. It accepts no SQL, shell, network, path, filename, raw bytes, digest, or provider
identity. Exact cancellation is cooperative until a one-way irreversible transition;
restart pauses new admission and discards only the queued follow-up. The old bundle is
joined before a fresh archive guard is acquired. Runtime notifier generation is checked
while holding the bundle-slot mutex, preventing a check/use race into a replacement
controller. Task 12B.2a supplies selected restore below; Task 12B.2b.1 supplies the
bounded worker/manual-backup and sealed config subset below.

Implemented Task 12B.2a adds only the selected-restore authority contour. The public
choice remains generation/ordinal; application state alone may bind it to one fully
verified package identity, and that binding exposes neither path, filename, slot,
length, nor digest. Current-directory revalidation and every post-publication deletion
share one process-local RAII pin gate. A retention cycle admitted before selection must
replan around the late pin or fail closed. The same identity remains protected while
publishing the mandatory healthy-source `PreRestore` point. Only after every old owner
joins may a fresh fixed guard enter the existing journaled recovery coordinator.

The returned recovery receipt MUST be durably attached to the retained run session
before archive inspection, migration backup, or fresh-bundle construction. This closes
the crash window in which a completed manual restore could otherwise be rediscovered as
a new recovered launch. Restored legacy bytes MUST repeat the full pre/pending/post
migration protocol; direct current-bundle start is forbidden. Catalog mutex ownership
MUST NOT span online snapshot, package verification, recovery, or migration I/O. Any
stale selection, changed bytes, failed safety point, receipt conflict, or ambiguous
lifecycle leaves admission resumed only into safe mode with no archive owner.

Task 12B.2b/Task 15 binds native selection and the remaining reliable-state commands
without moving file or recovery authority into Desktop. The UI submits fixed intents
and receives only bounded path-free projections. Every durable mutation publishes the
non-cancellable `AtomicPromotion` phase at its exact irreversible boundary; a late
cancel cannot relabel published state. Dialog cancellation admits no worker command and
performs no write. Restore confirmation is bound to the exact reviewed generation and
ordinal rather than a mutable row index. Unknown counts/bytes remain typed unavailable,
and each queued follow-up publishes `Running` only when it becomes the executing permit.

Implemented Task 12B.2b.1 owns one operation thread with no async runtime, per-command
thread, unbounded sender, result history, path, file capability, SQL, shell, browser,
network, or credential authority. The wake channel and completion mailbox each retain
at most one value; the coordinator still retains only one active permit and one
follow-up. The callback runs outside the mutex. Worker panic output is thread-locally
redacted, the active command completes with a stable internal failure unless an exact
cancellation already won the shared state lock; in that race the command remains
cancelled while the worker still faults closed. Pending work is discarded and admission
closes in either case. Shutdown and `Drop` never detach the owned
thread. Clean run publication is forbidden unless the operation worker and every bundle
owner join successfully.

Manual backup crosses irreversible state before the worker hands mutation authority to
the maintenance runtime and holds the bundle slot through the exact-root wait, so
restart/restore/shutdown cannot replace the bundle during publication. Config export is
create-new only, uses a 2 MiB stage, and rereads the published file through the typed
codec. An occupied target is preserved. Config import fully consumes and verifies an
already open bounded reader before preview, retains no reader/path/raw bytes, and commits
only the retained typed candidate against the previewed base identity. Device-local
settings never enter the package or preview and remain unchanged.

Task 14 implements native file selection only inside `tokenmaster-platform`. Windows
uses the in-process Common Item Dialog in a balanced STA COM lifetime with exact typed
filters, filesystem-only/path-must-exist/no-link/no-working-directory-change flags,
strict save types, no test-file creation, and distinct `ERROR_CANCELLED` handling. It
does not invoke Explorer, shell, PowerShell, `cmd`, browser, network, or another process.
Every returned path is transient, length-bounded, split into one canonical validated
local parent plus one bounded Unicode child, and rejected for remote/device/mapped-
remote namespace, directory, symlink/reparse, hard link, wrong suffix, or oversized
input before a capability leaves platform. Final-component input opens use no-follow
Windows handle semantics and repeat the selected parent's physical-identity check after
open. `NativeFileDialog` is deliberately thread-affine, requires a current active owner,
and fails unavailable instead of running COM from an arbitrary worker or producing an
unowned dialog.

Input leaves platform only as an already open bounded reader. Output is bound to target
absence or opaque physical identity at selection, rechecks before stage and publication,
uses an adjacent create-new bounded stage, and does not truncate existing bytes before
the candidate is sealed. One selected output capability can successfully grant only one
stage over its complete lifetime. On Windows, a delete-capable retained handle pins
exact stage cleanup.
Existing replacement atomically captures the displaced path, checks that physical
identity after the replace boundary, rolls back a raced target, and deletes displaced
bytes only after the published identity and complete bytes verify. Ambiguous rollback
retains recovery evidence. Public results and `Debug` expose only selected/cancelled/
stable-error. The synchronous platform primitive is not yet bound to Slint, so active-
owner invocation, post-selection worker dispatch, and interactive acceptance are not
yet evidenced as application behavior.

The displaced-target protocol closes a single replacement between precheck and native
publication without expanding the repository-wide same-user threat boundary. Malicious
code repeatedly mutating the namespace under the same user token can still force
`RecoveryRequired` and retained evidence; it is not claimed to be contained or out-raced.
Native selection is unavailable on Unix in Task 14. The deterministic controlled
selector there revalidates parent/file identity before path deletion, but hostile
same-user unlink-after-check resistance remains a portability hardening gate before a
future Unix native selector may claim equivalent cleanup behavior.

No-backup reconstruction is authorized only after complete active verification proves
definitive corruption and the bounded verified catalog yields no usable point. It does
not parse or salvage corrupt rows. Store creates a fresh normal-schema archive through
its ordinary constructor; state fully verifies it before and after staging, journals
the explicit no-backup mode, preserves the prior main/WAL/SHM set in bounded quarantine,
and atomically promotes under the fixed writer guard. Resume treats journal backup
absence as valid only for that reconstruction mode.

The reconstructed archive is not healthy authority by itself. Application forces one
bounded recovery-urgency source refresh and waits through the worker's condition-based
completion channel before starting healthy backup maintenance. Failure or timeout
returns to safe mode with no fabricated data. The durable UI receipt is path-free and
must explicitly state that quota, reset-credit, reminder, and Git history are
non-reconstructible and unavailable. It never exposes quarantined bytes, filenames,
source identities, provider payloads, or raw errors.

A completed no-backup journal is not proof that source truth has been reconciled. When
bootstrap starts that reconstructed candidate, application preflight records a durable-
evidence-derived reconciliation obligation and presents recovery-required truth until
the bounded recovery refresh succeeds. Process death or refresh failure cannot clear
the obligation, start healthy maintenance, fabricate zeros, or authorize a second
destructive reconstruction of the already promoted archive.

Optional manual password protection uses the implemented standard age v1 stream with
fixed bounded scrypt work and never stores the passphrase. New passphrases are
confirmed, contain 12 through 128 Unicode scalar values, and are not trimmed or
normalized. Automatic backups remain credential-free and recoverable under the user's
local ACL.
Neither hashes nor local encryption are claimed to defend against malicious code
running as the same user or complete-disk destruction.

Task 16 closes the consolidated adversarial rail without widening any production
capability. Every proper prefix and every one-bit mutation of deterministic config and
backup packages is rejected. The existing Zstd window/bomb, age work-factor/password,
SQLite structural/semantic, six-phase crash, schema compatibility, settings rollback,
automatic data-only, and mandatory-safety-point contracts are bound into dedicated
state/application gates instead of being duplicated behind a weaker parser.

Archive quarantine now preflights main plus both active/quarantine sidecar locations
before the first new move. A WAL or SHM that was added, removed, changed, or conflicts
with an operation target returns `ArtifactMismatch` while the other active child remains
in place. An exact sidecar already moved by the same persisted operation is the only
accepted partial layout, so process-death resume can complete deterministically.
Per-move identity checks remain, so this preflight does not weaken later race detection.

The separate backup-package audit pins the seven-file typed codec boundary, twenty-three
adversarial/executable coverage anchors, two immutable MIT upstream references/notices,
and SHA-256 identities for the exact 196-package name/version/license and resolved-
feature closures. It rejects
process, network, shell, generic extraction, plugin, UI, and SQL authority in the codec
and scans 247 production Rust/Slint sources, a synthetic exported package, and the
release executable for fixed path, credential, prompt, response, reasoning, command,
and source canaries. This is a
development security gate, not the future P6 SBOM/advisory/attestation or release claim.

Task 17 adds a separate Windows release-mode containment rail. Deterministic 8/96 MiB
fixtures prove the package path stays below a fixed 64 MiB private-growth ceiling,
keeps the 8 MiB decoder window and one-thread compression policy, and creates no child
process. After backup, acquired-candidate cancellation, and real restore warm-up, 256
backup/import-cancel/retention cycles, 16 forced cancellation/recovery cycles, and 16
complete isolated restores must return private memory within 16 MiB, handles within one
handle for stable process-global measurement state, and threads/USER/GDI at or below
the original baseline. Verification staging returns to zero and retained bytes equal the filled
15-point plateau on every cycle. Manual compact age encryption is measured inside the
same window and must return to that original baseline rather than receiving a second
tolerance. The clean P3-D.0 receipt rejects dirty/mismatched identity and excludes all
private content. This rail grants no release authority and does not weaken the separate
interactive, soak, packaging, signing, or Unix portability gates.
