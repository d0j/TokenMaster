# TokenMaster P3-D.0 Reliable State Implementation Plan

> Execute task by task with focused red/green tests and one coherent commit per task.
> Never use real user data. Preserve the fixed live archive path and existing writer
> lease. Do not begin the next task while its focused validator is red.

**Goal:** Add versioned settings, portable configuration import/export, verified
compressed full backups, automatic bounded retention, corruption containment,
idempotent restore/rollback, startup auto-recovery, and complete Data & Recovery UI
without blocking Slint, leaking private data, or creating unbounded memory/disk state.

**Architecture:** A new `tokenmaster-state` package owns pure settings, package,
backup-policy, catalog, health, and recovery orchestration. `tokenmaster-store` owns
SQLite Online Backup and candidate verification. `tokenmaster-platform` owns bounded
durable file primitives and sealed native file selections. `tokenmaster-app` owns
startup, runtime shutdown/restart, restore authority, and safe mode. Product/Desktop
receive copied bounded values and emit typed intents only.

**Stack:** Rust 1.97, edition 2024, Slint 1.17 software renderer, bundled SQLite
3.53.2, `rusqlite = 0.40.1` with `backup`, `serde/serde_json`, `sha2`, pinned
`zstd = 0.13.3` with default features off and no `zstdmt`, optional manual
passphrase wrapping through `age = 0.12.1` with default features off, existing
Windows bindings, PowerShell/Pester authority audits.

**Normative design:**
`docs/superpowers/specs/2026-07-17-tokenmaster-reliable-state-design.md`.

---

## Execution rules

- Keep `tokenmaster.sqlite3` and
  `tokenmaster.sqlite3.tokenmaster-writer.lock` fixed.
- Do not copy a live main database file as a snapshot.
- Do not add a general ZIP/TAR extractor, arbitrary path API, shell command, HTTP,
  external executable, or raw SQL surface.
- All imported files and SQLite candidates are untrusted.
- Use a focused failing test before each behavior change.
- Every operation is bounded by count, bytes, time, and retained state.
- Use synthetic databases and canary strings only; never inspect a real TokenMaster or
  Codex archive for tests.
- Update project truth only after behavior and the full gate pass.
- Each task's commit message is a recommendation; do not commit a red task.

## Milestone map

| Milestone | Tasks | Exit condition |
| --- | --- | --- |
| RS-1 — durable primitives | 1-4 | crate boundary, atomic files, redundant records, settings pass |
| RS-2 — verified packages | 5-8 | snapshot, package, encryption, catalog/retention pass |
| RS-3 — recovery runtime | 9-12 | bounded worker, journal, startup recovery, live restart pass |
| RS-4 — product surface | 13-15 | health model, dialogs, Data & Recovery/safe mode pass |
| RS-5 — closure | 16-18 | adversarial, resource, full workspace, docs/audits pass |

---

## Task 1 — Establish the reliable-state crate and authority audit (complete)

**Files:**

- Create: `crates/state/Cargo.toml`
- Create: `crates/state/src/lib.rs`
- Create: `crates/state/src/error.rs`
- Create: `crates/state/tests/authority_contract.rs`
- Create: `scripts/audit-reliable-state.ps1`
- Create: `scripts/tests/audit-reliable-state.Tests.ps1`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

### Red

1. Add a Pester test requiring one `tokenmaster-state` package, no binary target, no
   UI/Slint/runtime/provider/Codex/query/product dependency, and no network/shell/
   browser/archive-extraction library.
2. Add Rust compile/runtime tests requiring fixed path-free error codes, redacted
   `Debug`, checked byte/count wrappers, and no public constructor from an arbitrary
   filesystem path.
3. Make the audit reject `zip`, `tar`, `tokio`, `reqwest`, `ureq`, process spawning,
   sockets, command strings, arbitrary SQL input, and direct Slint types.

### Green

1. Add `tokenmaster-state` with dependencies only on the exact workspace support it
   needs initially: `serde`, `serde_json`, `sha2`, `thiserror`, and
   `tokenmaster-platform`.
2. Add stable `StateErrorCode` values for invalid input, unsupported version,
   capacity, integrity, unavailable, busy, disk capacity, recovery required, and
   internal invariant failures. Store no source error text.
3. Add private checked size/count helpers used by later codecs.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --locked
Invoke-Pester -Path scripts/tests/audit-reliable-state.Tests.ps1 -Output Detailed
pwsh -NoProfile -File scripts/audit-reliable-state.ps1 -RepositoryRoot (Get-Location).Path
```

**Commit:** `feat(state): establish reliable state boundary`

---

## Task 2 — Add controlled durable file primitives (complete)

**Files:**

- Create: `crates/platform/src/durable_file.rs`
- Create: `crates/platform/tests/durable_file_contract.rs`
- Create: `crates/platform/src/bin/durable_file_fixture.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/src/windows.rs`
- Modify: `crates/platform/src/unix.rs`
- Modify: `crates/platform/src/unsupported.rs`
- Review unchanged: `crates/platform/Cargo.toml`, `Cargo.toml` (the exact pinned
  `sha2`, `thiserror`, and Windows FileSystem bindings were already available)

### Red

Add tests for:

1. exact-child creation below one `ValidatedLocalDirectory`;
2. link/reparse/directory/unexpected-type rejection;
3. create-new staging with collision-safe bounded retry;
4. file flush, reopen, exact length, and digest verification;
5. same-volume write-through move;
6. `ReplaceFileW` replacement with the old target saved to an exact backup path;
7. injected failure before/after each move and replacement boundary;
8. source/destination preservation when replacement fails;
9. fixed redacted errors with no OS/path text;
10. a child-process crash fixture proving the target is always old or new, never a
    partially copied mixture.

### Green

1. Add sealed `DurableStagedFile` and fixed-purpose replace/move APIs; accept only
   descriptors created from a validated parent and fixed child name policy.
2. On Windows, use `ReplaceFileW` for existing-target replacement and
   `MoveFileExW(MOVEFILE_WRITE_THROUGH)` for same-volume moves. Do not set
   `MOVEFILE_COPY_ALLOWED`.
3. Flush file contents before publication and reopen after publication for exact
   verification.
4. Keep non-Windows behavior testable but do not claim the Windows release guarantee
   from it.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-platform --test durable_file_contract --locked
cargo +1.97.0 test -p tokenmaster-platform --locked
```

Completion evidence: strict platform Clippy passes; 9 library and 11 durable-file
integration contracts pass. Windows crash evidence covers 20 handshake-kills before
publication, 20 after verified publication, and 20 additional race kills immediately
after entering the replacement call. Every round retains a complete old or new target;
published rounds retain the exact old backup. Independent Sol High review found no
Critical issue and required post-publication failures to become unambiguously
`RecoveryRequired`; the focused regressions enforce that contract and the final
read-only review reports no remaining Critical or Important finding.

**Commit:** `feat(platform): add durable file replacement`

