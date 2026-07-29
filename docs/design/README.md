# Handoff: TokenMaster — Desktop Usage Dashboard

## Overview
Single-window desktop dashboard for a local AI-usage/token accounting tool (project: `C:\code\tokenmaster`, target UI toolkit: **Slint**). One screen shows everything a user needs about today's spend: cost, tokens, cache efficiency, plan-window consumption, code output produced by agents, a cost/token trend, live agent sessions, an activity heatmap and per-model spend. No page scrolling — the entire dashboard fits a 1400×900 window.

The design is a re-composition of the reference app "WhereMyTokens" (two source screenshots included in `source-screenshots/`), rebuilt with a tighter grid, one clear typographic scale and explicit color semantics.

## About the Design Files
The files in this bundle are **design references created in HTML** — prototypes that show intended look, spacing and behavior. They are **not production code to copy**. The task is to **recreate these designs in the target codebase's environment** — here **Slint (`.slint` components + Rust)** — using its own layout primitives (`VerticalLayout`, `HorizontalLayout`, `GridLayout`, `Rectangle`, `Path`), its palette/global singletons, and its data model. If a different environment is chosen later (Tauri + web, Qt, etc.), the same spec applies.

`TokenMaster Desktop v2.dc.html` needs `support.js` (bundled) next to it to render in a browser. It is a rendering harness for the prototype only — nothing in it should be ported.

## Fidelity
**High fidelity (hifi).** Colors, type sizes, paddings, card heights and radii below are the final intended values, measured from the prototype. Recreate pixel-accurately at 1400×900, then make the layout elastic per "Responsive behavior".

## Recommended variant
Implement **`TokenMaster Desktop v2.dc.html`** — this is the most complete and most refined version.
`reference-v1/` holds earlier explorations, kept only for context:
- `TokenMaster Desktop v1.dc.html` — same content, denser 3-column arrangement, stepped range switches, purple heatmap. Superseded.
- `TokenMaster Compact 462px.dc.html` — narrow companion window (462×900) with tabs `overview / models / log`. Useful later for a "compact widget" mode; not part of this handoff's scope.

## Screens / Views

### Screen: Dashboard (only screen)
**Purpose:** at-a-glance answer to "what did I spend today, how much of my plan window is left, what are the agents doing right now".

**Window / shell**
- Fixed size 1400×900; background `#090D13`; 1px border `#1A2331`; radius 10px; content clipped (`overflow: hidden`).
- Vertical stack: **title/header bar 46px** → **body (fills)** → **status bar 26px**.
- Body is horizontal: **sidebar 150px** (1px right border `#1A2331`) + **content area** with `padding: 10px` and `gap: 10px` between rows.

**Content rows (top → bottom), heights are fixed except TREND which takes the remainder**
| Row | Height | Contents |
|---|---|---|
| Hero band | 76px | Today cost · Tokens · Cache efficiency |
| Row 2 | 272px | 3 cards, `grid-template-columns: 1fr 1fr 1.16fr`, gap 10px: Plan Usage / Code Output / Live Sessions |
| TREND | fills (~208px) | full-width chart card |
| Row 4 | 224px | 2 cards, `1.5fr 1fr`, gap 10px: Activity / Model Usage |

---

#### 1. Header bar (46px)
- Background `#0C1219`, bottom border 1px `#1A2331`, horizontal padding 14px, item gap 12px, vertically centered.
- **Logo lockup** (left): 26×26 SVG tile — rounded rect `rx 7`, fill `#0F1620`, 1.6px stroke = linear gradient `#F0B429 → #3FD8A4` (135°); inside a "T" (2px round caps, `#F0B429`) and an "M" (2px round joins, `#3FD8A4`). Next to it, two lines (line-height 1.05): `TokenMaster` — JetBrains Mono 700, 14px, `#F4F8FC` with "Master" in `#F0B429`; below `USAGE INTELLIGENCE` — JetBrains Mono 10px, letter-spacing 1.4px, `#4D5B6B`.
- 1px × 20px divider `#1E2937`.
- **Scope segmented control** `today | all`: container background `#0F1721`, 1px border `#1E2937`, radius 6px, padding 2px; each segment padding 2px 9px, radius 4px, JetBrains Mono 11px. Active: background `#16232E`, color `#F4F8FC`, weight 700. Inactive: transparent, `#67788A`.
- **Source pills** `● Claude local`, `● Codex`: height 26px, padding 0 11px, background `#0F1721`, border 1px `#1E2937`, radius 6px, Mono 11px `#9DAFC0`, dot `#3FD8A4`.
- Right cluster: `synced 10s ago` (Mono 11px `#6D7D8F`), then window controls `▭ ▤ — ✕` (Mono 13px, `#4A5867`; hover `#C3CFDB`, close hover `#E05252`), gap 14px.

