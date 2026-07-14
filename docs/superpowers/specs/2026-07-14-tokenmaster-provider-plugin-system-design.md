# TokenMaster Provider Plugin System Design

Status: approved architecture; implementation is deferred until the built-in Codex
engine and query contracts are stable.
Date: 2026-07-14.

## 1. Decision

TokenMaster uses two provider paths behind one typed ingest contract:

1. `CodexProvider` is compiled into the product and remains the default native fast
   path. Normal Codex-only operation does not load WebAssembly, start a plugin host,
   or pay plugin runtime memory.
2. External providers are portable `.tmplugin` packages containing a WebAssembly
   Component. Each enabled package executes in its own on-demand
   `tokenmaster-plugin-host` process under explicit capabilities and resource limits.

Native DLL, shared-library, and arbitrary executable plugins are not supported by the
normal product. They have unstable ABIs, inherit broad OS authority, and can corrupt
or leak the main process. A native developer bridge may be reconsidered only after a
real provider proves impossible within the component contract; it is not part of this
design.

The WebAssembly Component Model is selected because WIT defines language-neutral
interfaces and a standardized ABI. Wasmtime is selected for the host because it
supports components, explicit store resource limits, fuel/epoch interruption, and
cross-platform embedding. The plugin runtime is isolated from the GUI dependency
graph.

Primary references checked on 2026-07-14:

- WIT interface model:
  <https://component-model.bytecodealliance.org/design/wit.html>;
- cross-language component composition:
  <https://component-model.bytecodealliance.org/composing-and-distributing/composing.html>;
- Wasmtime store limits:
  <https://docs.wasmtime.dev/api/wasmtime/struct.StoreLimitsBuilder.html>;
- Wasmtime interruption and untrusted execution controls:
  <https://docs.wasmtime.dev/api/wasmtime/struct.Config.html>;
- Rust/C/Go language bindings:
  <https://github.com/bytecodealliance/wit-bindgen>;
- JavaScript/TypeScript components: <https://github.com/bytecodealliance/jco>;
- Python components: <https://github.com/bytecodealliance/componentize-py>.

## 2. Goals and non-goals

### 2.1 Goals

- A third party can implement a new usage/quota provider without changing or
  rebuilding TokenMaster.
- Installing a valid package makes its provider available without restarting the GUI.
- A plugin crash, trap, timeout, memory growth, or malformed response cannot crash,
  block, or corrupt the GUI, engine, or archive.
- Codex-only startup, idle memory, scan latency, and package size do not regress from
  carrying the plugin feature.
- Plugins written in any language with compatible Component Model tooling share the
  same WIT contract and conformance kit.
- All sources terminate at one provider-neutral observation boundary; analytics,
  storage, CLI, MCP, and UI do not know whether a provider is built in or external.
- Provider-defined quota windows and metadata render through existing dynamic UI
  models. Plugins do not ship executable UI.

### 2.2 Non-goals

- Arbitrary extension of TokenMaster UI, SQL schema, automation policy, MCP tools,
  skins, or application commands.
- In-process native code or platform-specific dynamic library ABIs.
- General shell, subprocess, unrestricted filesystem, raw socket, inbound listener,
  or arbitrary credential access.
- A mandatory central marketplace or automatic download service.
- Running external plugins in the initial Codex-only 1.0 release. The common ingest
  seam is implemented now; the package runtime and SDK are a later gated slice.
- Guaranteeing that every language runtime fits TokenMaster resource gates. The ABI is
  language-neutral, but each built component must pass the same package, memory,
  latency, and conformance limits.

## 3. Architecture

```text
                         TokenMaster engine
                                |
                 provider-neutral SourceAdapter
                    /                           \
                   v                             v
        built-in CodexProvider          WasmProviderProxy
        discovery/read/decode                  |
                   |                    framed strict JSON
                   |                           |
                   |                  tokenmaster-plugin-host
                   |                    one plugin/process
                   |                           |
                   |                    Wasmtime Component
                   |                           |
                   |                 host capability imports
                   \___________________________/
                                |
                   bounded ObservationDraft batches
                                |
                                v
                    TokenMaster canonicalizer
          fingerprint + replay signature + validation
                                |
                                v
                   staging/current SQLite archive
                                |
                                v
                   shared immutable query snapshots
```

