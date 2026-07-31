# TokenMaster UI reference

**The specification is `docs/design/README.md`, delivered by the designer, and it is
authoritative.** It carries the exact hex values, type scale, paddings, card heights,
radii, interaction rules and mock data, measured from the prototype rather than described
from memory. This file deliberately does **not** re-transcribe it: a second copy of a
specification drifts from the first, and the plan already records why a stale rail is
worse than no rail.

What lives here instead is the part the specification cannot contain -- the distance
between it and the built product, measured against the code.

```
docs/design/
├─ README.md                          ← the specification
├─ TokenMaster Desktop v2.dc.html     ← the target, 1400x900
├─ support.js                         ← prototype runtime; the handoff says do not port it
├─ screens/                           ← what the target looks like, 2x captures
├─ source-screenshots/                ← the reference app the design re-composes
└─ reference-v1/                      ← superseded explorations, and a 462px compact mode
```

**Every number in the handoff is mock data.** The layout is the specification; the values
come from TokenMaster's own archive. A figure copied from that document into the product is
a fabrication, and unavailable, partial and legitimate zero are exactly what a copied
placeholder destroys.

**This supersedes the earlier Dashboard mockup**, whose four-cell top strip, Dashboard
Sessions table and Cost by Model card this file used to transcribe. Where the two disagree,
the handoff wins.

## What the drawing is for

The owner's instruction with it: gather everything from WhereMyTokens and carry it across
into our own variant, improving whatever adaptation allows. So the drawing is **the visual
acceptance for `docs/FEATURE_PARITY.md`**, not a separate ambition -- and that ledger
already tracks every card on it as a WMT lineage row, almost all of them `partial`:

| Card in the handoff | Ledger row | What the row still says remains |
|---|---|---|
| Hero band | `WMT / header metrics` | all-time and cache-savings labels |
| Plan Usage | `WMT / quota board` | history and detail |
| Code Output | `WMT / code output` | project detail and ranges |
| Live Sessions | `WMT / sessions` | grouping, stacking, active-state enrichment |
| Trend | `WMT / trend` | breakdown filters and broader parity |
| Activity | `WMT / activity` | broader heatmap parity |
| Model Usage | `WMT / model usage` | user aliases, final interactive acceptance |

Read the two together: the ledger says what is owed, the drawing says what paying it looks
like.

**A mockup is not a subtraction.** Its sidebar shows six items because six were drawn, not
because the other five are unwanted -- the compact window is designed separately in
`reference-v1`, and History and Notifications simply were not part of this screen. Nothing
here asks for a route to be removed.

## The distance from here

Measured against the code, not estimated.

### Structure

- **The sidebar is drawn with six items plus two pinned**: Dashboard, Sessions, Projects,
  Activity, Models, Data Health, then Settings and Help at the bottom. `ProductRoute::ALL`
  is eleven. That is a difference in what was drawn, not a demand: the eleven stay, and the
  drawn six define the styling -- 32px rows, a 3x16px accent bar 6px from the left edge on
  the active one, and a trailing status glyph.
- **The window is 1400x900 and nothing scrolls.** Four content rows: a 76px hero band, a
  272px row of three cards, the trend taking whatever is left, and a 224px row of two.
  Only the trend is elastic. The built Dashboard wraps its whole content in a `ScrollView`
  whose viewport height is `max(self.height, content.min-height)`, which is the
  construction that lets content outgrow the window instead of tightening.
- **Two font families are required**: JetBrains Mono for every number and technical label,
  IBM Plex Sans for navigation and prose, both bundled for an offline build.

### Cards the product does not have

- **Live Sessions.** Branch name, repo commit and line counts, host, agent, model chip,
  context percentage with tokens remaining, and tool-call chips. `DesktopSessionRow` carries
  an ordinal, two timestamps, tokens and cost -- nothing else, and no model.
- **Activity heatmap, 7 days by 24 hours.** The Dashboard's activity is
  `DASHBOARD_ACTIVITY_ROWS = 8` rows of something else. The grid itself now exists: `UsageRhythm`
  answers `cells` alongside its two marginals, 168 buckets row-major over
  `weekday * 24 + hour`, loaded by the store under the same invariant the marginals obey.
  Two earlier revisions of this file were wrong about it in opposite directions -- first
  that the grid already existed because two constants did, then that adding one was store
  work rather than one join expression. What remains is the projection and the drawing:
  no Slint component in the tree renders a two-dimensional field of cells, and the
  six-step intensity ramp needs colours the palette does not carry.
- **The SCALE slider**, continuous over 1 to 90 days, which re-scopes the trend summary,
  its axis and its series. The built trend has a fixed window.
- **The token mix bar** in Plan Usage -- cache, in and out as one segmented bar with a
  legend -- and the **elapsed marker** on the plan progress track, which is a different
  quantity from consumption and is drawn as a separate tick.

### Data the design needs

Already carried to the projection, after this cycle's work: today's cost, the token total
and its input, cached and output parts, the cache-hit rate, the event count, the instant
the archive is complete through, cost per hundred added lines, net changed lines, the
lifetime total, each model's share of the leading model, and how far through its window
each quota row was read.