#### 2. Sidebar (150px)
- Background `#0B1017`, padding 9px 7px, item gap 2px.
- Items (6): `Dashboard, Sessions, Projects, Activity, Models, Data Health`; height 32px, radius 6px, IBM Plex Sans 13px.
  - Active: background `#16202C`, color `#F4F8FC`, weight 600, plus a 3×16px radius-2 accent bar `#F0B429` at 6px from the left edge.
  - Inactive: transparent, `#93A3B5`; hover `#131C26`.
  - Trailing status glyph, Mono 10px: `▲` `#F0B429` when the view needs attention (Dashboard, Projects, Data Health in the mock), otherwise `●` `#3D4B5A`.
- Bottom-pinned: `Settings`, `Help` — height 30px, padding 0 10px, 13px `#7D8EA0`, hover background `#131C26`.

#### 3. Hero band (76px)
- Background `#0F1620`, border 1px `#223043`, radius 8px, padding 0 18px, `space-between`, centered.
- Left: label `TODAY COST` (Mono 11px, tracking 1.5px, `#8697A9`); value `$1115` Mono 700 **38px**, line-height 1, letter-spacing −1.6px, `#F4F8FC`; beside it `3.9K calls / 97 sessions` (Mono 12px `#6D7D8F`, numbers `#F4F8FC` 700). Whole group `white-space: nowrap`.
- Right group (gap 26px, right-aligned, nowrap):
  - `TOKENS` label; `282.3M` Mono 700 20px `#F4F8FC`; sub `across 3.9K calls` Mono 11px `#6D7D8F`.
  - 1×44px divider `#1E2937`.
  - `CACHE EFFICIENCY` label; `99%` Mono 700 **30px** `#3FD8A4`; beside it `+$8188 saved` Mono 12px `#3FD8A4`.

#### 4. Plan Usage card (primary emphasis)
- Background `#101927`, border 1px `#24344A`, radius 8px.
- Header 32px, padding 0 13px, bottom border `#1C2838`: title `PLAN USAGE` Mono 700 11px tracking 1.4px `#C3CFDB`; right meta `Codex 1W · Log` Mono 10px `#8697A9`.
- Body padding 12px 13px:
  - `6%` Mono 700 **30px** `#F4F8FC`; right column (Mono 11px `#6D7D8F`, line-height 1.7): `elapsed 6%` (value `#C3CFDB`), `↻158h 26m · resets Wed`.
  - Progress track: height 7px, radius 4px, background `#18222F`; fill 6% wide `#3FD8A4`; elapsed marker = 1px vertical line `#6B7D91` at 6%, extending 3px above/below.
  - 3-column metric row (gap 8px, margin-top 12px): `IN 6.6M`, `OUT 616.1K`, `CACHE 98% 275.1M` — labels Mono 10px tracking 1px `#6D7D8F`, values Mono 14px (`#C3CFDB`, `#C3CFDB`, `#3FD8A4`).
  - **TOKEN MIX** (margin-top 14px): label row `TOKEN MIX` / `282.3M total` (Mono 10px `#6D7D8F`); segmented bar height 7px, gap 2px — `cache 97.5%` `#3FD8A4` (radius 4px left), `in 2.3%` `#4A9EFF`, `out 0.2%` `#C3CFDB` (radius 4px right, min visible width ≈1.2%); legend row Mono 10px with matching dots.
- Footer strip: padding 9px 13px, top border `#1C2838`, background `#0D1520`, Mono 11px `#8697A9`: `window spend $112` (value `#E05252` 700) and `resets 1 · next 14d 1h` (value `#F4F8FC` 700).

