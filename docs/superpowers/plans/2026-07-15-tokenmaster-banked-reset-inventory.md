# TokenMaster Banked Reset Inventory, Expiry, Reminder, and Activation Plan

**Status:** Approved product requirement; execute in P2 after the P1 engine and
immutable query contracts.

**Scope:** provider-neutral benefit inventory with a built-in Codex presentation,
expiry safety, bounded reminders, activation receipts, and a capability-gated path to
future automatic activation.

## 1. Product outcome

TokenMaster must make expiring Codex reset benefits impossible to overlook without
misrepresenting them as ordinary weekly resets or usage credits. A user with two
available resets, one expiring on July 31 and another in the following month, sees two
separate lots, the nearest deadline, notification coverage, and the safe action that
is actually available for each lot.

The product has four distinct provider concepts:

1. a normal provider quota window and its scheduled or early full-reset transition;
2. a banked rate-limit reset granted to the user and available until an expiration;
3. purchased or promotional usage credits;
4. temporary usage benefits whose capacity or duration is provider-defined.

They share a provider/account/workspace scope but never share a type, balance, expiry
rule, or activation assumption. Consuming a banked reset can create a linked
`manual_or_banked_reset` quota transition; it does not turn the reset lot into quota
capacity or credits.

Official OpenAI material currently describes banked Codex rate-limit resets in the
usage summary and says promotional benefit, cap, expiration, and redemption details
can vary. It also distinguishes resets from credits. These are discovery inputs, not
a stable private protocol:

