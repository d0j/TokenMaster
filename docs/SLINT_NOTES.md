# Slint 1.17.1 operational notes

What is not obvious from the documentation, verified against the unpacked 1.17.1
sources and the upstream tracker. Configuration this applies to: winit backend, `renderer-skia` letting the backend be
chosen at runtime, `accessibility`, Windows 11 x86_64-pc-windows-msvc. Where a note is
about the software renderer it says so: that one was measured, rejected and is kept here
because the reasoning still applies to anyone tempted back to it.

## Rendering

**Replacing a `ModelRc` forces a full-window repaint.** `ModelRc`'s `PartialEq` is
pointer identity, so a freshly constructed one never compares equal. The repeater
then drops every instance, each drop calls `free_graphics_resources`, and that sets
`force_screen_refresh` unconditionally — the comment in `partial_renderer.rs` admits
it is a last resort because deleted items have no known screen region. Upstream
#7432, open.

This repository used to build a new `ModelRc` on every projection, so every data
update repainted the whole window even when the pixels were identical. `fn patch_rows`
now writes through `set_row_data` and returns a replacement only when the row count
changed; the 26 call sites go through the `set_rows!` macro. `VecModel::set_row_data`
is the cheap path and does not trip the repaint; `push`/`insert`/`remove` still
destroy instances, and `set_vec` and `clear` issue a full reset — so a new call site
must use the macro rather than reach for those.

**Partial rendering is on by default here and in its best mode.** softbuffer's Win32
backend returns buffer age 1 after the first present, which selects `ReusedBuffer` —
no union with the previous frame's dirty region. A resize or a minimize resets it to
`NewBuffer` and clears the cache.

**The dirty region holds at most 3 rectangles.** A 4th scattered change unions two of
them, inflating the repainted area. Grouping frequently-changing indicators spatially
is a real optimisation.

**`Path` ignores the dirty region.** Every other primitive writes through
`foreach_ranges`, which cannot escape the dirty region. `process_filled_path` and
`process_stroked_path` pass only the item clip. Slint still decides *whether* to draw
it, but once drawn it blends over its whole clipped bounding box. The Dashboard trend
chart uses two `Path`s. Whether this produces visible cumulative darkening was
inferred from the code, not observed.

**Silently unsupported by the software renderer:** `drop-shadow-*` draws nothing
(`draw_box_shadow` is an empty body), `rotation-angle` and `transform-scale` are
no-ops (`SUPPORTS_TRANSFORMATIONS = false`), and `image-rendering` does not exist —
image scaling is nearest-neighbour, so ship raster assets at their exact physical
size. Text is grayscale-antialiased with no hinting and no ClearType; colour emoji do
not render in colour (only `Source::Outline` is requested).

**No occlusion culling.** Overlapping translucent layers cost N blends per pixel per
repaint. An opaque rectangle does not stop what is beneath it from being drawn.

Safe and cheap: solid and `with-alpha` colours, `border-radius` fills, rectangular
`clip: true`, `Text`, images at 1:1, in-place `set_row_data`.

**Three renderers, measured on one machine at 200% DPI.** Switching is two lines --
the `renderer-*` feature in the workspace `Cargo.toml` and `SLINT_BACKEND` in
`.cargo/config.toml` -- so these numbers are what the choice costs, not an argument
about it.

| | software | femtovg | skia |
|---|---|---|---|
| binary | 34.83 MB | 34.32 MB | 40.37 MB |
| start to tray icon | 6.73 s | 16.74 s | 8.02 s |
| private bytes | 33.3 MB | 295 MB | 51.8 MB |
| working set | 58.5 MB | 172.4 MB | 92.7 MB |
| threads | 25 | 33 | 25 |
| idle CPU | 83.6% | ~90% | 83.9% |
| blank window after a move | **yes**, 27.5% of pixels differ over two backgrounds | no | no, 7.8% |
| text | grayscale AA, no hinting | crisp | crisp |
| CRT linkage | static | static | **dynamic** |

femtovg is the worst of the three on every axis but binary size. Skia costs 5.5 MB and
1.3 seconds of startup over software and removes the blank window entirely.

