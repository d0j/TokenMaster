# TokenMaster Benefit Inventory and Reminder Foundation Design

**Status:** approved for implementation on 2026-07-16

**Scope:** provider-neutral read-only benefit inventory, immutable change history,
bounded reminder profiles and durable scheduling, built-in Codex reset-credit
normalization, and integration into the existing quota refresh runtime. Provider
mutation, activation intents/receipts, UI, CLI, MCP, and external provider packages
remain later contours.

## 1. Outcome

TokenMaster preserves every independently expiring provider benefit and makes the
nearest loss deadline queryable and schedulable without confusing benefits with
normal quota windows.

The first production source is the already supported local Codex app-server response.
One successful poll may contain detailed reset-credit rows plus an aggregate available
count. TokenMaster maps detailed rows to stable opaque lots and represents any
unexplained remainder as one explicit provider-aggregate lot with unknown expiry. It
never invents a grant time, expiry, target window, capacity, monetary value, or
activation result.

Four kinds remain disjoint:

- `banked_rate_limit_reset`;
- `usage_credit`;
- `temporary_usage`;
- `unknown`.

This contour reads and reminds only. Inventory-read authority does not imply an
activation link, provider mutation, browser control, cookie access, or arbitrary
network capability.

## 2. Layering

### `tokenmaster-domain`

Owns validated provider-neutral value types:

- benefit scope and opaque lot/observation identities;
- kind, state, source, confidence, detail completeness, and target window;
- exact UTC, provider-local, provider-date, bounded, and unknown expiry;
- one complete bounded inventory observation;
- versioned reminder profile values and notification channels.

Domain values perform no hashing, I/O, reconciliation, time-zone lookup, scheduling,
or persistence.

### `tokenmaster-benefits`

Is a pure deterministic core with only `tokenmaster-domain` and `sha2` production
dependencies. It owns:

- scope/change/delivery identities;
- reconciliation of one current projection with one complete observation;
- meaningful change classification;
- reminder-profile normalization;
- conservative expiry and due-time evaluation;
- delivery deduplication keys and bounded overdue collapse.

It has no clock, thread, timer, SQLite, filesystem, network, UI, or provider code.

### `tokenmaster-codex`

Consumes the current strict Codex wire response and returns one owned snapshot
containing quota observations and a separate benefit observation.

Raw email and raw reset-credit IDs exist only transiently. A detailed credit identity
is SHA-256 domain-separated by the pseudonymous account ID and raw provider ID before
it leaves normalization. The raw ID, title, description, and response payload are not
persisted or exposed. Codex supplies a stable built-in label key instead of retaining
untrusted free text.

### `tokenmaster-store`

Schema v11 owns:

- one benefit publication state;
- per-scope inventory revision and last observation identity;
- current lot projection;
- immutable bounded lot change points;
- one global reminder profile and optional scope overrides;
- normalized reminder thresholds;
- one indexed durable due queue;
- immutable bounded delivery receipts.

One complete inventory observation is reconciled in one immediate transaction. An
identical observation refreshes bounded freshness only and does not append history or
duplicate due rows. Existing usage and quota publication revisions are independent.

### `tokenmaster-query`

Exposes immutable bounded values for:

- current lots sorted by conservative FEFO order;
- detailed versus aggregate-unknown quantity;
- expiry precision, freshness, confidence, and source;
- nearest conservative expiry and nearest due reminder;
- active inherited or overridden reminder profile;
- bounded change history and delivery coverage.

No query performs a usage-event scan or returns raw provider identities.

### `tokenmaster-runtime`

The existing `CodexQuotaRuntime` performs one app-server poll. After provider I/O and
one non-waiting writer-lease acquisition, it opens SQLite and publishes quota and
benefit facts through separate store transactions. Health reports separate processed,
changed, and failed counts for quota and benefits. A benefit failure never rolls back
already committed quota facts and is never reported as cross-domain atomic success.

