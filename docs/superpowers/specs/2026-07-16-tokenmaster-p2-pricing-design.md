# TokenMaster P2-C Pricing Design

**Status:** approved after upstream, data-path, performance, privacy, and failure-mode audit

**Scope:** release-pinned API-equivalent USD estimates, bounded validated overrides,
source-reported cost selection, explicit provenance/conflict/availability, and indexed
cost-basis queries. This is a local deterministic feature. It does not fetch prices at
runtime and does not claim that a subscription user was actually billed the estimate.

## 1. Product outcome

TokenMaster keeps the useful cost analysis of ccusage while removing behavior that can
silently produce a plausible but wrong number. The UI, CLI, and future MCP surface must
be able to answer all of these questions without reading raw event history:

- what is the estimated API-equivalent cost for the selected immutable dataset/range;
- whether the number is complete, partial, unavailable, or legitimately zero;
- whether it came from source-reported values, the embedded catalog, a user override,
  or a deterministic mixture selected by `auto` mode;
- which release-pinned catalog and override revision produced it;
- how many events were priced, assumed-standard, unpriced, omitted by a safety bound,
  or in conflict with a source-reported value;
- which bounded model/tier/context keys require attention.

Unknown price, unknown service tier, invalid token relationships, missing price basis,
and a truncated key set are never represented as zero.

## 2. Audited reference behavior

The exact ccusage reference is the commit pinned in `third_party/UPSTREAM.toml`.
TokenMaster adapts its three selection modes and per-model/cache-aware analysis, but
does not inherit these failure modes:

1. ccusage returns `0.0` when pricing or a model is missing. TokenMaster returns an
   explicit unavailable or partial state.
2. ccusage may refresh LiteLLM/models.dev at runtime. TokenMaster 1.0 performs no
   pricing network I/O; a reviewed release changes the embedded catalog.
3. ccusage uses binary floating point for rates and totals. TokenMaster uses checked
   integer fixed-point arithmetic and rounds once at the public boundary.
4. ccusage infers one Codex fast mode from the current configuration and applies it to
   historical aggregates. TokenMaster preserves the normalized tier per event.
5. ccusage falls back to a 2x fast multiplier and can multiply long-context pricing,
   even though OpenAI currently excludes long-context requests from Priority
   Processing. TokenMaster requires an explicit catalog rule for every supported
   tier/context pair.
6. ccusage's Codex aggregate cost path prices `output_tokens` without adding the
   separately logged reasoning bucket. TokenMaster derives billable output from the
   complete event total when possible, otherwise from checked output plus reasoning.
7. ccusage fuzzy model matching is convenient but can bind a future model to an older
   numeric version. TokenMaster uses exact canonical keys and reviewed aliases only.
8. A partial override in ccusage can create an entry whose unspecified rates are zero.
   TokenMaster validates the fully resolved rule; a new model override must be complete.

WhereMyTokens remains the presentation reference for cost prominence and cache savings.
It is not a pricing authority.

## 3. Authority and catalog policy

The embedded catalog is a small reviewed OpenAI/Codex catalog, not a vendored copy of
LiteLLM. Each release records:

- stable `catalog_id` and schema version;
- retrieval date;
- official price source URLs;
- reviewed exact aliases and snapshot aliases;
- fixed-point standard and supported service-tier rates;
- optional whole-request long-context threshold and rates;
- unsupported combinations as absence, never an inferred multiplier.

The first catalog is based on official OpenAI model and Priority Processing pages
reviewed on 2026-07-16. ccusage at the pinned commit is used only as a behavioral and
historical-model cross-check. Future catalog updates are pull-request/release changes
with value fixtures and changelog entries. A future signed catalog updater requires a
separate threat model and is outside 1.0.

## 4. Fixed-point money

Public USD amounts use unsigned microdollars: `1 USD = 1_000_000 usd_micros`. Catalog
rates use integer microdollars per one million tokens. Calculation accumulates
`tokens * rate` in checked `u128`, sums all buckets, then performs one deterministic
half-up division by one million. Conversion fails closed if the result does not fit the
public bounded type.

This representation exactly stores a rate to six decimal places per million tokens,
does not accumulate binary-floating drift, and can distinguish a legitimate explicit
zero rate from missing pricing.

User-facing formatting is a UI/localization responsibility. Core/query/API values never
use localized strings or floating-point JSON numbers.

