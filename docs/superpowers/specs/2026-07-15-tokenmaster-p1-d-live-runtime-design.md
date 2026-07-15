# TokenMaster P1-D Live Runtime Design

**Status:** Approved for autonomous execution. This design refines the already
approved P1 engine design after checking the real Codex, engine, store, and platform
interfaces. It is binding for P1-D and supersedes any P1-C assumption that cannot be
implemented by the real multi-file Codex source without unbounded memory or a full
history replay on every refresh.

## 1. Goal and release boundary

P1-D turns the provider-neutral engine core into the built-in live Codex runtime while
preserving TokenMaster's primary product requirements:

- JSONL append work is tail-only and does not rescan complete history;
- descriptor, path, event, relation, chunk, diagnostic, watcher, and request memory is
  bounded independently of history size and event burst size;
- one cross-process writer lease covers GUI, CLI refresh, and future MCP refresh;
- watcher events are lossy pathless hints backed by authoritative complete
  enumeration and incremental reconciliation;
- cancellation, restart, replacement, truncation, and port faults leave the last
  trustworthy canonical truth readable;
- the engine remains independent of Codex paths, SQLite, OS handles, Slint, async
  runtimes, and watcher implementations.

P1-D does not add product queries, quota transport, pricing, Git metrics, CLI, MCP,
full UI, skins, Wasm providers, packaging, or release acceptance. P1-E still owns
owned immutable query publication, UI-generation races, sleep/resume integration
evidence, and long-running resource gates.

## 2. Audit findings that must be fixed first

### 2.1 The current source identity is not a file identity

One Codex profile exposes a small bounded set of source roots such as `sessions` and
`archived_sessions`, but each root may contain thousands of JSONL files. Every file
descriptor under one root currently carries the same provider/profile/source ID.
The engine `SourceIdentity` contains only those three values, so different JSONL files
compare equal. `ReplaySourcePage` also rejects duplicate identities. A fake adapter
with one unique source ID per record could pass P1-C while the real Codex adapter
cannot.

Binding correction: the sealed engine identity becomes
`provider + profile + provider source + logical file key`. The fixed 32-byte logical
file key participates in equality, ordering, and archive lookup but remains redacted
from `Debug`, errors, snapshots, and external interfaces. Observation matching still
uses provider/profile/provider-source because that is the provider fact carried by an
observation; the logical file key selects the physical archive stream.

### 2.2 The current pull cannot recover a descriptor safely

After streaming discovery, `Adapter::read_batch(SourceIdentity, checkpoint)` receives
no path-bearing descriptor. A Codex adapter can therefore only:

1. retain every file descriptor/path until replay, making peak memory proportional to
   file count;
2. enumerate the complete tree once per replayed source, producing quadratic work; or
3. persist a path in the archive checkpoint, violating the path-private boundary.

All three are rejected.

Binding correction: full rebuild uses the same two linear streaming passes already
proved by P0-E. The second pass calls an engine replay sink while the adapter still
owns the current path-bearing descriptor. The sink receives a temporary
descriptor-bound `SourceBatchReader`; it cannot store or serialize that reader. The
engine performs archive calls and bounded pulls inside the callback, then the
descriptor is dropped before enumeration continues. Peak file-descriptor memory is
constant and total enumeration work is O(N), not O(N squared).

### 2.3 The apparent archive batch is not yet atomic in the real store

The engine treats `Archive::append_replay_batch` as one compare-and-swap transition.
The current P0-E driver writes the event batch in one transaction and then applies
each late relation in separate transactions. A relation failure can therefore advance
the store epoch while the engine still holds the prior epoch; its exact discard then
fails stale.

Binding correction: the store receives canonical events, late relations, chunk
proofs, and the next checkpoint in one bounded replay append input and one immediate
transaction. It validates and applies relations in deterministic input order, runs
classification/selection updates, advances the evidence epoch once, and either
commits everything or writes nothing. The runtime archive bridge performs no sequence
of independently committing store calls behind one engine call.

### 2.4 Full rebuild per hint violates the fast-append contract

