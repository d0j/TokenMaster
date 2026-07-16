# TokenMaster Codex Quota Transport Design

**Status:** implemented and verified on 2026-07-16

**Scope:** one read-only, credential-blind, bounded Codex connector that converts the
official local app-server rate-limit snapshot into the implemented provider-neutral
quota core. Runtime scheduling, benefit persistence/reminders, activation, UI, CLI,
MCP, and external provider packages remain separate contours.

## 1. Outcome

TokenMaster reads live Codex quota windows without scraping a browser, replaying a
private endpoint, parsing a slash-command screen, reading credentials, or estimating
provider allowance from local token history.

The connector uses the installed Codex native executable as an isolated short-lived
child:

1. start `codex app-server --stdio`;
2. complete the documented `initialize` / `initialized` handshake;
3. call stable-surface `account/read` with `refreshToken=false`;
4. call stable-surface `account/rateLimits/read`;
5. validate and normalize the complete bounded response;
6. terminate and reap the child before returning.

If the executable, tested protocol version, account identity, schema, response, or
deadline is unavailable, the connector returns one stable redacted error. Existing
quota history remains readable and ages through its stored freshness boundaries.

## 2. Evidence and maturity decision

The locally installed Codex 0.144.1 generated non-experimental JSON Schema contains:

- `account/rateLimits/read`;
- `rateLimitsByLimitId` plus the backward-compatible single-bucket view;
- primary and secondary windows with integer `usedPercent`, `resetsAt`, and
  `windowDurationMins`;
- reset-credit count and detail rows with opaque ID, grant time, expiration, type, and
  status.

The official Codex manual documents app-server JSONL-over-stdio, the required
handshake, version-specific generated schemas, and the distinction between the stable
API surface and fields/methods that require `experimentalApi`. The request above is
present when schemas are generated without `--experimental`, and the connector never
opts into experimental API.

The CLI command is still labelled experimental as a product surface. TokenMaster
therefore pins a tested protocol version, fails closed on every schema/version change,
and keeps the connector replaceable behind a narrow source boundary. It does not claim
generic compatibility with arbitrary future Codex releases.

A credential-free live probe on the reference machine returned two distinct metered
buckets, one backward-compatible duplicate, a weekly duration supplied by the
provider, and two expiring reset credits. The complete account-plus-rate-limit read
finished in about 0.9 seconds with about 24 MiB peak private memory in the child. These
numbers are development evidence, not a release gate or a hard-coded product value.

## 3. Rejected alternatives

- **Session JSONL quota parsing:** local but not an official versioned quota contract;
  incomplete when no recent response carries the facts and coupled to conversation
  wire evolution.
- **WhereMyTokens private request replay:** useful reference behavior, but browser
  session reuse/private endpoint coupling violates TokenMaster security boundaries.
- **Dashboard or slash-command scraping:** presentation is not an integration
  contract and is locale/layout fragile.
- **Persistent app-server child:** lowers an already infrequent startup cost but
  permanently increases process/memory surface and complicates suspend, shutdown, and
  version replacement.
- **Shared daemon/socket attachment:** adds lifecycle, ownership, authentication, and
  cross-client failure modes without a current product benefit.
- **Local token-derived allowance:** produces plausible but false provider capacity
  and remains forbidden.

## 4. Component boundary

`tokenmaster-codex` owns:

- the exact supported app-server protocol revision;
- a path-private executable descriptor supplied by composition;
- bounded JSONL request/response framing;
- child deadline, termination, and reap;
- strict Codex wire decoding;
- account pseudonymization;
- multi-bucket/primary/secondary normalization;
- provider-neutral quota definitions and samples.

It does not own executable discovery, user settings, polling, SQLite writer
coordination, query publication, UI state, benefit storage, reminders, activation, or
arbitrary subprocess execution.

The public connector accepts one already resolved absolute native executable path and
one caller-supplied positive observation time. It returns owned bounded normalized
observations only. `Debug` and errors expose counts/codes, never the executable path,
Codex home, email, opaque reset-credit IDs, raw frames, or inner process errors.

## 5. Process and protocol contract

- One poll owns one child and at most one I/O helper thread.
- The only command is the exact executable plus fixed arguments
  `app-server --stdio`.
- Windows uses a hidden/no-console child.
- stdin/stdout are inherited pipes; stderr is discarded rather than persisted.
- Each JSONL frame is capped before allocation; total frames and total response bytes
  are capped.
- Initialization, account read, and quota read use fixed numeric request IDs.
- Unknown response fields, methods, duplicate IDs, wrong IDs, malformed JSON, early
  EOF, oversized frames, and JSON-RPC errors fail the complete poll.
- The complete child session has one monotonic deadline.
- Success, error, panic, or timeout always closes stdin, terminates the task-owned
  child if still running, waits for exit, and joins the I/O thread.
- No SQLite transaction, writer lease, UI callback, or immutable query snapshot is
  held while awaiting the child.

Initial bounds:

- complete session deadline: 15 seconds;
- one frame: 256 KiB;
- complete stdout: 1 MiB;
- frames: 64;
- quota windows after primary/secondary expansion: 32;
- reset-credit detail rows decoded transiently: 64;
- provider string: 512 bytes unless a narrower domain bound applies.