Skia will not build with `+crt-static`: no prebuilt bindings exist for that target and a
source build needs LLVM, which `skia-bindings` panics about at
`build_support/platform/windows.rs:32`. Without the flag it builds in about four minutes
and the binary then imports `VCRUNTIME140.dll` and `MSVCP140.dll`.

The idle CPU is within a point of itself on all three, which is the third independent
result ruling the renderer out as its cause.

**femtovg was measured too, and lost to both.** The blank window after a move disappeared
under it -- 29.1 mean luma where software gave 48.4 -- which is what proved the defect
belongs to softbuffer rather than to Slint. But sampled every ten seconds for eighty
seconds it held about 368 MB of private bytes and 245 MB resident against software's 41.6
and 69, and it started in 16.7 s against 8. Skia beats it on every one of those. Forcing
Skia onto OpenGL specifically is a separate trap: 518 MB private and 61 threads against 56
MB and 24 when the backend is chosen at runtime, so do not name the API.

**The CPU cost was ingestion, not idling, and not Slint.** An empty Slint window built with
this repository's exact features measures 0% of a core against the product's peak, which
placed the cost in our tree; thread-name profiling then placed it in `tokenmaster-refresh`,
at 76.3% of a core averaged over 92 seconds from a cold start, falling to nothing after
about 43 seconds. Renderer, animation and the `accessibility` feature were each ruled out by
measurement first.

Two instrument defects cost more than the investigation did, and both looked exactly like
product defects. `GetThreadDescription` needs `THREAD_QUERY_LIMITED_INFORMATION` (0x0800);
asking for 0x0400 returns no name for any thread, which is what made the standard library
look as if it never named them -- it does. And two live threads share the name
`tokenmaster-refresh`, so a probe that takes the first match can sample the sleeping one and
report zero while a core is busy.

