# TokenMaster P3 Desktop UI Design

Status: approved for execution from the previously approved product architecture and
the operator's explicit autonomous `go` instruction.
Date: 2026-07-17.

## 1. Decision

P3 builds a new production `tokenmaster-desktop` leaf package. The existing
`tokenmaster-m0` probe remains an evidence artifact and is not renamed, promoted,
depended upon, or used as a source of production data. WhereMyTokens and ccusage
remain external behavior and information-design references only.

The production path is:

```text
bounded query worker
        |
        v
ProductReducer -> Arc<ProductSnapshot> -> DesktopProjection -> Slint models
                                              ^                 |
                                              |                 v
                                      bounded UI intents <- Slint callbacks
```

P3-A implements the first vertical contour: one real product snapshot maps to a
bounded route/navigation projection and a compiled software-rendered Slint shell for
all 11 product routes. It contains no fixture or mock usage values. Later P3 slices
fill those routes through the same projection boundary without changing the shell's
ownership model.

## 2. Options considered

### A. Promote `tokenmaster-m0` into the product

This would reuse the largest amount of visible code. It is rejected because the M0
package contains seeded demonstration models, stress-only entry points, diagnostic
renderer fallback, and receipt-bound behavior. Promotion would blur product truth,
invalidate the meaning of earlier M0 evidence, and couple future UI work to a probe.

### B. Make production desktop depend on `tokenmaster-m0`

This preserves the probe binary but makes a diagnostic executable's library the UI
foundation. It is rejected because production would inherit probe data helpers,
FemtoVG selection, stress code, and test-only lifecycle policy. The dependency
direction would also make later removal or freezing of the probe unsafe.

### C. Add a separate production desktop leaf over product contracts

Selected. The new package depends on `tokenmaster-product` and Slint only for P3-A.
It reuses proven architectural constraints, not probe modules. The M0 package can
continue to prove the historical native lifecycle while production evolves without
receipt ambiguity.

## 3. Phase boundary

P3 is decomposed because a complete desktop, every analytical route, notifications,
settings persistence, and compact lifecycle are not one safely reviewable change.

1. **P3-A — production shell:** bounded snapshot projection, 11-route navigation,
   truthful state/reasons, software renderer, no mock data.
2. **P3-B — controller:** one bounded query worker, intent coalescing, product reducer
   publication, stale-generation rejection, startup and shutdown ownership.
3. **P3-C — dashboard:** header plus Plan Usage, Code Output, Usage and Cost Trend,
   Sessions, Activity, and Model Usage sections from immutable query payloads.
4. **P3-D — exploration routes:** History, Sessions/detail, Models, Projects, Activity,
   and Data Health with keyset paging and bounded charts/tables.
5. **P3-E — product routes:** Notifications lease/display/ack, typed settings,
   Help/About, command palette, and Compact Widget lifecycle.

P4 remains the presentation-completion phase: independent skin, layout, density,
color-scheme, and locale axes; external skin validation; full en/ru/pseudo coverage;
accessibility; DPI; and visible-paint/resource gates. P3 components must nevertheless
use stable semantic roles and translation keys so P4 does not require a rewrite.

## 4. P3-A contracts

### 4.1 Production package

`crates/desktop` is a leaf package named `tokenmaster-desktop`. Its binary name is
`TokenMaster`. It may consume public product snapshots but may not open SQLite, parse
provider input, own runtime/store handles, depend on `tokenmaster-m0`, or expose a
frontend back-reference from product/query/runtime crates.

The production Slint dependency enables the software renderer and excludes FemtoVG.
`tokenmaster-m0` opts into FemtoVG explicitly for its diagnostic fallback. A package
build of `tokenmaster-desktop` therefore does not link the diagnostic renderer.

### 4.2 Desktop projection

`DesktopProjection` is an owned, immutable, bounded view of exactly one
`ProductSnapshot`. It contains:

- the product generation;
- the selected product route;
- exactly 11 `DesktopRoute` values in `ProductRoute::ALL` order;
- stable English ASCII route keys, label keys, state values, and bounded reason codes;
- no query payload history, database handle, OS handle, path, provider raw text, or UI
  object.

Each route contains at most the 11 fixed product reason codes already represented by
`ProductRouteReasons`. P3-A copies them into a fixed-size array plus count; it does not
allocate an unbounded diagnostic list. Product reason ordering is the canonical enum
ordering.

