# TokenMaster P3-B.3 Application Composition Design

Status: approved for execution from the approved product architecture and the
operator's explicit autonomous `go` instruction.
Date: 2026-07-17.

## 1. Decision

P3-B.3 introduces `tokenmaster-app`, the only production application-composition
package and owner of `TokenMaster.exe`. It selects one validated installed or
portable archive root, starts the existing usage, quota, reminder, and nested Git
runtimes, connects one bounded desktop query controller to that archive, and keeps
runtime health current through completion hints.

`tokenmaster-desktop` remains a frontend leaf. It owns the Slint projection, one
query worker, one product reducer, and the capacity-one event bridge, but receives no
runtime, platform, provider, Codex, store, process, network, or filesystem authority.

```text
validated environment
        |
        v
tokenmaster-app ---------------------------------------------+
  | data-root policy                                         |
  | sole runtime owners                                      |
  | fixed runtime-health observations                        |
  +------------------------+---------------------------------+
                           |
                           v
Live/Quota/Reminder workers --completion hint--> DesktopController
       | sole archive writers                       | read-only query
       v                                            v
 bundled SQLite archive --> ProductReducer --> one latest snapshot
                                                 |
                                                 v
                                  one coalesced Slint event
```

## 2. Options considered

### A. Let `tokenmaster-desktop` own all runtimes

Rejected. It would erase the audited UI authority boundary, give Slint-adjacent code
filesystem/process/store access, and make future CLI/MCP or service composition reuse
the frontend package.

### B. Add a separate application-composition package

Selected. The executable package may depend on platform, Codex, runtime, product, and
desktop packages while the frontend remains independently auditable. Runtime owners
have one lifecycle and one shutdown authority, and future non-GUI entry points do not
need to inherit Slint internals.

### C. Poll runtime snapshots from the UI or another application timer

Rejected. Polling adds idle CPU, latency, a new timer/thread, shutdown ordering, and a
second cadence unrelated to the authoritative workers. It also makes long-run memory,
handle, and wake behavior harder to bound.

The selected event path adds one nonblocking completion hint to the existing engine
worker. It does not add a queue or dispatch thread. Each hint recomputes one fixed
health observation and asks the already capacity-one desktop controller to refresh.

## 3. Deterministic data-root policy

### 3.1 Mode selection

The production executable recognizes exactly one opt-in marker:
`tokenmaster.portable`, adjacent to the resolved current executable.

- If the marker is absent, installed mode uses
  `%LOCALAPPDATA%\TokenMaster\tokenmaster.sqlite3`.
- If the marker is a zero-length regular non-link file, portable mode uses
  `<exe-directory>\data\tokenmaster.sqlite3`.
- If a marker exists but is a directory, link/reparse point, non-empty file, or cannot
  be inspected, startup fails with a stable path-free code.

The P6 portable ZIP will contain the empty marker. An installer or unpackaged local
build omits it and therefore uses installed mode. There is no fallback from an
unwritable portable directory to installed storage because that would split user
truth silently.

### 3.2 Validation and creation

The selected base directory must already exist and pass
`ValidatedLocalDirectory`: absolute local fixed/removable/RAM media only on Windows,
no network/device namespace, mapped remote drive, traversal, symlink, or reparse
ancestor. Composition creates only the exact `TokenMaster` or `data` child with
non-recursive `create_dir`, tolerates an already-existing directory, and validates
the final child again before constructing the archive filename.

Production selection does not accept a command-line archive path, arbitrary
environment override, current-working-directory fallback, recursive directory tree,
or user-supplied filename. Tests inject an environment value object; production alone
captures `current_exe`, `LOCALAPPDATA`, `USERPROFILE`, and `CODEX_HOME`.

Paths remain private application state. Application errors, `Debug`, UI snapshots,
logs, completion receipts, and tests expose only stable codes and mode, never the
absolute path.

## 4. Provider discovery and runtime ownership

The built-in Codex source is composed from the current `USERPROFILE\.codex` and
optional `CODEX_HOME` values through `build_discovery_request`. Invalid individual
roots remain bounded provider diagnostics; an invalid aggregate request fails with a
stable discovery code. Configured external roots are intentionally empty until the
settings contract exists.

Application startup order is:

1. select the software renderer and validate the data-root policy;
2. build the bounded Codex discovery request;
3. start `LiveRuntime`, which creates/recovers the archive and owns usage ingestion,
   watcher, scheduler, writer, and its nested `GitRuntime`;
4. start the quota and reminder runtimes against the same archive;
5. open one read-only desktop query controller and create the truthful initial shell;
6. attach the existing capacity-one Slint bridge;
7. publish one initial fixed runtime-health observation and request one query refresh;
8. show the window and run the Slint event loop.

`LiveRuntime` is mandatory because no truthful data application exists without the
archive/ingestion owner. Quota and reminder startup failures degrade independently:
the app retains only a stable `RuntimeErrorCode` for the absent owner and publishes an
unavailable observation while usage/history remain usable. A later quota transport or
Codex executable discovery failure is normal runtime health, not startup failure.

No application code reads JSONL, enumerates sessions, writes SQLite, executes Codex,
inspects Git, activates reset benefits, or acknowledges reminders directly. Those
authorities remain in their existing packages.

## 5. Completion-hint contract

`RefreshWorker` gains an optional `WorkerCompletionNotifier` and a
`spawn_notified` constructor; the existing `spawn` behavior remains unchanged.
After coordinator completion and capacity-one receipt publication, the worker invokes
the notifier outside every worker/coordinator/result lock. The callback receives one
copied `WorkerCompletion` and must be bounded and nonblocking.

