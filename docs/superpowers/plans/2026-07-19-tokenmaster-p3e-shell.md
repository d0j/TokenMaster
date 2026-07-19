# TokenMaster P3-E shell implementation plan

Design: `docs/superpowers/specs/2026-07-19-tokenmaster-p3e-shell-design.md`

## Invariants

- The root workspace and sole production binary remain unchanged.
- `tokenmaster-m0` is evidence only and never becomes a production dependency.
- One window, one current snapshot, one controller, and bounded replace-only models.
- Desktop remains free of store/runtime/platform/native-handle authority.
- Native integration is optional and app/platform owned; failures are explicit.
- No release, package, signing, M0, or soak claim follows from P3-E developer evidence.

## Task 1 — implement the bounded route command palette

Status: complete as developer evidence. Focused/release Desktop gates, 134 mutation
cases, clean-root, formatting, strict full-workspace Clippy, and the complete locked
workspace test/doctest gate pass. This status does not close Tasks 2-6 or any release
gate.

**Files:**

- Create `crates/desktop/ui/components/command-palette.slint`.
- Modify `crates/desktop/ui/main.slint`.
- Modify `crates/desktop/src/ui.rs`.
- Modify `crates/desktop/tests/ui_contract.rs`.
- Modify `scripts/audit-desktop-shell.ps1`.
- Modify `scripts/tests/audit-desktop-shell.Tests.ps1`.

**RED:**

1. Prove the compiled shell starts with the palette hidden and exactly 11 reusable
   route commands.
2. Prove `Ctrl+K`, visible header activation, Escape, no-match, checked keyboard
   selection, mouse/default action, and successful route activation in the same
   window.
3. Prove a 10,000-scalar input is returned/truncated to at most 64 scalar values,
   retains at most 11 rows, and no sequence retains prior models.
4. Add mutations rejecting a missing shortcut, query cap, replace-only model,
   accessible labels, route-only activation, or any timer/query/worker/authority.

**GREEN:**

Reuse `RouteRow` and the current `DesktopState` projection. Add one filtered model and
one query callback. Do not add a Rust source file, dependency, worker, timer, query,
cache, or command string that can trigger mutation.

**Focused gate:**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_contract --locked
Invoke-Pester -Path scripts/tests/audit-desktop-shell.Tests.ps1
pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-desktop --all-targets --locked
```

## Task 2 — implement compact content in the existing window

Status: complete as developer evidence. The same `MainWindow` reuses all current
bounded quota rows, explicit unavailable/unknown-ratio truth, one geometry restore
slot, and one accessible Dashboard return. Focused/release Desktop gates, 141 mutation
cases, 10,000 same-component switches, independent 0/0/0 review, clean-root,
formatting, strict workspace Clippy, and the complete locked workspace test/doctest
gate pass. Native maximized/mixed-DPI/screen-reader behavior remains interactive
acceptance evidence.

Create one bounded Compact Widget view from current quota projection. Entry replaces
only presentation mode; it does not issue a query or create a second window/snapshot.
Cover dynamic provider windows, explicit unavailable/partial state, resize/restore,
keyboard/accessibility, 10,000 switches, and no retained-resource growth.

## Task 3 — compose the production tray lifecycle

Status: complete as developer evidence. One isolated native tray owner and asset, five
typed lifecycle intents, one queue-free single-install router, same-window Dashboard/
Compact actions, visible-first deferred installation, availability-aware close, and
joined Quit pass focused/full Desktop and app tests, strict package Clippy, 226
combined mutation cases, and both release audits. Checked Explorer re-registration,
fail-visible fallback, and explicit foreground activation are implemented; live
Explorer/focus/sleep/resource acceptance remains Task 6/P6.

Add one production tray owner and typed lifecycle intents. App owns show/hide/mode/
quit consequences and joined shutdown. Cover close-to-tray only when available,
visible fallback without tray, exact quit ordering, checked Explorer re-registration,
and capacity-one lifecycle coalescing. Keep M0 imports/dependencies forbidden.

## Task 4 — add current-session activation and hotkey

Specify exact fixed Win32 identifiers and stable errors, then add one platform owner
for primary/secondary activation plus one fixed global hotkey. Secondary activation
must signal before SQLite/runtime startup. Cover ownership, security, conflicts,
10,000-signal coalescing, panic/failure cleanup, and handle/thread return.

## Task 5 — add opt-in current-user startup

Extend device-local settings only. Add typed current-user startup read/write/readback,
verified executable identity, explicit stale relocation, access-denied degradation,
no shell/elevation/machine-wide authority, and config export exclusion.

## Task 6 — P3-E closure

Update specification, API/data/security contracts, traceability, decisions, current
state, roadmap, changelog, project history, and handoff. Run focused audits, independent
high-risk review, clean-root, fmt, strict full-workspace Clippy, full locked workspace
tests, release composition, and Windows resource-return gates. Leave P4/P5/P6,
activation, M0, packaging, signing, soak, and release explicitly open.