---

## Task 3 — Implement redundant bounded records (complete)

**Files:**

- Create: `crates/state/src/record.rs`
- Create: `crates/state/src/record_contract_tests.rs` (`cfg(test)` only, so the
  generic filesystem authority remains crate-private)
- Modify: `crates/state/src/lib.rs`
- Modify: `crates/platform/src/durable_file.rs`
- Modify: `crates/platform/src/windows.rs`
- Modify: `crates/platform/src/unix.rs`
- Modify: `crates/platform/src/unsupported.rs`
- Modify: `crates/platform/src/bin/durable_file_fixture.rs`
- Modify: `crates/platform/tests/durable_file_contract.rs`
- Modify: `scripts/audit-reliable-state.ps1`
- Modify: `scripts/tests/audit-reliable-state.Tests.ps1`

### Red

Add table-driven tests for:

1. exact magic/version/header/payload-length/SHA-256 validation;
2. choosing the highest valid A/B generation;
3. newest-slot first-byte/middle/footer corruption falling back to the older slot;
4. truncation at every header boundary;
5. duplicate/trailing bytes, invalid UTF-8, oversized JSON, and unknown required
   version rejection;
6. checked generation overflow leaving both slots unchanged;
7. process interruption before staging flush, before publish, and after publish;
8. both slots invalid returning typed `NoValidRecord`, never a partial payload;
9. redacted `Debug` and errors.

### Green

1. Define one fixed record envelope reused by settings, run state, and recovery
   journal. Cap payload at the caller-supplied hard limit; do not allocate from the
   declared length before checking it.
2. Save to the inactive slot through `DurableStagedFile`, verify, publish, then read
   both slots again to prove the selected generation.
3. Expose typed `RedundantRecordStore<T>` only inside `tokenmaster-state`; do not
   expose raw paths or JSON bytes.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --lib --locked -- --test-threads=1
```

Completion evidence: the fixed 64-byte header, strict JSON payload, 40-byte footer,
checked generation, payload SHA-256, and record SHA-256 are implemented behind a
crate-private typed store. Save uses a bounded measure pass followed by direct
256 KiB-chunk staging without retaining a full encoded payload. Thirteen unit
contracts cover every header field, corrupt/truncated/malformed input, equal-generation
conflicts, overflow, nondeterministic serialization, post-publication readback failure,
and process death at three exact phases of a third-generation replacement. Platform
evidence adds bounded reads, inactive-slot replacement without a third backup, an
injected before/after OS boundary, 40 deterministic redundant-replacement kills, and
20 replacement-entry race kills. The authority audit passes 33 mutation cases and
permits only six literal record children plus the bounded writer error/result surface.
Independent final review reports no Critical or Important finding; same-user no-follow
open/handle identity validation remains a documented non-blocking hardening item.

**Commit:** `feat(state): add redundant durable records`

---

## Task 4 — Add the settings schema, store, and config preview (complete)

**Files:**

- Create: `crates/state/src/settings/mod.rs`
- Create: `crates/state/src/settings/value.rs`
- Create: `crates/state/src/settings/store.rs`
- Create: `crates/state/src/settings/migration.rs`
- Create: `crates/state/src/settings/preview.rs`
- Create: `crates/state/tests/settings_contract.rs`
- Modify: `crates/state/src/lib.rs`

### Red

Add tests for:

1. exact settings schema v1 and `deny_unknown_fields`;
2. 1 MiB payload cap, bounded strings/lists, exact enum/range checks, duplicate
   reminder threshold rejection, and relationship validation;
3. portable versus device-local versus forbidden fields;
4. current/older valid slot selection and corrupt-newest fallback;
5. both slots invalid loading safe defaults while preserving invalid files;
6. a typed bounded import preview containing only changed field categories and counts;
7. unsupported newer version and malformed older version writing nothing;
8. all-or-nothing publish, generation overflow, and injected save failures;
9. password/path/credential/source-content canaries absent from serialized settings,
   errors, `Debug`, and preview;
10. a full-backup settings candidate preserving only portable fields and never
    restoring device-local state.

### Green

1. Implement only current real settings plus backup policy. Add presentation fields
   when P4 owns them; do not persist placeholders.
2. Define explicit schema migration functions even though v1 has no predecessor.
3. Implement `SettingsStore::load`, `preview_import`, and `commit_import` over the
   redundant record store.
4. Return `SettingsLoadOutcome::{Current,Fallback,Defaults}` and a stable health code.
5. Expose an exact staged portable-settings target generation/digest for the restore
   journal; publication must be idempotent and independently reread-verifiable.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --test settings_contract --locked
```

**Commit:** `feat(state): persist versioned settings`

---

## Task 5 — Add SQLite snapshot and candidate verification primitives (complete)

**Files:**

- Create: `crates/store/src/backup.rs`
- Create: `crates/store/tests/backup_contract.rs`
- Create: `crates/store/tests/backup_adversarial_contract.rs`
- Modify: `crates/store/src/lib.rs`
- Modify: `crates/store/src/error.rs`
- Modify: `crates/store/Cargo.toml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

### Red

Add tests proving:

1. a committed WAL-only row appears in a snapshot while a copied main file does not;
2. Online Backup produces a standalone reopenable database during concurrent bounded
   writes;
3. page-stepped copy honors cancellation/deadline and cleans the destination;
4. busy/locked retry is bounded and all other failures fail immediately;
5. candidate verification applies defensive mode, trusted schema off, DQS off,
   `cell_size_check`, zero mmap, exact bundled SQLite identity, and no checkpoint on
   close;
6. `integrity_check`, `foreign_key_check`, exact schema SQL, and application semantic
   checks are all independently required;
7. header, page, index, FK, schema, count, generation, and semantic corruption are
   rejected with distinct stable categories;
8. old supported schema can be inspected without migration and newer schema is
   classified without mutation;
9. `VACUUM INTO` runs only from an isolated snapshot, returns a smaller/equal valid
   candidate, and a cancelled/failed output is never accepted;
10. no store error includes SQLite text or a path.

### Green

1. Enable the pinned `rusqlite` `backup` and compile-only `limits` features in
   addition to current features; the latter closes untrusted candidate allocation
   bounds identified by independent review.
2. Add `create_online_snapshot`, `create_compact_snapshot`, `inspect_archive_version`,
   and `verify_backup_candidate` fixed APIs. Accept sealed controlled files rather than
   arbitrary SQL or output names.
3. Reuse exact existing runtime/schema policy constants and semantic validators; do
   not duplicate a weaker schema definition.
4. Map corruption, schema-too-new, schema-mismatch, foreign-key, semantic, busy,
   cancelled, deadline, and I/O failures to fixed typed results.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-store --test backup_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test backup_adversarial_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

**Commit:** `feat(store): add verified sqlite snapshots`

---

## Task 6 — Implement the fixed `.tmconfig` and `.tmbackup` container

**Files:**

- Create: `crates/state/src/package/mod.rs`
- Create: `crates/state/src/package/header.rs`
- Create: `crates/state/src/package/manifest.rs`
- Create: `crates/state/src/package/reader.rs`
- Create: `crates/state/src/package/writer.rs`
- Create: `crates/state/tests/package_contract.rs`
- Create: `crates/state/tests/package_adversarial_contract.rs`
- Modify: `crates/state/Cargo.toml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