**The original bisection, kept because it is what narrowed the search.** An empty Slint window built with
this repository's exact features -- skia renderer, winit backend, accessibility -- measured
**0% of a core, 10 threads, 7.1 MB** while the product measured 81% and 24 threads. Slint and
winit are therefore not the source. Two named upstream suspects were tested and both failed:
building without the `accessibility` feature changed nothing (86.7% against 81%, so not
AccessKit/UIA despite slint#3867 and egui#4527), and the burn is identical across all three
renderers. Gating the tree's only `animation-tick()` changed nothing either. What remains is
roughly fifteen threads the product starts that the empty window does not.

Slint does reach `ControlFlow::wait_duration(next_timer)` -- winit's `WaitUntil`, implicated by
winit#1610 on Windows -- but only when a Slint timer is armed, and the empty window proves that
path is quiet here.

## Window lifecycle

**`window().is_visible()` does not mean visible.** It returns whether `show()` was
called and `hide()` was not — true for a minimized, occluded or off-screen window.
The docs.rs prose claiming otherwise is stale for the winit backend. The closest
honest signal is `is_visible() && !is_minimized()`; occlusion is not detectable at
all, because winit emits no `Occluded` event on Windows. Anything that must be seen
to count as delivered should therefore be retryable rather than asserted.

**`hide()` on Windows destroys nothing.** It is `set_visible(false)`. The HWND, the
renderer, the softbuffer surface, the position and the item tree all survive. Only
Wayland or `SLINT_DESTROY_WINDOW_ON_HIDE` takes the teardown path.

**The window can render nothing at all, and the buffer age is why.** softbuffer's Win32
surface reports a buffer age of 1 after the first present, so Slint keeps
`RepaintBufferType::ReusedBuffer` and paints only the dirty region. Windows clears that
buffer on events the age does not reflect. Slint compensates in the software renderer's
`occluded()` hook and says so in the source -- *"the buffer is completely cleared when
the window is hidden and the buffer age doesn't respect that"* -- but winit emits no
`Occluded` on Windows, so that hook is reached only through `Resized(0,0)`, which means
a minimize and nothing else. After any other clear the untouched area holds nothing and
the desktop shows through the window.

Measured here: a capture taken after a plain `SetWindowPos` move differed from one taken
before it in 27.5% of sampled pixels, and mean luma rose from 28 to 44 as the lighter
desktop bled through. A one-pixel size change heals it completely, because a resize takes
the `NewBuffer` path. `fn force_full_repaint` applies that nudge on the restore path,
which measured 27.7 luma afterwards against 48.4 for a move. **A window the user drags is
still affected**: Slint surfaces no move event, so there is nothing to hook. That half is
upstream.

**hide → show skips the cache invalidation that minimize → restore gets.** The
software renderer has an `occluded()` hook that resets to `NewBuffer` and clears the
partial-render cache, with the comment that the Windows surface is cleared while
hidden. winit never emits `Occluded` on Windows, so Slint wires that hook to
`Resized(0,0)` — which minimize produces and `hide()` does not. This is the most
likely mechanism behind a stale repaint after restoring from the tray. Workarounds:
`SLINT_DESTROY_WINDOW_ON_HIDE=1`, or force a real size change on show.

**`show()` is a no-op on a minimized window** — it early-returns when the visibility
state already matches, and minimized is still `Shown`. Restoring needs
`set_minimized(false)` first, then `show()`, then activation. `crates/app/src/application.rs:1230`
already does this plus a raw `SW_RESTORE`; do not "simplify" it.

**Slint has no focus or raise API.** Raw Win32 is the only route, and
`SetForegroundWindow` legitimately fails when the caller is not the foreground
process — which is exactly the tray-click case.

**Timers and callbacks keep running while hidden; only drawing stops.** `draw()`
early-returns on hidden *before* clearing `pending_redraw`, so a property write made
while hidden arms the monitor-refresh-rate frame throttle and nothing disarms it
until the window is shown. Work scheduled while hidden must be safe with no visible
UI, and anything whose purpose is to be seen must be re-asserted on show rather than
marked delivered when written.

**`run_event_loop()` exits when the last window hides.** Hide-to-tray requires
`run_event_loop_until_quit()`.

**DPI:** winit sets per-monitor-v2 awareness at event-loop creation and Slint
re-lays-out through the reactive `scale_factor` property, so no manual re-layout is
needed. `SLINT_SCALE_FACTOR`, if set anywhere in the environment, freezes it for the
whole session and drops every DPI change.

## Language

**A binding is severed by the first user interaction, permanently.** This is
documented and by design: `checked: root.x` on a `CheckBox`, `value:` on a `SpinBox`,
`text:` on a `LineEdit`, `current-index:` on a `ComboBox`, `value:` on a `Slider`.
After the user touches the control, Rust-side `set_*` pushes are silently dropped.
The fix is `<=>`, which requires `in-out` on both ends, or an `edited`/`toggled`
handler that routes through Rust. The backup-policy card was converted this way and
is the worked example: four declarations in `main.slint`, four in `settings-view.slint`,
four forwards and four control bindings, all of which must change together. A test
proves it only if it interacts with the control *before* asserting the push — one
that merely pushes passes on the broken version too.

**`visible: false` is compiled into a 0×0 `Clip` wrapper**, not removal. The subtree
still exists, still evaluates its bindings, and still occupies layout space — but its
clip rect is empty, which is why it vanishes from `ElementQuery`. `if` removes the
element from the item tree: no layout space, and handles become invalid.

**Since 1.17.0 `for` and `if` contents are instantiated eagerly.** `if` is no longer a
cost-deferral mechanism; a route view behind a condition pays its binding cost at
construction.

**A stretch factor of 0 means "do not stretch" unless every sibling is also 0**, in
which case all stretch. Adding one `horizontal-stretch: 1` therefore changes the
behaviour of every sibling.

**A callback holds exactly one handler.** Registering twice silently replaces the
first. Invocation from Rust is synchronous and re-entrant.

**`@tr()` takes a string literal only** — it is extracted at compile time, so a
computed string cannot be translated. `build.rs` sets
`DefaultTranslationContext::None`, so the lookup key is the bare msgid with no
context. A msgid missing from a catalog renders as the raw English.

## Testing

**`ElementQuery` walks the item tree, not the accessibility tree**, and hard-skips
any item where `ItemRc::is_visible()` is false. That predicate is purely geometric:
the item's rect against the intersection of every clipping ancestor. One mechanism
explains two surprises — `visible: false` empties the clip, and a `ScrollView` is a
`Flickable` which always clips, so rows scrolled out of the viewport are not found.
Partial overlap still counts as visible; only fully clipped items disappear.

**The testing window starts at the root's preferred size**, 880×760 here, which is
below the 1180px sidebar breakpoint. A test that wants the sidebar must
`set_size` first.

**`find_by_accessible_label` is exact string equality**, and a bare `Text` carries an
implicit `text` role and label — so filter by role as well, not instead.

**Element-id and type-name queries need debug info.** `build.rs` enables it only when
the profile is not release, and without it those queries return *empty*, not an
error. `cargo test --release` silently blinds them.

**`init_no_event_loop()` is per-thread** and may be called in every `#[test]` in a
binary; `init_integration_test_*` installs a process-wide proxy and panics on the
second call, hence one test per binary. `take_snapshot` needs the software renderer
backend, not the default mock one.

**The testing window ignores the scale factor in `set_size`.** `TestingWindow::set_size`
calls `size.to_logical(1.)` and `size.to_physical(1.)`, so a `LogicalSize` handed to it
is stored unconverted no matter what `scale_factor()` reports. Dispatching
`ScaleFactorChanged` does move `scale_factor()`, but any assertion that reads the size
back measures the harness rather than the product. DPI-dependent arithmetic has to be
unit-tested as a pure function instead.

**Not assertable in-process:** what a screen reader announces, focus location, and
per-pixel text (system fonts are live).

## This repository

Established patterns, so a change follows the existing shape:

- One `MainWindow`, ~200 scalar `in` properties and ~30 replace-only models, all
  pushed from Rust. The `.slint` side owns no domain state.
- Every behaviour is attached by a `wire_*(&window, …)` free function called from
  `new_with_optional_lifecycle_sink` (`ui.rs:574-655`). New interactions go there.
  Note `wire_reliable_state_intents` tail-calls `wire_reminder_policy_editor`.
- Callbacks reach domain code only through single-install `*IntentRouter`
  indirections, so the shell can exist before the application does.
- Localized display strings live in the `ProjectionStrings` global and are called
  from Rust as functions; a locale switch must re-project everything, because the
  strings were baked into model rows.

Known-fragile. Each is scheduled in the "Recorded, with a phase" table of
`docs/RELEASE_EXIT_PLAN.md`; this section explains the mechanism, that one commits
to when:

Anchors here are symbols, not line numbers: every line reference this file once
carried had drifted.

- `fn update_compact_window_mode` — compact-window sizing samples `window().size()`
  and `scale_factor()` before the window is shown, and compares physical pixels
  against logical-looking constants. Wrong on any non-100% display.
- `dashboard-view.slint`, `out property <DashboardSectionRow> active-section`, indexes
  sections positionally with nothing enforcing the order.
- `high-contrast` and `reduced-motion` are declared and consumed by the tree but
  never set from Rust, so both affordances are inert.
- The Win32 tray reconstructs a `&'static Inner` from `GWLP_USERDATA`. It is correct
  today only because `Drop` clears the pointer before `DestroyWindow` and no message
  handling runs above that point.

## Attributing journal frames to the tables that cause them

When a database grows a journal without growing itself, guessing which statement is responsible
from reading code is slower and less reliable than reading the journal. Every write-ahead-log
frame carries the page number it replaces; every page belongs to exactly one b-tree, and that
map is recoverable by walking each `rootpage` in `sqlite_master` down through the interior pages
in the main database file. Opening the database with `immutable=1` guarantees the reading cannot
write, checkpoint or recover anything.

`scratchpad/wal_owner.py` does this. On the ingestion blocker it reported 8032 of 9564 frames --
31.4 MB of 39 -- against `usage_source` over only 237 distinct pages, with 8044 frames carrying
their own commit: one transaction, one frame, per file. Two hypotheses read out of the code had
already been wrong before this measurement was taken, and both were wrong in the same direction,
naming a plausible statement rather than the one that ran.

A frame count is also the honest unit for a guard. A test that observes 200 sources and counts
journal frames fails at exactly 200 for a batch implemented as a loop of transactions, and the
number it prints is the defect itself rather than a proxy for it.
