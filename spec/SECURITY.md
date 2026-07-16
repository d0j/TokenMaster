# TokenMaster security contract

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

Providers emit bounded observation/session-relation drafts only. They cannot create
event fingerprints, replay signatures/evidence, event IDs, replay dispositions, or
canonical events. Those values are created only by TokenMaster accounting code. Store
append MUST reject canonical events whose provider, profile, or source identity does
not match the registered source.

## TM-SEC-003 — Path privacy

Source descriptors are path-private. Public errors, debug surfaces, serialized values,
and diagnostics use stable codes and counters, never absolute paths or wrapped OS
messages.

## TM-SEC-004 — Archive integrity

The frontend query path cannot receive `UsageStore`, a SQLite connection, transaction,
statement, archive path, source key, or raw fingerprint. `UsageReadStore` opens only an
existing read-only archive, enables SQLite query-only/defensive/no-checkpoint policy,
disables trusted schema and double-quoted compatibility, validates exact schema before
reads, and exposes fixed queries only. Every public error and Debug value is path-free;
activity/cursor fingerprints are redacted. Deadline interruption is cleared on every
success/error path before reuse.

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
quality. `Archive` has no provider descriptor/raw-input operation and accepts only
normalized discovery state or scope-exact canonical batches. Adapter checkpoints are
opaque and capped at 32 KiB; observation and relation batches cap independently at
256; chunk updates cap at 18; every persisted counter fits SQLite `i64`. Full rebuild
lends exactly one temporary descriptor-bound source reader per callback and exposes no
path, file handle, raw bytes, or replay-source collection to the engine. Debug and
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