`OneShotExecutor` currently creates a zero-offset staging generation for every
present source and replays all JSONL content. That is correct as a rebuild and wrong as
the steady-state watcher path. It contradicts TM-FUNC-002 and the approved
incremental-append p95 target, and its CPU/I/O cost grows monotonically with history.

Binding correction: the existing executor is the bootstrap/rebuild executor only.
P1-D adds a replay-aware incremental executor. Normal watcher and periodic work does
a complete source enumeration, probes the current checkpoint, reads only appended
complete bytes, and mutates canonical accounting through the current replay evidence.
A full rebuild occurs only when there is no replay-safe baseline, accounting versions
changed, a reader proves replacement/truncation/rewrite/anchor mismatch, recovery
finds incompatible state, or an explicit repair requests it.

## 3. Alternatives and decision

### 3.1 Composition ownership

| Alternative | Consequence | Decision |
| --- | --- | --- |
| Make Codex, store, and platform crates implement engine traits directly | Fewer files, but runtime policy and conversions spread into low-level crates and dependency direction becomes harder to audit | Rejected |
| Add a runtime bridge but cache every Codex descriptor | Small engine diff, but memory grows with file count and paths live longer than needed | Rejected |
| Add `tokenmaster-runtime`, repair the streaming port, and keep low-level crates engine-agnostic | One composition authority, bounded descriptor lifetime, no dependency cycle, reusable by GUI/CLI/MCP | Selected |

`tokenmaster-runtime` starts with only the direct dependencies its bridge code uses:
engine, Codex, store, platform, and provider. Engine does not depend back on it. Codex
retains no production dependency on store or engine. Store and platform retain no
production dependency on Codex or engine. Local wrapper types implement engine ports,
satisfying Rust orphan rules without reversing low-level dependencies.

### 3.2 Writer exclusion

| Alternative | Consequence | Decision |
| --- | --- | --- |
| SQLite busy timeout only | Provider I/O and recovery can race before a transaction starts | Rejected |
| Lock-file existence or PID/timestamp row | Crash can leave a false permanent owner and PID reuse is ambiguous | Rejected |
| OS file lock held by an open sidecar handle | Non-blocking, automatically released on process death, portable in Rust 1.97 std | Selected |

### 3.3 Filesystem scheduling

| Alternative | Consequence | Decision |
| --- | --- | --- |
| Queue raw watcher events/paths | Burst memory and privacy surface grow with activity | Rejected |
| Poll only | Simple but either stale or repeatedly expensive | Rejected |
| Native watcher hint plus one fixed aggregate and periodic reconciliation | Fast reaction, bounded state, watcher loss cannot corrupt truth | Selected |

The selected watcher implementation is the cross-platform `notify` backend, pinned
exactly and wrapped inside `tokenmaster-runtime`. Its callback drops event paths
immediately and only updates fixed atomic hint state. TokenMaster performs its own
quiet-window and capacity-one wake handling; no debouncer/event-history crate is used.
Dependency and resource gates are stop conditions: if the pinned watcher cannot meet
bounded thread/handle/memory behavior, P1-D does not silently weaken the gates.

## 4. Corrected provider-neutral streaming ports

The engine keeps `visit_scopes` and the first `visit_sources` discovery pass. P1-D
adds a second descriptor-bound pass:

```text
Adapter::visit_replay_sources(scope, control, replay_sink)
ReplaySink::on_source(discovered, initial_checkpoint, source_reader)
SourceBatchReader::read_batch(checkpoint, control) -> AdapterBatch
```

The exact Rust names may be adjusted for clarity, but these invariants are binding:

- `SourceBatchReader` is object-safe, temporary, non-serializable, and cannot escape
  the callback lifetime;
- the reader exposes no path, file handle, provider descriptor, raw bytes, or generic
  payload to engine/archive code;
- the second pass independently enumerates the complete exact scope and returns
  truthful complete/partial/cancelled/failed quality;
- the engine validates the logical file identity and exact scope before asking the
  archive to prepare or append;
- a source not in the completed scan set, a duplicate second-pass source, or a source
  omitted by the second pass prevents rebuild seal/promotion;
- the adapter creates a fresh zero-offset initial checkpoint from a bounded open/probe,
  not by parsing and discarding the first event batch;
