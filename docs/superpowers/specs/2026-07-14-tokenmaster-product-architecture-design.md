# TokenMaster Product Architecture Design

Status: approved; the corrected P0 sequence begins with the core accounting authority boundary.
Date: 2026-07-14.

## 1. Product decision

TokenMaster 1.0 is a Codex-first, provider-ready, Windows-first local desktop
application. It combines the useful product breadth of WhereMyTokens with the usage
analysis semantics of ccusage, but does not inherit their runtime architecture.

The product remains one Rust workspace with Slint and bundled SQLite. Rewriting the
application in Electron, Tauri, React, Go, or another GUI stack is rejected. The
existing bounded reader, parser, source identity, SQLite, and resource-gate work is
retained and extended.

The complete product is too large for one safe implementation plan. Work is divided
into independently testable slices with fixed contracts:

1. Canonical accounting and fork/replay correctness.
2. Staging generations, scan epochs, reconciliation, and the runtime engine.
3. Indexed query snapshots, analytics, pricing, and quota transport.
4. Universal CLI and MCP automation connector.
5. Complete desktop information architecture and supporting views.
6. Dynamic bars, modular presentation, skins, layouts, density, and localization.
7. Windows shell integration, portability hardening, and release evidence.

Each slice receives its own implementation plan and red/green tests. A later slice
must consume the previous slice through a public contract rather than reach through
its internals.

## 2. Research basis and compatibility target

`third_party/UPSTREAM.toml` remains the provenance anchor. WhereMyTokens defines the
quota-first information hierarchy, compact widget, alerts, customization, and the
breadth of exploration views. ccusage defines token-field semantics, model aliases,
period reports, pricing, and usage breakdowns. No upstream source, runtime, or asset
is vendored into TokenMaster.

The automation connector targets the stable MCP 2025-11-25 protocol and negotiates
older versions supported by the selected SDK. The announced 2026-07-28 protocol is
not a 1.0 dependency until it is stable and passes the TokenMaster compatibility
matrix.

The following current clients all support local MCP servers launched as subprocesses:

- Hermes Agent;
- local Codex clients;
- Claude Code;
- Gemini CLI;
- OpenCode.

The connector therefore uses MCP stdio. Streamable HTTP is not enabled in 1.0 because
it introduces a listener, authentication, origin validation, daemon lifecycle, and a
second attack surface without improving local single-user automation.

The official Rust MCP SDK is preferred over a hand-written JSON-RPC implementation.
Its async runtime is isolated in a separate on-demand executable, so the desktop GUI
does not link it, initialize it, or pay its steady-state memory cost. The dependency
must be an exact crates.io release locked in `Cargo.lock`, with only server, stdio,
schema, and macro features enabled. A git branch dependency is prohibited.

Primary sources checked for this decision on 2026-07-14:

- MCP stable transports:
  <https://modelcontextprotocol.io/specification/2025-11-25/basic/transports>;
- MCP tools and structured output:
  <https://modelcontextprotocol.io/specification/2025-11-25/schema>;
- official Rust SDK: <https://github.com/modelcontextprotocol/rust-sdk>;
- Hermes MCP client:
  <https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/features/mcp.md>;
- Codex MCP configuration:
  <https://learn.chatgpt.com/docs/extend/mcp.md>;
- Claude MCP overview: <https://docs.anthropic.com/en/docs/mcp>;
- Gemini CLI MCP client:
  <https://google-gemini.github.io/gemini-cli/docs/tools/mcp-server.html>;
- OpenCode MCP client: <https://opencode.ai/docs/mcp-servers>.

The alternatives were rejected deliberately:

| Alternative | Benefit | Reason rejected for 1.0 |
| --- | --- | --- |
| Hand-written synchronous MCP | Small dependency graph | Protocol/conformance risk and duplicated security work |
| MCP linked into the GUI | One executable | Makes every GUI session pay async/runtime size and memory |
| Loopback Streamable HTTP daemon | Multi-client and push-friendly | Listener, authentication, origin checks, daemon lifecycle |
| Periodic JSON export files | Trivial consumers | Polling, stale/racy reads, extra retained surface |

## 3. Target architecture

