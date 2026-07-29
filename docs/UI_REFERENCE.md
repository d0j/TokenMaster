# TokenMaster UI reference

The owner produced a Dashboard mockup and named it the standard the product is built to.
This file transcribes it, because an image does not survive a repository and a description
does. Where the built product differs from what is written here, the product is wrong.

`AGENTS.md` already permits studying WhereMyTokens and reimplementing what is learned. This
mockup is that study made concrete: it is the target, and every visual element in it is in
scope.

## The system

**Two type families.** Proportional for navigation, card titles and prose. **Monospaced for
every number and every label.** This is the single decision the whole look rests on: it is
what makes a usage tool read as an instrument rather than a page.

**Labels are uppercase with tracking**, in the secondary colour, at caption size. `COST`,
`TOKENS`, `EVENTS`, `CACHE HIT`, `SAVED`, `RESETS LEFT`, `COMMITS`, `+ADDED`, `-REMOVED`,
`NET`, `VIEWS`, `PRO · WEEKLY WINDOW`.

**Numbers dominate.** A hero figure runs roughly three times the height of its label.

**Colour is semantic, never decorative.** Green for gain and for healthy state, red for
loss, amber for degraded and for warnings, blue for the cost series, teal for the token
series. The wordmark is amber.

**Every number carries evidence under it.** This is the density that distinguishes the
mockup from what is built: not more numbers, but each number explained by smaller facts
directly beneath.

## Screen by screen

### Header

- `TokenMaster` wordmark, amber, bold, proportional
- vertical rule, then `Local usage intelligence · Dashboard` in secondary
- right side, in order: `● synced 14:33:54` — green dot plus monospaced clock; `Refresh`;
  `Go to route`; a `Degraded` pill in amber

### Sidebar

- `VIEWS` — uppercase, tracked, caption size, secondary colour, above the list
- eleven routes; the active one carries a left accent bar and a lighter background
- per-route status glyph at the right edge: green `●` healthy, amber `▲` attention
- a bordered footer panel pinned to the bottom: `sources 98/104` with the count in amber,
  and `db size 412 MB` monospaced

### Today — three bordered cells in one strip

| cell | contents |
|---|---|
| 1 | `Today` title, `Fresh · Authoritative` monospaced on the right; `COST` label; `$46.98` hero; `-12% vs avg` in green, inline and smaller |
| 2 | `TOKENS` label; `111.9M` hero; beneath it `in 3.1M · out 337.6K` and `cache 108.5M` |
| 3 | `EVENTS` label; `902` hero; beneath it `194 sessions` and `peak 14:31 · 68/min` |

### Plan Usage

- title, `Ready` pill in green
- `PRO · WEEKLY WINDOW` uppercase tracked caption
- `34%` at display size in green
- right-aligned pair: `elapsed 41%` and `resets Sat · 152h 52m`
- a thin progress bar, green fill, with a separate vertical tick marking elapsed time
  against consumed quota — the two are different quantities and the bar shows both
- three bordered sub-cards: `CACHE HIT 97%`, `SAVED $293`, `RESETS LEFT 1`

### Code Output

- title, a `day` / `week` segmented toggle, `Ready` pill
- four metrics: `COMMITS 3`, `+ADDED 3056` green, `-REMOVED 309` red, `NET +2747`
- `$ / 100 lines $1.54` on the left, `all-time avg $5.77` on the right
- a sparkline in green with a filled dot at the current end
- axis labels `07-22`, `07-25`, `today`

### Usage and Cost Trend

- title, then `30d · $4552 · 1.08B tokens` as a monospaced subtitle on the same line
- legend on the right: `● tokens`, `● cost`, then the `Ready` pill
- y-axis extremes only: `1,080,434,426` top left, `$202.20` top right in blue
- two series: teal for tokens, blue for cost, each with a soft area fill beneath
- faint horizontal gridlines
- x-axis `06-29`, `07-07`, `07-14`, `07-21`, `07-28`

### Bottom bar

Four entries: `Dashboard` with a green dot, `Alerts` with `!`, `Settings` with a gear,
`Go to route` with `≡`.

## Built, and not yet built

Done: the type scale and the monospaced numeric voice; uppercase tracked labels;
semantic colour on `+Added` and `-Removed`; per-column accessible labels on the trend.

Not yet, and purely presentational — no new data needed:

- `VIEWS` heading over the route list
- the two-dot legend on the trend (**built**), and three bordered Today cells (**built**)
- the trend subtitle `30d · $4552 · 1.08B tokens` was listed here by mistake: it needs the
  period's summed cost and tokens, which the projection does not carry, so it belongs in the
  list below
- the `day` / `week` toggle in Code Output
- area fill under the trend series

Not yet, and blocked on data the projection does not currently carry:

`-12% vs avg` · `in / out / cache` split · `194 sessions` · `peak 14:31 · 68/min` ·
`elapsed 41%` · `resets Sat · 152h 52m` · `$ / 100 lines` · `all-time avg` ·
`sources 98/104` · `db size 412 MB` · `synced 14:33:54` · `CACHE HIT` · `SAVED` ·
`RESETS LEFT`

That second list is the more valuable half. Each entry is a fact the product either
already computes and discards, or could compute cheaply — and each one is why the mockup
reads as an instrument and the current build reads as a placeholder.