#### 5. Code Output card (secondary emphasis)
- Background `#0D131C`, border 1px `#1A2331`, radius 8px; header title color `#8697A9` (muted vs. primary cards), inner borders `#172029`.
- Header 32px: `CODE OUTPUT` + segmented `today | all` (same segment spec, container background `#0A1017`, border `#1A2331`).
- 3 metrics (padding 11px 13px 0): labels Mono 10px `#6D7D8F`; values Mono 700 **22px** `#F4F8FC` — `73`, `+3692`, `$22.74`.
- **Output growth** mini chart: caption row `OUTPUT GROWTH · 7d` / `+528.5K net` (`#3FD8A4`), Mono 10px. Plot fills remaining height: baseline 1px `#172029`; polyline 2px `#3FD8A4`, round joins, `vector-effect: non-scaling-stroke`, `preserveAspectRatio: none`, viewBox `0 0 351 100`, points `14,78 64,77 114,75 163,74 213,72 270,30 330,20`; end marker = 8px circle `#3FD8A4` positioned at 94% / 20% of the plot box (a separate element so it stays round when the plot stretches). Axis labels `7/23 7/25 7/27 Today` Mono 10px `#4D5B6B` / today `#9DAFC0`.
- Footer strip: padding 8px 13px, background `#0A1017`, top border `#172029`, Mono 10px `#6D7D8F`: `835 commits all-time` · `+4904 / −1212` · `avg $4.92`.

#### 6. Live Sessions card (primary emphasis)
- Same shell as Plan Usage (`#101927` / `#24344A`). Header: `LIVE SESSIONS` + `2 active · just now` (Mono 10px `#3FD8A4`).
- List padding 8px 10px, gap 7px, vertical scroll only as overflow safety (must not be needed for 2 items).
- **Session item** (background `#0C1319`, border 1px `#1C2838`, radius 7px, padding 8px 10px, ≈100px tall):
  1. Row: branch name Mono 12px `#E6EDF4`, `flex: 1`, ellipsised; model chip; `Details` button (Mono 10px `#8697A9`, border 1px `#1E2937`, radius 4px, padding 1px 7px, hover text `#F4F8FC`). Chips are `nowrap; flex-shrink: 0`.
     - Model chip Codex: text `#C3CFDB`, border `#24344A`, background `#141D2A`. Model chip Claude/Opus: text `#F0B429`, border `#4A3A10`, background `#1C1607`. Both Mono 700 10px, radius 4px, padding 1px 7px.
  2. Meta row (Mono 10px `#6D7D8F`, nowrap, left part ellipsised): repo/host stats on the left, `context NN% · <left>` on the right — context label `#6D7D8F` normally, `#E05252` 700 when at limit.
  3. Context bar: track height 5px radius 3px `#18222F`; fill = context % — `#3FD8A4` under limit, `#E05252` at 100%.
  4. Tool chips row (gap 5px, wrap): Mono 10px `#8697A9`, background `#111A24`, border 1px `#1A2331`, radius 4px, padding 1px 7px — e.g. `wait×1975`, `shell×968`, `pre_action_check×…`, `+18`.

#### 7. Trend card (full width, fills leftover height)
- Shell `#0D131C` / `#1A2331`; header 32px: `TREND` + window summary `14d · $6.8K · +387.8K tokens · peak $1249` (Mono 11px `#5D6D7F`, nowrap).
- Right cluster: legend `● tokens` `#3FD0C9`, `● cost` `#4A9EFF` (Mono 11px `#8697A9`); then **SCALE slider** (see Interactions): label `SCALE` Mono 10px tracking 1px `#5D6D7F`; track 168×4px radius 2px `#18222F`; fill `#3FD8A4`; thumb 13px circle `#F4F8FC` with `box-shadow: 0 0 0 2px #0D131C, 0 0 0 3px #3FD8A4`; value readout Mono 700 11px `#F4F8FC`, min-width 38px, right-aligned (`14d`).
- Plot (padding 9px 13px 7px): 4 gridlines 1px `#151D27` at y = 15/60/105/143 of viewBox `0 0 1204 150`; two 2px polylines with `vector-effect: non-scaling-stroke` — tokens `#3FD0C9`, cost `#4A9EFF`; last cost point marked with an 8px `#4A9EFF` circle at 97.8% / 14% of the plot box. `preserveAspectRatio: none` so the plot fills the card.
- Date axis: 8 labels distributed `space-between`, Mono 10px `#4D5B6B`, last one `Today` `#9DAFC0`; separated by a 1px top border `#151D27`, padding-top 5px.

