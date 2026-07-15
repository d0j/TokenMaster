# TokenMaster Weekly Quota Reset History Plan

**Status:** Approved product requirement; execute in P2 after the P1 engine and
immutable publication contracts are complete.

## User outcome

The Codex weekly quota card must make every full reset obvious, including an early or
repeated reset before the previously advertised week ends. The user can open the reset
and see exactly what TokenMaster knew immediately before and after it, how much of the
weekly allowance had been consumed, whether the advertised next-reset time changed,
and whether the conclusion is explicit or inferred.

When OpenAI does not expose an absolute allowance, the honest exact answer is a ratio:
for example, `84% used / 16% remaining -> 0% used / 100% remaining`. TokenMaster must
not convert local token totals, task counts, or message counts into a fictitious Codex
limit because provider consumption depends on plan, model, task complexity, and
execution surface.

## Model

Use four immutable provider-neutral records:

1. `QuotaWindowDefinition`: stable provider/scope/window key, label key, direction,
   unit, fixed/rolling/credit/unknown semantics, optional nominal duration, and
   provider capabilities.
2. `QuotaSample`: observation ID/time, optional provider epoch ID, used/remaining
   ratios, optional used/remaining/capacity units, advertised reset time, source,
   freshness, quality, and confidence.
3. `QuotaEpoch`: first/last sample, maximum observed use, advertised reset history,
   start/close interval, and terminal transition ID.
4. `QuotaTransition`: deterministic ID and sequence, previous/current epoch IDs,
   pre/post sample IDs, scheduled/early/manual-or-banked/unknown reset kind, optional
   simultaneous allowance change, old/new reset times and capacities, evidence source,
   confidence, and exact or bounded detection time.

An account or workspace change starts a different scope; it is never displayed as a
quota reset. A rolling-window recovery is not a full reset unless the provider emits
an epoch/reset signal or the complete before/after/reset-time evidence is coherent.

## Detection authority

Process samples in provider/window observation order and reject stale or time-regressed
updates from current state. Detection strength is:

1. explicit provider reset/epoch sequence;
2. explicit local rate-limit reset event tied to the same scope/window;
3. coherent same-window transition where used returns to the reset floor, remaining
   returns to the reset ceiling, and the advertised reset time advances;
4. an otherwise coherent full drop with no exact reset time, recorded as inferred and
   lower confidence;
5. a drop alone: no reset event.

Crossing the advertised weekly boundary yields `scheduled_reset`. The same evidence
before that boundary yields `early_full_reset` unless an explicit manual/banked signal
is available. Several full resets create several monotonically sequenced transitions.
Allowance/capacity changes are separate facts even when they occur in the same sample.

If polling misses the exact instant, preserve the interval
`previous.observed_at < reset <= current.observed_at`; never invent a timestamp. A
transition is committed atomically with closing the old epoch and opening the new one.
Its deterministic identity makes retry/restart idempotent.

## Storage and bounds

- one current sample and current epoch per provider/scope/window;
- change-point samples rather than every timer tick;
- at most 512 recent samples and 256 epochs/transitions per window by default;
- hard upper caps of 2,048 samples and 1,024 epochs/transitions per window;
- bounded daily/monthly aggregates for older history;
- keyset pages of at most 256 rows;
- no credentials, cookies, request bodies, headers, paths, prompts, responses, or raw
  provider payloads.

Compaction may discard redundant poll samples only after every epoch boundary,
pre/post reset sample, maximum-use sample, and aggregate has durable coverage. It may
never merge two reset transitions or rewrite their before/after values.

## UI and automation

The weekly card shows current usage, next reset, freshness, and a persistent
`Reset detected` badge until acknowledged. Reset detail shows:

- before -> after used and remaining;
- maximum use reached before reset;
- recovered headroom;
- scheduled, early, manual/banked, or unknown kind;
- old -> new advertised reset time and capacity when available;
- exact time or observation interval, source, freshness, and confidence.

History renders one vertical marker per transition and can filter `all`, `scheduled`,
`early`, and `allowance changes`. English, Russian, and pseudo-locale strings use
locale-aware percentages, units, dates, and durations; color is never the only signal.

CLI/MCP expose the same bounded snapshots with `transitionSequence`, `preReset`,
`postReset`, `maxUsedBeforeReset`, `oldResetsAt`, `newResetsAt`, `kind`, `confidence`,
and `observedBetween`. Automation can trigger once on a sequence and optionally
require kind/confidence, e.g. resume a deferred Hermes job only after a verified full
weekly reset. It remains advisory: TokenMaster does not launch arbitrary commands.

## Implementation order

1. Replace the M0 `QuotaTarget` placeholder with immutable provider-neutral window,
   sample, epoch, and transition types plus boundary/serialization tests.
2. Freeze the built-in Codex quota adapter contract using credential-free local
   evidence first and optional allowlisted HTTPS second; retain no raw payload.
3. Add strict versioned SQLite tables, exact migration, indexes, deterministic IDs,
   atomic transition commit, compaction, and rollback faults.
4. Implement the pure transition detector and the full fixture matrix below.
5. Add immutable query snapshots, keyset history, CLI JSON, MCP schema, and policy
   evaluator fields.
6. Build the weekly card, reset badge/detail, history markers, notifications, and
   en/ru/pseudo accessibility contracts.
7. Pass latency, request-frequency, offline/stale, memory, restart, privacy, and long-
   run gates before enabling notifications or automation by default.

## Required fixture matrix

- scheduled weekly reset with absolute units;
- scheduled weekly reset with ratios only;
- early full reset while the old weekly deadline is still in the future;
- two early resets inside one previous nominal week;
- explicit manual/banked reset;
- reset plus allowance increase/decrease;
- allowance change without reset;
- normal rolling-window recovery;
- stale/out-of-order/duplicate samples;
- account/workspace switch;
- offline gap with an interval but no exact instant;
- process restart between pre/post samples;
- unavailable ratios or reset time;
- local and HTTPS evidence disagreement;
- compaction/reopen with exact transition preservation.

No P2 quota test may hard-code a five-hour or seven-day product assumption. Durations
and capacities come from provider fixtures; the UI and interfaces render whatever
valid window sequence the provider reports.
