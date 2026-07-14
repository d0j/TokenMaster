# TokenMaster P0-E Transactional Pipeline Proof Design

**Status:** Approved and unblocked. The scalable all-source replay-manifest correction
passed on 2026-07-14; this is the next implementation gate.

## 1. Goal and boundary

P0-E proves that the already implemented Codex discovery/enumeration/reader,
TokenMaster accounting authority, and P0-D replay archive compose into correct
end-to-end outcomes over real synthetic JSONL files. It covers append, fork replay,
restart, truncation, replacement, exact totals, replay-quality counts, privacy, and
bounded working state.

P0-E is not the production runtime engine. It does not add filesystem watchers, scan
epochs, missing-source finalization, coalescing, worker pools, writer leases,
sleep/resume policy, immutable query snapshots, UI updates, CLI, or MCP. Those remain
P1 and later work. This separation prevents a test proof from accidentally becoming a
second scheduler or freezing the future provider-neutral `SourceAdapter` ABI.

The original 256-source P0-D manifest cannot represent a normal long-lived Codex
profile because one store source is one JSONL file. P0-D.1 schema v3, disk-backed
all-source begin, and keyset-paged seal are therefore a hard prerequisite. P0-E may
not claim success against only a small manifest.

## 2. Considered approaches

### A. Test-only cross-crate pipeline driver — selected

Add an integration contract under `tokenmaster-codex` with development-only
dependencies on accounting and store. The driver uses only public production APIs and
holds at most one bounded reader batch. Add only the path-private store reads that a
restart-safe future engine necessarily needs.

Advantages: no runtime dependency inversion, no throwaway production scheduler,
actual Codex parser/reader and SQLite behavior, and failures identify missing public
seams before P1. The driver is evidence, not product authority.

### B. Start `tokenmaster-engine` now

This would reuse more implementation later, but P0-E would have to decide cancellation,
leases, scheduling, scan generations, reconciliation, and adapter traits before the
transactional data flow is proven. It conflates approved P0-E and P1 gates and creates
more failure modes than the proof requires.

### C. Put Codex ingestion in `tokenmaster-store`

This is smaller mechanically but rejected. It would make SQLite understand paths,
Codex JSONL, reader checkpoints, and parser policy, violating the provider-neutral
authority boundary and making future adapters require storage changes.

## 3. Dependency and ownership shape

Production dependencies remain unchanged:

```text
tokenmaster-codex -> tokenmaster-domain/provider/platform
tokenmaster-accounting -> tokenmaster-domain
tokenmaster-store -> tokenmaster-accounting/domain
```

Only the Codex integration-test target adds development dependencies on
`tokenmaster-accounting` and `tokenmaster-store`. The test driver performs orchestration
but owns no new semantics:

```text
synthetic private JSONL tree
  -> Codex discovery + streaming enumeration
  -> bounded ReadBatch (maximum 256 events)
  -> tokenmaster-accounting canonicalizer
  -> P0-D staging append/relation/continuation
  -> exact seal + atomic promotion
  -> bounded canonical pages + replay quality
```

The driver never calls SQLite directly, retains raw lines, serializes descriptors, or
creates accounting authority fields.

## 4. Required store prerequisites and restart-safe reads

P0-D.1 supplies schema v3 and `begin_replay_revision_all_sources`, which snapshots the
registered file set into SQLite without an in-memory source-key manifest. P0-E uses
that product path and includes fixtures with more than 256 files.

P0-D writes staging checkpoints and chunk proofs but currently exposes only the current
generation snapshot. P0-E adds two bounded path-private reads:

- `replay_generation_snapshot(revision_id, source_key)` returns the one staging
  `GenerationSnapshot` owned by that exact revision/source or a stable stale-revision
  error. It cannot list sources or read a current generation implicitly.
- `source_chunk(source_key, generation, chunk_index)` returns at most one
  `StoredSourceChunk`. It is used by `verify_full_prefix`; absent proof remains
  explicit and fails verification.

Both validate stored integers, enums, digest lengths, and generation ownership. Debug
surfaces remain redacted. They expose no path, raw JSON, source bytes, prompt,
response, reasoning, command, output, or credential. They are provider-neutral and are
expected to be reused by P1.

The stored checkpoint contains the already hashed 32-byte physical identity, while the
platform type originally supported only live handle discovery. P0-E therefore also
adds `PhysicalFileIdentity::from_persisted_bytes([u8; 32])`. This is an explicit,
infallible reconstruction of an opaque fixed-size digest, symmetric with
`LogicalFileIdentity::from_bytes`; it does not accept OS fields or paths and cannot
mint an identity from variable-length untrusted input. `ReaderCheckpointV1::new`
continues to validate the reconstructed checkpoint as a whole.