### 3.1 Core ownership

- `tokenmaster-domain` owns bounded provider-neutral values and observation drafts.
- A provider-neutral ingest/canonicalization crate owns event fingerprinting, replay
  signature construction, and conversion from an observation draft to a canonical
  event.
- `tokenmaster-provider` owns discovery descriptors, capability declarations, and the
  native `SourceAdapter` contract.
- `tokenmaster-codex` owns only Codex discovery, wire decoding, and conversion to
  drafts. It does not own canonical identity rules.
- `tokenmaster-engine` owns scheduling, cancellation, bounded channels, generations,
  plugin registry revisions, and writer leases.
- `tokenmaster-plugin-host` owns Wasmtime, WIT bindings, host capabilities, guest
  limits, and the child-process protocol. The GUI does not depend on Wasmtime.
- `tokenmaster-store` accepts only canonicalized, validated bounded values and owns
  transactional persistence/selection.
- Query, UI, CLI, and MCP consume immutable provider-neutral snapshots only.

### 3.2 One conformance contract

The built-in Codex provider and `WasmProviderProxy` implement the same native
`SourceAdapter` semantics and pass the same conformance suite. Codex is not packaged
as WebAssembly because doing so would impose startup, JIT, host-process, and memory
costs on every user. It remains the reference adapter for correctness, not a privileged
alternate data model.

## 4. Canonicalization boundary

An adapter returns `ObservationDraft`, never `CanonicalUsageEvent`. A draft contains
only bounded normalized inputs:

- provider, profile, logical source, and session identities;
- optional explicit parent session identity and zero-based session ordinal;
- source-local offset/order evidence;
- UTC timestamp and normalized/raw bounded model identity;
- explicit delta token availability/values;
- optional cumulative token snapshot used for strong replay evidence;
- bounded service-tier, project alias, originator, and activity values;
- source verification level and stable diagnostic codes.

The TokenMaster canonicalizer performs all shared authority decisions:

- validates bounds and internal relations;
- computes the deterministic event fingerprint;
- computes the versioned replay signature from normalized model, delta, and optional
  cumulative snapshot;
- derives strong/weak replay evidence;
- rejects self-parenting unless already marked conflict;
- produces the only `CanonicalUsageEvent` type accepted by the store.

Plugins cannot supply event IDs, fingerprints, replay signatures, canonical
dispositions, SQL keys, freshness, quality, price, or totals. This prevents a plugin
from bypassing core replay/deduplication rules and keeps the P0 accounting proof valid
for future sources.

## 5. Component API

The first stable WIT package is `tokenmaster:provider@1.0.0`. Its provider world
exports five bounded operations:

- `metadata()` returns plugin/provider identity, display label, provider capabilities,
  declared permission classes, and checkpoint schema version.
- `health()` performs a non-destructive readiness check with no archive mutation.
- `discover(request)` returns a bounded page of path-private source descriptors plus a
  continuation token.
- `scan(request)` returns at most 256 observation drafts, one opaque checkpoint, a
  completion state, and bounded diagnostic counters.
- `quota(request)` returns at most 32 provider-defined quota windows with observation
  time, reset semantics, freshness evidence, and availability reasons.

Every WIT list/string/byte payload has a corresponding host-side byte/count bound.
Continuation and checkpoint tokens are opaque byte arrays capped at 32 KiB. Unknown
fields are impossible at the canonical ABI boundary; the child-process envelope uses
strict deny-unknown schemas.

The host imports capability interfaces, not general WASI authority:

- `tm-filesystem`: enumerate/stat/read-at within user-granted roots and declared file
  patterns; no write, rename, delete, watch outside the engine, or reparse traversal;
- `tm-https`: bounded HTTPS requests to manifest-declared origins/methods through the
  host proxy; no inbound server, raw socket, redirect to another origin, or unlimited
  body;