A separate reminder runtime owns one scheduler/worker pair and one nearest-due wakeup,
not one timer or thread per lot. Startup, resume, wall-clock discontinuity, profile
change, and inventory publication force bounded reconciliation from the durable queue.

## 3. Observation contract

`BenefitInventoryObservation` contains:

- one provider/account/workspace scope;
- one opaque observation ID;
- `observed_at_ms`, `fresh_until_ms`, and `stale_after_ms`;
- at most 64 owned lot observations;
- an explicit completeness value.

Each `BenefitLotObservation` contains:

- opaque lot ID;
- kind;
- quantity in `1..=i64::MAX`;
- state;
- optional target quota window;
- optional granted time;
- typed expiry;
- source and confidence;
- detail kind: `provider_detail`, `provider_aggregate`, or `manual`;
- stable built-in label key.

The lot vector is authoritative for the represented inventory quantity. An adapter
that receives only an aggregate count emits one aggregate lot instead of omitting the
inventory or inventing individual IDs. Different detailed IDs or expirations remain
different lots.

Codex normalization maps:

- `available` -> `available`;
- `redeeming` -> `activation_pending`;
- `redeemed` -> `activated`;
- unknown status or reset type -> visible `ambiguous`/`unknown`;
- positive `expiresAt` seconds -> checked exact UTC milliseconds;
- absent expiry -> `unknown`;
- each detailed row -> quantity one;
- `availableCount - detailed available rows` -> one stable aggregate available lot.

`availableCount` smaller than the number of detailed available rows, duplicate raw
IDs, non-positive/overflowing times, excessive rows, or invalid strings fail the
benefit observation. Quota normalization from the same response remains valid only
if the complete response contract is valid; no partially trusted raw payload escapes.

## 4. Identity and privacy

All stored/public identities are fixed 32-byte values with redacted `Debug`.

- detailed Codex lot:
  `SHA256(domain, account pseudonym, raw provider credit ID)`;
- Codex aggregate lot:
  `SHA256(domain, account pseudonym, benefit kind, aggregate slot)`;
- inventory observation:
  `SHA256(domain, account pseudonym, observed time, normalized ordered lot facts)`;
- transition and delivery:
  deterministic hashes over stable scope/lot/revision/sequence/threshold/channel
  fields.

Every variable field is length-framed and every integer uses architecture-independent
big-endian encoding. Raw provider IDs, email, titles, descriptions, paths, payloads,
headers, cookies, prompts, responses, commands, and credentials are forbidden from
SQLite, errors, health, logs, `Debug`, fixtures, and public snapshots.

## 5. Expiry semantics

`BenefitExpiry` is:

- exact UTC instant;
- provider-local date/time plus bounded IANA time-zone identifier;
- provider date plus optional bounded IANA time-zone identifier;
- bounded UTC interval;
- unknown.

Exact and bounded expiries provide a conservative UTC boundary directly. Other forms
remain losslessly typed until a time-zone-aware projection can resolve them. Unknown
or unresolved expiries remain visible but cannot produce a false precise countdown.

This design does not hard-code a universal 30-day lifetime. Provider `expiresAt` is
authoritative when supplied; absent expiry remains unknown even if public promotional
documentation describes a common validity period.

## 6. Reconciliation and history

The pure reconciler matches opaque lot IDs only. It does not merge different IDs,
equal expirations, or detailed and aggregate lots.

For each observation it returns one bounded plan:

1. validate scope, observation ordering, uniqueness, counts, and times;
2. classify each lot as awarded, changed, unchanged, reappeared, or terminally
   updated;
3. classify prior missing non-terminal lots as ambiguous disappearance rather than
   silently activated, expired, or revoked;
4. advance the inventory revision only for a visible projection change;
5. append one immutable change point per meaningful lot change;
6. rebuild due entries only for changed lot revisions or changed profiles.