### Red

Add fixture and mutation tests for:

1. deterministic v1 header/manifest/entry/footer vectors;
2. settings-only and settings-plus-database packages;
3. at most eight exact typed entries, 64 KiB manifest, 1 MiB settings, 64 GiB
   database, and checked total size;
4. SHA-256 for expanded entries and manifest/descriptor binding, plus a footer end
   marker and SHA-256 of every preceding package byte;
5. one Zstandard frame per entry, checksum/content-size enabled, one thread, 8 MiB
   window, and exact levels 6/12/19;
6. byte flips and truncation at every structural boundary;
7. false lengths, integer overflow, duplicate/unknown entries, unknown flags,
   concatenated/trailing frames, missing frame end, wrong checksum, and wrong digest;
8. decoder-window excess and decompression-bomb output stopped by the independent
   byte counter;
9. no filename, path, link, permission, device, credential, prompt, response, command,
   or source-content field in the format;
10. streaming a large synthetic database without a full-size allocation.

### Green

1. Pin `zstd = 0.13.3` with `default-features = false`; do not enable `zstdmt`,
   dictionary training, legacy formats, or experimental features.
2. Implement exact little-endian parsing with checked arithmetic and fixed maximum
   buffer sizes.
3. Stream entries directly between controlled files and finish/reopen the encoder.
4. Write and verify the exact whole-package footer without buffering the package or
   making the digest self-referential.
5. Expose typed `ConfigPackage` and `BackupPackage` readers, not a generic extractor.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --test package_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test package_adversarial_contract --locked
cargo +1.97.0 tree -p tokenmaster-state -e features
```

**Commit:** `feat(state): add bounded backup packages`

---

## Task 7 — Add optional age passphrase protection

**Files:**

- Create: `crates/state/src/package/encryption.rs`
- Create: `crates/state/tests/encryption_contract.rs`
- Modify: `crates/state/src/package/mod.rs`
- Modify: `crates/state/Cargo.toml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `scripts/audit-reliable-state.ps1`
- Modify: `scripts/tests/audit-reliable-state.Tests.ps1`

### Red

Add tests for:

1. age v1 passphrase round-trip around a valid complete `.tmbackup` stream;
2. fixed scrypt `log_n = 16` on export and maximum accepted `log_n = 16` on import;
3. malicious higher work factor rejected before expensive derivation;
4. wrong password, corrupt header/body/footer, truncation, and trailing data;
5. plaintext staging cleanup on every encryption failure;
6. passphrase canaries absent from serialized files, errors, `Debug`, health, and
   process arguments/environment;
7. secret wrapper/drop and caller-owned UI buffer clearing;
8. automatic-backup APIs rejecting encrypted mode;
9. new passphrases accepting exactly 12 through 128 Unicode scalar values only,
   requiring exact confirmation, and performing no trim or normalization.

### Green

1. Pin `age = 0.12.1` with `default-features = false`; do not enable CLI, plugin,
   SSH, armor, or async features.
2. Construct the scrypt recipient explicitly and set work factor 16. Construct the
   identity explicitly and set maximum work factor 16 before decrypt.
3. Stream the complete TokenMaster package through age; do not invent cryptographic
   primitives or store a recovery password.
4. Validate confirmation and scalar-count bounds before scrypt work, then clear both
   caller-owned UI inputs immediately after conversion to the secret wrapper.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --test encryption_contract --locked
pwsh -NoProfile -File scripts/audit-reliable-state.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 tree -p tokenmaster-state -e features
```

**Commit:** `feat(state): protect manual backups with age`

---

## Task 8 — Add self-describing catalog and bounded retention

**Files:**

- Create: `crates/state/src/catalog.rs`
- Create: `crates/state/src/retention.rs`
- Create: `crates/state/tests/catalog_contract.rs`
- Create: `crates/state/tests/retention_contract.rs`
- Create: `crates/platform/src/backup_directory.rs`
- Create: `crates/platform/tests/backup_directory_contract.rs`
- Modify: `crates/platform/src/durable_file.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/src/windows.rs`
- Modify: `crates/state/src/lib.rs`
- Modify: `crates/state/src/package/capability.rs`
- Modify: `crates/state/src/package/mod.rs`
- Modify: `crates/state/src/package/reader.rs`
- Modify: `crates/state/src/package/writer.rs`
- Modify: `scripts/audit-reliable-state.ps1`
- Modify: `scripts/tests/audit-reliable-state.Tests.ps1`

### Red

Add tests for:

1. rebuilding a catalog from package headers when a cache is missing/corrupt;
2. maximum 32 relevant files and rejection of links, directories, duplicates, and
   ambiguous identities;
3. deterministic four-newest/seven-daily/four-weekly selection;
4. fifteen-retained and 2 GiB defaults with an exact checked configurable range from
   256 MiB through 64 GiB;
5. protecting the newest two verified backups and a pre-migration point;
6. unpinning pre-migration only after a verified post-migration point;
7. retention running only after new backup publication;
8. capacity failure that deletes no protected/known-good backup;
9. interruption on every deletion leaving the next scan deterministic;
10. catalog values exposing ordinal/time/size/type/health only, never a path or raw
    digest through public `Debug`.
11. the platform-owned backup-directory capability exposing at most 32 exact
    TokenMaster package children, rejecting links/reparse points/unexpected names and
    types, and permitting only bounded reader/create-stage/publish/delete operations;
12. state catalog/retention code having no direct directory enumeration, path, or
    generic filesystem authority;
13. the production sequence writing a typed package, fully verifying that exact
    sealed unpublished stage, completing no-delete admission, publishing, binding,
    and confirming the same proof;
14. same-length corruption of the candidate, selected deletion target, or any other
    current verified/protected point blocking all deletion until rebuild/replan;
15. mixed source/destination failure precedence and discarded-stage error
    classification matching the existing durable package contract.

### Green

1. Treat each package as authoritative for its own metadata; keep any catalog index
   disposable.
2. Use a checked catalog generation plus bounded ordinal as the UI selection token.
3. Delete only exact validated files inside `backups/`, one at a time, after candidate
   publication and selection recomputation.
4. Add a sealed platform `BackupDirectory` capability whose public values are opaque
   child tokens, never names or paths. It owns bounded enumeration and exact deletion;
   state receives only `DurableFileReader`, staged publication, length, time/type, and
   ordinal facts needed by the typed package/catalog contract.
5. Use exactly 32 private slot children (`point-00.tmbackup` through
   `point-31.tmbackup`). Names never encode time, purpose, profile, identity, or user
   data and never cross the platform boundary. A token is bound to its directory,
   slot, physical identity, and observed length; open/delete revalidate that binding.
6. Distinguish `header_valid` from `verified`. Cold cache rebuild validates the exact
   fixed header/manifest without inventing a prior full-package/SQLite proof. Only a
   point bound to current full verification evidence is known-good or deletion-
   eligible; corrupt/unavailable/unchecked points are preserved for later maintenance.
7. Define retention in UTC. Select protected points first, then four newest distinct
   points, then the newest remaining point per distinct UTC calendar day, then the
   newest remaining point per distinct ISO-8601 UTC week. Daily/weekly tiers add at
   most seven/four distinct points and stop at the shared fifteen-point cap; a pinned
   pre-migration point consumes one of those fifteen slots.
8. Split retention into a no-delete preflight over the proposed verified candidate and
   a post-publication pass. Capacity failure before publication deletes nothing. After
   publication, recompute the exact catalog generation and expose only the next
   oldest unprotected deletion; after every deletion rebuild/replan, so interruption
   is equivalent to applying a deterministic prefix and never creates a batch gap.
9. Keep `BackupStagedFile` sealed: it exposes only bounded write/seal/discard and a
   path-free reader after seal. `BackupPackage::verify_backup_stage` fully parses the
   same unpublished bytes before admission; `BackupDirectory::publish` rechecks the
   stage seal before moving it into the exact slot.
10. Before planning any deletion, stream and revalidate every point currently marked
    `Verified`; then revalidate the exact selected target again immediately before the
    physical deletion. Any changed proof returns `RecoveryRequired` without deleting.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --test catalog_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test retention_contract --locked
```