```text
native CodexProvider (1.0)       future sandboxed .tmplugin
           |                         WasmProviderProxy
           |                               |
           |                    tokenmaster-plugin-host
           |                               |
           +---------------+---------------+
                           v
            provider-neutral ObservationDraft
                           |
                           v
              TokenMaster replay canonicalizer
                       |
                       v
       staging generations and scan reconciliation
                       |
                       v
            canonical SQLite archive
                       |
                       v
             tokenmaster-query facade
                       |
          +------------+-------------+
          |            |             |
     Slint desktop  CLI JSON   tokenmaster-mcp
          |                          |
       human UI              Hermes/Codex/Claude/
                             Gemini/OpenCode/other MCP
```

### 3.1 Ownership

- Provider crates discover and normalize bounded provider input.
- The engine schedules reads, coalesces changes, manages cancellation, and commits
  generations.
- The store owns persistence and transactional canonical selection.
- The query facade owns bounded immutable read models and no writes.
- The automation evaluator turns immutable quota snapshots plus declarative local
  policy into advisory decisions.
- UI, CLI, and MCP are adapters. They do not parse provider files or issue arbitrary
  SQL.

### 3.2 Source adapter seam

Codex local files are the only ingestion promise for 1.0, but they are not an
architectural dependency of the engine, store, query facade, or UI. Ingestion is
split into typed capability boundaries:

- a source catalog returns bounded `SourceDescriptor` values with stable provider,
  profile, logical-source, kind, identity, and capability fields;
- a source reader returns bounded sequential chunks, identity evidence, and opaque
  adapter checkpoints under engine-owned cancellation, timeout, and backpressure;
- a provider decoder converts those chunks into source-neutral observation drafts,
  lineage basis, and bounded diagnostic codes;
- an optional quota adapter returns provider-defined immutable quota snapshots and
  never exposes credentials or raw provider responses.

The engine depends on these contracts, not on Codex paths or JSONL shapes. A
provider-neutral TokenMaster canonicalizer validates drafts and computes event
fingerprints, replay signatures/evidence, and canonical-event values. Providers cannot
supply those authority fields. The store persists stable provider-neutral identities
and canonical observations. Queries and all presentation surfaces consume
provider-neutral snapshots.

Codex is the statically linked default adapter. Future third-party providers are
portable `.tmplugin` WebAssembly Components executed one package per on-demand
`tokenmaster-plugin-host` process. They implement the same source semantics through a
versioned WIT contract and explicit host capabilities. This allows a later local-file,
import, remote-agent, or authenticated HTTPS provider without changing accounting,
storage, analytics, automation, or UI contracts.

The source contract uses bounded owned data and streaming pull so a slow or remote
adapter cannot create an unbounded queue or retain complete history. Adapter-specific
checkpoint payloads are versioned, size-bounded, and opaque outside that adapter.
Native DLL/executable plugins, ambient filesystem/network access, plugin-supplied UI,
and arbitrary command/SQL interfaces are forbidden. The complete future package,
host, permissions, SDK, hot-replacement, and gate design is recorded in
`docs/superpowers/specs/2026-07-14-tokenmaster-provider-plugin-system-design.md`.

### 3.3 External provider isolation

The GUI and normal Codex path do not depend on Wasmtime or start a plugin process.
When an external provider is enabled, the engine talks through strict length-prefixed
stdio frames to a separate `tokenmaster-plugin-host`. Each host loads one component,
has bounded memory/instances/tables/stack/time, exposes only user-granted read-only
filesystem, allowlisted HTTPS, host-injected credential, and clock capabilities, and
exits after bounded work or idle timeout. Plugin failure affects only that provider
request and cannot hold a UI object, writer transaction, or source-reconciliation
authority.

### 3.4 Runtime model

The desktop process uses a small fixed set of workers and bounded channels. Filesystem
events are hints, not authority: they are coalesced and backed by periodic bounded
reconciliation. UI updates receive monotonically increasing snapshot generations; an
older asynchronous result may never overwrite a newer generation.

No permanent companion daemon is installed. The GUI owns its live engine while it is
running. CLI and MCP open the archive in query-only mode for normal reads. An explicit
refresh may run a bounded one-shot engine operation under the same generation and
writer-lease rules as the GUI. The future plugin host is an on-demand child, not a
daemon, and is absent from Codex-only operation.

## 4. Accounting correctness before analytics

Forked or subagent Codex histories can replay an earlier transcript prefix. File-local
cumulative deltas and timestamp-based fingerprints are insufficient when the replayed
copy changes timestamps or appears under another source identity.

Before analytics or UI integration, TokenMaster must define and test:

