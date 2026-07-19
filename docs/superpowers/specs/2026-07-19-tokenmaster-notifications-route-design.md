# TokenMaster P3-D.6 Notifications Route Design

Status: approved for autonomous execution by the operator on 2026-07-19. This design
continues the product-first supporting-view rail after P3-D.5 Recent activity.

## 1. Goal

Replace the production Notifications placeholder with a truthful, responsive,
bounded expiry-safety center over the already-published `BenefitOverviewSnapshot`.
It must expose every current benefit lot and its effective reminder coverage without
adding a query, worker, timer, queue, cache, connection, archive owner, or route-time
authority.

P3-D.6 closes the read-only route. It does not yet claim that the unfinished GUI has
presented and acknowledged a leased runtime notification. That later bridge requires
an explicit event-loop presentation receipt plus failure release/retry semantics; an
acknowledgement at product publication or route selection would be false delivery.

## 2. Product intent retained and improved

The pinned WhereMyTokens reference makes expiring reset benefits visible. TokenMaster
already improves the backend foundation with separate expiry lots, configurable
profiles, durable due/outbox/ack storage, restart replay, deduplication, one-nearest-
deadline scheduling, and capacity-one backpressure. The Dashboard currently reduces
that truth to one summary per scope.

The Notifications route exposes the useful missing detail:

- up to 32 identity-free scope profiles with coverage, source, lead times, nearest
  expiry/due, freshness, quality, and warning truth;
- up to 256 separate current lots with kind, quantity, state, expiry precision,
  source, confidence, detail quality, and localization label;
- exact distinction between banked resets, credits, temporary usage, and unknown
  benefits;
- exact distinction between exact UTC, bounded UTC, provider-local, provider-date,
  and unknown expiry instead of collapsing uncertain dates to an exact instant;
- visible `In-app only` or `Disabled` coverage without claiming OS delivery;
- responsive wide/narrow and accessible presentation from the same bounded models.

## 3. Options considered

### A. Lease and acknowledge notifications when the route opens

Rejected. Route selection is presentation-only. It may happen while the route is
hidden, the window is not paintable, or the event loop is closing; none proves a
successful visible presentation. It would also couple navigation to runtime mutation.

### B. Acknowledge the runtime batch when a product snapshot reaches Desktop

Rejected. Snapshot publication is not UI presentation. A failed or closed window
would permanently suppress an event that the user never saw.

### C. Add a second frontend notification scheduler or query the outbox

Rejected. The runtime already owns one scheduler, worker, deadline, and leased batch.
A second owner creates duplicate delivery, lifecycle, memory-growth, and correctness
risk. The UI receives neither SQL nor delivery identity.

### D. Build the read-only expiry-safety center over `BenefitOverviewSnapshot`

Selected. It closes the placeholder with complete current inventory/profile truth,
adds no backend work, and establishes the privacy/accessibility boundary. A separate
P3 presentation bridge can later consume the existing leased batch, publish one
bounded transient GUI notification surface, acknowledge only after a successful
event-loop presentation receipt, and release on presentation failure.

## 4. Architecture and ownership

The existing controller already captures one `BenefitOverviewRequest::all_current()`
inside the same capacity-one query worker. `ProductReducer` owns one compatible
`ProductSection<BenefitOverviewEnvelope<BenefitOverviewSnapshot>>`, including retained
last-good failure semantics. Product route readiness already joins benefit publication,
query section state, and reminder runtime health.

`DesktopNotificationsProjection::from_snapshot` becomes the sole new frontend mapping
site. It selectively copies the existing overview into two immutable arrays:

- `DesktopReminderScopeRow`, capped at 32;
- `DesktopBenefitLotRow`, capped at 256 across all scopes.

One accepted product generation replaces each Slint model once. Route selection only
changes visibility. No scope identity, lot identity, delivery identity, or opaque
runtime receipt crosses the Desktop boundary; the safe zero-based scope ordinal is
presentation-only and exists only to associate a lot with its visible profile row.

## 5. Public projection

`DesktopReminderScopeRow` contains only:

- presentation ordinal;
- completeness, freshness, quality, and bounded stable warnings;
- reminder profile revision, inherited/override source, truthful coverage;
- up to eight canonical lead seconds;
- optional nearest conservative expiry and nearest due UTC milliseconds;
- visible current-lot count.

`DesktopBenefitLotRow` contains only:

- presentation scope ordinal;
- provider-neutral kind, quantity, state, and bounded localization key;
- optional granted UTC milliseconds;
- typed expiry value preserving exact/bounded/provider-local/provider-date/unknown;
- provider-neutral evidence source, confidence, and detail kind.