**Commit:** `feat(state): retain verified restore points`

---

## Task 9 — Build the capacity-one backup maintenance runtime

**Files:**

- Create: `crates/state/src/maintenance/mod.rs`
- Create: `crates/state/src/maintenance/coordinator.rs`
- Create: `crates/state/src/maintenance/worker.rs`
- Create: `crates/state/src/maintenance/scheduler.rs`
- Create: `crates/state/tests/maintenance_contract.rs`
- Create: `crates/state/tests/maintenance_resource_contract.rs`
- Modify: `crates/store/src/backup.rs`
- Modify: `crates/store/src/lib.rs`
- Modify: `crates/store/tests/backup_contract.rs`
- Modify: `crates/state/Cargo.toml`
- Modify: `crates/state/src/lib.rs`

### Red

Add deterministic tests for:

1. one active request and one merged follow-up under 10,000 hints;
2. urgency order: pre-migration/restore > manual > suspect retry > periodic;
3. five-minute quiet time, six-hour minimum ordinary period, and one catch-up after
   resume/clock rollback;
4. no automatic backup before first healthy publication or from suspect state;
5. one automatic source-failure retry and source-suspect escalation only after two
   failures against the same identity;
6. cooperative cancellation before publication and non-cancellable final publish;
7. pause/resume/shutdown/drop joining the one worker and scheduler;
8. no queue/list proportional to triggers and one latest bounded health snapshot;
9. repeated success/failure/cancel cycles returning handles/threads and private memory;
10. zero UI/Slint dependency and zero timer per backup;
11. disabling periodic backups suppressing only quiet-time/six-hour work while
    mandatory pre-migration, pre-restore, and pre-destructive safety points remain;
12. failure of a mandatory healthy-source safety point blocking the mutation, while
    a brand-new empty installation and definitively corrupt quarantined source follow
    their distinct explicit paths.
13. the verified SQLite candidate opening only a sealed, path-free, identity-bound
    streaming reader; replacement/truncation/append between verification and package
    completion fails stale/integrity and poisons package output.

### Green

1. Reuse engine constant-state coordination patterns where dependency direction
   permits; otherwise implement the same checked one-active/one-follow-up invariant
   locally without adding an async runtime.
2. Execute snapshot -> verify -> package -> verify -> publish -> retain as one owned
   operation. Never retain progress history.
3. Publish fixed phase/progress/last-success/failure/count/byte health only.
4. Keep a failed post-migration backup degraded and the pre-migration point pinned;
   do not report that a completed database migration rolled back.
5. Add a store-owned verified-candidate reader and one explicit state/store interop
   method over the existing private package stream codec. Do not make state accept a
   path or generic `Read`, do not copy the candidate into memory, and revalidate the
   verified physical identity/length/SHA-256 before and after package consumption.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --test maintenance_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test maintenance_resource_contract --locked
