# TokenMaster Scalable Replay Manifest Correction Design

**Status:** Approved corrective prerequisite for P0-E under the operator's delegated
autonomous architecture authority.

## 1. Finding and impact

The completed P0-D implementation treats one physical JSONL file as one
`usage_source`, but `ReplayManifest` and `usage_replay_revision.expected_source_count`
allow at most 256 sources. Codex discovery limits provider roots, not the number of
session files below those roots. A long-lived profile can therefore exceed the replay
manifest limit while remaining fully valid input.

This is a product blocker, not a scale optimization. Once more than 256 files are
registered, no revision can include every registered source, and exact seal correctly
refuses promotion. Raising a test fixture's limit or proving only a small subset would
hide the failure and make real history unrebuildable.

The correction is named P0-D.1 and precedes P0-E. Existing P0-D transaction,
classification, seal, promotion, and recovery semantics remain valid.

## 2. Considered corrections

### A. Disk-backed all-source revision — selected

Add a store-owned `begin_replay_revision_all_sources` operation that snapshots every
registered source into one staging revision using SQLite set operations. Source count
uses a checked non-negative 64-bit value. Manifest validation and seal iterate source
rows in keyset pages of at most 256 and retain only one page.

This preserves one atomic cross-file replay truth while making memory independent of
history file count. It also keeps callers from loading thousands of source keys merely
to pass them back to SQLite.

### B. Raise `ReplayManifest` to a larger in-memory cap

Rejected. Any chosen limit is either still a correctness failure for a larger valid
history or reserves avoidable memory at startup. It also duplicates the registered
source set in application memory and SQLite.

### C. Split one revision into 256-file shards

Rejected. Parent/child replay relations and deterministic fingerprint selection cross
file boundaries. Independent shard promotion could expose a mixed generation, miss
cross-shard ancestry, or require a second global transaction model. Deterministic
bucket generations would also require a different checkpoint schema.

## 3. Schema v3

The on-disk schema advances from 2 to 3. The only shape change is:

```sql
expected_source_count INTEGER NOT NULL CHECK(expected_source_count >= 1)
```

SQLite signed integers remain the storage ceiling. Rust exposes this value as `u64`
but rejects any stored value outside `1..=i64::MAX`. No collection is allocated from
the count.

### 3.1 Exact v2-to-v3 migration

Migration follows SQLite's documented generalized ALTER TABLE procedure:

1. validate the exact v2 tables, indexes, triggers, legacy snapshot, and foreign keys;
2. disable foreign-key enforcement outside a transaction and verify it is disabled;
3. start one immediate transaction;
4. create `usage_replay_revision_v3` with the revised constraint;
5. copy every revision column exactly and verify row/count equality;
6. drop the old table, rename the new table to `usage_replay_revision`, and recreate
   both partial unique revision indexes;
7. run `foreign_key_check`, validate the complete exact v3 schema and legacy counts,
   and set `user_version = 3`;
8. commit, re-enable foreign keys, and revalidate runtime policy.

