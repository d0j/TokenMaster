# TokenMaster P2-D Quota History Core Design

**Status:** approved after critical self-review under the user's delegated product
authority; implements the already approved quota-reset requirements without changing
their product boundary

**Scope:** provider-neutral quota window definitions, exact normalized observations,
pure full-reset and allowance-change detection, schema-v10 persistence, bounded
history, and immutable read snapshots. Codex transport, banked-benefit inventory,
reminder delivery, UI, CLI/MCP, and every mutation capability remain separate later
contours.

## 1. Outcome

TokenMaster must preserve every trustworthy full quota reset instead of replacing one
mutable percentage bar. Scheduled, early, repeated, and explicit manual/banked resets
remain distinct transitions with exact pre/post evidence, maximum observed use in the
closed epoch, old/new reset times and capacities, confidence, and an exact timestamp
or observation interval.

The core remains useful even when a provider exposes ratios only. Absolute capacity is
optional and never inferred from local tokens, messages, tasks, cost, or elapsed time.
If no permitted Codex quota source exists, the later adapter reports unavailable; the
data core does not weaken its evidence rules to manufacture a live quota.

## 2. Audited architecture choice

Three approaches were considered:

1. **One P2-D monolith:** transport, detector, SQLite, inventory, reminders, UI, and
   activation in one implementation. Rejected because failures and authority
   boundaries could not be reviewed independently.
2. **Attach quota to usage generations:** reuse usage events, aggregate generations,
   and `QueryHeader.dataset_identity`. Rejected because provider quota can change with
   no usage event, usage rebuilds must not rewrite quota history, and quota freshness
   has provider-defined timing.
3. **Independent quota revision in the same bundled SQLite file:** fixed-point domain
   values, a pure detector crate, quota-owned tables/revision, and a separate immutable
   query header. Selected because it preserves one portable database while keeping
   quota authority, recovery, cursors, and freshness independent from usage data.

The selected design adds no network, browser, timer, UI, or provider-specific code.

## 3. Component boundaries

### `tokenmaster-domain`

`crates/domain/src/quota.rs` replaces the M0 `QuotaTarget` placeholder with bounded
provider-neutral values:

- `QuotaScope`: provider, account, and optional workspace identity;
- `QuotaWindowKey`: scope plus stable provider window ID;
- `QuotaRatio`: integer parts per million in `0..=1_000_000`, never floating point;
- optional `QuotaUnits`: one bounded unit ID with used/remaining/capacity integers;
- `QuotaWindowDefinition`: revision, presentation direction, fixed/rolling/credit/
  unknown semantics, optional nominal duration, and optional provider-defined reset
  thresholds;
- `QuotaSample`: opaque observation ID, exact observation/freshness times, optional
  provider epoch, ratios/units/reset time, quality, source, confidence, and explicit
  reset evidence.

The domain crate validates values and relationships only. It performs no detection,
hashing, SQLite, time-zone conversion, or provider I/O.

### `tokenmaster-quota`

A new pure crate owns:

- deterministic epoch and transition identities;
- constant-state current-epoch tracking;
- ordered duplicate/stale/conflict handling;
- reset and allowance-change classification;
- exact pre/post/maximum-use transition values;
- stable path-free errors.

It depends only on `tokenmaster-domain` and pinned `sha2`. It has no SQLite,
filesystem, environment, clock, async runtime, network, UI, or mutable global cache.

### `tokenmaster-store`

Schema v10 adds quota-owned tables and one independent quota revision inside the
existing database. Usage dataset generation and aggregate generation do not change
when quota data changes. One quota transaction validates and applies one normalized
observation, publishes a new quota revision only for a visible change, and preserves
the previous publication on every failure.

`UsageReadStore` remains the single defensive read-only connection and gains fixed
quota queries. The name is historical; its authority remains read-only SQLite, not
provider or detector authority.

### `tokenmaster-query`