```

**Commit:** `feat(state): schedule bounded backups`

---

## Task 10 — Implement the durable restore journal and quarantine

**Implementation status (2026-07-18):** implemented with focused recovery, crash,
authority, and strict-Clippy evidence. Final independent review is Critical 0,
Important 0, Minor 0 and `Ready`. The final clean-root, formatting, strict locked
workspace Clippy, complete locked workspace test/doctest, reliable-state audit, 52/52
mutation, and changed-platform MSVC target gates pass. Task 10 is accepted as a
developer library milestone; product release remains unclaimed.

**Files:**

- Create: `crates/platform/src/archive_recovery.rs`
- Create: `crates/platform/tests/archive_recovery_contract.rs`
- Modify: `crates/platform/src/lease.rs`
- Modify: `crates/platform/src/durable_file.rs`
- Modify: `crates/platform/src/lib.rs`
- Create: `crates/store/tests/recovery_verification_contract.rs`
- Modify: `crates/store/src/backup.rs`
- Modify: `crates/store/src/lib.rs`
- Create: `crates/state/src/recovery/mod.rs`
- Create: `crates/state/src/recovery/journal.rs`
- Create: `crates/state/src/recovery/restore.rs`
- Create: `crates/state/tests/recovery_journal_contract.rs`
- Create: `crates/state/tests/restore_contract.rs`
- Create: `crates/state/tests/support/recovery_crash_fixture.rs`
- Modify: `crates/state/Cargo.toml`
- Modify: `crates/state/src/lib.rs`
- Modify: `crates/state/src/package/capability.rs`
- Modify: `crates/state/src/package/reader.rs`
- Modify: `crates/state/src/settings/store.rs`
- Modify: `scripts/audit-reliable-state.ps1`
- Modify: `scripts/tests/audit-reliable-state.Tests.ps1`

### Red

Add failure-injection and child-process tests for every phase:

1. `prepared` journal durable before any active-file mutation;
2. current WAL/SHM moved into one exact quarantine set or absence recorded;
3. active main replaced only after sidecar quarantine;
4. replaced main saved in the same quarantine set;
5. active reopen/full verification before any settings publication;
6. restart at every transition resumes idempotently before SQLite open;
7. replacement failure restores sidecars and leaves old main active;
8. post-replacement validation failure performs staged rollback;
9. forward and rollback uncertainty enters safe mode without deleting artifacts;
10. one corrupt journal slot falls back; both invalid plus artifacts never guesses;
11. candidate digest mismatch, stale catalog generation, and changed active identity
    abort before replacement;
12. maximum three quarantine sets; a fourth request changes nothing;
13. no arbitrary filenames, traversal, links, path-bearing errors, or auto-deletion;
14. old-or-new complete main after forced termination, never mixed content;
15. an existing main using `ReplaceFileW`, a missing damaged main using same-volume
    write-through `MoveFileExW`, and a brand-new directory using neither recovery path;
16. manual data-only restore preserving all settings, manual data-plus-portable-
    settings restore never changing device-local settings, and automatic recovery
    always selecting data only;
17. settings publication failure rolling the database back while the prior settings
    generation remains selected;
18. a crash after durable settings publication but before journal advance detecting
    the exact target generation/digest and completing without a duplicate generation.
19. crashes after sidecar quarantine or atomic main promotion but before journal
    advance resuming from the exact already-completed mutation;
20. abandoned pre-journal recovery stages bounded globally, removed only on exact
    journal absence/completion, and unexpected staging evidence preserved;
21. store-verifier and first-journal-slot process death, repeated independent restore
    generations, fixed backup-slot resume after catalog rebuild, and settings-record
    publication ambiguity;
22. physical lock-file substitution, post-reservation directory collision, missing-
    main recovery from a verified backup, healthy-main corruption rejection, native
    replacement ambiguity, and rollback continuation from staged intermediate facts.

### Green

1. Bind `ExclusiveFileLeaseGuard` privately to the exact active archive scope and
   physical locked sidecar identity; a guard from another or namespace-substituted
   archive must fail before observation or mutation.
2. Add one sealed platform recovery scope that owns only the fixed active main/WAL/
   SHM, staging, and quarantine namespaces. It generates checked opaque operation IDs,
   uses create-new reservation, exposes path-free observations/readers/stages, rejects
   links and unexpected entries, and retains at most three never-auto-deleted sets.
3. Add a store-owned recovery verifier that copies a bounded platform reader into the
   existing controlled candidate namespace and applies complete SQLite, schema,
   foreign-key, count/generation, and semantic checks. Its public proof contains only
   schema version, length, and SHA-256.
4. Require promotion and active reopen to match the exact sealed-stage/store proof;
   no raw path, generic `Read`, SQL connection, or caller-selected child crosses into
   state.
5. Implement exact states `prepared`, `sidecars_quarantined`, `main_replaced`,
   `reopened_verified`, `settings_published`, and `complete` in the redundant record
   store. A data-only restore records an explicit settings no-op before entering
   `settings_published`.
6. Record fixed main/WAL/SHM presence, lengths, and digests needed to prove an
   idempotent forward or rollback step without recording a path.
7. Derive all controlled names from one bounded opaque operation ID generated with
   checked time/process/counter material and `create_new` collision retry; do not
   accept names from a package or UI.
8. Require the caller to present an already-held `ExclusiveFileLeaseGuard` for the
   fixed active archive before mutation.
9. Add prepared settings publication: compute and journal the exact next generation
   and portable digest before mutation, then publish or verify that same target
   idempotently without changing device-local state.
10. Keep recovery code independent of runtime/UI packages.
11. Record the selected settings mode and optional staged settings target generation/
   digest in the path-free journal. Conflicting settings state fails to safe mode.
12. Reinvoke the exact integration-test executable for crash-phase fixtures; do not add
    a `tokenmaster-state` binary target or an auto-discovered `src/bin` surface.
13. Cap the complete recovery-staging namespace at three exact artifacts, preflight
    actual free space for `max(2B, B+A) + 8 MiB`, enforce the same cap in platform and
    store, authorize the physical guard before any cleanup, remove only
    recognized abandoned stages after journal absence/completion, and resume an already-promoted
    candidate without allocating another inert stage.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --test recovery_journal_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test restore_contract --locked
cargo +1.97.0 test -p tokenmaster-platform --test archive_recovery_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test recovery_verification_contract --locked
```

**Commit:** `feat(state): restore through durable journal`

---

## Task 11A — Add run markers, startup diagnosis, and verified-backup recovery

**Implementation status (2026-07-18):** complete for the state/store/runtime boundary.
The delivered slice publishes the unclean marker before archive access, performs
read-only normal/quick diagnosis, resumes a journal before ordinary SQLite open,
reverifies backups newest-first, limits one recovered candidate to two failed launches,
distinguishes first install from missing damaged state, and hands one continuously held
writer guard into `LiveRuntime`. The original items for no-backup reconstruction and
pre/post-migration safety points require the application-owned maintenance/provider
lifecycle and are explicitly reassigned to Task 12 below. Until Task 12 lands, no valid
backup returns `RecoveryRequired` with the corrupt set preserved; it never fabricates
empty truth.

**Files:**

- Create: `crates/state/src/bootstrap.rs`
- Create: `crates/state/src/run_state.rs`
- Create: `crates/state/tests/bootstrap_contract.rs`
- Create: `crates/state/tests/automatic_recovery_contract.rs`
- Modify: `crates/state/src/lib.rs`
- Modify: `crates/runtime/src/lease.rs`
- Modify: `crates/runtime/src/live.rs`
- Modify: `crates/runtime/src/lib.rs`
- Modify: runtime live/startup tests

### Red

Add tests for:

1. `unclean` published and reread before first SQLite open, write/verification failure
   entering safe mode without a writable archive, and `clean` only after joined
   shutdown;
2. clean startup using header/schema/semantic checks without full live integrity scan;
3. unclean/missing/invalid run state adding `quick_check`;
4. pending recovery resumed before any SQLite open;
5. corruption selecting the newest fully reverified backup, skipping corrupt newer
   candidates;
6. no automatic restore for busy, permission, disk full, unsupported location,
   transient I/O, or schema-too-new;
7. same restored candidate failing two launches and then entering safe mode;
8. no valid backup returning `RecoveryRequired` without mutating or discarding the
   corrupt set;
9. one continuously held writer guard across state preflight and live runtime startup,
   with no unlock race;
10. legacy `LiveRuntime::start` callers retaining behavior;
11. no main plus no prior durable artifacts using normal first-install schema creation;
12. no main plus prior run/settings/backup/recovery artifacts classifying damage and
    using recovery rather than silently creating empty truth;
13. periodic-backup disablement leaving every mandatory safety point active.

### Green

1. Add `StateBootstrap::prepare` that receives the controlled data root and held
   platform writer guard, diagnoses state, resumes recovery, optionally restores, and
   returns `Healthy`, `RebuildRequired`, or `SafeMode` plus fixed receipts.
2. Add a guarded LiveRuntime start path accepting the already-held platform guard.
   It constructs the existing `RuntimeWriterLease` for later operations, performs
   archive open and existing startup recovery under the same guard, then releases that
   startup guard. Later writes reacquire the same fixed lease per operation. Preserve
   existing start constructors as wrappers.