No arbitrary SQL, source enumeration, or bulk chunk vector is added.

Replay begin is provider-neutral and therefore cannot manufacture a valid empty Codex
resume payload or know that an atomic file replacement has a new physical identity.
Copying the old physical identity while clearing offsets makes the first reader call
classify a legitimate replacement as stale, and an empty opaque resume cannot be
decoded after restart. P0-E therefore adds one constrained mutation:

- `prepare_replay_source(revision_id, expected_epoch, source_key, checkpoint)` accepts
  only a validated zero-offset, empty-anchor, incremental checkpoint for an untouched
  pending staging generation. It requires the registered logical identity, rejects
  observations/chunks/work or a stale/sealed revision, updates only that invisible
  generation, and advances the evidence epoch by CAS.

The adapter obtains the live path-private physical identity and its own valid empty
resume state from an initial bounded read, prepares staging, and may apply that same
batch immediately. This supports unchanged files, atomic replacement, parser schema
upgrades, cancellation, and restart without teaching SQLite any provider format.

## 5. Deterministic proof-driver algorithm

### 5.1 Discover and enumerate

1. Build a bounded Codex discovery request for a synthetic temporary root.
2. Require available profiles and `EnumerationCompletion::Complete`; partial or
   cancelled enumeration cannot start or seal a rebuild.
3. Stream file descriptors through the enumeration callback without retaining the
   complete file list. Provider source bounds apply to roots; file count is checked as
   a 64-bit counter and is not silently capped at 128 or 256.
4. Derive the store `SourceKey` from the built-in Codex logical-file identity bytes.
   This is a 1.0 bridge convention for the proof, not a public plugin ABI. The value is
   already provider/profile/path-class framed and path-private.

### 5.2 Register and begin

The driver uses two deterministic streaming enumeration passes. In the first pass, a
bounded initial read supplies each newly observed file's physical/logical identity;
registration uses a zero-offset checkpoint with those identities, empty anchor, a
valid serialized empty parser resume, and incremental verification, then drops the read batch. After exact
complete enumeration, `begin_replay_revision_all_sources` snapshots all registered
files on disk. The second pass rereads each file from its zero-offset staging
checkpoint and drives append. This deliberately trades test I/O for bounded memory and
restart-realistic public seams.

The store creates one disk-backed fixed manifest containing every registered synthetic
source. A rebuild always starts a new invisible generation for the complete manifest.
P0-E intentionally uses complete streaming rebuilds for correctness; incremental live
scheduling is P1.

### 5.3 Stream, canonicalize, and persist

For each source in stable key order:

1. Reconstruct the reader checkpoint from the exact staging generation snapshot.
2. Call `read_source_batch` with no cancellation for the normal proof or an injected
   bounded cancellation point for failure fixtures.
3. Canonicalize each `ObservationDraft` through `tokenmaster-accounting`; no provider
   or driver creates fingerprints, signatures, event IDs, dispositions, versions, or
   epochs.
4. Convert only checkpoint numbers/digests, at most 256 canonical events, and bounded
   chunk updates into one `ReplayAppendBatch` using the current staging epoch.
5. Apply late `SessionRelationDraft` values after the batch in deterministic emitted
   order, advancing the epoch by compare-and-swap for each relation.
6. Drop the batch before reading the next page. Close/reopen fixtures reconstruct all
   state from the archive rather than retaining the batch, parser state, or chunks.

When the reader reaches a complete snapshot end, `verify_full_prefix` retrieves one
expected chunk at a time through `source_chunk`. A verified result is persisted with
an otherwise unchanged checkpoint marked `full_prefix`; this marks the manifest source
complete. Cancellation, mismatch, rebuild-required, incomplete tail, missing chunk, or
reader/store error cannot mark the source complete.

### 5.4 Reconcile, seal, and promote

After all sources are complete, the driver runs bounded `continue_replay` calls with
the latest epoch until work is exhausted. A fixed test iteration cap greater than the
maximum fixture work proves termination without weakening the store's 32/256 bounds.
Then it seals and promotes using exact revision/epoch CAS.

Any failed/cancelled proof leaves the prior canonical page visible. The driver invokes
`discard_replay_revision` only with the latest exact staging epoch, proving a clean
retry without deleting current or legacy state.

## 6. Required fixture matrix

### 6.1 Replay-safe baseline

- Parent and child files contain equal ordinal-zero strong cumulative evidence.
- Child declares the parent through real Codex session metadata.
- Expected quality includes one replay observation and only the parent contribution in
  canonical totals.
- A divergent child observation is independently expected and counted once.