The safe create-new/copy/drop/rename order is required; rename-old-first is forbidden
because SQLite can rewrite child foreign-key references. `writable_schema` is never
used. The procedure is based on the official
[SQLite ALTER TABLE guidance](https://sqlite.org/lang_altertable.html#making_other_kinds_of_table_schema_changes).

Every early return or injected migration failure restores `foreign_keys=ON`. A failed
migration leaves the exact v2 archive reopenable and byte-for-byte logical data
unchanged. Newer or malformed schemas continue to fail closed before mutation.

Fresh archives and exact v1 migration create v3 directly. Exact v1 legacy copy and
immutability behavior do not change.

## 4. API and type changes

### 4.1 Product path

`UsageStore::begin_replay_revision_all_sources()`:

- requires at least one registered source;
- rejects any existing staging revision or staging generation;
- derives the next checked revision ID and compiled accounting versions;
- counts registered sources in SQLite and records the checked count;
- creates one staging generation per current source with generation
  `max(existing generation) + 1`, zero offsets, empty resume/anchor, incremental
  verification, and copied physical/logical identity;
- inserts the fixed `usage_replay_source` rows in the same immediate transaction;
- verifies inserted revision/source/generation counts and foreign keys before commit;
- never updates `usage_source.current_generation` or canonical events.

The implementation uses `INSERT ... SELECT` or keyset-paged statements without a
source-key vector. Any source lacking one valid current generation, generation
overflow, count mismatch, constraint failure, or injected fault rolls the entire
begin operation back.

### 4.2 Explicit bounded manifest path

The existing `ReplayManifest` API remains capped at 256 for focused tests, repair
tools, and negative subset contracts. It is not the production full-history path.
Seal still requires its stored manifest to match every registered source, so a subset
cannot become canonical accidentally.

`ReplayRevisionSnapshot.expected_source_count()` changes from `u16` to `u64`. No
caller may use it as an allocation size without a separate local bound.

## 5. Paged validation, seal, promotion, and discard

`replay_manifest_is_complete` no longer uses a global `LIMIT 257` or collects every
manifest row. It:

- compares checked registered/manifest/generation counts with the stored expected
  count;
- reads source state in deterministic `file_key` keyset pages of at most 256;
- validates one page's generation state, checkpoint flags/offsets, and exact chunk
  coverage before advancing the cursor;
- requires the number of visited rows to equal the expected count;
- returns false/error on duplicate, missing, extra, changed-registration, or malformed
  state.

Bounded continuation does not repeat full chunk validation for every work item. It
uses a cheap aggregate `replay_manifest_sources_closed` check: registered count,
manifest count, staging-generation ownership, and zero `pending` source states must all
match the stored expected count. A source reaches `complete` only in the transactional
append that accepts its full-prefix checkpoint and chunk proofs. Final seal and every
promotion still repeat the full keyset-paged checkpoint/chunk validation, so tampered
staging can never become canonical.

Promotion continues as one transaction. SQLite mutation row counts are checked by
fallible conversion to `u64`, never by narrowing the stored count to `usize` or
`u16`. Discard remains set-based and O(1) application memory.

Registering a new source after begin makes the fixed manifest incomplete and blocks
seal. The caller must discard and restart from the new all-source snapshot. Deleted or
missing-source authority remains P1 scan-finalization work.

## 6. Required tests

### 6.1 Schema and migration

- fresh schema is exact v3 and has no upper-256 source-count constraint;
- exact populated v2 migrates current and staging revision rows plus every child table
  without changing values or legacy immutability;
- empty-revision v2 also migrates;
- malformed v2, count corruption, index corruption, migration fault, or foreign-key
  corruption rolls back and restores enforcement;
- exact v1 migrates directly to v3;
- v3 reopen is validation-only and a newer version fails before mutation.

### 6.2 All-source begin

- 300 registered sources produce exactly 300 staging generations and manifest rows
  without changing current pointers/pages;
- an empty archive is rejected;
- a second staging revision, missing current generation, generation overflow, and
  injected failures are atomic;
- the legacy explicit-manifest cap remains enforced and a subset cannot seal.

### 6.3 Paged lifecycle

- at least 300 zero-length sources are completed through bounded append/checkpoint
  updates, sealed across more than one validation page, promoted, reopened, and
  reported with the exact source count;
- registering source 301 after a 300-source begin blocks seal without mutation;
- promotion and exact discard retain prior canonical/legacy state under fault.

The test fixture count deliberately exceeds 256 but remains small enough for normal
CI. P0-E later exercises many real JSONL descriptors and multi-batch event streaming.

## 7. Bounds, privacy, and performance

- Application memory during all-source begin is constant apart from SQLite's reviewed
  bounded cache; no source-key collection is created.
- Validation retains at most 256 fixed-size source states and one chunk query result at
  a time.
- The SQLite cache remains 8 MiB, mmap remains disabled, WAL/journal bounds remain
  unchanged, and no new dependency is added.
- Counts are numeric only. Errors and Debug output expose neither source keys nor paths.
- No prompt, response, reasoning, command, output, file content, raw JSON, incomplete
  tail, absolute path, or credential enters the schema or reports.

No measured startup/RSS claim follows from structural bounds. Those remain explicit
performance gates.

## 8. Acceptance and handoff

P0-D.1 is complete only when focused schema/migration/all-source/page tests pass,
existing P0-D behavior remains green, strict Clippy and full workspace tests pass,
privacy/diff/clean-root gates pass, and all durable documents state that the 256-source
product blocker is removed.

The next step is the separately specified P0-E transactional pipeline proof. P1 live
engine semantics remain out of scope.