3. Keep schema-too-new intact and report upgrade required.
4. Distinguish first install from missing damaged state using bounded validated
   TokenMaster-owned durable artifacts, never the absence of the main file alone.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --test bootstrap_contract --locked
cargo +1.97.0 test -p tokenmaster-state --test automatic_recovery_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --locked
```

**Commit:** `feat(state): recover before live startup`

---

## Task 12 — Integrate application-owned recovery, migration, and service restart

**Status:** Implemented through the application/UI boundary. Task 12A implements items 1-4, 10, and 14-17 plus the concrete
application backup operation. Task 12B.1 adds the bounded typed command admission core,
one active/one follow-up under 10,000 hints, exact cancellation/irreversible boundary,
controlled current-bundle restart, and obsolete-notifier suppression for items 7-9,
11, and 12. Task 12B.2a adds the identity-bound selected restore lifecycle in items 6-7:
current-directory binding, one deletion-serialized RAII pin, protected `PreRestore`,
joined old owners, journaled replacement, immediate run-session receipt binding,
restored-legacy pre/post-migration gates, and one fresh bundle or safe mode. Task
12B.2b.1 adds one joined capacity-one operation worker, real manual-backup execution,
the 2 MiB config codec ceiling, sealed create-new/reread export, and base-bound typed
import preview/commit. Task 12B.2b/Task 15 bind native selection, config/backup/verify/
restore/rebuild/retry/cancel/policy intents, exact irreversible operation phases, and
the no-backup reconstruction in items 5-9 and 13. Reconstruction completes one
mandatory recovery-urgency authoritative-source refresh before healthy publication and
reports non-reconstructible domains explicitly unavailable. This status is
implementation truth, not Task 16-18, interactive, M0, package, or release acceptance.

**Files:**

- Create: `crates/app/src/state.rs`
- Create: `crates/app/src/command.rs`
- Create: `crates/app/tests/state_composition_contract.rs`
- Create: `crates/app/tests/restore_lifecycle_contract.rs`
- Modify: `crates/app/src/data_root.rs`
- Modify: `crates/app/src/application.rs`
- Modify: `crates/app/src/application_tests.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/Cargo.toml`
- Modify: `Cargo.lock`

### Red

Add composition tests for:

1. exact `reliable-state` child tree creation and path-redacted `DataRoot`;
2. state bootstrap before live/query/runtime construction;
3. healthy startup owning exactly one maintenance runtime;
4. safe mode constructing a window without starting any archive/runtime/query owner;
5. typed config export/import, backup, verify, data-only restore, data-plus-portable-
   settings restore, rebuild, retry, and cancel commands through one capacity-one
   application command coordinator;
6. restore closing admission, shutting down controller/reminder/quota/live/nested-Git,
   proving no archive handle, acquiring the fixed lease, restoring, then rebuilding one
   fresh bundle;
7. any shutdown/restore/restart failure leaving either the old bundle or safe mode,
   never two runtime owners;
8. completion hints from an obsolete bundle unable to publish after restart;
9. settings-only live application and controlled service restart for runtime-affecting
   settings;
10. clean run marker only after backup worker, controller, and all runtimes join;
11. 10,000 UI command hints retaining one operation/follow-up;
12. all errors and observations path/private-data free;
13. no usable backup preserving the corrupt set while normal store code creates one
    fresh schema and the authoritative Codex bootstrap repopulates only reconstructible
    usage, with unavailable non-reconstructible domains reported explicitly;
14. a mandatory verified pre-migration backup before any writable old-schema open and
    one verified post-migration point after success;
15. migration failure preserving the old archive and its pinned pre-migration point;
16. periodic-backup disablement unable to suppress either migration safety point.
17. failure after migration commit but before post-migration publication retaining a
    durable pending-post obligation, with restart completing it before live or clean.

### Green

1. Extend `DataRoot` with sealed controlled subdirectories while preserving the fixed
   archive and writer sidecar.
2. Add one application state owner and typed command coordinator. Do not put state or
   filesystem authority in `tokenmaster-desktop`.
3. Refactor bundle construction into a reusable function with a checked bundle
   generation. Drop obsolete weak notifiers after restart.
4. Keep the Slint window alive across a healthy restore when possible; switch to safe
   mode if reconstruction fails.
5. Force automatic recovery to data-only regardless of package contents. Manual full
   restore must carry its confirmed settings mode into the journal; device-local
   settings are never a restore input.
6. Route no-backup reconstruction through the ordinary guarded store/runtime/provider
   composition; never add corrupt-row salvage or a state-layer provider dependency.
7. Gate every migration with the Task 9 mandatory-maintenance receipt, keep the
   pre-migration point pinned until a verified post-migration point exists, and enter
   safe mode on any ambiguous failure.
8. Persist the exact pending source/target schema pair before writable migration and
   clear it only after the verified post point; use one atomic exact-root maintenance
   submit/wait so another completion cannot overwrite the awaited receipt.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-app --test state_composition_contract --locked
cargo +1.97.0 test -p tokenmaster-app --test restore_lifecycle_contract --locked
cargo +1.97.0 test -p tokenmaster-app --locked
```

**Commit:** `feat(app): compose reliable state lifecycle`

---

## Task 13 — Publish bounded reliable-state health and choices

**Status:** Implemented at the Desktop projection boundary. The planned product-owned
snapshot was deliberately not added: reliable-state health must remain available in
safe mode without an archive-backed `ProductSnapshot`. Application maps sealed state
into one latest-only `DesktopReliableStateProjection` with at most fifteen ordinal
choices, scalar health/policy/receipt fields, and no file or recovery authority.

**Files:**

- Create: `crates/product/src/reliable_state.rs`
- Create: `crates/product/tests/reliable_state_contract.rs`
- Modify: `crates/product/src/snapshot.rs`
- Modify: `crates/product/src/reducer.rs`
- Modify: `crates/product/src/route.rs`
- Modify: `crates/product/src/lib.rs`
- Modify: `crates/app/src/application.rs`

### Red

Add tests for:

1. fixed healthy/degraded/suspect/recovering/safe-mode status;
2. last backup/verification/recovery UTC times, counts, compressed bytes, schedule,
   and stable failure code;
3. at most fifteen backup choices with catalog generation and ordinal only;
4. generation-ordered replacement and equal/older observation rejection;
5. last compatible healthy values retained during an operation failure;
6. corruption invalidating Data Health without zeroing independent Dashboard truth;
7. safe mode route readiness allowing only Data Health/Settings/Help operations;
8. no path, raw digest, archive ID, account/workspace/source identity, password, or OS
   error in public values/`Debug`.

### Green

1. Add immutable bounded `ReliableStateSnapshot` and `BackupChoice` values.
2. Add one independent reliable-state observation generation to the existing reducer;
   do not merge it with database dataset identity.