#### 8. Activity card
- Shell `#0D131C` / `#1A2331`. Header: `ACTIVITY` + `Jerusalem · last 7 days` (Mono 10px `#5D6D7F`) + mode segmented control `7d | 5mo | Hourly | Weekly | Rhythm` (segments Mono 10px, otherwise same spec).
- Heatmap: 7 rows (Thu…Wed) × 24 columns, row gap 3px, cell gap 3px, cells `flex: 1` radius 2px; row label column 26px, Mono 10px `#5D6D7F`.
- Intensity scale (6 steps, level 0 = empty): `#101B20`, `#173F38`, `#1F6B58`, `#2A9A78`, `#34C294`, `#5CF0B8`.
- Bottom axis `0h 6h 12h 18h 23h` (Mono 10px `#4D5B6B`, left inset 29px) and `less ▪▪▪▪▪ more` legend using the same 5 non-empty shades, 9×9px, radius 2px.

#### 9. Model Usage card
- Shell `#0D131C` / `#1A2331`. Header: `MODEL USAGE` + `top 4 · all time`.
- 4 rows, vertically `space-around`, each: model name Mono 700 13px `#E6EDF4`; vendor Mono 10px `#5D6D7F`; tokens Mono 11px `#8697A9` (pushed right); cost Mono 700 13px `#F4F8FC`, min-width 66px right-aligned. Below: bar track height 4px radius 2px `#18222F`, fill `#3D5570` (deliberately neutral so mint stays semantic), width = share of the top model.
- Data: `GPT-5.4 / 29981.4M / $11546 / 100%`, `GPT-5.6-SOL / 25421.6M / $9919 / 86%`, `GPT-5.5 / 8830.1M / $4639 / 30%`, `GPT-5.6-TERRA / 1695.3M / $692 / 8%`.

#### 10. Status bar (26px)
Background `#0C1219`, top border `#1A2331`, padding 0 14px, gap 20px, Mono 11px `#5D6D7F`: `● daemon running` (dot `#3FD8A4`), `queue 0`, `db 412 MB`, `indexing 98/104` (`#F0B429`), right-aligned `↻ 10s ago`.

## Interactions & Behavior
- **Scope `today | all`** (header) and **`today | all`** (Code Output) are segmented switches; they re-scope the aggregates below them. In the prototype both write the same `range` state — in production keep the header one global and either remove the Code Output one or give it its own state.
- **SCALE slider (continuous, not stepped)** — the only "zoom" control for the trend:
  - `pointerdown` on the track jumps the thumb to the clicked position and starts a drag; `pointermove` on the window updates continuously; `pointerup` ends it. `touch-action: none`, cursor `ew-resize`.
  - Value maps linearly to **1…90 days**: `days = round(1 + p × 89)`, `p = clamp((clientX − trackLeft) / trackWidth, 0, 1)`; fill width and thumb position = `((days − 1) / 89) × 100%`.
  - Updates: readout (`14d`), header summary (window length, spend, tokens) and the date axis. Axis: ≤2 days → hour offsets `−48h … Today`; otherwise dates ending in `Today`, with a repeated date rendered as `·`.
  - In production the slider must also refetch/redraw the series; the prototype keeps a static polyline.
- **Activity modes** switch the aggregation of the heatmap (`7d` = 7×24 hours as shown; `5mo`, `Hourly`, `Weekly`, `Rhythm` need their own grids — not designed yet).
- **Sidebar items** switch the active view (only Dashboard is designed).
- **Hover states**: sidebar item background `#131C26`; window controls lighten to `#C3CFDB` (close → `#E05252`); `Details` text → `#F4F8FC`; session rows may lighten to `#131C26`.
- **`Details`** opens per-session detail (not designed).
- No animations beyond instant state changes; if any transition is added keep it ≤120ms ease-out. Avoid pulsing/blinking except for a genuine "indexing" indicator.

