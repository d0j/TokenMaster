# TokenMaster P3-E shell design

## Status and decision

This design resolves the remaining P3-E ownership and sequencing ambiguity. P3-E is
implemented as five independently reviewable vertical slices. It reuses the sole
production `MainWindow`, immutable product snapshot, route model, and application
lifecycle. It does not import or depend on `tokenmaster-m0`.

The delivery order is:

1. bounded in-window command palette;
2. compact content projection in the existing window;
3. tray-driven show/hide/quit lifecycle;
4. current-session single-instance activation and global hotkey;
5. opt-in current-user startup plus Explorer/restart/resource closure.

P4 remains responsible for complete skins, density, locale, DPI, visible-paint, and
accessibility acceptance. P5 remains responsible for CLI/MCP. P6 remains responsible
for package, signing, installation, and release evidence.

## Non-negotiable boundaries

- Desktop is a presentation leaf. It receives bounded typed values and emits bounded
  typed intents. It never opens SQLite, provider input, arbitrary files, registry
  paths, processes, sockets, or credentials.
- The application owns runtime start/shutdown ordering and composes optional native
  integration. Platform owns Win32 handles, registration, waiting, and cleanup.
- There is one production Slint window, one current product snapshot, one route model,
  one controller worker, and no route-specific scanner, timer, cache, or query owner.
- Compact mode is a presentation mode of the existing window, not a second retained
  renderer or a second snapshot graph.
- Tray, hotkey, startup, and secondary-instance failures are explicit bounded health
  states. They never fabricate quota truth and never make an invisible orphan process.
- No prompt, response, reasoning, command, command output, source content, credential,
  absolute path, provider identity, account identity, session identity, or raw OS
  error crosses the shell boundary.

## P3-E.1 — bounded command palette

The first slice is an in-window route palette. It reuses the exact 11 `RouteRow`
values already published for navigation. It adds no backend query and retains at most
one additional filtered model of 11 rows plus a 64-scalar query.

The palette opens from a visible header control or `Ctrl+K`. It closes with Escape,
outside dismissal, or successful activation. Search is case-insensitive over the
stable route key and current bounded visible label. Empty search returns all routes;
no match is explicit. Arrow keys move one checked selected ordinal and Enter activates
that row. Mouse/touch and accessibility default action use the same activation path.

The palette may navigate to an unavailable route because route selection already
preserves its truthful state/reasons. It may not execute backup, restore, import,
activation, or any other mutation. Later primary actions require separately typed
palette commands and authority tests; they are not encoded as magic strings now.

Filtering is synchronous and constant-size. The UI cannot retain an unbounded input:
Rust truncates the returned editor value to 64 Unicode scalar values before rebuilding
the model. Every rebuild fully replaces one model; no query history is retained.

## P3-E.2 — compact mode

Compact mode projects only current quota truth and bounded freshness/reason state from
the existing immutable snapshot. It never issues a query on entry. It uses the same
window and renderer, changes one presentation mode, and stores no second snapshot.

The compact surface shows all currently published provider-defined quota windows that
fit the existing cap, never hard-codes five-hour or weekly rows, and never renders an
unknown ratio as empty or full. It provides one checked return-to-dashboard action.
Window size/position are device-local presentation settings; data and portable policy
remain unchanged. Compact close behavior follows the tray capability rule below.

## P3-E.3 — tray lifecycle

The production Desktop package may declare one Slint `SystemTrayIcon`, but it exposes
only `Show`, `Hide`, `OpenCompact`, `OpenDashboard`, and `Quit` typed lifecycle intents.
It owns no application runtime and cannot mark the reliable run clean.

The application consumes these intents on the UI thread. Show restores and focuses the
existing window; Hide hides it; OpenCompact/OpenDashboard select the exact mode/route
before showing; Quit requests the Slint loop to return. Application shutdown then
closes admission, joins all owned workers, and only after successful joins publishes a
clean run. Close-to-tray is enabled only while the tray is proven available. Without a
tray, a close request exits normally instead of leaving an undiscoverable process.

Tray creation failure degrades to the visible main window with bounded health. It does
not fail data collection, create a retry timer, or spin. Explorer recreation is one
event-driven re-registration attempt at a time with capacity-one coalescing.

## P3-E.4 — single instance and global hotkey

Platform owns one current-session, current-user native integration owner. It uses
fixed product identifiers with no path or identity text. The primary owns fixed native
handles and one joined integration thread. A secondary instance signals the primary
to show the existing window and exits before opening SQLite or starting workers.

The primary retains at most one pending activation bit. Ten thousand secondary-instance
or hotkey signals coalesce without queue growth. The default global shortcut is a
documented fixed chord; conflict is explicit and leaves tray/window operation intact.
Registration and unregistration occur on the owning native thread, and shutdown joins
that thread before clean-run publication.

No named socket, HTTP listener, shell command, arbitrary window message payload,
clipboard payload, or path is accepted. Exact Win32 identifiers, security boundary,
message-loop topology, and stable failure codes must be specified in the slice plan
before implementation.

## P3-E.5 — current-user startup and native closure

Startup is an explicit device-local opt-in. Platform exposes a typed enabled/disabled
operation for the current user only. It writes one exact TokenMaster entry referring
to the verified running executable and reads it back before reporting success. It does
not invoke a shell, Task Scheduler, installer, elevation, or machine-wide policy.

Portable settings/config export never contains the executable path or startup choice.
Relocation is shown as stale device-local configuration and requires user confirmation;
it is not silently rewritten during ordinary startup.

The final P3-E gate covers Explorer restart, secondary activation, hotkey conflict,
tray absence, startup access denial, sleep/resume, rapid show/hide/mode changes, and
clean shutdown. Repeated cycles must return private bytes, handles, threads, USER, and
GDI objects to the measured post-warm-up envelope.

## Failure and headless semantics

"Headless degradation" does not authorize a permanently invisible GUI process. Test
and embedding consumers may run the application core without constructing Desktop.
The production GUI requires its main window; if renderer/window creation fails it exits
with a stable path-free failure after joined cleanup. If optional tray/hotkey/startup
integration fails, the visible main window and local data engine remain usable.
Future CLI/MCP binaries provide intentional no-window operation under P5.

## Acceptance gates

Each slice requires focused RED/GREEN tests, the relevant source mutation audit,
strict package Clippy, formatting, and independent review. P3-E closure additionally
requires the exact clean-root/fmt/workspace-Clippy/workspace-test baseline plus Windows
resource return. These developer gates do not claim interactive M0, package, signing,
soak, or release acceptance.
