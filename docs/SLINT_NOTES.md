# Slint 1.17.1 operational notes

What is not obvious from the documentation, verified against the unpacked 1.17.1
sources and the upstream tracker. Configuration this applies to: winit backend,
`renderer-software`, `accessibility`, `SLINT_BACKEND=winit-software`, Windows 11
x86_64-pc-windows-msvc.

## Rendering

**Replacing a `ModelRc` forces a full-window repaint.** `ModelRc`'s `PartialEq` is
pointer identity, so a freshly constructed one never compares equal. The repeater
then drops every instance, each drop calls `free_graphics_resources`, and that sets
`force_screen_refresh` unconditionally — the comment in `partial_renderer.rs` admits
it is a last resort because deleted items have no known screen region. Upstream
#7432, open.

`crates/desktop/src/ui.rs:4127` builds a new `ModelRc` on every projection, and all
30 `set_*_rows` sites go through it. Every data update therefore repaints the whole
window even when the pixels are identical. `VecModel::set_row_data` is the cheap
path and does not trip it; `push`/`insert`/`remove` still destroy instances,
`set_vec` and `clear` issue a full reset.

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

## Window lifecycle

**`window().is_visible()` does not mean visible.** It returns whether `show()` was
called and `hide()` was not — true for a minimized, occluded or off-screen window.
The docs.rs prose claiming otherwise is stale for the winit backend. Any "the user
can see this" decision needs a different signal.

**`hide()` on Windows destroys nothing.** It is `set_visible(false)`. The HWND, the
renderer, the softbuffer surface, the position and the item tree all survive. Only
Wayland or `SLINT_DESTROY_WINDOW_ON_HIDE` takes the teardown path.

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
handler that routes through Rust.

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

Known-fragile, recorded rather than fixed:

- `settings-view.slint:248-256` — four backup-policy controls bound one-way with no
  handler. After one user interaction the pushes at `ui.rs:2254-2260` are dead.
- `ui.rs:1952-1987` — compact-window sizing samples `window().size()` and
  `scale_factor()` before the window is shown, and compares physical pixels against
  logical-looking constants. Wrong on any non-100% display.
- `dashboard-view.slint:535` indexes sections positionally with nothing enforcing
  the order.
- `high-contrast` and `reduced-motion` are declared and consumed by the tree but
  never set from Rust, so both affordances are inert.
- The Win32 tray reconstructs a `&'static Inner` from `GWLP_USERDATA`. It is correct
  today only because `Drop` clears the pointer before `DestroyWindow` and no message
  handling runs above that point.