Quota results use `QuotaQueryHeader` and `QuotaEnvelope<T>`, not the usage
`DatasetIdentity`. The header carries process-local snapshot generation, exact quota
revision, generated/data-through time, aggregate freshness/quality, scopes, and stable
warnings. Current windows and transition pages are immutable owned values.

## 4. Exact domain decisions

### Identity and privacy

Provider, account, workspace, window, unit, and provider-epoch identifiers are bounded
ASCII IDs. They cannot contain whitespace, separators, control characters, URLs, or
absolute paths. Observation, epoch, transition, and internal scope identities are
32-byte opaque values with redacted `Debug`.

Account/workspace changes produce another `QuotaScope`; they never look like a reset.
An absent workspace remains typed `None`. SQLite uses a deterministic opaque scope key
rather than a public sentinel or nullable composite identity.

### Ratios and units

Ratios use integer parts per million. Formatting into percentage text is a later
presentation concern. Used and remaining ratios may be independently unavailable.
When both exist, the core retains both facts and their quality; it does not force them
to sum exactly because provider rounding can be explicit.

Absolute values use one `QuotaUnits` value with a bounded provider unit ID and optional
used, remaining, and capacity integers. A sample with no absolute facts has
`units=None`. A capacity change is comparable only when the old and new unit IDs
match.

### Provider-defined reset thresholds

The core never hard-codes zero, one hundred percent, five hours, or seven days.
`QuotaResetThresholds` may define:

- maximum post-reset used ratio;
- minimum post-reset remaining ratio;
- optional minimum used-ratio drop.

At least one post-reset boundary is required when thresholds are present. Thresholds
authorize inferred detection only for fixed windows. Rolling and unknown windows need
an explicit provider epoch/reset signal.

### Observation and freshness time

Wall-clock milliseconds are retained as signed integers after checked ordering
validation:

`observed_at <= fresh_until <= stale_after`.

Advertised reset time may already be past in a stale observation and therefore is not
required to follow `observed_at`. An explicit reset occurrence, when known, must be
positive, no later than the current observation, and later than the previous
observation before it can become an exact transition time.

## 5. Detection contract

`evaluate_sample` consumes one definition, optional current epoch state, optional
previous sample, and one new sample. It returns one of:

- `Started`: open the first epoch;
- `Duplicate`: identical observation ID and content, no publication;
- `Stale`: older or equal observation time with another ID, no publication;
- `Advanced`: current epoch/sample changed without a full reset;
- `AllowanceChanged`: current epoch remains open and one capacity transition is added;
- `Reset`: close the current epoch, add one reset transition, and open the next epoch;
- an error for identity/content conflicts, overflow, or incoherent caller state.

Detection precedence is:

1. explicit manual/banked evidence;
2. changed explicit provider epoch identity;
3. explicit provider/local reset evidence;
4. coherent provider-threshold transition plus advanced advertised reset time;
5. coherent threshold transition without an exact reset time, classified unknown and
   lower confidence;
6. a ratio or amount drop alone is not a reset.

Manual evidence and conflict/unknown-quality samples cannot independently create an
automatic inferred reset. Crossing the previous advertised boundary classifies a
reset as scheduled; strong evidence before it is early; unavailable boundary evidence
is unknown. Manual/banked evidence always keeps its explicit kind.

A reset transition stores the interval
`previous.observed_at < reset <= current.observed_at` unless a valid exact occurrence
is supplied. An allowance change may accompany the same reset transition. A
standalone allowance change receives its own monotonically sequenced transition
without closing the epoch.

Deterministic identities cover the normalized window key, definition revision,
pre/post observations, transition kind, and sequence inputs. Retry/restart cannot
duplicate a transition, while two repeated resets necessarily have different
pre/post observations and identities.

## 6. Schema-v10 model

The quota tables are:

- `quota_state`: singleton quota revision and bounded retained counts;
- `quota_window_definition`: immutable normalized definition revisions;
- `quota_sample`: immutable retained normalized samples;
- `quota_epoch_current`: one current projection per window;
- `quota_epoch_history`: immutable closed epochs;
- `quota_transition`: immutable reset/allowance transitions with per-window sequence;
- `quota_window_current`: exact current definition/sample/epoch and health projection.