3. Map app-owned maintenance/recovery health into copied product values.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-product --test reliable_state_contract --locked
cargo +1.97.0 test -p tokenmaster-product --locked
```

**Commit:** `feat(product): publish reliable state health`

---

## Task 14 — Add sealed native import/export file selection

**Status:** Implemented below the application/UI boundary. The Windows Common Item
Dialog and deterministic controlled selector return only a bounded opened input or an
identity-bound staged output with exact type, no-follow/parent/link/type/path/size
validation and stable selected/cancelled/error outcomes. Existing replace captures and
post-checks displaced identity, rolls back a raced target, and retains Windows handle-
bound stage cleanup. The native selector is thread-affine and requires an active owner.
Task 15 now owns and implements application invocation, post-selection worker dispatch,
and visible preview/confirm. Interactive Windows evidence remains Task 17/release work.

**Files:**

- Create: `crates/platform/src/file_dialog.rs`
- Create: `crates/platform/tests/file_dialog_contract.rs`
- Modify: `crates/platform/src/windows.rs`
- Modify: `crates/platform/src/unsupported.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/Cargo.toml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

### Red

Add abstraction tests and a Windows manual/automated fixture for:

1. exact `.tmconfig`, `.tmbackup`, and `.tmbackup.age` filters;
2. open-existing versus create/replace output capabilities;
3. returned sealed descriptor implementing only the required read or write behavior;
4. UI-facing result containing selected/cancelled/stable-error only;
5. link/reparse, directory, device, network, and unsupported namespace rejection after
   selection;
6. output opened without truncating an existing target until a complete candidate is
   ready;
7. cancellation writing nothing;
8. redacted `Debug`/errors and no path persistence.

### Green

1. Use the Windows common file-dialog API behind `tokenmaster-platform`; do not invoke
   Explorer, PowerShell, `cmd`, or another process.
2. Return sealed `SelectedInputFile`/`SelectedOutputFile` capabilities to app
   composition only. Do not expose path strings to product/Desktop.
3. Keep a deterministic injected selector for tests and unsupported platforms.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-platform --test file_dialog_contract --locked
cargo +1.97.0 test -p tokenmaster-platform --locked
```

**Commit:** `feat(platform): add sealed backup file dialogs`

---

## Task 15 — Build Data & Recovery, settings import, and safe-mode UI

**Status:** Implemented for the P3-D.0 application/Desktop contour. Data Health and
Settings render bounded latest-only state, at most fifteen restore points, config
preview, exact operation phases, a destructive restore review, backup/rebuild controls,
and an explicit verified-backup versus authoritative-source recovery banner. Sealed
dialog capabilities are dispatched to the single joined worker; Slint receives no path,
file, state/store/runtime, SQLite, provider, or recovery authority. Passphrases clear
after admission, cancel is disabled during atomic promotion, and no UI timer/progress
queue was added. Hot English/Russian locale switching and interactive accessibility/
Windows acceptance remain P4/Task 17 rather than being claimed by this developer slice.
Independent review hardening binds confirmation to the exact previewed restore identity,
publishes queued follow-up running state at actual execution start, keeps manual backup
cancellable until its real irreversible boundary, preserves unavailable metrics as
typed unknowns, and makes source-reconciliation restart/retry durable.

**Files:**

- Create: `crates/desktop/src/reliable_state.rs`
- Create: `crates/desktop/ui/views/data-health-view.slint`
- Create: `crates/desktop/ui/views/settings-view.slint`
- Create: `crates/desktop/ui/components/backup-row.slint`
- Create: `crates/desktop/ui/components/operation-progress.slint`
- Create: `crates/desktop/ui/components/recovery-banner.slint`
- Create: `crates/desktop/tests/reliable_state_projection_contract.rs`
- Create: `crates/desktop/tests/recovery_ui_contract.rs`
- Modify: `crates/desktop/ui/models.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/presentation.rs`
- Modify: `crates/desktop/src/shell.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/app/src/application.rs`
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`

### Red

Add tests proving:

1. exact bounded projection of status, policy, latest times, count/bytes, failure, and
   at most fifteen restore points;
2. typed intent callbacks for export/import/preview/confirm, normal/compact/encrypted
   backup, verify, restore/confirm, rebuild, retry, cancel, and retention settings;
3. destructive restore requires a second confirmation containing age/size/quality but
   no filename/path, plus an explicit **Data only** or **Data + portable settings**
   choice;
4. passphrase and confirmation accept exactly 12 through 128 Unicode scalar values,
   are never stored in a Slint list/global model, and are both cleared after admission;
5. cancel disabled during atomic promotion and enabled only in cancellable phases;
   manual backup remains cancellable before its irreversible boundary;
6. safe mode renders Data Health/Settings/Help without a query controller;
7. section-local backup failure does not rebuild Dashboard models or window;
8. no SQLite, state/store/runtime/platform dependency or arbitrary path callback in
   `tokenmaster-desktop`;
9. no UI polling timer, progress-event queue, idle animation, or per-backup object
   retention;
10. English/Russian label keys, keyboard order, screen-reader names, high contrast,
    reduced-motion behavior, and narrow/wide layout hooks.
11. restore confirmation consumes the exact reviewed generation/ordinal after a newer
    projection arrives; unknown counts/bytes render unavailable, and a promoted queued
    operation publishes running at actual execution start.
12. process death after no-backup promotion but before source reconciliation forces the
    barrier on restart, while failed reconciliation retries without reconstructing the
    already promoted archive.

### Green

1. Add pure `DesktopReliableStateProjection` and bounded Slint models.
2. Wire callbacks to one injected application intent sink; the sink returns admission
   only and never blocks.
3. Render operation progress from newest immutable product snapshots.
4. Keep routes and semantic styling ready for P4 hot skin/locale application without
   embedding storage behavior in presentation.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test reliable_state_projection_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test recovery_ui_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --locked