- observation identity: the bounded fact observed in one physical source;
- logical usage identity: the provider event represented by an observation;
- canonical selection: the one observation authorized to contribute to totals;
- replay lineage: evidence that a child source repeats an already-accounted prefix;
- divergence point: the first event after which a child contributes new usage;
- confidence and conflict state when lineage cannot be established safely.

Required fixtures cover exact replay, rewritten timestamps, partial prefix replay,
nested subagents, truncation, replacement, interrupted tails, conflicting copies, and
legitimate equal-valued events. Unknown or conflicting usage remains explicit; it is
never silently converted to zero or counted twice.

The solution must not rescan or retain complete historical transcripts on the append
path. Replay proof is based on bounded structural digests, chunk coverage, ancestry
metadata when available, and staged reconciliation.

## 5. Immutable query contract

All frontends consume the same `QuerySnapshot` family. Every response contains:

- `schemaVersion`;
- `snapshotRevision`;
- `generatedAt`;
- `dataThrough`;
- `freshness`: `fresh`, `aging`, `stale`, or `unavailable`;
- `quality`: `authoritative`, `derived`, `estimated`, `partial`, `conflict`, or
  `unknown`;
- stable provider/profile identifiers;
- bounded payload data;
- stable reason and warning codes.

Missing values remain absent with an availability reason. They are not fabricated as
zero. Human labels may be localized in the GUI, but schema fields, stable identifiers,
reason codes, CLI JSON, and MCP structured output remain invariant English ASCII.

Snapshots use owned immutable bounded arrays, preferably shared behind `Arc`, and do
not hold a SQLite transaction, writer lock, source handle, or UI object alive.

## 6. Universal automation connector

### 6.1 Product surfaces

The workspace adds three isolated components when their prerequisite query contract
exists:

- `tokenmaster-query`: synchronous bounded read facade shared by every frontend;
- `tokenmaster-automation`: pure deterministic policy evaluation;
- `tokenmaster-mcp`: separate on-demand MCP stdio executable using the official Rust
  SDK.

The packaged process boundary is explicit:

- `TokenMaster.exe`: desktop GUI and live engine;
- `tokenmaster-cli.exe`: synchronous CLI and configuration-example printer;
- `tokenmaster-mcp.exe`: on-demand MCP stdio server.

The CLI and MCP executables share typed request/response models. The GUI binary must
not depend on the MCP SDK or its async runtime.

### 6.2 MCP capabilities

TokenMaster connector v1 advertises MCP tools only. It does not advertise prompts,
resources, sampling, roots, elicitation, subscriptions, or experimental tasks. Tools
are the most consistently supported primitive among target clients and keep the
contract explicit.

Each tool defines:

- JSON Schema 2020-12 input and output objects;
- `additionalProperties: false` at every object boundary;
- `structuredContent` conforming to `outputSchema`;
- the same serialized JSON in a text content block for older clients;
- accurate read-only, destructive, idempotent, and open-world annotations;
- stable path-free error codes.

Server instructions state, within the first 512 characters, that TokenMaster returns
local usage observations and advisory automation decisions, that `unknown` is not an
authorization to proceed, and that no prompt/transcript/credential data is available.

### 6.3 Tool set

`tokenmaster_get_capabilities`

- Returns supported schema versions, providers, metrics, ranges, policies, and hard
  collection limits.
- Read-only, closed-world.

`tokenmaster_get_status`

- Returns archive/engine availability, snapshot freshness, data-through time,
  diagnostics counters, and quality state.
- Does not return source paths or raw OS errors.
- Read-only, closed-world.

`tokenmaster_get_quotas`

- Returns the bounded provider-defined quota windows and normalized bar semantics.
- Optional provider/profile filters are enumerated from capabilities.
- Read-only, closed-world.

`tokenmaster_get_usage_summary`

- Returns today, day, week, month, or bounded custom-range usage and cost.
- Breakdowns are explicitly selected and independently capped.
- Read-only, closed-world.

`tokenmaster_list_sessions`

- Returns keyset-paged session metadata, usage, cost, activity, model, project-private
  identifiers, and quality state.
- Never returns prompts, responses, commands, command output, file contents, or
  absolute paths.
- Read-only, closed-world.

`tokenmaster_evaluate_automation`

- Evaluates a named local policy against one immutable quota snapshot.
- Returns `proceed`, `throttle`, `defer`, or `unknown`, plus stable reasons,
  `validUntil`, `nextCheckAt`, and applicable reset times.
- Has no actuator and cannot start, stop, or modify an external agent.
- Read-only, deterministic for the same policy and snapshot, closed-world.

