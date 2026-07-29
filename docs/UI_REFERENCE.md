# TokenMaster UI reference

The owner produced a Dashboard mockup and named it the standard the product is built to.
This file transcribes it, because an image does not survive a repository and a description
does. Where the built product differs from what is written here, the product is wrong.

**Every number below is illustrative.** The owner's instruction is explicit: the layout is
the specification, and the values come from TokenMaster's own archive. A figure copied from
this file into the product is a fabrication, and the distinction the whole interface exists
to protect -- unavailable, partial, legitimate zero -- is exactly what a copied placeholder
destroys.

`AGENTS.md` already permits studying WhereMyTokens and reimplementing what is learned. This
mockup is that study made concrete: it is the target, and every visual element in it is in
scope.

## The system

**Two type families.** Proportional for navigation, card titles and prose. **Monospaced for
every number and every label.** This is the single decision the whole look rests on: it is
what makes a usage tool read as an instrument rather than a page.

**Labels are uppercase with tracking**, in the secondary colour, at caption size. `COST
TODAY`, `TOKENS`, `EVENTS`, `CACHE HIT`, `COMMITS`, `+ADDED`, `-REMOVED`, `$ / 100 LINES`,
`IN / OUT`, `BURN / DAY`, `RESETS LEFT`, `VIEWS`, `PRO · WEEKLY WINDOW`, `WINDOW`, `MODEL`.

**Numbers dominate.** A hero figure runs roughly three times the height of its label.

**Colour is semantic, never decorative.** Green for gain, for healthy state and for the
token series; red for loss; amber for degraded, for warnings and for a count that is running
out; blue for the cost series and for the largest spender's bar. **The wordmark is green**,
beside a filled green square.

**Every number carries evidence under it.** This is the density that distinguishes the
mockup from what is built: not more numbers, but each number explained by smaller facts
directly beneath, in the secondary colour at caption size.

**The whole Dashboard fits at the size the mockup is drawn at, and nothing scrolls.** This
is a layout constraint, not a preference: everything on this screen is visible at once, so
density is achieved by making cells compact rather than by giving the view a scroll
surface. A scrollbar appearing on the Dashboard at this size is a defect. Below this size
the responsive rules still apply -- the constraint is that scrolling is never how the
design is made to fit at its own size.

**Inner cells inherit from TokenMaster.** The nested cells -- the four in the top strip,
Plan Usage's three sub-cards, the Sessions rows, the Cost by Model blocks -- take their
surface, border, radius, spacing, type size and weight from the existing token set, and
their values from the archive. Nothing here is styled per cell and nothing is filled from
this document. Apply the design everywhere else; inside a cell, inherit.

## Screen by screen

### Header

- filled green square, then the `TokenMaster` wordmark in green, bold, proportional
- vertical rule, then `Local usage intelligence · Dashboard` in secondary
- right side, in order: `● synced 14:33:54` -- green dot plus monospaced clock, inside its
  own bordered pill; a `Refresh` button; a `Degraded` pill in amber; then the window
  minimise, maximise and close controls
- `Go to route` is **not** in the header. It lives at the right end of the bottom bar.

### Sidebar

- `VIEWS` -- uppercase, tracked, caption size, secondary colour, above the list
- eleven routes in fixed order: Dashboard, History, Sessions, Models, Projects, Activity,
  Data Health, Notifications, Settings, Help / About, Compact Widget
- the active route carries a left accent bar in green and a lighter background
- per-route status glyph at the right edge: green `●` healthy, amber `▲` attention
- a bordered footer panel pinned to the bottom: `sources 98/104` with the count in amber,
  and `db size 412 MB` monospaced

### Top strip -- four bordered cells across the full width

Four, not three. `CACHE HIT` is a headline figure in its own right, not a sub-card of Plan
Usage.

| cell | contents |
|---|---|
| 1 | `COST TODAY` label with `-12% vs avg` in green at the right of the same line; `$46.98` hero |
| 2 | `TOKENS` label; `111.9M` hero; beneath it `in 3.1M · out 337.6K` |
| 3 | `EVENTS` label; `902` hero; beneath it `194 sessions · peak 68/min` |
| 4 | `CACHE HIT` label; `97%` hero in green; beneath it `saved $293 · cache 108.5M` |

There is no `Today` title and no `Fresh · Authoritative` line above the strip. The strip is
the first thing under the header.

### Plan Usage

Left half of the second row.

- title, `Ready` pill in green
- `PRO · WEEKLY WINDOW` uppercase tracked caption
- `34%` at display size in green
- right-aligned pair: `elapsed 41%` and `resets Sat · 152h 52m`
- a thin progress bar, green fill, with a separate vertical tick marking elapsed time
  against consumed quota -- the two are different quantities and the bar shows both
- three bordered sub-cards: `IN / OUT 9.2 : 1`, `BURN / DAY $52.10`, `RESETS LEFT 1` with
  the count in amber

### Code Output

Right half of the second row.

- title, a `day` / `week` segmented toggle with `day` selected, `Ready` pill
- four metrics across: `COMMITS 3`, `+ADDED 3056` green, `-REMOVED 309` red,
  `$ / 100 LINES $1.54`