The expected event IDs/counts/token totals are fixture constants calculated outside
the archive query path. Tests page canonical events at no more than 256 rows and sum
only explicit available totals.

### 6.2 Append rebuild

Append a complete child event after the replayed prefix, run a new full staging
rebuild, and prove:

- the previous current page remains visible before promotion;
- complete-manifest continuation resolves the outgrown parent ordinal;
- promotion adds exactly the divergent suffix once;
- quality counts and exact totals match the independent oracle after reopen.

### 6.3 Mid-rebuild restart

Use more than 256 observations so the reader produces multiple batches. Close the
store after the first batch, reopen it, reconstruct the staging reader checkpoint and
chunk proof from public path-private reads, and finish without duplicate or skipped
events. Record the maximum observed batch size and require it not to exceed 256.

### 6.4 Truncate and replace

After a promoted append result:

- truncate the synthetic file to an earlier complete-line boundary and rebuild;
- replace the file atomically with a different physical file and complete content;
- prove old canonical state remains visible throughout staging;
- prove a complete replacement that covers prior evidence can promote, while a
  truncation that omits prior visible evidence fails promotion and exact discard keeps
  the old projection current;
- prove reader `RebuildRequired` classifications are observed when current checkpoints
  are probed, without treating them as permission for partial destructive writes.

File deletion/missing-source reconciliation and explicit carry-forward of prior
evidence are excluded because their authority belongs to P1 scan finalization and
retention policy. Reader rebuild classifications alone never authorize erasure.

### 6.5 Cancellation and failure

Cancel enumeration and reading at deterministic points. Also inject a malformed or
incomplete tail. None may seal, promote, change the current page, or leave a rebuild
that cannot be discarded and retried.

## 7. Bounds and performance evidence

- Reader events per batch: at most 256.
- Store append events and chunk updates: existing hard caps.
- Provider roots retain their existing bound; enumerated JSONL files are streamed and
  counted up to the checked SQLite/Rust numeric limit without a history-sized vector.
- Manifest validation retains at most 256 fixed-size source states per keyset page.
- Canonical reads: keyset pages of at most 256.
- Full-prefix verification: one expected chunk lookup at a time; no chunk-history
  vector is loaded from SQLite.
- Driver working set: descriptors for the bounded fixture manifest plus one reader
  batch, one canonical batch, and one page. No whole-history event vector or queue.
- Restart proof: no retained in-memory checkpoint or event data across reopen.

P0-E records structural bounded-memory evidence, not an RSS claim. M0/M1 measured
memory, latency, soak, and one-million-row gates remain separate and must not be
inferred from these contracts.

## 8. Error and privacy behavior

The driver maps failures only inside assertions; it does not add a product error API.
Production additions return existing stable `StoreErrorCode` values and never wrap
SQLite, OS, parser, or path text. Tests assert that Debug output for descriptors,
checkpoints, generation snapshots, chunks, manifests, and errors does not contain the
temporary root or sentinel private content.

Synthetic fixture files may contain privacy sentinels only in ignored/unrecognized
fields. The sentinel must not appear in archive pages, debug output, generated reports,
or tracked files outside the test literal. No real user Codex data is read or copied.

## 9. Acceptance gate

P0-E is complete only when:

1. P0-D.1 schema v3/all-source/paged-manifest gates pass first;
2. focused RED/GREEN tests prove persisted physical-identity reconstruction, both new
   store reads, exact staging preparation, and the cross-crate pipeline;
3. replay, append, restart, truncate, replacement, cancellation, and incomplete-tail
   fixtures all preserve exact atomic behavior;
4. independent expected counts/totals and `ReplayQualityCounts` match after reopen;
5. fixtures exceed both 256 files and 256 events while retaining bounded pages/batches
   and restarting without duplication;
6. staging remains invisible until promotion and exact discard restores retryability;
7. changed-code privacy/path/secret scans are clean;
8. store, Codex, accounting, format, strict Clippy, clean-root, and full workspace tests
   pass; the explicitly ignored one-million-row M0 gate is reported, not claimed;
9. traceability, current state, project history, handoff, roadmap, and recovery docs
   distinguish the completed proof from the unimplemented P1 engine.

## 10. Explicit non-goals and next step

P0-E does not optimize away full rebuilds, reconcile deleted sources, create a public
`SourceAdapter`, add a new worker/thread/channel, mutate UI state, expose query totals
to automation, or package a release. The next design/implementation slice is P1: one
provider-neutral runtime engine with scan epochs, missing-source finalization,
coalescing, cancellation, writer lease, sleep/resume, restart recovery, and immutable
snapshot revisions.