- [Using Codex with your ChatGPT plan](https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan/)
- [Codex referral promotions](https://help.openai.com/en/articles/20001271)
- [Credits for flexible usage](https://help.openai.com/en/articles/12642688-using-credits-for-flexible-usage-in-chatgpt-pluspro)

## 2. Non-goals and authority boundary

The first implementation does not scrape ChatGPT pages, automate browser clicks,
retain cookies, replay session requests, guess private endpoints, or claim that an
activation happened because a button was opened. It does not infer the number of
tokens restored by one reset. Manual inventory is useful for reminders but cannot
authorize automatic activation.

CLI, MCP, Hermes, and other LLM integrations remain read-only for benefit inventory by
default. A general plugin cannot gain activation authority through arbitrary HTTP,
shell, browser, filesystem, SQL, or credential access. An external mutation requires
a narrow host-owned capability, an explicit local policy, and an auditable receipt.

## 3. Domain model

### 3.1 Benefit kind

`ProviderBenefitKind` is closed for the current contract:

- `banked_rate_limit_reset`;
- `usage_credit`;
- `temporary_usage`;
- `unknown`.

Unknown benefits remain visible but cannot be valued, merged with known kinds, or
activated automatically. A provider adapter may add bounded display metadata but not
new execution semantics.

### 3.2 Banked reset lot

A `BankedResetLot` is the smallest independently expiring inventory item. It contains:

- provider, account, workspace, and target quota-window scope;
- provider benefit ID when one exists, otherwise a deterministic TokenMaster identity
  derived from scope, kind, normalized expiry, and first-observed sequence;
- available quantity, normally an integer reset count;
- optional grant/redeemable timestamps;
- typed expiration, state, source, freshness, confidence, and observation revision;
- provider-declared effect descriptor, if any;
- provider-declared activation and lot-selection capabilities.

Two different expirations are always two lots. Equal-expiry quantities may remain an
aggregate only when the provider reports them as one aggregate. TokenMaster never
merges ambiguous observations merely to make the count look stable.

### 3.3 Expiration precision

`BenefitExpiry` is one of:

- `exact_utc(timestamp)`;
- `provider_local(timestamp, time_zone)`;
- `provider_date(date, time_zone_or_unknown)`;
- `bounded(earliest_utc, latest_utc)`;
- `unknown`.

TokenMaster must not silently turn “July 31” into local midnight. Date-only or
timezone-unknown data is rendered with its lower precision. Reminder scheduling uses
the earliest safe boundary that could cause loss, while the UI says that the provider
did not expose an exact instant. Wall-clock UTC determines expiry; monotonic time is
used only while waiting inside one process lifetime.

### 3.4 State and history

Lot state is one of `available`, `activation_pending`, `activated`, `expired`,
`revoked`, or `ambiguous`. State changes append immutable
`BenefitInventoryTransition` records such as awarded, quantity increased/decreased,
activation requested/confirmed, expired, revoked, or corrected. Expired and consumed
lots stay in bounded history and do not disappear from the audit trail.

Current inventory is a projection over immutable change points, not a poll log. A
redundant identical poll only refreshes bounded health/freshness metadata; it does not
append unbounded history.

### 3.5 Activation intent and receipt

Before any external activation TokenMaster persists an `ActivationIntent` containing:

- deterministic idempotency key and checked monotonic local sequence;
- selected scope/lot, expected inventory revision, and policy revision;
- preflight quota sample and benefit observation IDs;
- requested-at time, deadline, and capability version;
- trigger (`user_confirmed`, `policy_threshold`, or `last_chance`).

An `ActivationReceipt` stores only bounded normalized evidence: provider acknowledgement
or stable result code, observed inventory result, linked post-action quota sample and
quota transition, confidence, and final or ambiguous status. Raw responses, headers,
tokens, cookies, prompts, commands, and credentials are forbidden.

Activation is not reported as successful until an official acknowledgement is
available or a coherent independently observed inventory decrement and full quota
reset establish the result. An acknowledgement without the expected effects remains
visible as `acknowledged_unreconciled` rather than being silently treated as success.

## 4. Inventory reconciliation

The connector returns a complete bounded inventory observation for one scope. The
engine compares it transactionally with the current projection:

1. validate scope, count, expiry precision, capability version, and hard bounds;
2. match stable provider IDs first and deterministic fallback identities second;
3. append only meaningful change points;
4. preserve disappeared available lots as ambiguous until an activation, expiry,
   revocation, or later observation explains the change;
5. publish one new immutable inventory snapshot;
6. recompute reminders and activation eligibility after commit.

Provider selection of which equal-purpose lot is consumed is authoritative. FEFO
(first expiry, first out) is a TokenMaster recommendation and local display order, not
a claim of provider behavior. If the API cannot select a lot, a receipt records
`provider_selected`; it does not falsely name the consumed lot.

An automatic weekly quota reset must not reduce banked inventory unless the provider
observation says it did. Conversely, an inventory decrement does not prove a quota
reset without correlated quota evidence.

## 5. Reminder engine

### 5.1 Policy

Recommended defaults are 7 days, 24 hours, and 1 hour before the conservative expiry
boundary. Users may disable, add, remove, or reorder up to eight lead times per scope.
The settings also define:

- in-app and optional OS notification channels;
- quiet hours and whether critical expiry warnings may override them;
- default snooze and allowed 10-minute, 1-hour, 4-hour, tomorrow, or bounded custom
  snooze choices;
- per-lot mute without deleting inventory;
- locale, time zone display, and accessibility behavior.

Each delivery is keyed by `(lot identity, lot revision, threshold, channel)`. Restart,
refresh, and repeated observations cannot duplicate it. A changed expiry creates a
new revision, cancels obsolete due items, and schedules the new thresholds.

### 5.2 Scheduling and resource bounds

There is one durable indexed due queue and one nearest-due runtime timer, never one
thread or timer per benefit. The engine recomputes the next due item after inventory
publication, settings changes, clock/time-zone changes, startup, resume from sleep or
hibernation, and notification delivery. A resume processes overdue reminders in one
bounded batch and collapses stale lower-severity notices into the most urgent current
message.

Quiet hours defer a reminder only when the benefit still survives past the quiet-hour
end. Otherwise the configured critical-expiry rule applies. Snooze can never schedule
beyond the conservative expiry boundary.

Notification coverage is explicit:

- `in_app_only`: delivery requires TokenMaster to be running;
- `tray_process`: the background tray process owns the timer;
- `os_scheduled`: a platform notification task is durably registered;
- `unavailable` or `permission_denied`.

The UI must not promise an offline reminder when only in-app coverage exists. OS
registration is idempotent and old registrations are removed when a lot changes,
expires, is consumed, or notifications are disabled.

### 5.3 Message actions

Every notice offers `View` and `Snooze`. `Activate` appears only when the connector
supports an official activation link or mutation capability. An activation link opens
the exact official usage surface and remains assisted; opening it is not a receipt.

## 6. Activation modes

Each provider/account/window has one explicit versioned mode:

1. `off` — inventory remains visible; no reminders or actions;
2. `remind_only` — recommended default;
3. `confirm_each` — assisted link or official mutation after user confirmation;
4. `automatic_policy` — explicit opt-in and available only for an official narrow
   connector mutation capability.

Manual inventory and low-confidence/unknown-expiry data support only `off` and
`remind_only`. A capability downgrade immediately disables pending automatic policy
eligibility without deleting the saved preference.

### 6.1 Connector capabilities

Provider descriptors declare these separately:

- `banked_reset_inventory_read`;
- `banked_reset_activation_link`;
- `banked_reset_activate_idempotent`;
- `banked_reset_activation_status`;
- `banked_reset_lot_select`.

Capabilities have a schema version and effect descriptor. Inventory read does not
imply mutation. A normal allowlisted HTTPS quota capability does not imply activation.
The host, not provider-supplied UI or an LLM, checks grants and constructs requests.

### 6.2 Automatic policy

An automatic policy can include:

- activation horizon, for example within 24 hours of expiry;
- minimum current used ratio or maximum remaining ratio, preventing needless reset
  consumption while substantial quota remains;
- last-chance lead, for example one hour before expiry;
- explicit `use_rather_than_lose` permission to override the usage threshold at the
  last chance;
- safe-boundary deferral while an observed provider operation is active;
- quiet-hour and notification requirements.

Defaults are no automatic activation and no last-chance override. The settings preview
must explain the next action from current facts, including the lot, expiry precision,
current usage freshness, threshold, and latest safe activation time.

### 6.3 Transactional activation protocol

The engine follows this sequence:

1. refresh inventory and target quota immediately;
2. require fresh, high-confidence data, known effect, adequate expiry precision, and
   the exact official idempotent capability;
3. compare-and-swap the lot and inventory revisions and reject expired, revoked,
   changed, already-pending, or already-consumed state;
4. abort if the weekly quota reset already occurred or current quota is stale;
5. persist the intent before the external action;
6. invoke one host-owned bounded request with the deterministic idempotency key;
7. reconcile official status, inventory, and quota in bounded polling passes;
8. atomically finalize the receipt and link any `manual_or_banked_reset` transition.

Only one activation may be in flight per provider/account/window. A timeout, process
crash, lost response, or unknown status never causes a blind retry. The intent becomes
`ambiguous`; recovery queries official status and refreshes inventory/quota before it
can retry. Exactly-once execution is claimed only if the provider supplies durable
idempotency/status semantics. Otherwise automatic activation remains unavailable.

If the provider operation cannot safely overlap an active Codex task, the connector
must explicitly declare that constraint. TokenMaster may defer to a safe boundary but
never beyond the configured last-chance deadline without warning the user.

## 7. UI and UX

The quota-first board adds an inventory summary adjacent to the relevant weekly card:

- `2 banked resets`;
- `1 expires Jul 31` plus a relative countdown;
- the next lot and its expiration;
- reminder coverage and source freshness;
- a visible warning when no safe reminder channel is active.

The summary opens a bounded FEFO-sorted inventory drawer. Every row shows quantity,
target window, expiry precision/time zone, state, source/confidence, activation mode,
and available actions. Urgency uses text and iconography as well as color. Date,
plural, duration, and notification text use the shared English/Russian/pseudo-locale
pipeline and remain keyboard/screen-reader accessible.

The weekly usage and benefit inventory remain visually distinct. “If used now” is
shown only for a provider-declared effect; TokenMaster never predicts a token balance
from local usage. After a confirmed activation, detail shows the observed quota
`before -> after`, maximum used before reset, recovered headroom when available, and
the linked receipt. Ratios remain exact when absolute units are unavailable.

History has linked but separate filters for automatic quota resets, user/banked reset
activations, inventory awards/expirations/revocations, credits, and corrections.

## 8. Public read interfaces and automation

The immutable UI/query facade exposes:

- bounded current benefit inventory by provider/account/workspace;
- nearest expiry, conservative due time, precision, state, freshness, and coverage;
- bounded transition and activation-receipt pages;
- a pure `benefit_action_evaluation` explaining whether activation is unavailable,
  user-confirmed, policy-eligible, deferred, or blocked.

CLI and MCP may read the same snapshots and evaluate named policies. They do not
accept arbitrary activation requests in 1.0. A later mutation method must be a
separate capability with strict schema, local consent, idempotency key, scope, and
receipt; it cannot be invoked by a provider plugin or LLM merely because it can read
statistics.

Manual entry accepts only bounded normalized facts: provider label, account alias,
quantity, target window, expiration and its precision. It is tagged `manual`, can be
edited through a new revision, and can never upgrade itself to official evidence.

## 9. Storage, retention, and privacy bounds

Per provider/account scope:

- at most 64 current benefit lots;
- at most 8 reminder lead times per lot;
- at most 512 recent inventory change points by default, hard cap 2,048;
- at most 256 activation intents/receipts by default, hard cap 1,024;
- query and maintenance pages no larger than 256 rows;
- one unresolved activation intent is never pruned;
- important award, expiry, activation, revocation, ambiguity, and linked before/after
  evidence survives compaction while redundant observations do not.

Maintenance is bounded, keyset-paged, and transactional. No inventory operation scans
the usage-event archive. SQLite indexes cover current scope/state/expiry, next reminder
due, transition sequence, unresolved intent, and receipt-to-quota-transition linkage.

Stored and external values exclude raw provider payloads, URLs with secrets, headers,
cookies, credentials, prompts, responses, reasoning, commands, source content, and
absolute paths. Logs expose stable IDs, counts, state codes, durations, and redacted
provider error categories only.

## 10. Failure behavior

- Offline/stale inventory remains visible with age and disables automatic action.
- A malformed or over-limit provider response fails only that refresh and leaves the
  last immutable snapshot current.
- Clock rollback, leap, time-zone change, sleep, or hibernation triggers rescheduling
  from durable expiry facts; it does not replay old notifications.
- Notification permission loss becomes visible coverage degradation.
- Expiry during preflight aborts activation and records no external success.
- A revoked or changed lot invalidates reminders and any not-yet-sent intent by CAS.
- Provider acknowledgement without inventory/quota reconciliation is retained as an
  unresolved receipt and escalated to the user.
- Inventory disappearance without evidence remains ambiguous, never silently expired
  or activated.

## 11. Delivery sequence

1. Freeze domain types, SQLite schema, query snapshots, limits, and English/Russian
   copy fixtures.
2. Extend the built-in Codex quota adapter with inventory observation only when a
   credential-safe supported source exists; otherwise ship manual inventory first.
3. Implement transactional projection, change-point history, expiry precision, and
   restart/reconciliation tests.
4. Implement one-timer reminder scheduling, sleep/resume and clock-change recovery,
   in-app notifications, and explicit platform coverage.
5. Build inventory summary/drawer, history linkage, settings, locale, accessibility,
   and compact-mode views.
6. Add activation-link assistance if the official provider surface exposes one.
7. Add confirmed mutation and finally automatic policy only after the official
   idempotent activation/status capabilities have independent security review and
   conformance fixtures.
8. Expose bounded read-only inventory/evaluation through CLI/MCP after the immutable
   query facade is stable.

P2 may complete steps 1–5 without claiming automatic activation. Step 7 is capability
gated, not date gated.

## 12. Required fixture matrix

- two resets with different expirations, including the user’s July/next-month case;
- one aggregate quantity and two separate equal-expiry lots;
- exact UTC, provider-local, date-only, bounded, and unknown expiry;
- 7-day/24-hour/1-hour reminders, quiet hours, critical override, snooze, and mute;
- restart, duplicate poll, sleep/hibernation, clock rollback/advance, and time-zone
  change without notification duplication;
- manual inventory and official inventory replacing or disagreeing with manual data;
- award, count increase/decrease, correction, expiration, and revocation;
- automatic weekly reset without benefit consumption;
- confirmed benefit activation with linked quota before/after transition;
- acknowledgement without quota change and quota change with ambiguous inventory;
- duplicate callback, idempotent retry, timeout, crash, and recovery reconciliation;
- expiry/revocation/capability downgrade during preflight;
- provider-selected lot when FEFO selection is unavailable;
- credits and temporary usage alongside reset inventory without type confusion;
- stale/offline provider and notification permission loss;
- no official mutation capability, ensuring automatic mode cannot be enabled;
- policy threshold, active-operation deferral, and last-chance override interactions;
- multiple accounts/workspaces with strict scope isolation;
- bounded retention/backlog, unresolved-intent preservation, and injected rollback;
- English, Russian, pseudo-locale, keyboard, screen-reader, non-color urgency, high DPI,
  and compact layout.

## 13. Acceptance gates

The feature is not accepted until fixtures prove:

- inventory identity and expiry semantics survive restart without false merging;
- each reminder threshold delivers at most once and due scheduling stays constant-state;
- notification coverage is truthful when TokenMaster is not running;
- no read-only connector, plugin, CLI, MCP, or LLM path can acquire mutation authority;
- automatic activation is impossible without the official idempotent/status capability,
  fresh evidence, explicit policy, CAS, durable intent, and reconciliation;
- ambiguous external outcomes never trigger blind retries or false success;
- activation receipts link to immutable quota transitions without inventing capacity;
- retention, memory, threads, handles, and SQLite growth remain within fixed bounds;
- security review finds no session scraping, raw payload storage, secret-bearing logs,
  or arbitrary network/browser execution.

This plan adds no current account integration and makes no claim that the user’s two
existing resets are already discoverable or activatable by TokenMaster.