## State Management
| State | Type | Default | Effect |
|---|---|---|---|
| `activeView` | enum(6 sidebar views) | `Dashboard` | sidebar highlight + routed content |
| `scope` | `today` \| `all` | `today` | hero band, Plan Usage, Code Output aggregates |
| `trendDays` | int 1…90 | `14` | trend window: summary, axis, series |
| `activityMode` | enum(`7d`,`5mo`,`Hourly`,`Weekly`,`Rhythm`) | `7d` | heatmap aggregation |

Data the real app must supply: today's cost/calls/sessions; cache efficiency and saved amount; token totals split in/out/cache; plan window (percent used, elapsed percent, reset time, window spend, resets available + next expiry); code output (commits, added/removed lines, $/100 lines, all-time totals, 7-day net-growth series); trend series (cost + tokens per bucket over `trendDays`, peak); live sessions (branch, repo stats, host, agent, model, context percent + remaining tokens, tool call counts, last activity); activity buckets (24×7 counts, timezone); per-model all-time tokens and cost; daemon/queue/db/indexing status.

## Design Tokens
**Surfaces**
| Token | Hex | Use |
|---|---|---|
| `bg/window` | `#090D13` | window background |
| `bg/rail` | `#0B1017` | sidebar |
| `bg/chrome` | `#0C1219` | header + status bar |
| `surface/primary` | `#101927` | emphasized cards (Plan Usage, Live Sessions) |
| `surface/secondary` | `#0D131C` | normal cards |
| `surface/hero` | `#0F1620` | hero band, logo tile |
| `surface/inset` | `#0C1319` / `#0A1017` / `#0D1520` | session items, footer strips |
| `track` | `#18222F` | progress/bar tracks |

**Borders:** `#1A2331` (shell/secondary), `#24344A` (primary card), `#223043` (hero), `#1C2838` (inside primary), `#172029` / `#151D27` (inside secondary, gridlines), `#1E2937` (controls/dividers).

**Text:** `#F4F8FC` primary values · `#E6EDF4` strong labels · `#C3CFDB` secondary values · `#9DAFC0` de-emphasized · `#8697A9` labels · `#6D7D8F` meta · `#5D6D7F` faint meta · `#4D5B6B` axis/micro.

**Semantics (strict):** mint `#3FD8A4` = savings / healthy / active; teal `#3FD0C9` = tokens series; blue `#4A9EFF` = cost series and "in" tokens; red `#E05252` = limit reached / window spend warning / close button; amber `#F0B429` = attention (indexing, flagged views, Claude/Opus chip, brand). Neutral slate `#3D5570` for ranking bars so mint never means "just a bar".

**Heatmap scale:** `#101B20 → #173F38 → #1F6B58 → #2A9A78 → #34C294 → #5CF0B8`.

**Typography**
- Numeric / technical: **JetBrains Mono** 400/500/700.
- UI labels & sidebar: **IBM Plex Sans** 400/500/600/700.
- Scale: 38 (hero value) → 30 (secondary hero: cache %, plan %) → 22 (card metrics) → 20 (hero token total) → 14/13 (values, item titles, sidebar) → 12 (branch names, hero sub) → 11 (card titles, meta, segments) → 10 (micro labels, axis, chips). **Never below 10px.**
- Uppercase labels use letter-spacing 1.0–1.5px; big numerals use −1.4…−1.6px.

**Spacing:** 2 · 4 · 5 · 7 · 8 · 10 · 12 · 13 · 14 · 18 · 26 (px). Content padding 10, card padding 12–13, card gap 10, card header height 32, control height 26–30, chip padding 1×7.

**Radii:** window 10 · card 8 · inner item 7 · control/chip 4–6 · bars 2–4 · thumb/dots 50%.
**Shadows:** only the window itself (`0 30px 80px rgba(0,0,0,.6)`) and the slider thumb ring. No card shadows.