- physical replacement may update only the untouched staging source through the
  existing exact prepare CAS.

Zero configured scopes is an explicit non-authoritative no-op: scope discovery
returns partial/unavailable quality before scan-set creation and cannot mark any prior
source missing. A configured but missing profile remains an emitted scope whose source
visit closes partial; it is never converted into an authoritative empty profile.

Archive replay lookup changes from archive-page-driven descriptor recovery to exact
logical-file lookup during the second pass. Store seal remains the whole-manifest
proof, so a missing second-pass descriptor cannot be hidden by a caller counter.

## 5. Built-in Codex adapter and checkpoint codec

The runtime adapter retains only the configured-root input plus the current bounded
provider discovery snapshot: at most 64 profiles, 128 provider source roots, and the
existing path-size limits. It never retains a list of JSONL descriptors.

Each run performs:

1. refresh the bounded Codex discovery snapshot;
2. emit one scope for every configured profile with explicit availability;
3. enumerate each available scope in the established active/direct/archive order;
4. emit a sealed per-file identity and versioned zero-offset checkpoint;
5. for rebuild or incremental reading, enumerate again and lend one temporary
   descriptor-bound reader at a time.

A missing, rejected, unreadable, reparse, or partially enumerated profile is not an
empty authoritative profile. It closes partial/failed and cannot mark unseen sources
missing. An available completely enumerated profile with zero JSONL files is a valid
zero-source scope.

`CodexCheckpointV1` is a manual versioned bounded binary envelope over the existing
reader checkpoint fields. It contains only schema/version flags, physical and logical
fixed identities, offsets, file observation metadata, boundary anchor, verification,
and the bounded parser resume payload. It contains no absolute/relative path, source
content, raw tail, credential, prompt, response, reasoning, command, or output. Decode
rejects unknown versions, trailing bytes, invalid flags, overflow, mismatched logical
identity, and payloads above 32 KiB before allocation.

## 6. Replay-aware incremental archive

### 6.1 Persisted publication generation

Schema v6 adds one strict singleton archive-state row with:

- checked non-negative `archive_generation`;
- the current replay accounting revision;
- the latest complete source scan set;
- bounded current incremental/recovery quality state.

The row is updated in the same transaction as every visible canonical mutation or
freshness publication. It does not retain publication history. An owned consumer
snapshot records the generation it read and is never mutated; a newer consumer
replaces it only with a higher generation. This supplies restart-monotonic identity
without a row per refresh or event.

The replay revision remains the current accounting-evidence baseline. `sealed` means
that baseline passed a complete full-prefix rebuild. Valid append-only observations
may extend its current generations and replay overlay under checked evidence epochs;
the singleton generation identifies each visible extension. The latest scan-set field
is separate from the baseline revision's original rebuild provenance.

### 6.2 Incremental operation

Normal live refresh follows this sequence under the writer lease:

1. complete the same provider/profile scan-set enumeration and finalization used by a
   rebuild;
2. inspect archive mode and versions;
3. select incremental mode only for an exact replay-verified current baseline;
4. enumerate descriptors again without retaining them;
5. load the exact current checkpoint by logical file key;
6. return `unchanged` after bounded identity/metadata/anchor checks, or read at most
   one existing bounded tail batch;
7. atomically append events, relations, chunks, checkpoint, replay classifications,
   selections, affected canonical rows, evidence epoch, and archive generation;
8. repeat while the fixed source snapshot has more complete lines;
9. run bounded durable replay continuation and publish complete/partial quality.

The store must expose one replay-aware current append transaction rather than reusing
the older canonical-only `apply_append_batch` in replay-verified mode. Canonical
materialization selects only `eligible` observations from the current replay overlay.
Late relation work can reclassify affected descendants; every changed fingerprint is
refreshed in the same transaction or through the existing bounded durable continuation
before complete quality is published.

A newly observed source may join the current replay evidence only when it belongs to
the exact latest complete scan set, has a validated zero-offset current generation,
and the current accounting versions match. Admission and its first replay-aware
append are compare-and-swap protected. Missing sources remain historical evidence and
are never deleted by incremental refresh.