`tokenmaster_request_refresh`

- Requests an idempotent bounded refresh; concurrent requests coalesce.
- It may update TokenMaster's own archive but cannot change provider sources or any
  external system.
- Marked non-read-only, non-destructive, idempotent, and closed-world.
- Returns `completed`, `coalesced`, `busy`, or `deadlineExceeded` with the resulting
  snapshot revision when available.

### 6.4 Automation policies

TokenMaster is a sensor and policy advisor, not a scheduler or enforcement authority.
External automation owns the final action. Policies are declarative local data and may
contain only bounded fields:

- stable policy ID and display name;
- minimum remaining ratio per selected window class;
- warning and defer thresholds;
- maximum acceptable data age;
- reset-soon horizon;
- response to missing, stale, partial, or conflicting data;
- optional quiet hours and user-reserve ratio.

Built-in policies are:

- `interactive`: preserves a moderate user reserve;
- `background-safe`: conservative default for unattended agents;
- `overnight`: may defer work until the nearest useful reset;
- `observe-only`: always reports state and never recommends automatic work.

Custom policies may extend one built-in policy with stricter or looser bounded numeric
thresholds. There is no expression language, code, shell, URL, or arbitrary condition.
The default unattended behavior maps stale, conflict, and unknown data to `defer`.

The evaluator does not claim to predict the exact token cost of an unknown future LLM
task. It reports measured headroom and policy outcomes. A client may supply a bounded
workload class (`tiny`, `short`, `medium`, `long`) as an advisory input, but a class
never overrides hard reserve or freshness gates.

### 6.5 CLI parity

CLI commands return the same schemas:

```text
tokenmaster-cli status --format json
tokenmaster-cli quota --provider codex --format json
tokenmaster-cli usage summary --range week --format json
tokenmaster-cli sessions list --limit 50 --format json
tokenmaster-cli automation decide --policy background-safe --format json
tokenmaster-cli refresh --deadline-seconds 30 --format json
tokenmaster-cli integrations print-config --client hermes
tokenmaster-mcp serve
```

Automation decision exit codes are stable:

- `0`: proceed;
- `10`: throttle;
- `20`: defer;
- `30`: unknown;
- `40`: invalid request or configuration;
- `50`: archive or runtime failure.

JSON output is written to stdout; diagnostics are written to stderr. MCP stdout must
contain valid protocol messages only.

### 6.6 Easy client integration

TokenMaster prints, but does not silently modify, client configuration for Hermes,
Codex, Claude Code, Gemini CLI, and OpenCode. Generated examples use an absolute path
to `tokenmaster-mcp` and no credentials.

Hermes shape:

```yaml
mcp_servers:
  tokenmaster:
    command: "C:\\Program Files\\TokenMaster\\tokenmaster-mcp.exe"
    args: ["serve"]
```

Codex shape:

```toml
[mcp_servers.tokenmaster]
command = 'C:\Program Files\TokenMaster\tokenmaster-mcp.exe'
args = ["serve"]
enabled_tools = [
  "tokenmaster_get_status",
  "tokenmaster_get_quotas",
  "tokenmaster_evaluate_automation",
]
```

Client-specific installers are not required for 1.0. A future explicit `--install`
operation requires separate design and confirmation because it mutates another
application's configuration.

### 6.7 Connector bounds

- Maximum request body: 64 KiB.
- Maximum complete response: 1 MiB.
- Maximum collection page: 256 items.
- Maximum requested breakdown dimensions: 4.
- Maximum selected providers/profiles/windows: 32 each.
- Normal query deadline: 2 seconds.
- Refresh deadline: 30 seconds.
- Maximum one stdio client per connector process.
- Connector memory target: 24 MiB; hard gate: 48 MiB for a status/decision workload.
- No network socket, telemetry, cloud upload, credential store, or background daemon.

The process exits cleanly on stdin close, protocol shutdown, parent termination, or
deadline. Repeated start/query/exit stress must show no orphan processes, locked files,
or monotonic resource growth.

## 7. Desktop information architecture

The main window has a persistent header and six reorderable board sections. Today
totals and data freshness live in the header and do not count as a section.

1. **Plan Usage**: provider quota windows, remaining headroom, reset information,
   pace, staleness, and automation signal.
2. **Code Output**: added, removed, and net lines plus cost/output efficiency when
   available.
3. **Usage and Cost Trend**: token/cost modes, bounded range selectors, comparison,
   and drill-down.
4. **Sessions**: active and recent sessions with project-private identity, model,
   activity, usage, and cost.
