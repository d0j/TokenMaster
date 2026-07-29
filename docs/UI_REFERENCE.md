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

## The distance from here

Measured against the code, not estimated.

### Structure

- **The sidebar is six items plus two pinned**: Dashboard, Sessions, Projects, Activity,
  Models, Data Health, then Settings and Help at the bottom. `ProductRoute::ALL` is eleven.
  History, Notifications and Compact Widget have no place in the drawing -- the compact
  window is designed separately as a future mode in `reference-v1`. **Whether those three
  routes lose their sidebar entry, or the sidebar grows, is a product decision and not a
  drawing decision.**
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
  `DASHBOARD_ACTIVITY_ROWS = 8` rows of something else. **The grid the design asks for
  already exists one layer down**: `USAGE_RHYTHM_HOURS = 24` and
  `USAGE_RHYTHM_WEEKDAYS = 7` are computed in the store's analytics and reach no Dashboard
  projection.
- **The SCALE slider**, continuous over 1 to 90 days, which re-scopes the trend summary,
  its axis and its series. The built trend has a fixed window.
- **The token mix bar** in Plan Usage -- cache, in and out as one segmented bar with a
  legend -- and the **elapsed marker** on the plan progress track, which is a different
  quantity from consumption and is drawn as a separate tick.

### Data the design needs

Already carried to the projection, after this cycle's work: today's cost, the token total
and its input, cached and output parts, the cache-hit rate, the event count, the instant
the archive is complete through, cost per hundred added lines, net changed lines, and the
lifetime total.

Computed upstream and still dropped: the per-model input, cached and output split, both
derivations of the quota window's elapsed fraction, and the 24x7 rhythm above.

Genuinely absent: cost saved by caching, the session count for a period, peak calls per
minute, the archive's size on disk, live session context and tool-call counts, and the
per-repository commit and line statistics the Live Sessions card shows.

### Known gaps the designer flagged

The handoff itself lists what was not designed: Activity modes other than `7d`, the
`Details` drill-in, the five non-Dashboard views, and a real series behind the trend
slider. Those are questions for the designer, not decisions to make here.