If any source reports identity change, truncation, rewrite, anchor mismatch, invalid
resume, or incompatible accounting state, incremental work stops and requests one
coalesced full rebuild. Already committed append-only evidence from other sources is
still valid; the rebuild stages a complete replacement and publishes atomically.

### 6.3 Full rebuild remains authoritative but exceptional

The corrected `OneShotExecutor` is used for:

- empty, legacy-unverified, or accounting-version-stale archives;
- physical replacement, truncation, rewrite, anchor mismatch, or explicit repair;
- incompatible/stale recovery state;
- adversarial and periodic integrity verification selected by later policy.

The normal periodic reconciliation is a complete enumeration plus incremental probe,
not a forced full-history replay. This distinction is mandatory for long-run CPU,
SSD I/O, and responsiveness.

## 7. Portable OS writer lease

`tokenmaster-platform` exposes a small path-owning `ExclusiveFileLease`. The runtime
owns the wrapper that implements the engine `WriterLease` port.

- The sidecar resides beside the archive under the same controlled data directory.
- The parent directory is resolved once; reparse/symlink ambiguity and non-local
  unsupported locations fail closed rather than producing two lock identities.
- The sidecar is opened read/write without truncation and contains no owner data,
  PID, timestamp, path, credential, or diagnostic text.
- Rust 1.97 `File::try_lock` supplies the non-blocking exclusive OS lock. `WouldBlock`
  maps only to engine `busy`; other I/O failures map to stable unavailable/failed
  codes without wrapping OS messages.
- The guard owns exactly one file handle. Drop closes it and process death releases
  it automatically. The persistent empty sidecar is not deleted on unlock, avoiding
  Unix inode replacement races.
- SQLite's WAL/FULL/busy-timeout policy remains the final transaction exclusion; it
  does not replace the outer lease.

Contracts use independent handles and a child process, prove same-process and
cross-process contention, release after normal exit and forced child termination,
alias normalization, no sidecar payload, stable redacted `Debug`, and reacquisition.

## 8. Bounded hint scheduler and watcher

The live topology is fixed:

```text
notify callback(s) -> fixed atomic hint aggregate -> capacity-one scheduler wake
periodic timeout    -> fixed atomic hint aggregate -> capacity-one scheduler wake
scheduler thread    -> RefreshWorker::submit
refresh worker      -> incremental or rebuild executor under one writer lease
```

The hint aggregate stores only:

- dirty/not-dirty;
- force-reconcile/not-force;
- highest urgency;
- checked latest monotonic hint tick for the one quiet window;
- watcher health/overflow bits;
- lifecycle running/paused/stopping bits.

It stores no event, path, source ID, provider descriptor, timer node, or request
history. Repeated callbacks update the same atomics and attempt one non-blocking wake.

Policy defaults for the first implementation:

- immediate startup reconciliation;
- 250 ms quiet window after the latest filesystem hint;
- 15 minute safety reconciliation while the watcher is healthy;
- 60 second fallback reconciliation while the watcher is unavailable;
- watcher overflow, rescan flag, error, settings/root change, resume, or clock
  discontinuity forces a complete reconciliation hint;
- all intervals are fixed bounded runtime policy in P1-D and become validated settings
  only in the later product settings slice.

The watcher observes at most the bounded configured profile roots recursively. Missing
roots are found by the periodic discovery path rather than by watching an unrelated
large ancestor. After discovery makes a root available, the watch set is replaced as
one bounded generation. Old watcher generations cannot submit authoritative state;
their hints merely dirty the next reconciliation.

## 9. Lifecycle and restart recovery

Shutdown is cooperative and ordered:

1. stop watcher admission;
2. mark scheduler stopping and wake it;
3. cancel the exact active refresh permit;
4. join the scheduler and refresh worker;
5. drop the writer lease guard, watcher, store, and remaining handles.

No task, timer, watcher, channel sender, store transaction, or thread is detached.

Startup recovery runs only while holding the OS writer lease:

- close an orphan running scan child/set with truthful failed/cancelled quality;
- resume a replay staging revision only when versions, complete scan-set membership,
  revision/epoch, checkpoints, and durable work are exact;
