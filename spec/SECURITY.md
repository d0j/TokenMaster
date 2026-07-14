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

Ingestion adapters are statically linked, allowlisted, and capability-bounded. The
engine owns cancellation, timeouts, backpressure, and maximum chunk/count sizes. No
generic arbitrary filesystem, network, HTTP, SSH, command, SQL, executable-plugin, or
credential interface may be exposed to UI, CLI, MCP, configuration, or skin data.

A future remote or authenticated provider adapter requires its own reviewed allowlist,
transport bounds, secret-lifetime rules, and tests. Raw provider responses and
credentials MUST NOT cross the adapter boundary or enter the archive, snapshots,
diagnostics, logs, or external interfaces.
