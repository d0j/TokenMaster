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

### Known gaps the designer flagged

The handoff itself lists what was not designed: Activity modes other than `7d`, the
`Details` drill-in, the five non-Dashboard views, and a real series behind the trend
slider. Those are questions for the designer, not decisions to make here.