`DesktopNotificationsProjection` contains section state/reasons, the two capped arrays,
and explicit scope/lot truncation facts. It never contains provider/account/workspace/
scope/lot/delivery identity, target window identity, cursor, archive identity, path,
prompt, response, reasoning content, command, credential, SQL, or activation authority.

## 6. State, ordering, and completeness

- Waiting without payload: waiting, no evidence, no rows.
- Unavailable without payload: unavailable with stable reason, no fabricated empty
  inventory.
- Compatible retained payload after failure: degraded, retained rows/evidence plus the
  current failure reason.
- Ready authoritative empty overview: ready, zero scopes/lots, explicit no-current-
  benefits state.
- Aging/stale/partial/conflicting/unknown evidence or warnings: degraded while usable
  rows remain visible.
- Plus-one or frontend cap overflow: retain only the fixed prefix, mark explicit
  truncation, and never call it a complete inventory.

The query-provided scope and FEFO lot order is retained. There is no frontend sort,
filter, page history, selection, or cached previous overview.

## 7. Information design

The route title is `Notifications` with the context `Expiry reminders`. The header
shows section state/reasons, loaded scope/lot counts, and whether the current inventory
is complete. A reminder-profile strip shows each scope's coverage, policy source,
lead schedule, next due/expiry, and evidence.

The main list shows each lot separately. Wide rows expose scope, benefit, quantity,
state, expiry, and evidence. Narrow rows retain the same meaning in two lines. Full
accessible labels include every field and never rely on color alone.

Exact UTC expiry is labelled `Expires`; bounded UTC is labelled as a range; local/date
and unknown precision remain visibly non-exact. Available zero is a legitimate zero.
No row exposes or enables activation.

## 8. Delivery and settings boundary

P3-D.6 is a read-only inventory/profile route. It does not:

- call `take_notifications`, `acknowledge_notifications`, or `release_notifications`;
- create a delivery history from current lots;
- claim an event was shown, read, dismissed, snoozed, or OS-scheduled;
- mutate the portable reminder policy or a scope override;
- activate a reset or imply activation availability.

The durable settings schema already holds one canonical in-app reminder policy, while
the benefit store owns effective per-scope profiles. A later settings/application slice
must define one transactional synchronization boundary before editing is enabled; the
route displays the effective query profile only and does not pretend those stores are
already synchronized.

## 9. Performance and memory budget

- Zero new query calls, workers, threads, timers, queues, watchers, caches, connections,
  polling loops, animations, dependencies, crates, or schema.
- At most 32 scope rows, 256 lot rows, and eight lead seconds per scope.
- One replace-only model per row type; zero model replacements on route-only switching.
- One `MainWindow`; responsive layout changes presentation only.
- No retained prior overview, delivery batch, notification history, selection, or raw
  provider payload.
- 10,000 projection replacements must release the previous scope/lot arrays.

## 10. Verification and acceptance

P3-D.6 is complete only when tests and audits prove:

- exactly one existing all-current benefit overview query and no new owner;
- exact waiting/unavailable/retained/empty/ready/evidence/truncation behavior;
- 32-scope, 256-lot, and eight-lead bounds;
- exact FEFO order, separate lots, kinds/states/quantities, reminder profile source and
  coverage, and every expiry precision across product/Desktop/Slint;
- no private identity, delivery receipt, authority, or direct runtime/store/query API
  crosses Slint;
- wide/narrow/accessibility completeness from the same two models;
- route switching performs no query, mutation, acknowledgement, scheduling, model
  rebuild, or window recreation;
- deterministic source/mutation audits, clean-root, format, strict Clippy, locked
  workspace tests, release composition, and independent read-only review pass;
- specification, contracts, traceability, decisions, parity, current state, handoff,
  roadmap, changelog, and project history remain synchronized without claiming visible
  delivery, editable settings, activation, M0, packaging, signing, or release.

## 11. Deferred notification slices

The next notification-specific work is deliberately split:

1. a capacity-one GUI presentation bridge over the existing leased batch, with one
   transient model, event-loop presentation receipt, post-presentation acknowledgement,
   release on failed presentation, restart replay, shutdown behavior, and bounded retry;
2. typed reminder settings synchronization and UI for default/custom lead subsets;
3. snooze and quiet-hours contracts;
4. OS/tray scheduling after Windows capability and receipt gates;
5. usage-percentage alerts with per-window thresholds/cooldown;
6. activation only behind the separately required official idempotent capability and
   durable compare-and-swap policy.

## 12. Non-goals

P3-D.6 does not implement toast delivery, notification history, OS notifications,
snooze, quiet hours, usage thresholds, policy editing, per-scope editing, activation,
tray, skins, localization, CLI/MCP, plugins, packaging, signing, M0 acceptance, or
release evidence.