## 6. Identity and normalization

### Account

`account/read` must report a ChatGPT account with a non-empty bounded email. The email
exists only inside the bounded request. TokenMaster trims it, hashes it with a
domain-separated SHA-256 account identity, emits `acct_<hex>` as `QuotaAccountId`,
and immediately drops the raw value. It is never serialized, logged, stored, returned,
or included in `Debug`.

API-key, Bedrock, missing-account, missing-email, and unknown account shapes return
explicit unavailable because they cannot safely isolate ChatGPT quota history.

The current stable response has no workspace identifier. `workspace_id` is therefore
typed `None`; this limitation is recorded rather than guessed from plan type, local
paths, auth-file metadata, or rate-limit names.

### Buckets and windows

When `rateLimitsByLimitId` is present and non-empty, it is authoritative. The legacy
`rateLimits` member is not emitted again. Otherwise the legacy snapshot is used as one
fallback bucket.

Each metered bucket may produce a `primary` and `secondary` window. A safe provider
limit ID is retained in the internal stable window key; an invalid/oversized ID is
replaced by a domain-separated hash. The slot and provider-supplied duration are part
of window identity, so a duration-class change becomes a new window rather than an
in-place immutable-definition mutation.

Provider `limitName` is bounded and retained only as connector metadata for a future
presentation registry. It is not forced into a localization key or current schema-v10
storage.

### Values

- `usedPercent` must be in `0..=100` and maps exactly to parts-per-million by
  multiplication by 10,000.
- Missing remaining ratio and absolute capacity remain unavailable.
- `resetsAt` seconds map to checked positive UTC milliseconds.
- Positive `windowDurationMins` maps to checked seconds.
- The response is official provider evidence: source is `provider_official`, quality
  is authoritative, and confidence is medium because reset history is inferred from
  successive official snapshots rather than an explicit reset event.
- A Codex rate-limit window with a supplied reset timestamp is modeled as fixed.
- Reset inference requires a later advertised reset plus at least one integer-percent
  directional recovery. The minimum drop is one provider percentage point; no
  five-hour, weekly, zero-used, or capacity value is hard-coded.
- Definition revision is the TokenMaster mapping revision. Dynamic duration is kept
  in the window identity, so one mapping revision remains immutable.

Observation identity is a domain-separated SHA-256 over the normalized scope/window,
observation time, freshness bounds, ratios, reset time, duration/mapping revision, and
evidence metadata. Retry with the same normalized response/time is idempotent.

## 7. Freshness

The app-server response has no freshness timestamps. The caller supplies a positive
wall-clock lower bound captured immediately before poll admission. The transport
validates it before spawning and uses it as the conservative observation time.
Process duration can therefore age the sample by at most the configured bounded
deadline but can never overstate freshness. Codex transport policy v1 marks the
sample:

- fresh through 20 minutes;
- aging after 20 minutes;
- stale after 2 hours.

This is an explicit connector policy chosen around the existing 15-minute healthy
reconciliation interval; it is not inherited implicitly from usage queries. Failed
polls write no replacement sample, so the last successful evidence naturally ages.
Clock overflow or non-positive observation time fails before publication.

## 8. Reset credits boundary

The same official response currently contains the exact banked-reset source needed by
TM-FUNC-010. This contour validates the bounded credit schema transiently so malformed
combined responses fail closed, but does not persist, expose, remind, or activate a
credit.

The following benefit-inventory contour will map credit rows into TM-DATA-009 typed
lots, hash public identity separately from any future activation handle, and add FEFO,
expiration reminders, reconciliation, and opt-in activation policy. Quota read access
does not imply mutation authority.

## 9. Runtime integration sequence

1. Implement and verify the pure strict wire-to-domain normalizer.
2. Implement and verify the bounded short-lived app-server process transport.
3. Add executable discovery/configuration as a separate platform/composition task.
4. Add a dedicated quota refresh worker that performs transport I/O without a writer
   lease, then acquires the existing process writer lease only for bounded normalized
   store writes.
5. Publish quota refresh health separately from usage-engine health.
6. Add benefit inventory/reminders, then UI.

The usage scan worker is not extended with app-server I/O: doing so would couple
history ingestion latency and writer ownership to a provider network read.

## 10. Acceptance

The connector contour is accepted only when tests prove:

- exact stable protocol handshake and request shapes;
- current multi-bucket and legacy fallback normalization without duplication;
- primary/secondary expansion and 32-window cap;
- account-switch pseudonym separation with no email/path in serialized/debug errors;
- exact ratio/time arithmetic and clock overflow rejection;
- strict unknown/malformed/oversized/out-of-order/duplicate frame rejection;
- unsupported version, unauthenticated account, JSON-RPC error, early exit, and timeout
  mapping;
- child/thread/process cleanup after success and every failure class;
- repeated polling returns host process resources to baseline;
- no browser, cookie, private endpoint, raw credential, or raw response persistence;
- focused tests, workspace formatting, strict clippy, and full workspace tests pass.