Invoke-Pester -Path scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed
pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
```

**Commit:** `feat(desktop): add data recovery controls`

---

## Task 16 — Complete adversarial, privacy, and compatibility coverage

**Files:**

- Create: `crates/state/tests/fault_matrix_contract.rs`
- Create: `crates/app/tests/recovery_adversarial_contract.rs`
- Create: `scripts/audit-backup-package.ps1`
- Create: `scripts/tests/audit-backup-package.Tests.ps1`
- Modify: `scripts/audit-reliable-state.ps1`
- Modify: `scripts/tests/audit-reliable-state.Tests.ps1`
- Modify: `scripts/audit-application-composition.ps1`
- Modify: `scripts/tests/audit-application-composition.Tests.ps1`

### Red/Green matrix

1. Generate every header/manifest/entry/footer truncation and one-byte mutation.
2. Exercise decompression bombs, oversized Zstd windows, age work-factor abuse, wrong
   passwords, duplicate entries, trailing data, and new required versions.
3. Corrupt SQLite header, table pages, indexes, FKs, schema SQL, counters,
   generations, and semantic projections independently.
4. Exercise hot/missing/mismatched WAL/SHM and reject main-only copied backups.
5. Inject disk full, access denied, sharing violations, reparse children, unsupported
   media, ID/generation overflow, and catalog over-capacity.
6. Force process death at every settings, backup, retention, recovery, rollback, and
   clean-marker phase; rerun startup and assert deterministic outcome.
7. Cover current, every supported old schema, schema-too-new, migration failure,
   pre/post-migration backup identity, and old application rejection of newer package
   formats.
8. Search source, packages, errors, `Debug`, UI models, release binary, and synthetic
   exported archives for path/credential/prompt/response/command/source canaries.
9. Audit exact dependency/features/licenses and forbid external executables, generic
   extraction, network, shell, plugin, and UI authority drift.
10. Exercise first install versus missing damaged main, all six journal states,
    settings-publish rollback/resume, data-only automatic recovery, and mandatory
    safety points while periodic scheduling is disabled.

### Verify

```powershell
cargo +1.97.0 test -p tokenmaster-state --test fault_matrix_contract --locked
cargo +1.97.0 test -p tokenmaster-app --test recovery_adversarial_contract --locked
Invoke-Pester -Path scripts/tests/audit-backup-package.Tests.ps1 -Output Detailed
pwsh -NoProfile -File scripts/audit-backup-package.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts/audit-reliable-state.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts/audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path
```

**Commit:** `test(state): close backup fault matrix`

---

## Task 17 — Prove responsiveness and resource return

**Files:**

- Create: `crates/state/tests/backup_performance_contract.rs`
- Create: `crates/state/tests/recovery_resource_contract.rs`
- Create: `crates/app/tests/backup_ui_latency_contract.rs`
- Create: `P3D0_ACCEPTANCE.md`

### Red

Create ignored/release-mode gates for:

1. automatic/normal/compact snapshot throughput on deterministic database sizes;
2. one-thread compression and maximum 8 MiB decoder window;
3. no full-database allocation, with sampled private-memory high-water evidence;
4. 10,000 triggers retaining one active/follow-up operation;
5. 256 repeated backup/verify/import-cancel cycles returning private memory, handles,
   threads, USER, and GDI objects to the post-warm-up envelope;
6. forced failure and recovery cycles leaving no child/process/thread/file handle;
7. Dashboard cached-query p95 and measured input-to-paint p95 increasing by no more
   than 10 ms while automatic backup runs;
8. hibernation/resume coalescing one due catch-up without schedule burst;
9. retention and staging bytes plateauing under repeated backups;
10. encrypted/manual compact memory being temporary and released after completion.

### Green

Tune bounded page steps, yields, buffer sizes, and maintenance scheduling only from
measured evidence. Do not increase UI queues, compression threads, Zstd window, or
steady retained caches to pass throughput.

Create a separate P3-D.0 acceptance contract whose receipts bind the exact full
commit, `dirty=false`, executable SHA-256, schema/container/settings versions,
deterministic fixture identity, command, duration, latency, private-memory high-water/
return, handle/thread/USER/GDI return, disk plateau, and individual gate result. The
contract must distinguish developer evidence from M0 and product-release acceptance.

### Verify

```powershell
$arguments = @('+1.97.0', 'test', '-p', 'tokenmaster-state', '--test', 'backup_performance_contract', '--release', '--locked', '--', '--ignored', '--nocapture')
& cargo @arguments
cargo +1.97.0 test -p tokenmaster-state --test recovery_resource_contract --release --locked
cargo +1.97.0 test -p tokenmaster-app --test backup_ui_latency_contract --release --locked
```

Do not convert these developer measurements into M0 or release acceptance.
Do not modify `M0_ACCEPTANCE.md` or `scripts/verify-m0.ps1`; P3-D.0 evidence is a
separate non-release acceptance rail.

**Commit:** `perf(state): verify backup resource bounds`

---

## Task 18 — Close documentation, traceability, and the full gate

**Files:**

- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/RECOVERY_PLAYBOOK.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `P3D0_ACCEPTANCE.md`
- Modify: `third_party/NOTICE.md` or the repository's actual notices/SBOM inputs for
  new pinned dependencies

### Closure review

1. Change `planned` traceability rows to `implemented` only for gates that actually
   pass.
2. Record exact package versions/features/licenses and measured resource/latency
   evidence.
3. Document operator recovery without exposing real paths or suggesting manual row
   edits, main-only copies, lock deletion, or automatic `.recover`.
4. Confirm P3-D.0 completion does not claim remaining P3-D/P3-E, P4, P5, activation,
   M0 acceptance, packaging, signing, or release.
5. Run the focused audits, then the complete baseline.

### Full verification

```powershell
pwsh -NoProfile -File scripts/audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts/audit-reliable-state.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts/audit-backup-package.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts/audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
cargo +1.97.0 test --workspace --doc --locked
cargo +1.97.0 build -p tokenmaster-app --release --locked
```

After all commands pass, inspect:

```powershell
git status --short
git diff --check
git diff --stat
```

**Commit:** `docs(state): close reliable state milestone`

---

## Final acceptance checklist

P3-D.0 is complete only when all are true:

- settings survive one corrupt slot and interrupted save;
- `.tmconfig` import is strict, previewed, and atomic;
- live backups use Online Backup and include WAL-committed truth;
- every published backup passes container, hash, integrity, FK, schema, and semantic
  validation;
- the package footer binds every preceding byte and rejects any one-byte structural
  mutation or trailing data;
- automatic and compact compression stay streaming and bounded;
- optional manual encryption is interoperable age v1 with fixed work bounds and exact
  12-through-128-scalar confirmed passphrases;
- catalog loss is rebuildable and retention is bounded by count and bytes;
- retention enforces the exact 256 MiB-through-64 GiB configurable range;
- restore preserves current main/WAL/SHM in quarantine and resumes every crash phase;
- an existing main, a missing damaged main, and a brand-new install take distinct
  deterministic promotion/creation paths;
- manual restore offers data only or data plus portable settings, automatic recovery
  is always data only, and device-local settings are never restored;
- database/settings publication either commits both selected truths or rolls back the
  database while retaining the prior settings generation;
- disabling periodic backups cannot disable mandatory healthy-source safety points;
- inability to publish the pre-open `unclean` marker enters safe mode without opening
  a writable archive;
- busy, access denied, disk full, transient I/O, and schema-too-new never trigger
  destructive recovery;
- corrupt current state restores the newest reverified candidate or rebuilds with
  explicit data-loss truth;
- a restored candidate cannot create an infinite crash loop;
- safe mode works without runtime/query ownership;
- Data & Recovery UI never blocks or receives a path/SQLite/state authority;
- 10,000 hints remain capacity one;
- repeated operations return memory/handles/threads and meet UI/query latency gates;
- privacy/dependency/release audits and the complete locked workspace gate pass;
- project truth remains honest about unfinished UI, M0, packaging, signing, and
  release evidence.