- resume an incremental pending source from its committed current checkpoint when the
  current revision and archive generation are exact;
- otherwise discard only the exact unpublished staging revision/epoch and schedule a
  new complete reconciliation;
- never delete the database, current revision, current generation, canonical page,
  immutable legacy snapshot, or a source merely because it is missing.

P1-D exposes `pause`, `resume`, and `shutdown` inputs without Slint. Pause stops new
admission and cancels at a safe boundary. Resume invalidates watcher assumptions and
submits one forced reconciliation. P1-E supplies the real Windows power-event binding
and race/resource evidence.

## 10. Error, privacy, and dependency rules

- Runtime errors are stable enums with optional bounded counters only. They never
  retain or format paths, OS messages, checkpoint bytes, provider payload, SQL, or
  source identities.
- Adapter/store conversion validates count, size, scope, logical identity, generation,
  epoch, and archive generation on every boundary.
- No production crate may expose `SourceFileDescriptor`, `Path`, `UsageStore`, watcher
  event types, or OS handles through engine, query, UI, CLI, or MCP APIs.
- `notify` is absent from engine, Codex, store, platform, and future query/UI crates;
  only runtime owns it. Wasmtime and Tokio remain absent from the Codex-only graph.
- Runtime tests use synthetic paths, but failure output and `Debug` scans must not
  contain those paths or privacy sentinels.
- All counters remain checked within SQLite's signed 64-bit ceiling. An exhausted ID,
  epoch, generation, or counter fails closed without wrap.

## 11. Delivery slices and stop gates

1. **P1-D.0 — real-source port repair:** per-file logical identity, temporary
   descriptor-bound second pass, corrected executor fakes, and 300-file linear/bounded
   contracts.
2. **P1-D.1 — atomic archive batch:** events plus late relations in one store
   transaction, epoch/discard fault tests, and bridge-safe inputs.
3. **P1-D.2 — bootstrap composition:** versioned Codex checkpoint codec, cheap initial
   probe, `tokenmaster-runtime`, Codex adapter, store archive wrapper, and real JSONL
   bootstrap/replacement/restart tests. This slice is not yet attached to a watcher.
4. **P1-D.3 — replay-aware incremental path:** schema v6 archive generation, current
   append/new-source/continuation/recovery contracts, unchanged/tail-only proof, and
   p95 microbenchmark harness.
5. **P1-D.4 — portable writer lease:** std OS file lock plus independent-process death
   and reacquisition contracts.
6. **P1-D.5 — scheduler and watcher:** fixed hint aggregate, capacity-one wake,
   quiet/periodic/degraded policy, bounded watch generations, and no-path burst tests.
7. **P1-D.6 — live assembly and recovery:** worker/executor selection, startup recovery,
   pause/resume/shutdown ownership, real synthetic append/replacement/reopen fixtures,
   and dependency/privacy audits.

Each slice is test-first and independently committed. A failed atomicity, identity,
tail-only, lease-death, cancellation, thread-join, privacy, or bounded-resource
contract stops progression. Full root format, strict Clippy, locked workspace tests,
and clean-root audit remain mandatory after substantial changes.

## 12. Acceptance evidence and non-claims

P1-D is complete only when contracts prove:

- at least 300 files under shared Codex source IDs remain distinct and are processed
  in two linear streaming passes without a descriptor list;
- unchanged refresh reads no historical event bytes and append reads only the bounded
  tail from the committed checkpoint;
- event/relation/checkpoint application is atomic and exact cleanup succeeds after
  every injected failure boundary;
- a 10,000-event watcher burst retains one aggregate, at most one follow-up refresh,
  and no path/event queue;
- two processes cannot refresh one archive concurrently, and process death releases
  the lease;
- replacement/truncation switches to full rebuild without erasing prior truth;
- restart resumes exact work or performs exact discard, never ad-hoc cleanup;
- shutdown leaves no task-owned thread, watcher, timer, handle, or transaction;
- Codex-only runtime dependency and sensitive-content audits pass.

This evidence does not accept M0, replace the independent interactive or uninterrupted
soak receipts, prove the complete product UI, package TokenMaster, or authorize a
release.