Notifier panic is caught and redacted. It cannot fault ingestion, allocate a retry,
or discard the capacity-one completion receipt. A later completion retries delivery.
This is deliberately a lossy wake hint: product truth is always recomputed from
runtime snapshots and the archive.

Usage, nested Git, quota, and reminder runtimes expose additive `start_notified`
constructors and pass the same application notifier to their existing workers. Their
legacy `start` constructors remain for tests and non-GUI composition.

The application notifier holds only a `Weak` reference to the application state. It
does not own a runtime, controller, window, or thread. An early completion before the
application bundle is installed sets one atomic pending bit; the post-install flush
closes that startup race.

## 6. Runtime health join

The application copies each runtime snapshot into the existing fixed product health
types. It creates one checked nonzero `ProductRuntimeGeneration` per recomputation and
publishes exactly four observations:

- usage health from `LiveRuntimeSnapshot`;
- Git health from the nested live snapshot;
- quota health or one stable unavailable code;
- reminder health or one stable unavailable code.

`ProductReducer` gains typed health publication methods so the desktop package never
depends on runtime source types. `DesktopController` gains one capacity-one
`DesktopRuntimeObservation` mailbox containing only copied product health/error values
and one generation. Equal or older observations are ignored. Each query attempt takes
the newest observation, applies it to its worker-confined reducer, and publishes it
only with a complete non-cancelled product snapshot. An observation racing an active
query remains in the one slot and the notifier's refresh coalesces to the existing one
follow-up.

There is no second `ProductSnapshot`, runtime owner, callback history, per-runtime
channel, or unbounded queue. The Slint thread still receives only one immutable
product snapshot through the P3-B.2 bridge.

## 7. Lifecycle and failure rules

After the event loop exits, shutdown takes the application bundle out of the shared
slot and releases the slot lock before any join. It then:

1. closes runtime admission by pausing usage, quota, and reminder owners;
2. shuts down and joins the desktop query worker;
3. shuts down reminder, quota, and live owners (live also closes nested Git);
4. drops the bridge/window and remaining weak notifier state.

Every cleanup step is attempted even if an earlier step fails; the first stable error
is returned. No mutex is held while joining a thread. A concurrent completion sees an
empty/expired weak state and becomes a no-op. `Drop` remains the final idempotent
cleanup backstop for all existing owners.

Startup uses local temporaries until every owner is ready, so ordinary Rust drop order
unwinds partial startup. Errors are code-only and never wrap OS, SQLite, provider,
process, Slint, or path-bearing source errors.

## 8. Boundedness and responsiveness

- Existing worker count is unchanged except for the already approved runtime and
  desktop owners; the notifier adds no thread, timer, queue, or task executor.
- Completion retention remains one receipt per worker.
- Runtime observation retention is one fixed copy; product retention remains one
  reducer snapshot plus one shared UI mailbox.
- A completion callback performs only bounded snapshot copies, short mutex sections,
  one capacity-one observation replacement, and one controller admission.
- Query and SQLite reads remain on the desktop worker, never the Slint event thread.
- Runtime writes remain on their existing workers and never wait for UI delivery.
- Generation counters are checked and never wrap; overflow produces a stable fault.

## 9. Security and future boundaries

This slice does not add CLI/MCP, plugin loading, HTTP, shell, browser, arbitrary SQL,
arbitrary filesystem access, benefit activation, or reminder acknowledgement. P5
automation remains a separate process/API surface, and provider plugins remain the
approved isolated WebAssembly Component design for 1.1. The app package is a fixed
composition root, not a general service locator or plugin host.

Power-monitor/tray wiring, user-configured roots, safe benefit-scope discovery,
visible route payloads, skins/locales, notifications UI, compact widget, packaging,
signing, and release acceptance remain later contours. Deferring power wiring avoids
creating a second lifecycle decision before P3-E owns tray/close/suspend behavior.

## 10. Verification and acceptance

P3-B.3 is complete only when focused tests prove:

- installed and portable roots are selected exactly and create only one validated
  child directory;
- invalid/non-empty/linked markers, missing installed base, network/reparse roots,
  and path-bearing errors fail stably without fallback or leakage;
- worker completion notifiers run after completion, are optional, add no queue, and a
  panicking notifier cannot fault the worker or lose its receipt;
- all four runtime constructors propagate the notifier, including nested Git;
- desktop observations are capacity one, generation ordered, copied into the reducer,
  and coalesce refreshes without partial publication;
- application composition has exactly one production binary, one live owner, one
  query controller, one bridge, no polling/timer, and no desktop authority regression;
- the release binary contains no private path, credential, fixture, old-project,
  probe, or raw provider strings;
- clean-root, format, warnings-as-errors Clippy, locked workspace tests, desktop audit,
  and application-composition audit pass.

This acceptance does not claim visible feature-complete routes, long-run P4/P6
resource gates, M0 acceptance, portable ZIP/signature, or a product release.

## 11. Closure review

The design was checked against the specification, data/API/security contracts,
ADR-049 through ADR-051, P3-B.1/B.2 designs, current state, handoff, roadmap, existing
runtime lifecycle APIs, product health values, platform path validation, and desktop
authority audit.

The selected split is the smallest architecture that keeps the UI reactive and
authority-free while making current runtime truth visible without polling. Data-root
selection is deterministic and portable, completion delivery is lossy-but-truthful,
startup/shutdown races are bounded, optional domains degrade independently, and no
new unbounded retention or hidden ingestion owner is introduced.