An identical poll may advance `last_observed_at_ms` and freshness metadata but appends
no transition. A previously ambiguous lot may become available again. Expiry by local
clock alone changes presentation/evaluation but does not rewrite provider evidence;
an explicit expiry transition is appended only by bounded maintenance under the same
store contract.

Default retention per scope is 512 change points with a hard cap of 2,048. Current
lots and unresolved ambiguity are protected. Maintenance is keyset-paged at no more
than 256 rows and never scans usage events.

## 7. Reminder profiles and queue

Recommended profile version 1 contains:

- 7 days;
- 24 hours;
- 12 hours;
- 6 hours;
- 1 hour.

A profile has one positive revision and at most eight unique checked lead times.
Each lead is one minute through 365 days. Duplicates collapse and storage uses
descending seconds. An empty explicit profile is valid and means reminders disabled.

One global profile is seeded on schema creation. A provider/account/workspace scope
inherits it until an explicit override is created. Updating the global profile does
not rewrite overrides. `Reset to recommended` is an explicit replacement operation;
upgrades never silently mutate existing user thresholds.

For each available lot with a conservative expiry, the store materializes due rows
for active thresholds. The key is:

`(scope, lot ID, lot revision, threshold seconds, channel)`.

Removing and re-adding a delivered threshold does not erase its receipt. A changed
lot expiry advances the lot revision, cancels obsolete pending rows, and creates new
keys. At most 8 rows per lot and 64 current lots are retained.

The first notification channel is `in_app`; coverage is therefore truthfully
`in_app_only`. OS scheduling, tray ownership, permissions, quiet hours, snooze, and
platform delivery are later extensions over the same queue and receipt schema.

The due worker asks for at most 256 due rows, collapses overdue thresholds for one lot
to the most urgent still-useful notice, records one receipt transactionally, and
recomputes the nearest due time. It never replays every missed threshold after sleep
or hibernation.

## 8. Failure and publication behavior

- Malformed or over-capacity benefit input leaves the previous inventory untouched.
- A quota publication may succeed while benefit publication fails; health reports the
  exact independent result.
- A benefit publication may succeed after quota duplicates; neither revision
  masquerades as the other.
- Writer contention opens no SQLite connection and schedules an accelerated bounded
  retry.
- Unknown expiry remains visible and unscheduled.
- Clock rollback or a future wall time does not duplicate a delivered key.
- Resume forces one queue reconciliation and one Codex recovery poll.
- Store/query corruption fails closed with stable codes and no raw values.
- Reminder runtime failure does not fault usage ingestion or quota refresh.

## 9. Bounds

- 64 current lots per scope;
- 64 Codex detailed rows per response;
- 8 thresholds per active profile;
- 512 default / 2,048 hard-cap changes per scope;
- 256 rows per history, maintenance, or due batch;
- one pending due row per lot revision/threshold/channel;
- one scheduler and one worker for reminder delivery;
- no per-lot thread, timer, channel, callback, or retained payload;
- no usage-event scan during benefit write, query, reminder, or maintenance.

## 10. Acceptance

The foundation is accepted only when tests prove:

- two independently expiring reset lots survive restart without merging;
- detailed IDs are stable and raw IDs cannot escape values, errors, `Debug`, SQLite,
  or release artifacts;
- aggregate count gaps remain visible with unknown expiry;
- credits, temporary usage, banked resets, and unknown kinds never coerce;
- duplicate, changed, missing, reappearing, activated, and ambiguous lots reconcile
  deterministically;
- schema v10 migrates transactionally to strict schema v11 without changing usage,
  price, or quota facts;
- current/history query ordering, continuation, freshness, and corruption rejection
  are bounded and immutable;
- recommended, custom-only, subset, empty, inherited, and override profiles persist
  exactly;
- each reminder key is delivered at most once across restart, duplicate poll,
  profile edits, clock changes, suspend, and hibernation;
- quota/benefit/reminder failure isolation and resource plateaus pass;
- clean-root, formatting, warnings-as-errors Clippy, complete workspace tests, and
  the benefit release-authority audit pass.