## Responsive behavior
Designed for exactly 1400×900 and hard-pinned there in the prototype. For the real window: keep the header/sidebar/status bar fixed; let the 4 content rows keep their pixel heights and give extra height to TREND (it is the flexible row); grow cards horizontally with the 3-column and 2-column ratios (`1fr 1fr 1.16fr`, `1.5fr 1fr`). Below ~1200px width, stack Live Sessions under the other two cards. The narrow 462px layout in `reference-v1/TokenMaster Compact 462px.dc.html` is the model for a future compact mode.

## Assets
No bitmaps. The only asset is the inline SVG **TM monogram** in the header (26×26, gradient-stroked rounded square + T/M paths) — reproduce as a vector/`Path` in Slint. Fonts: JetBrains Mono and IBM Plex Sans (Google Fonts; bundle them for an offline desktop build). Screenshots in `screens/` are references, not assets to ship.

## Files
```
design_handoff_tokenmaster_dashboard/
├─ README.md                            ← this spec
├─ TokenMaster Desktop v2.dc.html       ← IMPLEMENT THIS (1400×900 dashboard)
├─ support.js                           ← prototype runtime (do not port)
├─ screens/
│  ├─ dashboard-default-14d.png         ← default state (SCALE = 14d), 2× capture of the 1400×900 window
│  └─ state-scale-90d.png               ← SCALE dragged to max (89d): window summary, date axis and readout all follow the slider
├─ source-screenshots/
│  ├─ original-top.png                  ← reference app, top of window
│  └─ original-scrolled.png             ← reference app, scrolled down
└─ reference-v1/
   ├─ TokenMaster Desktop v1.dc.html    ← earlier dense variant (context only)
   └─ TokenMaster Compact 462px.dc.html ← narrow companion window (future compact mode)
```

## Mock data used in the prototype (copy for fixtures)
- Header: scope `today`, sources `Claude local`, `Codex`, `synced 10s ago`.
- Hero: cost `$1115`, `3.9K calls`, `97 sessions`, tokens `282.3M` (`across 3.9K calls`), cache `99%`, `+$8188 saved`.
- Plan Usage: `Codex 1W · Log`, used `6%`, elapsed `6%`, `↻158h 26m · resets Wed`, IN `6.6M`, OUT `616.1K`, CACHE 98% `275.1M`, token mix `cache 97.5% / in 2.3% / out 0.2%` of `282.3M total`, window spend `$112`, `resets 1 · next 14d 1h`.
- Code Output: commits `73`, net lines `+3692`, `$/100 added $22.74`; growth `+528.5K net` over 7 days (`7/23…Today`); footer `835 commits all-time`, `+4904 / −1212`, `avg $4.92`.
- Trend (14d default): `$6.8K`, `+387.8K tokens`, `peak $1249`; summary scales linearly with the slider in the prototype (1d ⇒ `$485 / +27.7K`, 89d ⇒ `$43.1K / +2.47M`) — replace with real per-window aggregates.
- Live sessions: (1) `cx/r1-contract-spine` · `Codex Desktop · active · just now` · model `GPT-5.6-SOL` · context `25%` · `193.0K left` · tools `wait×1975, shell×968, pre_action_check×…, +18`; (2) `cx/tokenmaster-product-archit…` · `tokenmaster · 64 commits · +2564 / −817 · claude-desktop` · model `Opus` · context `100%` · `at limit` · tools `Bash×457, PowerShell×87, Read×73, +10`.
- Activity: timezone `Jerusalem`, rows `Thu…Wed`, 24 hourly buckets per row, intensity levels 0–5 (the exact matrix used is in the prototype's logic class, `levels`).
- Model usage: see table in §9.
- Status bar: `daemon running`, `queue 0`, `db 412 MB`, `indexing 98/104`, `↻ 10s ago`.

**Known gaps (not designed, ask the designer):** Code Output says "3 repos" but only 2 live sessions exist in the source screenshots (the third repo's header was cut off) — decide whether the sessions list shows all repos; Activity modes other than `7d`; the `Details` drill-in; the 5 non-Dashboard sidebar views; and a real series for the trend slider (the prototype keeps one static polyline while the labels change).