- `tm-auth`: attach a named credential slot inside the host request path without
  returning the secret value to the plugin;
- `tm-clock`: bounded UTC and monotonic time needed for freshness/timeout semantics.

All other imports fail component validation. Direct WASI filesystem, socket,
environment, subprocess, and stdio imports are denied. Additional capability
interfaces require a reviewed API-minor or API-major change; plugins never receive
ambient OS authority.

## 6. Package format and trust

A `.tmplugin` is a deterministic ZIP container with this bounded shape:

```text
plugin.toml
provider.wasm
LICENSE.txt
signature.ed25519     optional for manual side-load
```

No path may be absolute, contain traversal, collide case-insensitively, use an
alternate data stream, or expand outside the staging directory. Symlinks, reparse
entries, nested archives, executables, scripts, and extra files are rejected.

Package limits:

- archive: 64 MiB;
- manifest: 64 KiB UTF-8;
- component: 64 MiB;
- license: 256 KiB UTF-8;
- signature: 8 KiB;
- compression expansion ratio: 20:1;
- entries: exactly the required three plus the optional signature.

The manifest uses strict TOML and contains:

- schema version, reverse-DNS plugin ID, provider ID, display name, semantic version,
  required TokenMaster API major/minor, and checkpoint schema version;
- component SHA-256, publisher public-key fingerprint when signed, SPDX license ID,
  project/homepage URLs, and minimum host platform/architecture;
- declared provider features and permission requests;
- filesystem scope labels/patterns, HTTPS origins/methods, and named credential slots;
- requested guest memory class and per-operation timeout class within host maxima.

`tokenmaster.*`, `codex`, and other reserved IDs cannot be claimed externally.
Provider ID plus publisher key is immutable across updates.

Signed packages use Ed25519 over a domain separator, the exact validated UTF-8
`plugin.toml` bytes, and the component/license SHA-256 values. The package builder
emits one normalized manifest format, but verification never reparses and reserializes
TOML to decide what bytes were signed.
On first install, the user sees publisher fingerprint, permissions, and package hash.
Updates must use the same trusted key and may not silently add permissions. Unsigned
manual side-load is permitted only after an explicit local trust confirmation, is
hash-pinned, and is never eligible for unattended update. TokenMaster does not claim a
publisher is safe merely because a signature is mathematically valid.

## 7. Installation and hot replacement

Windows installed mode uses `%LOCALAPPDATA%\TokenMaster\plugins`; portable mode uses
the configured portable data root. The directory itself is not an execution surface:
packages are copied into an internal staging area and processed as data.

The UI install action and a bounded/debounced package-inbox watcher invoke the same
validation sequence. Dropping a package into the inbox never loads it directly; it
becomes visible only after validation, permission approval, health check, and atomic
promotion.

Installation sequence:

1. stream-copy under the archive/package bound while hashing;
2. validate ZIP paths, expansion, exact entries, manifest, component structure, WIT
   world, imports, digest, signature, API version, IDs, and requested permissions;
3. instantiate under the final resource limits and run `metadata()` plus `health()`;
4. persist the user grant and package identity transactionally;
5. atomically promote the package generation;
6. publish a new plugin-registry revision to the engine and UI.

The provider appears without GUI restart after step 6. Existing query snapshots remain
valid until a new provider snapshot revision is published.

An update is validated beside the active version. The engine cancels or completes the
old bounded request, commits no partial staging generation, then swaps generations.
Failure retains the previous package and grant. Disable/uninstall cancels its work,
terminates its host, removes it from future registry revisions, and preserves archive
history unless the user separately requests bounded data deletion.

## 8. Process and resource model

- External plugins are never instantiated in the GUI process.
- One `tokenmaster-plugin-host` process runs one package, so failure and resource
  attribution are unambiguous.
- At most two external plugin hosts execute concurrently; additional work remains in a
  bounded engine queue.
- A host starts only for validation, discovery, scan, or quota work and exits after the
  bounded request session or 60 seconds idle.
- The component cache is content-addressed by component hash, Wasmtime version,
  target, and CPU features; it has a bounded disk LRU and is never part of plugin trust.