Selection is UI state, not product authority. A route request must match one of the
11 stable route keys. An unknown key is rejected and leaves the prior selection
unchanged. A selected unavailable route remains selectable so its truthful reason and
recovery guidance can be displayed; navigation never fabricates readiness.

### 4.3 Slint boundary

Slint receives one bounded model replacement per accepted product generation. It
does not receive `Arc<ProductSnapshot>`, a reducer, query service, connection, runtime
owner, path, or unbounded collection. Generated Slint code is confined to the desktop
package.

Callbacks emit typed presentation intents. They never perform SQLite, provider,
filesystem, process, network, or blocking work. P3-A implements only route selection;
P3-B adds a single bounded worker and coalesced refresh/range/page intents.

The P3-A window includes:

- a persistent TokenMaster header;
- navigation for Dashboard, History, Sessions, Models, Projects, Activity, Data
  Health, Notifications, Settings, Help/About, and Compact Widget;
- route state text and reason codes from the current projection;
- an explicit initial waiting/unavailable presentation derived from
  `ProductReducer::new().snapshot()`;
- no hard-coded quota values, session rows, charts, costs, or reset claims.

### 4.4 Update ordering and memory

The desktop adapter retains one last-applied `ProductGeneration` and one current Slint
route model. A candidate older than or equal to the applied generation is ignored.
The next model is built completely before it replaces the previous model. Failed
mapping leaves the previous model visible.

P3-A adds no background worker, timer, watcher, cache, history, or per-route window.
Its steady retained state is constant: one 11-row route model, one selected route, and
generated component state. The application creates one native window and defaults to
the Slint software renderer.

P3-B may add only one worker, bounded/coalesced channels, and one current product
snapshot. Long tables remain keyset paged at 256 rows and charts remain bounded at 240
points. No later route may introduce an independent scanner or per-card timer.

## 5. Route truth

The shell uses the existing product route derivation unchanged:

- Settings and Help/About are ready without an archive.
- Initial data-dependent routes are unavailable with
  `data_status_unavailable`.
- Dashboard may be degraded section by section.
- History, Sessions, Models, and Projects become unavailable while aggregate data is
  rebuilding, rebuild-required, or failed.
- Activity and Data Health remain reachable during aggregate rebuild when their own
  prerequisites permit it.
- Compact Widget depends on quota truth and never shows an invented empty/full bar.

The UI maps stable codes to localized explanatory text later. Codes themselves remain
English ASCII and visible in Data Health/diagnostic contexts.

## 6. Security and failure behavior

- No prompt, response, reasoning, command, command output, source content, raw tail,
  credential, absolute path, or wrapped provider/OS/SQLite error enters the UI model.
- Display strings are bounded stable keys in P3-A.
- Unknown route requests are rejected without state change.
- Stale generations cannot overwrite a newer visible state.
- A Slint model conversion failure retains the last valid projection.
- Production desktop has no HTTP, shell, browser, direct SQL, plugin, or arbitrary
  filesystem authority.
- Settings/help readiness never implies archive, provider, or activation authority.

## 7. Verification and gates

P3-A is complete only when all of the following pass:

- focused projection tests prove exact 11-route order, stable keys, state/reason
  mapping, initial truth, bounded reason count, selection rejection, and stale update
  rejection;
- compiled Slint tests prove the model is applied, route callbacks switch selection
  without window recreation, and no seeded usage model exists;
- `cargo tree -p tokenmaster-desktop` contains no `tokenmaster-m0`, store, provider,
  engine, runtime implementation, SQLite, network, browser, shell, or FemtoVG feature;
- a deterministic source audit proves one production package, no mock/seed helper,
  11 fixed routes, software-renderer default, and no forbidden frontend authority;
- clean-root, format, warnings-as-errors locked Clippy, and full locked workspace tests
  pass.

Visible-paint timings, DPI, screen reader, 10,000 route/skin/layout switch retention,
and uninterrupted product soak remain P4/P6 evidence and must not be claimed from
P3-A unit tests.

## 8. Closure review

The design was rechecked against the normative specification, data/API/security
contracts, feature-parity ledger, current state, handoff, and roadmap. It preserves
the approved stack, keeps the M0 evidence identity intact, introduces no upstream or
legacy source dependency, gives every future view one immutable product truth, and
keeps the GUI's retained state constant. The only normative drift found during this
review is two stale schema-v12 reader references; P3-A corrects them to the already
implemented schema v13 without changing runtime behavior.

No blocking product ambiguity remains for P3-A.
