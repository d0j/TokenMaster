# TokenMaster security contract

## TM-SEC-001 — Local-first operation

No telemetry, cloud sync, remote listener, automatic upload, analytics SDK, or
developer-controlled service is permitted. Any future local API binds loopback only;
MCP uses stdio only.

## TM-SEC-002 — Untrusted boundaries

JSONL, configuration, archive files, CLI/MCP requests, generated reports, and future
provider output are untrusted. Each boundary MUST validate type, size, count, encoding,
path safety, timeout, and allowed values before allocation or interpretation.

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

Archive writes use explicit transactions and compare expected generation, identity,
checkpoint, and proof state. Failed writes roll back completely. Incomplete, cancelled,
or failed scans MUST NOT authorize destructive source reconciliation.

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
256; chunk updates cap at 18; replay source pages cap at 256; every persisted counter
fits SQLite `i64`. Debug and error surfaces redact identities, checkpoint/proof bytes,
and keyset cursors. Compile-fail contracts reject private-field construction, path
substitution for source identity, and raw byte archive writes.

The one-shot executor validates scope ownership before an adapter discovery reaches
the archive and validates revision identity/epoch monotonicity before trusting every
returned replay handle. It rejects non-progressing adapter checkpoints and replay
cursors, caps continuation calls per execution, and never retains a source list or
history-sized batch. Cancellation and deadline are checked across the full execution
phase matrix. Only writer-lease contention is exposed as `busy`; later port misuse of
that code fails the operation. Replay failure attempts exact last-confirmed-handle
discard, and cleanup failure is reported separately without exposing data or masking
the initiating stable code.

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
