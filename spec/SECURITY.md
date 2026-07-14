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

Replay rebuilds use a fixed bounded source manifest, store-owned accounting versions,
and an evidence-epoch compare-and-swap. Legacy v1 rows are copied into an immutable
snapshot before v2 becomes current. Replay observations, classifications, selections,
and checkpoints remain private staging state until an explicit sealed promotion.
Stored parent facts from another accounting version MUST fail closed; staging MUST NOT
change current event pages, current source metadata, or externally visible totals.

Late relations are accepted only from a fixed-manifest source whose validated provider,
profile, source ID, committed range, revision, and evidence epoch match. The archive
stores only bounded identifiers plus the deterministic first source-key/offset tuple,
never source content or paths. Continuation rejects any stale durable-work epoch before
writing. Parent disagreement and confirmed cycles are irreversible conflict; ancestry
or fanout bounds remain explicit pending work and cannot be treated as proof.

Seal proves exact all-registered-source manifest completion, full-prefix checkpoint
and chunk coverage, one replay row per staged observation, eligible-only selections,
compiled accounting versions, exhausted work, and foreign-key integrity in one
immediate transaction. Promotion additionally requires zero pending rows and complete
evidence coverage of the prior visible projection, then materializes the newly
eligible events and swaps revision,
generation, and source-pointer state atomically. Injected failure at every mutation
phase MUST roll back to the prior canonical page. Recovery may discard only an exact
epoch-matched staging revision and staging generations; current and immutable legacy
state remain untouched, and any integrity failure rolls the discard back.

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