- Guest linear memory defaults to 32 MiB and cannot exceed 64 MiB without an explicit
  elevated grant; the absolute guest limit is 128 MiB.
- The host process has a 256 MiB transient compile ceiling and a 128 MiB post-load
  working-set gate. Exceeding a hard platform limit terminates only that host.
- Metadata/health calls have a 500 ms budget, discovery pages 2 seconds, local scan
  pages 5 seconds, and HTTPS quota calls 15 seconds. A complete scan is composed from
  bounded pages and engine cancellation points rather than one long guest call.
- Wasmtime store limits cap memories, tables, instances, stack, and linear growth.
  Epoch interruption plus an outer host timeout stops runaway guest execution;
  blocking host calls use separately timed asynchronous operations.
- Results are capped at 256 observations, 32 quota windows, 1 MiB per child-process
  frame, and the existing provider/profile/source/string limits.

The exact pinned Wasmtime/toolchain versions live in Cargo.lock and generated SDK
metadata. Upgrades require conformance, adversarial, startup, memory, and compatibility
gates; there is no floating runtime download.

## 9. Child-process protocol

The engine and plugin host communicate through inherited stdin/stdout only. There is
no socket or listener. Each message is an unsigned 32-bit little-endian length followed
by strict UTF-8 JSON, capped at 1 MiB before allocation. The envelope contains protocol
version, request ID, plugin-generation ID, operation, deadline, and one typed payload.

Stdout is protocol-only. The host may emit bounded stable diagnostic codes to stderr;
guest stdout/stderr imports are absent. Unexpected bytes, duplicate responses, unknown
fields, stale generation IDs, oversized lengths, or response-after-cancellation
terminate the host and fail only the current provider request.

The engine never holds a SQLite transaction or UI object while awaiting a host. A
plugin response is fully decoded/validated into bounded owned drafts before the
canonicalizer or store is called.

## 10. Permissions and privacy

External plugin packages and all their output are untrusted. Grants are per plugin,
publisher, capability, scope, and package generation.

- Filesystem grants expose only virtual scope handles, not ambient roots or unrelated
  paths. Public descriptors remain path-private.
- HTTPS grants specify exact `https` origins, methods, redirect policy, request/response
  byte ceilings, and credential slot. DNS/IP rebinding checks and private-network
  policy are host-owned.
- Credential values are stored through an OS-backed secret provider when available,
  attached by the host, and never returned through WIT, JSON, UI, CLI, MCP, logs, or
  SQLite.
- A plugin requesting both raw local source reads and outbound HTTPS is classified as
  data-egress capable and requires a separate prominent grant. It is never enabled by
  package installation alone.
- Raw source bytes/provider responses exist only during a bounded guest request and
  are not retained by TokenMaster. Crash diagnostics contain codes/counters only.
- Plugins cannot read TokenMaster's archive, settings, other plugin directories,
  process environment, user profile, clipboard, UI state, MCP requests, or another
  provider's data.

## 11. Versioning and SDK

WIT package and manifest versions are independent:

- manifest schema changes package metadata/installation rules;
- provider API major changes WIT compatibility;
- API minor additions are optional capabilities and never change existing meanings;
- checkpoint schema is provider-owned and scoped to plugin ID/publisher/version.

The host accepts only explicitly compiled API majors. Once provider API 1 reaches a
stable TokenMaster release, it remains supported through the next API-major release;
removal requires migration documentation and cannot silently reinterpret checkpoints.
If a plugin update cannot read its old checkpoint schema, the engine discards only the
opaque checkpoint and performs a full bounded staging rescan. It does not reuse an
incompatible checkpoint or mutate current canonical data.

The public SDK repository/artifacts contain:

- versioned WIT and manifest JSON/TOML schemas;
- generated Rust and TypeScript bindings/templates first;
- a language-neutral conformance runner and synthetic fixtures;
- package builder, inspector, signer, and local host simulator;
- permission, privacy, replay, quota, paging, cancellation, and upgrade examples;
- CI commands that produce a deterministic `.tmplugin` and conformance report.