5. **Activity**: heatmap, hourly/weekly rhythm, tool-category counts, and agent/subagent
   activity without retaining tool arguments.
6. **Model Usage**: ranked bounded bars/tables for model, service tier, token mix,
   calls, and cost.

Supporting routes are Dashboard, History, Sessions, Models, Projects, Activity, Data
Health, Notifications, Settings, Help/About, and Compact Widget. Command palette and
keyboard navigation reach every route and primary action.

Users may reorder, hide, collapse, and restore board sections. These operations modify
presentation settings only; data remains in the archive and the automation connector
is unaffected.

### 7.1 Reference lineage

| Product idea | Reference | TokenMaster inheritance and improvement |
| --- | --- | --- |
| Quota-first hierarchy and reset visibility | WhereMyTokens | Provider-defined windows, explicit freshness/quality, no hard-coded 5-hour assumption |
| Compact always-available widget | WhereMyTokens | Native bounded view model, shared snapshots, no Electron renderer |
| Alerts and headroom warnings | WhereMyTokens | Stable reason codes and the same policy evaluator used by automation |
| Reorder/hide/collapse dashboard | WhereMyTokens | Presentation-only settings, no archive mutation |
| Daily/weekly/monthly/session reports | ccusage | Indexed incremental queries instead of a live whole-history rescan |
| Input/output/cache/model/cost breakdowns | ccusage | Explicit availability, pricing provenance, conflict state |
| Dense report tables | ccusage | Workbench family with keyboard/accessibility and bounded paging |
| Multi-provider extensibility | Both | Provider contract retained, but only Codex ingestion is a 1.0 promise |

This is requirements and interaction lineage, not source-code or asset inheritance.

## 8. Dynamic quota bars

Quota windows are provider-defined data, not hard-coded UI concepts. A window model
contains:

- stable provider/window ID;
- bounded label key and provider fallback label;
- usage direction: used or remaining;
- optional used and remaining ratios with explicit availability;
- fixed, rolling, credit, or unknown reset semantics;
- optional duration and reset timestamp;
- source observation time, freshness, quality, and confidence;
- soft warning and hard reserve thresholds;
- optional pace ratio and projected exhaustion time;
- stable reason codes.

If a provider adds, removes, or renames a quota window, a bounded keyed list changes;
Slint components and MCP schemas do not. The UI displays at most 32 simultaneous quota
windows and uses compact overflow/grouping beyond the primary set.

The user may display `used`, `remaining`, or `pace` as the dominant bar mode. Every bar
also includes numeric text and a semantic state icon or pattern, so color is never the
only signal. Unknown is rendered as unknown, not as an empty or full bar.

Value changes use a single bounded transition no longer than 180 ms. Reduced-motion
mode disables it. There are no per-card timers. One shared visible-window clock ticks
once per minute, increasing to once per second only for a visible reset countdown
under five minutes. It suspends when the window is hidden.

### 8.1 Weekly full-reset history

The Codex weekly window is not treated as one mutable bar. Every full reset starts a
new immutable quota epoch, including an early or repeated reset inside the previously
advertised week. The prior epoch is closed with:

- the last trustworthy pre-reset sample and its observation time;
- the maximum used ratio observed in that epoch;
- available used/remaining units and capacity without inventing unavailable values;
- the old advertised reset timestamp and window duration;
- the first trustworthy post-reset sample, new reset timestamp, and new duration;
- a transition kind (`scheduled_reset`, `early_full_reset`, `manual_or_banked_reset`,
  `allowance_changed`, or `unknown_reset`), evidence source, and confidence;
- an observed time interval when polling cannot prove the exact reset instant.

Strong detection requires an explicit provider epoch/reset signal or a coherent
transition from the same weekly window: usage falls to the provider-defined reset
floor, remaining headroom returns to the reset ceiling, and/or the advertised reset
time advances. A lower value alone is insufficient because rolling recovery, stale
samples, account/workspace changes, and provider errors can look similar. An
allowance/capacity change is recorded separately even when it accompanies a reset.

The weekly card shows the current epoch plus a visible `Reset detected` action. Its
detail displays `before -> after`, maximum use before reset, scheduled versus early,
old/new reset times, source freshness, confidence, and the detection interval. A
timeline uses vertical reset markers and keeps repeated resets distinct. If OpenAI
exposes only ratios, TokenMaster says `84% used -> 0% used`; it never fabricates an
absolute message, token, or credit limit.