## 5. Canonical price basis

Pricing is a function of facts, not already rounded monetary snapshots:

- canonical provider and model key;
- normalized service tier;
- whole-request long-context state;
- source-reported-cost presence;
- uncached input tokens;
- cached input tokens;
- billable output tokens, including reasoning;
- optional source-reported USD microdollars.

For Codex, `input_tokens` includes cached input. Therefore:

```text
uncached_input = input - cached
```

The relationship must be known and `cached <= input`. Billable output is derived in
this order:

1. if total and input are known and `total >= input`, use `total - input`;
2. otherwise, if output and reasoning are both known, use `output + reasoning`;
3. otherwise calculated price basis is unavailable for that event.

All additions/subtractions are checked. Raw total, prompt, response, reasoning text,
commands, paths, and source identifiers are never retained in price rollups.

### 5.1 Service tier

The normalized v1 keys are:

- `standard_reported`: source explicitly reported `standard` or `default`;
- `standard_assumed`: Codex supplied no tier; calculation may proceed but provenance
  and counters expose the assumption;
- `priority`: source reported `priority` or legacy `fast`;
- `unknown`: any other non-empty value; calculated pricing is unavailable unless a
  future reviewed catalog explicitly supports it.

Tier is preserved per event. Current configuration is never used to rewrite history.

### 5.2 Long context

The Codex adapter already records `yes` only when event input is greater than 272,000.
For a catalog rule with whole-request long-context pricing, `yes` selects the complete
long-context rate set for all input/cache/output buckets. `unavailable` is unpriceable
for such a rule. Models without a long-context rule may use the same explicit short
rate only when the catalog declares that context-independent behavior.

Priority plus long context is unavailable in the initial catalog because the official
Priority Processing contract excludes long context. It is not synthesized by applying
a multiplier.

## 6. Store schema and aggregation

P2-C introduces schema v9 and two generation-owned tables:

- `usage_time_price_rollup` for minute/hour range queries;
- `usage_session_price_rollup` for session/detail queries.

The primary keys include dataset/generation/scope plus model, normalized tier,
long-context state, and reported/unreported state. Values include event count,
calculable event count, checked token-basis sums, and reported-cost sum/count.

Current inserts update the active generation transactionally. Delete/replace paths
subtract or remove rows in the same transaction. Legacy and recovery rebuilds populate
inactive tables page by page and publish only with the existing aggregate-generation
swap. A catalog or override change does not rebuild usage rollups because rollups store
facts, not rates.

Each event contributes at most one minute, one hour, and one session price row. The
extra row amplification is constant and measured by the million-event gate. No Rust
history map or view-time raw event grouping is allowed.

## 7. Bounded query contract

The store returns immutable price-basis captures using the same dataset generation,
range segments, scope limits, deadlines, and read-only transaction identity as token
analytics. Rows are ordered deterministically by impact and key and capped at 512
distinct price keys per requested result. The capture separately returns exact omitted
event/calculable/reported counts.

Hitting the cap produces partial cost; it never discards evidence and reports complete.
Model/tier keys are bounded and path-free. Public Debug and errors must not contain SQL,
table names, source IDs, raw models, prompts, commands, reasoning, or absolute paths.

Series and breakdown costs are obtained by bounded grouped reads from the price
rollups. They are not produced by issuing one query per visible point. Session lists use
the session rollup and preserve keyset paging.

## 8. Pricing engine and overrides

`tokenmaster-pricing` is a pure synchronous crate with no filesystem, SQLite, HTTP,
async runtime, environment, or global mutable cache. An immutable engine contains the
embedded catalog plus a validated override snapshot.

Bounds:

- at most 512 overrides;
- model/provider/tier keys use existing bounded ASCII contracts;
- at most one reviewed alias hop; cycles and alias chains reject the snapshot;
- decimal input is accepted only through a strict non-exponent parser with at most six
  fractional digits per million-token rate;
- rates and thresholds have explicit upper bounds;
- duplicate keys, unknown fields, incomplete new rules, and unsupported combinations
  reject the whole candidate while the previous snapshot stays active.

Overrides can replace a complete catalog rule or selected fields of an existing rule.
They cannot remove provenance. A resolved rule records `embedded` or `override`, the
catalog ID, and the immutable override revision. P2-C supplies validation and immutable
snapshots; atomic settings-file persistence belongs to the settings slice and reuses
this contract without changing pricing code.