Every table is `STRICT`; opaque IDs are exact 32-byte blobs; IDs/text/enums/times/counts
have checks; foreign keys and fixed indexes are part of the schema contract. Published
closed epochs and transitions are insert-only. Current projections may change only in
the same transaction that advances `quota_state.revision`.

An exact v9-to-v10 migration adds empty quota state and tables without reading,
rewriting, or reclassifying usage rows. Failure rolls back to exact v9. Current schema
validation rejects missing, weakened, extra-authority, or malformed quota objects.

Every unique incoming sample is inserted inside the transaction. If it is a redundant
poll, the previous unprotected equivalent sample is removed after the current pointer
moves, preserving the newest trustworthy pre-reset boundary without retaining a poll
log. Samples referenced by first/last/max/reset evidence are protected.

Default per-window retention is 512 samples and 256 closed epochs/transitions; hard
caps are 2,048 and 1,024. Maintenance is keyset-paged at 256 rows. It may discard only
redundant, unprotected samples and covered old detail after bounded aggregates exist;
it never merges transitions or changes pre/post/max evidence.

## 7. Read and publication contract

One deferred read transaction captures quota revision, current window projections,
and an optional transition page. Requests accept at most 32 scopes/windows and pages
retain at most 256 rows plus one lookahead. Transition order is sequence descending
then deterministic ID; continuation is bound to exact quota revision and filter.

Each current window exposes:

- exact normalized definition and current sample;
- current epoch first/last/max facts;
- advertised reset and optional duration;
- freshness and quality derived from the sample's provider-defined boundaries;
- last transition summary and acknowledgement state later owned by settings/UI.

Quota capture never reads `usage_event`, usage rollups, price facts, or provider raw
content. Usage query calls never read quota tables.

## 8. Failure, performance, and security

- Invalid, stale, duplicate, conflicting, or over-limit input leaves the last
  publication intact and returns a stable result/error.
- A changed observation ID with different content at the same time is stale/conflict,
  never an overwrite.
- Checked sequences, revisions, counts, times, ratios, capacities, and hashes fail
  closed on overflow.
- Public `Debug`, errors, and serialized values exclude raw provider payloads, paths,
  URLs, credentials, cookies, prompts, responses, reasoning, commands, SQL, and
  private IDs.
- Detector work and retained Rust state are constant per window. Store/query work is
  bounded by fixed windows/pages and indexes.
- Release gates use at least 32 windows, repeated resets, 10,000 redundant polls,
  restart/fault injection, keyset history, storage bounds, and repeated Windows
  private-memory/handle/thread/USER/GDI high-water evidence.

## 9. Delivery rail

This contour is implemented in eight independently reviewable tasks:

1. exact domain values replacing `QuotaTarget`;
2. pure detector and deterministic identities;
3. schema v10 and exact migration;
4. transactional observation application;
5. bounded retention/restart/fault evidence;
6. defensive read-store snapshots and keyset history;
7. immutable query facade and freshness/quality mapping;
8. scale/resource/privacy gates and project-truth closure.

After this core is green, P2-D continues with two separate plans:

1. permitted Codex quota transport, which may truthfully remain unavailable;
2. banked-benefit inventory and reminder policy, followed later by UI.

No provider mutation, activation link, automatic activation, notification delivery,
UI, CLI, MCP, or external plugin work is authorized by this design.

## 10. Self-review result

The design contains no placeholders. It resolves the ambiguities left by the older
product plans:

- inferred reset thresholds are provider data, not hard-coded;
- ratios are fixed-point, not floating point;
- quota revision is independent from usage dataset identity;
- current epochs and closed immutable history have separate storage roles;
- redundant polls preserve the newest possible pre-reset boundary without unbounded
  history;
- allowance changes are orthogonal and may accompany a reset;
- transport, inventory, reminders, UI, and mutation authority are separate gates.

Within the approved product boundary this is the lowest-coupling, fastest, and safest
implementation rail.
