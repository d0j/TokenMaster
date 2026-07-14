# TokenMaster security contract

## TM-SEC-001 — Local-first operation

No telemetry, cloud sync, remote listener, automatic upload, analytics SDK, or
developer-controlled service is permitted. Any future local API binds loopback only;
MCP uses stdio only.

## TM-SEC-002 — Untrusted boundaries

JSONL, configuration, archive files, CLI/MCP requests, generated reports, and future
provider output are untrusted. Each boundary MUST validate type, size, count, encoding,
path safety, timeout, and allowed values before allocation or interpretation.

## TM-SEC-003 — Path privacy

Source descriptors are path-private. Public errors, debug surfaces, serialized values,
and diagnostics use stable codes and counters, never absolute paths or wrapped OS
messages.

## TM-SEC-004 — Archive integrity

Archive writes use explicit transactions and compare expected generation, identity,
checkpoint, and proof state. Failed writes roll back completely. Incomplete, cancelled,
or failed scans MUST NOT authorize destructive source reconciliation.

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