Other language bindings are accepted when their produced component passes the same
WIT and resource gates. SDK helpers cannot become semantic authority: the host and core
always revalidate package data and observation drafts.

## 12. UI and operator experience

The Data Sources view adds a plugin manager showing:

- built-in versus external provider;
- package/provider/publisher/version/API identity;
- enabled, healthy, unavailable, quarantined, update-ready, or permission-changed
  state;
- granted filesystem/HTTPS/credential scopes and data-egress classification;
- last successful operation, freshness, bounded diagnostic codes, process memory/CPU,
  and restart count;
- install, inspect, enable, disable, retry, update, rollback, and uninstall actions.

Plugin-provided labels are bounded untrusted display data. Providers can declare
dynamic quota-window metadata and semantic icon/category keys, but cannot provide
Slint code, SVG/HTML, scripts, styles, layouts, skins, localization catalogs, command
palette actions, or notification text templates. The normal TokenMaster UI renders
all data and states consistently.

## 13. Failure and quarantine behavior

- Trap, panic, protocol violation, timeout, OOM, abnormal exit, or invalid draft fails
  the current operation without committing its staging generation.
- One retry is allowed for a transient process-start failure. Repeated execution
  failures use bounded exponential backoff and then quarantine that package generation.
- Quarantine never disables the built-in Codex provider or another plugin.
- A stale host response cannot overwrite a newer plugin/scan/snapshot generation.
- A failed update retains the previous validated package and grants.
- A changed publisher key, provider ID, API major, or expanded permission set is a new
  explicit install decision, not an automatic update.
- Pending/conflicting provider observations remain non-canonical under the shared
  replay rules; plugin failure cannot turn unknown data into zero.

## 14. Testing and acceptance

Required test families:

- WIT/manifest golden schemas and backward-compatibility fixtures;
- built-in Codex versus Wasm proxy `SourceAdapter` conformance;
- deterministic package build, signature, hash, ZIP path, expansion, duplicate-name,
  case-collision, alternate-stream, malformed component, and import allowlist tests;
- malicious components for infinite loops, recursion, memory/table/instance growth,
  oversized values, invalid UTF-8, traps, panics, stale responses, and cancellation;
- capability tests proving denied filesystem/network/environment/subprocess/archive
  access and scoped allowed access;
- credential-injection tests proving secret bytes never cross WIT/JSON/debug/log/store;
- observation-draft validation and core-owned fingerprint/replay signature golden tests;
- crash/update/rollback/quarantine/restart/disable/uninstall generation races;
- install/hot-enable without GUI restart and snapshot-generation ordering;
- cold/warm host startup, page latency, CPU, process/guest memory, handles, threads,
  cache bounds, idle shutdown, 10K start/stop cycles, and 24/72-hour enabled-plugin
  soak gates;
- Windows x64 first, then Linux/macOS host/package compatibility without changing WIT.

Acceptance requires all adversarial tests to fail closed, no GUI resource regression
when no external plugin is enabled, no monotonic host/resource growth, and exact
package/host/commit identities in generated reports. A Component Model conformance
test does not prove compatibility with every language toolchain; supported SDK
templates receive separate build-and-run receipts.

## 15. Delivery order

1. Finish replay correctness with a provider-neutral observation draft and core-owned
   canonicalizer.
2. Build staging generations and the engine `SourceAdapter` contract; adapt built-in
   Codex and pass the native conformance suite.
3. Freeze immutable queries, analytics, pricing, quota-window drafts, and quality
   semantics.
4. Implement manifest/WIT schemas, package validation, plugin registry, child protocol,
   host capabilities, and isolated Wasmtime host.
5. Add install/hot replacement, permission UI, quarantine, signing, and rollback.
6. Publish Rust/TypeScript SDK templates, conformance tools, and one synthetic sample
   plugin.
7. Run plugin-specific performance/security/soak gates before claiming the ecosystem
   surface stable.

This ordering keeps the current critical path on correctness and the built-in Codex
engine. It also prevents freezing a public plugin ABI before the observation, quota,
quality, and paging contracts have executable evidence.