## 9. Selection, availability, and conflict

Modes match the useful ccusage intent:

- `auto`: use source-reported cost for reported events and catalog/override calculation
  for the rest;
- `calculated`: ignore reported values for selection and calculate every priceable row;
- `reported`: use only source-reported values.

Every result contains:

- `availability`: `complete`, `partial`, `unavailable`, or `zero`;
- selected amount only when meaningful;
- selected mode and actual source composition;
- catalog ID and override revision when calculated data participated;
- total/priced/reported/assumed/unpriced/omitted/conflict event counts;
- a capped deterministic set of missing reason codes and affected keys.

`zero` is valid only when all in-scope positive/zero usage is accounted for and the
selected explicit rates or source-reported values sum to zero. Missing data never maps
to `zero`.

When both reported and calculated values exist for the same rows, the result retains
the selected value and compares the two. Conflict is raised when the absolute delta is
greater than both 10,000 microdollars (one cent) and 2 percent of the larger value.
Conflict status is evidence, not a reason to silently switch selection mode.

Missing reason codes include at least `model_unpriced`, `tier_unknown`,
`tier_context_unsupported`, `token_basis_unavailable`, `key_limit_reached`,
`reported_cost_missing`, and `arithmetic_overflow`.

## 10. API and future providers

The domain/provider contract reserves an optional source-reported USD-micro amount.
Codex leaves it absent. A future provider adapter may populate it only from a documented
provider field and must retain its own capability/provenance evidence. This avoids a
future event/store rewrite for Hermes or other plugins while keeping the current Codex
binary free of plugin/runtime/network cost.

CLI/MCP wire schemas later serialize integers, enums, counters, catalog identity, and
bounded keys. They never accept arbitrary SQL, catalog URLs, filesystem paths, or
network update instructions.

## 11. Performance, memory, and security gates

P2-C acceptance adds:

- exact current/legacy migration, insert, delete/replace, rebuild/resume, and publication
  fixtures;
- official catalog golden values for standard, cached, long-context, priority, aliases,
  and unsupported combinations;
- fixed-point rounding/overflow/adversarial override tests;
- complete/partial/unavailable/zero and all three selection-mode fixtures;
- source-versus-calculated conflict threshold fixtures;
- 1,000,000-event current and legacy release gate with total database amplification at
  or below the existing 3.0x limit and all cost queries below 1 second cold;
- cached cost query p95 below 250 ms and session cost page p95 below 100 ms;
- repeated catalog/override/query switching with no handle/thread/USER/GDI growth and
  no private-memory plateau growth above 2 MiB;
- clean-root, format, strict Clippy, all locked workspace tests, privacy searches, and
  no runtime pricing-network strings in production binaries.

## 12. Rejected alternatives

- **Runtime LiteLLM/models.dev fetch:** non-reproducible, privacy/network/supply-chain
  expansion, and can change history without a TokenMaster release.
- **Floating-point USD:** drift and platform-sensitive aggregation.
- **Persisting calculated cost per event:** catalog changes require rewriting immutable
  history and mix fact retention with policy.
- **Calculating from the capped model breakdown:** truncation can look complete and
  current rollups cannot reconstruct tier or per-request long context.
- **Reading current Codex config for historical tier:** time-dependent and wrong after a
  setting change.
- **Applying a generic fast multiplier:** invents unsupported priority/long-context
  combinations.
- **Fuzzy numeric model matching:** can silently price a future model as an old model.
- **Unknown as zero:** directly violates TokenMaster's honesty contract.
- **One price query per chart point:** creates latency and allocation growth with range
  length.

## 13. Architecture self-review

The plan is approved because it passes every blocking question identified in the audit:

- fact/rate separation survives catalog updates without archive rewrite;
- tier, context, cache, reasoning, and source-reported values remain reconstructable;
- current and legacy generations stay atomic;
- every collection, key, override, and diagnostic is bounded;
- unknown, partial, legitimate zero, and conflict are distinguishable;
- arithmetic and rounding are deterministic;
- no runtime pricing network or mutable global cache is introduced;
- future provider-reported cost fits without changing the public cost model;
- series/session queries remain indexed and avoid raw scans;
- storage, latency, memory, privacy, and release evidence have executable gates.

Implementation may begin only against this approved contract. A discovered violation
reopens the design instead of being patched around.