- a line chart in green with a filled dot at the current end
- captions under the chart: `07-22` left, `net +2747` centred, `today` right

### Usage and Cost Trend

Full width, third row.

- title, then `30d · $4552 · 1.08B tokens` as a monospaced subtitle on the same line
- legend on the right: `● tokens` teal, `● cost` blue, then the `Ready` pill
- y-axis extremes only: `1,080,434,426` top left, `$202.20` top right in blue
- two series: teal for tokens, blue for cost
- x-axis `06-29`, `07-07`, `07-14`, `07-21`, `07-28`

### Sessions

Left half of the fourth row.

- title, `Degraded` pill in amber
- a four-column table with uppercase tracked headers: `WINDOW`, `TOKENS`, `COST`, `MODEL`
- `WINDOW` is a start-to-end clock range, `14:22:31-14:33:54`, monospaced and left aligned
- `TOKENS` and `COST` are right aligned; `MODEL` is right aligned and coloured by family
- six rows visible, newest first

### Cost by Model

Right half of the fourth row.

- title, with `today` in secondary at the right -- the period, not a control
- one block per model, ordered by cost descending
- each block: the model name on the left, its cost right-aligned on the same line; then a
  horizontal bar whose length is that model's share of the largest; then a caption line
  `312 calls · in 1.4M · out 121K · cache 92%`
- the leading model's bar is blue; the rest are teal

### Bottom bar

Two groups on one line, full width.

- left, monospaced: `● daemon running` with a green dot, then `events 902`, `queue 0`,
  `db 412 MB`
- right: `indexing 98/104` with the counts in amber, then `settings`, then `go to route`

## Built, and not yet built

Done: the type scale and the monospaced numeric voice; uppercase tracked labels; semantic
colour on `+Added` and `-Removed`; per-column accessible labels on the trend; the `VIEWS`
heading; the two-dot trend legend; the area fill under the trend series; three bordered
Today cells at equal stretch; the labelled route column.

Superseded by this revision, and to be changed in the product:

- the top strip becomes **four** cells; `CACHE HIT` is promoted out of Plan Usage
- Plan Usage's sub-cards become `IN / OUT`, `BURN / DAY`, `RESETS LEFT`; `CACHE HIT` and
  `SAVED` leave it
- `NET` leaves the Code Output metric row and becomes the centred caption under its chart;
  `$ / 100 LINES` takes the fourth metric slot and `all-time avg` is gone
- the wordmark is green, not amber
- `Go to route` leaves the header for the bottom bar
- the bottom bar is no longer four navigation entries; it is a daemon/queue/archive status
  line on the left and `indexing`, `settings`, `go to route` on the right

Not yet, and purely presentational -- no new data needed:

- the `day` / `week` toggle in Code Output

**Already carried to the shell, and this file said otherwise.** Checked against the code
rather than assumed, after an earlier revision of this list called them missing:

- `$ / 100 LINES` -- `DesktopCodeOutputProjection::cost_per_100_added_lines_micros`, bound
  to `dashboard-code-efficiency` and on screen today
- `net +2747` -- `DesktopCodeOutputProjection::net_lines`, bound to `dashboard-code-net`;
  only its position changes under this revision
- `sources n/m` and `indexing n/m` -- `DesktopDashboardProjection::import_progress`, but
  only while an import runs: `load_import_progress` returns nothing once publication
  quality stops being `Partial`, so a steady-state total is still missing
- the Sessions table's `WINDOW`, `TOKENS` and `COST` columns -- `DesktopSessionRow` already
  carries both timestamps, the token value and the cost, and `apply_sessions` already
  formats the clock range. **`MODEL` is the one column genuinely absent**, and it is absent
  all the way down to the store's session summary

**Computed upstream and thrown away, each at one nameable place.** This is the valuable
half, because none of it needs new data -- only carrying:

- the `in / out` split and the cached total -- `UsageMetrics` carries `input`, `cached`,
  `output` and `reasoning`, and every other route projection maps them faithfully;
  `map_analytics` takes `total()` alone. The `CACHE HIT` percentage is one division of two
  values already present.
- `elapsed 41%` -- both derivations exist on `QuotaWindowValue`, its epoch's first and last
  observation and the definition's nominal duration, and `map_quota_row` touches neither
- `synced 14:33:54` -- `QueryHeader::data_through_ms` rides on every envelope and
  `data_through_ms` appears nowhere in the desktop crate
- the per-model in/out/cache captions -- `map_models` keeps `total()` and the cost while the
  Models route keeps the full split from the identical breakdown item
- `resets Sat · 152h 52m` -- the instant is already projected; only a reference "now" is
  missing, and the envelope header carries one

**Genuinely absent, and needing real work:** `-12% vs avg` · `saved $293` (the counterfactual
never leaves the pricing crate, though the cached and uncached bases are both in
`TokenPriceBasis`) · `194 sessions` · `peak 68/min` (minute buckets are already materialised
in `usage_time_rollup`; nothing takes their maximum) · `db size 412 MB` · `IN / OUT` ratio ·
`BURN / DAY` · `daemon running` · `queue`

The lesson this list now carries is its own history: three of its entries were on screen
while it claimed they were impossible. A missing-data list is only useful if it is checked
against the code, because it tells the next reader not to look.