UI, CLI, and MCP consume the same bounded `QuotaEpochSnapshot` and
`QuotaTransitionSnapshot`. Automation may react to a new reset sequence only once and
may require a minimum confidence. Restart deduplication uses provider/window identity,
prior/current epoch identity when available, and the two observation IDs. The store
retains a fixed recent transition/epoch window per provider quota window and bounded
older aggregates, never an unbounded poll history.

This is deliberately data-driven. OpenAI documents Codex usage as dependent on plan,
task complexity, model, and execution surface, with reset options exposed through the
[usage page or limit banner](https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan/);
[banked rate-limit resets](https://help.openai.com/en/articles/20001271) are also
distinct from credits. Therefore TokenMaster never hard-codes a five-hour or weekly
capacity and never treats local token totals as the provider allowance.

## 9. Modular presentation system

Presentation is five orthogonal axes:

1. `skin`: visual tokens;
2. `layout`: board arrangement and component composition;
3. `density`: comfortable, compact, or ultra-compact sizing;
4. `colorScheme`: dark, light, or system;
5. `locale`: language and regional formatting.

Any supported combination switches without reparsing, SQL writes, source scans,
window recreation, or loss of selection. One presentation revision is committed
atomically; invalid input leaves the previous revision visible.

### 9.1 Built-in families

- **Refined**: original TokenMaster default, calm hierarchy and balanced density.
- **Control Center**: quota-first dense dashboard inspired by the useful hierarchy of
  WhereMyTokens.
- **Workbench**: table-rich technical layout inspired by the compact analytical
  readability of ccusage.

All three remain supported. They are original TokenMaster implementations, not copied
upstream source or assets. `docs/FEATURE_PARITY.md` records feature lineage and the
TokenMaster improvement for each inherited product idea.

### 9.2 Skin data and inheritance

User skins are strict `.tmskin.json` data files in the dedicated TokenMaster skin
directory. They may define colors, typography roles, spacing scale, radii, borders,
elevation, chart palette, semantic states, and component variants. They cannot define
code, expressions, paths, URLs, fonts outside approved families, scripts, SQL, shell,
or network operations.

Each skin has one parent. Maximum inheritance depth is two edges: built-in base,
optional derived skin, optional user override. Cycles, unknown tokens, non-finite
numbers, invalid contrast-critical states, and out-of-range dimensions reject the
candidate. The loader flattens a validated skin into one immutable token table before
the UI receives it.

Bounds:

- maximum 32 external skin files;
- maximum 128 KiB per file;
- maximum 512 token overrides;
- maximum identifier length 64 ASCII characters;
- maximum display name length 128 Unicode scalar values;
- one coalesced directory reload every 250 ms;
- failed reload retains the last valid skin and emits one bounded diagnostic.

Built-in skins are always available and cannot be overwritten. Safe mode loads the
Refined built-in skin with system color scheme and comfortable density.

### 9.3 Layout manifests

Slint components remain compiled and type-safe. A layout manifest may select a
built-in composition template, section order, width class, visibility, collapse
state, and bounded component variants. It cannot inject Slint code or arbitrary
geometry expressions. This provides instant modular layouts without a plugin runtime.

## 10. Localization

TokenMaster 1.0 ships complete English and Russian locales plus a pseudo-locale used
by tests. Switching locale is immediate and does not recreate the window.

Requirements:

- every visible string has one stable translation key;
- no sentence is assembled by concatenating independently translated fragments;
- plural, number, percentage, currency, date, duration, and reset-time formatting is
  locale-aware;
- user timezone is independent from UI language;
- English is the complete fallback;
- missing production translations fail the release localization audit;
- pseudo-locale expands long strings by at least 35 percent and marks boundaries;
- layout tests cover English, Russian, pseudo-locale, and 100/150/200 percent DPI;
- accessibility names and descriptions are translated with the same coverage gate;
- API/CLI/MCP keys and enum values are never localized.

External language packs are not supported in 1.0. They would require a separately
versioned message catalog, plural-rules compatibility, translation trust, and release
QA. The internal stable-key model must allow them later without changing UI component
contracts.

## 11. UI state and accessibility

Every data surface has explicit `loading`, `available`, `aging`, `stale`, `partial`,
`unavailable`, `conflict`, `empty`, and `error` behavior. Empty means a successful
query with no matching data; unavailable means the metric could not be observed.

Required interaction behavior:

- complete keyboard traversal and visible focus;
- command palette and documented shortcuts;
- screen-reader names, roles, values, and state changes;
- high-contrast-safe semantic states;
- reduced motion;
- no information conveyed by color alone;
- 100/150/200 percent DPI and mixed-monitor behavior;
- bounded virtualization/keyset paging for long lists;
- stable selection across snapshot revisions when the selected identity remains;
- focus does not jump when data refreshes.

## 12. Performance and memory contract

Existing gates remain binding:

- warm visible at or below 300 ms;
- warm interactive at or below 500 ms;
- cold interactive at or below 900 ms;
- input-to-paint p95 below 50 ms and p99 below 100 ms;
- theme switch p95 at or below 16.7 ms;
- layout switch p95 at or below 50 ms;
- incremental append p95 below 25 ms;
- cached million-row dashboard p95 below 250 ms;
- cold dashboard p95 below 1,000 ms.

Memory targets/hard gates remain 40/64 MiB empty, 64/96 MiB for 100K rows, 80/112
MiB for one million rows, 2/4 MiB retained after 10K switches/routes, and 8/16 MiB
growth across 72 hours.

Production defaults use only the software renderer. FemtoVG is excluded from the GUI
default feature set and may exist only as an explicitly built diagnostic artifact.

No unbounded collection, cache, channel, watcher queue, chart series, timer set,
diagnostic buffer, MCP body, or string interner is permitted. Bounds are executable
contracts, not comments.

The GUI binary and Codex-only runtime do not link or instantiate Wasmtime. External
plugin host memory/CPU/process gates are measured separately and must not cause
monotonic GUI growth. One external component runs per host, at most two hosts execute
concurrently, and package/frame/guest/process/time limits are enforced before an
external provider can publish a snapshot.

## 13. Security and privacy

- Local-first, no telemetry, cloud sync, automatic upload, or analytics SDK.
- TokenMaster connector v1 uses MCP stdio only and opens no network listener.
- Provider credentials may be used only through an explicit built-in adapter or a
  host-injected named external-plugin credential slot. Secret bytes are never returned
  through the component ABI, UI, CLI, MCP, logs, or archive.
- Prompts, responses, reasoning, commands, command output, source contents, raw tails,
  and absolute paths are prohibited from all retained or external surfaces.
- Provider strings are bounded and treated as untrusted display data.
- MCP output contains structured numeric/enum data and bounded labels; no provider
  transcript text can become tool instructions.
- Untrusted display strings reject control characters, are escaped by each frontend,
  and are never interpolated into MCP tool descriptions or server instructions.
- Skins and policies are declarative data, never authority or executable code.
- External provider components execute only in the isolated plugin host with declared
  capabilities. Native plugins and plugin-provided UI/commands are not accepted.
- Automation decisions are advisory. TokenMaster cannot invoke an agent, shell,
  scheduler, browser, filesystem mutation, HTTP request, purchase, or credential flow.

## 14. Failure behavior

Errors are stable enums with bounded public descriptions. Wrapped OS, SQLite, parser,
HTTP, or provider messages are not serialized. Internal diagnostics contain codes,
counters, stage, and timestamps without paths or content.

A failed or cancelled scan cannot reconcile missing sources. A failed staging
generation remains invisible. A failed skin reload retains the last valid skin. A
failed locale switch retains the current locale. A stale UI request cannot replace a
newer snapshot. An MCP timeout terminates that request without leaving a worker,
transaction, or refresh lease behind. A plugin trap, timeout, OOM, crash, protocol
violation, invalid draft, or failed update terminates/quarantines only that package
generation, commits no partial staging data, and retains the last validated version.

## 15. Testing and acceptance

Every behavior change uses a focused failing test before implementation. Required
test families are:

- parser/replay corpus and property tests;
- store transaction, crash, promotion, rollback, and reconciliation tests;
- immutable query and keyset paging contracts;
- analytics/pricing golden fixtures with explicit provenance;
- JSON Schema and MCP protocol conformance;
- structured-content/text-fallback equivalence;
- CLI/MCP parity from the same fixture archive;
- smoke compatibility with Hermes, Codex, Claude Code, Gemini CLI, OpenCode, and a
  generic MCP inspector when those clients are available;
- skin schema, inheritance, cycle, bound, fallback, and hot-reload tests;
- localization coverage, pseudo-locale, overflow, and accessibility contracts;
- deterministic UI state and generation-race tests;
- startup, input-to-paint, switch, query, append, memory, handle, thread, USER/GDI,
  and CPU gates;
- repeated MCP process start/query/shutdown stress;
- provider WIT/manifest compatibility, package/signature/archive validation,
  capability denial, malicious component, hot-replacement/rollback/quarantine,
  core-canonicalizer, host start/stop, and plugin resource stress;
- interactive Windows/DPI/screen-reader evidence and uninterrupted soak receipts.

Client absence is reported as unverified; protocol conformance never implies a named
client was run. Release claims remain bound to one clean commit and executable hashes.

## 16. Delivery roadmap and gates

### A. Specification normalization

Merge this reviewed design into `SPECIFICATION`, `DATA_CONTRACT`, `API_CONTRACT`,
`SECURITY`, `DECISIONS`, `TRACEABILITY`, `ROADMAP`, and `FEATURE_PARITY`. Add no
behavior until contradictions and unresolved drafting markers are absent.

### B. Canonical replay correctness

Deliver fork/replay fixtures, logical identity, lineage proof, canonical selection,
and conflict handling. Gate: no known replay fixture double-counts and append memory
remains bounded.

### C. Engine and staging

Deliver staging generations, atomic promotion, scan epochs, cancellation, coalesced
filesystem events, reconciliation, and restart recovery. Gate: injected failure never
changes canonical state or authorizes destructive reconciliation.

### D. Query, analytics, pricing, and quota snapshots

Deliver immutable query facade, indexed aggregates, cost catalog with source/version,
and provider-defined quota windows. Gate: UI/CLI fixture queries use no full scan and
meet latency/page bounds.

### E. Automation connector

Deliver pure policy evaluator, CLI JSON, separate MCP binary, schemas, configuration
printers, conformance, and process stress. Gate: target clients receive equivalent
bounded structured data; GUI binary/resource budgets do not regress.

### F. Product UI

Deliver shell, six board sections, all supporting routes, data states, keyboard,
accessibility, virtualization, command palette, and compact widget. Gate: no mock data
in production startup and all views consume immutable query snapshots.

### G. Presentation and localization

Deliver three built-in families, independent axes, validated external skins, bounded
hot reload, en/ru/pseudo coverage, dynamic bars, reduced motion, and accessibility.
Gate: switch latency and 10K-cycle retention gates pass.

### H. Windows integration and release

Deliver single instance, tray, startup, hotkey, notifications, Explorer/sleep/resume
recovery, fast/heavy CI split, dependency/license audit, Slint attribution, SBOM,
package rehearsal, interactive matrix, and 24/72-hour evidence.

### I. Provider plugin system (1.1)

After the Codex-only release freezes observation/query/quota contracts, deliver
WIT/manifest schemas, deterministic `.tmplugin` packages, isolated Wasmtime host,
capability grants, hot installation/update/rollback, quarantine, signatures, SDK
templates, and conformance tools. Gate: no Codex-only GUI/resource regression;
malicious components fail closed; plugin hosts have bounded resources and no ambient
authority.

Linux and macOS product packaging follow Windows 1.0. Core/query/provider crates must
avoid new unconditional Windows dependencies so portability is not designed out.

## 17. Traceability and change control

Each implementation slice must update:

- `spec/TRACEABILITY.md` with requirement, implementation, and test evidence;
- `docs/CURRENT_STATE.md` with implemented and unverified truth;
- `docs/PROJECT_HISTORY.md` with the milestone and commands;
- the affected API, data, security, decision, operations, and roadmap documents;
- `docs/FEATURE_PARITY.md` when a reference feature becomes implemented.

Commits are small, English, and scoped to one accepted slice. Tracked documents do
not embed the current commit hash. Generated receipts bind exact commit and executable
identity outside tracked project truth.

## 18. Explicit non-goals for 1.0

- Multi-provider ingestion beyond Codex.
- Shipping the external provider runtime before observation/query/quota contracts and
  plugin security/resource gates are stable.
- Electron, Tauri, webview, Go, or Node runtime.
- Remote/cloud MCP or public local HTTP listener.
- A background daemon required for normal operation.
- Arbitrary SQL, shell, HTTP, filesystem, prompt, transcript, or credential tools.
- Native/in-process plugins, plugin-supplied UI, executable skins, policy expressions,
  or ambient plugin filesystem/network/command authority.
- TokenMaster starting or controlling an LLM agent.
- Silent edits to Hermes, Codex, Claude, Gemini, or OpenCode configuration.
- Exact future-task token prediction presented as fact.
- Packaging or release claims without the documented evidence identity gates.