Computed upstream and still dropped: the per-model input, cached and output split.

Genuinely absent: cost saved by caching, the session count for a period, peak calls per
minute, the archive's size on disk, the vendor
beside a model name -- `ModelKey` is a plain ASCII value with no provider on it -- live
session context and tool-call counts, and the
per-repository commit and line statistics the Live Sessions card shows.

### The token gap, counted

The palette is the first blocker, and it is not a matter of taste. Counted from the
specification rather than estimated: it names colour **199 times** across **51 distinct
values**, against **fifteen roles** in `UiPalette`.

Not all fifty-one want a name. By how often each appears:

| Uses | Values | What follows |
|---|---|---|
| 5 or more | 14 | structural, each needs a role |
| 3 to 4 | 12 | a role |
| 2 | 13 | a role |
| once | 12 | one-off, may stay a literal |

So **39 roles** at a bar of two uses -- **and that number is wrong, because this count measured the wrong thing.** The handoff has a `## Design Tokens` section of its own that names the roles directly: `bg/window`, `bg/rail`, `bg/chrome`, four surfaces and a track, then seven border tones, eight text steps and five semantics, with the heatmap given as a single scale rather than six values. Counting hex occurrences across the document instead of reading that table turned states and mixes into roles: a hover is a lightened rail, a chip is a vendor accent mixed into a surface, and the heatmap is mint mixed toward the background. Roles are nearer **29**, and most of them are derived rather than chosen. Sorted by luminance the shape is 23 dark values
(surfaces and borders), 12 in the middle, and 16 light (text and accents) -- which is where
the fifteen fail, and the failure is structural rather than a shortage:

- **Surfaces are one role, `surface`, plus two modifiers.** The design distinguishes the
  window from the sidebar from the header bar, then a muted card shell from a primary one,
  then an inset item inside a card, a control container, a chip, a footer strip, a hover
  state and an active state. Ten depths where the tokens hold three.
- **Borders are one role.** The design uses a window border, a control border, a card inner
  divider, a primary-card border and a chip border, and the chip border differs per model
  vendor. Five where the tokens hold one.
- **Text is two roles, primary and secondary.** The design runs primary, emphasis,
  tertiary, muted, meta, caption, axis, inactive-segment, sidebar-inactive and window
  control -- ten steps down a single ramp, where the tokens step twice.
- **Two chart series colours have no role at all**: tokens and cost are separate hues in
  the trend legend, and nothing in the tokens expresses "series one" and "series two".

The accent and state roles are the part that survives: brand amber, positive green and
danger red map onto `accent`, `ready` and `degraded` without argument.

None of this is reachable by restyling a view. Until the ramp exists, every layout change
either invents a literal beside the tokens or picks the nearest of fifteen and drifts.

**And widening the ramp is not the mechanical job it first looked like -- though not for the
reason recorded here at first.** The product carries **six** token sets -- three skins, `Refined`, `Graphite`
and `Ember`, each in a light and a dark scheme -- and every role must have a value in all
six. The handoff specifies **one** set of values and does not mention a theme, a scheme or a
skin anywhere in its 202 lines. So going from fifteen roles to thirty-nine is not twenty-four
new colours; it is twenty-four in the set that was drawn and **a hundred and twenty in five
that were not**.

That is a product decision rather than a styling task, and it is the owner's:

- Derive the other five from the drawn one by rule -- a hue rotation for the skins and an
  inversion for light -- and accept that five of six themes are generated rather than
  designed.
- Ask the designer for the missing sets, which is the only route where all six are drawn.
- Reduce what the product offers, so the drawn set is the product's appearance and the skin
  and scheme choices go away or shrink.

**A second reading collapsed that arithmetic, and the fact that collapses it was never
checked here: no current set matches the drawing either.** `Refined dark` opens with
`background: rgb(13, 15, 19)`, which is `#0D0F13`, against the handoff's `#090D13` for the same
window. So the fear of five generated themes beside one drawn one describes an improvement on
the present, where **six of six** are undrawn -- and the light sets already prove derivation by
rule is not what happens: Refined's light accent is `rgb(0, 80, 125)` against a dark
`rgb(45, 212, 191)`, picked for contrast rather than inverted.

What follows is that the missing roles are mostly derivations from each set's **own** seeds --
a depth between `background` and `surface-raised`, a text step mixed toward the background, a
chip as accent mixed into a surface -- which cost nothing per extra theme and keep Graphite and
Ember their own. Genuinely new seeds look like two or three a set: a second series colour for
the trend, and a neutral for ranking bars so mint never means "just a bar". Ten to fifteen
values, not a hundred and twenty.

So the three routes above are not the owner's choice; the mechanism is an implementation
detail once the arithmetic is right. The one thing that is the owner's is written in the plan.

### Known gaps the designer flagged

The handoff itself lists what was not designed: Activity modes other than `7d`, the
`Details` drill-in, the five non-Dashboard views, and a real series behind the trend
slider. Those are questions for the designer, not decisions to make here.
