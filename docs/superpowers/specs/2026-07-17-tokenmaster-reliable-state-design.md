# TokenMaster Reliable State, Backup, and Recovery Design

Status: approved for planning by the operator's request to re-audit the logic and
record the detailed implementation rail.
Date: 2026-07-17.

## 1. Decision

TokenMaster will add one bounded reliable-state contour before the remaining P3-D
routes. It owns versioned settings, portable configuration export/import, verified
full backups, automatic retention, startup corruption containment, deterministic
restore, quarantine, and explicit Data Health truth.

The selected design keeps the implemented fixed live archive path and writer lease:

- `tokenmaster.sqlite3` remains the sole active database;
- `tokenmaster.sqlite3.tokenmaster-writer.lock` remains the stable process-wide
  mutation guard;
- live backups use SQLite's Online Backup API rather than copying the main file;
- restore uses a validated standalone candidate, a redundant durable operation
  journal, WAL/SHM quarantine, and same-volume atomic file replacement;
- every imported file is untrusted and is revalidated before and after promotion;
- corruption never becomes fabricated zero, a silent empty database, or permission
  to discard the damaged files.

The contour is named **P3-D.0 Reliable State**. It precedes ordinary History,
Sessions, Models, Projects, Activity, Settings, and Data Health route completion
because those surfaces otherwise create settings and migration state without a
durable recovery contract.

## 2. Existing facts and the missing boundary

The current implementation already has:

- bundled SQLite 3.53.2 through `rusqlite = 0.40.1`;
- file-backed WAL, `synchronous=FULL`, foreign keys, bounded busy timeouts, zero mmap,
  exact schema validation, and transactional migrations;
- one persistent empty writer-lease sidecar and process-death lock release;
- lease-first live startup, bounded staging recovery, immutable visible publication,
  and non-destructive JSONL rebuild;
- deterministic installed and portable data roots;
- one application composition owner and ordered joined runtime shutdown.

It does not yet have a persistent settings store, a public configuration format, a
consistent user backup format, backup scheduling/retention, pre-migration snapshots,
an application-level restore transaction, or a safe-mode UI. Directly copying only
`tokenmaster.sqlite3` is invalid because a hot WAL is part of the persistent database
state. Store-internal replay recovery is not whole-file corruption recovery.

## 3. Options considered

### A. Copy the live main file into ZIP

Rejected. It can omit committed WAL content, capture an inconsistent point, retain
free pages, and provides no schema, semantic, hash, decompression, or restore gate.

### B. Continuously mirror every write into a second database

Rejected. It doubles write work, couples foreground latency to redundancy, and
immediately mirrors valid-but-wrong logical writes, bad migrations, or application
bugs. A mirror is not historical recovery.

### C. Move the live database among generation directories

Rejected for 1.0. The implemented writer lease is derived from the fixed archive
identity. Moving the archive would require a new root lock and a compatibility
protocol with older binaries; an older binary could otherwise reopen the legacy path
and split user truth. The indirection also creates another active-pointer recovery
problem without improving live SQLite transactions.

### D. Verified immutable snapshots plus staged replacement

Selected. Online Backup creates a consistent live snapshot. Independent backup files
provide historical recovery. Restore is a bounded idempotent operation under the
existing stable writer lease. The current archive, WAL, and SHM are quarantined and
remain available for later operator-owned salvage.

## 4. Non-negotiable invariants

1. The Slint event thread never reads, compresses, hashes, verifies, or writes a
   database or backup.
2. A backup is not published until its container, payload hashes, SQLite structure,
   foreign keys, exact schema, and TokenMaster semantic invariants pass.
3. A restore candidate is always expanded into a new file and never into the active
   database.
4. Restore stops and joins all archive users before replacement and holds the same
   stable writer lease used by usage, quota, benefit, reminder, and Git publication.
5. The active main file is never copied alone as a backup.
6. The current main/WAL/SHM set is never automatically deleted.
7. Automatic recovery uses only a previously verified backup that is reverified in
   the current process.
8. `quick_check`, `integrity_check`, hashes, and semantic checks are evidence, not
   substitutes for one another.
9. An unavailable or corrupt value remains unavailable/corrupt; it never becomes
   numeric zero or a successful empty import.
10. A failed settings save, backup, import, migration, restore, or retention pass
    leaves the last published truth readable.
11. No central catalog, receipt, or settings file is a single point of failure.
12. All retained collections, directory scans, entry counts, input sizes,
    decompression output, compression windows, retries, and recovery attempts have
    hard limits.
13. Recovery and backup errors expose stable codes and counters, never paths, raw OS
    messages, SQLite text, source content, or credentials.
14. No automatic operation invokes SQLite `.recover` or treats salvaged rows as
    authoritative.

## 5. Controlled data-root layout

The existing active files stay unchanged. One exact non-reparse child contains the
new state:

```text
<data-root>/
  tokenmaster.sqlite3
  tokenmaster.sqlite3-wal                         transient SQLite state
  tokenmaster.sqlite3-shm                         transient SQLite state
  tokenmaster.sqlite3.tokenmaster-writer.lock     existing stable lease
  reliable-state/
    settings-a.tms
    settings-b.tms
    run-a.tms
    run-b.tms
    recovery-a.tms
    recovery-b.tms
    backups/
    staging/
    quarantine/
```

Only these exact children may be created. Existing links, reparse points, unexpected
file types, non-local media, or non-empty lease sidecars fail closed. Public values
expose mode, stable status, counts, byte totals, and UTC times only.

The settings, run-state, and recovery files use two alternating slots. Each slot has
a fixed binary header, format version, checked monotonic generation, exact payload
length, SHA-256 payload digest, and bounded strict JSON payload. Startup selects the
highest valid generation. One damaged slot therefore does not break startup. If both
recovery slots are invalid while staged artifacts exist, TokenMaster enters safe mode
instead of guessing file ownership.

## 6. Settings ownership and import semantics

### 6.1 Settings classes

The settings schema separates:

- **portable user settings:** locale, presentation choices once implemented,
  number/currency/time display, notification profiles, pricing overrides, backup
  schedule, retention, and other provider-neutral policy;
- **device-local state:** window placement, last route, runtime receipts, and
  platform integration state;
- **forbidden state:** source/repository/executable paths, credentials, cookies,
  OAuth material, raw provider payloads, prompts, responses, commands, file content,
  and incomplete input.

Only fields supported by the executing product are stored. The schema does not add
fake skin, locale, notification, or provider values before their owning feature
exists. Future features add explicit settings-schema migrations without changing the
slot or export container.

### 6.2 Load and save

Settings load validates both slots independently and selects the newest complete
slot. A corrupt newest slot falls back to the older verified slot and emits one
bounded health event. If neither slot is valid, safe defaults load and the invalid
files remain untouched until an explicit successful settings save creates a new
generation.

Save writes the inactive slot with `create_new`/truncate-safe staging, flushes payload
and metadata, reopens and verifies it, then publishes it with same-volume write-through
replacement. The prior slot remains a complete fallback. Checked generation overflow
fails without changing either slot.

### 6.3 Configuration import

`.tmconfig` contains only the portable settings entry. Import is all-or-nothing:

1. open the user-selected descriptor without exposing its path to UI state;
2. validate container header, counts, lengths, versions, hashes, codec, and optional
   encryption envelope before allocation;
3. stream-decode into a bounded candidate;
4. reject duplicate, unknown, unsupported, malformed, or relationship-invalid fields;
5. migrate only explicitly supported older versions;
6. reject a newer required version without partial interpretation;
7. compute a bounded typed preview without secrets or paths;
8. on explicit confirmation, publish one new settings generation;
9. apply presentation-only changes live and restart only the owning application
   services for runtime-affecting changes.

Cancellation before publication changes nothing. Publication itself is not
cancellable.

## 7. Backup package contract

### 7.1 Types

- `.tmconfig`: portable settings only.
- `.tmbackup`: portable settings plus one standalone SQLite snapshot and bounded
  backup metadata.
- `.tmbackup.age`: an optional standard age passphrase envelope around the complete
  `.tmbackup` byte stream. It is manual only and never used for unattended automatic
  recovery.

### 7.2 Fixed container

The TokenMaster container is not a general archive extractor. It contains:

- an exact magic and format version;
- flags from a closed set;
- at most eight typed entries;
- a strict manifest of at most 64 KiB;
- an exact compressed and expanded length per entry;
- SHA-256 of each expanded entry;
- SHA-256 binding the complete manifest and entry descriptors;
- a footer containing an exact end marker and SHA-256 of every preceding package
  byte, so header/manifest/descriptor corruption is covered without a
  self-referential digest;
- one Zstandard frame per compressed entry with frame checksum enabled;
- no file names, directory names, links, permissions, device entries, or paths.

Hard v1 limits are:

- settings payload: 1 MiB;
- manifest: 64 KiB;
- entry count: 8;
- expanded database: 64 GiB;
- total expanded package: checked database size plus 2 MiB;
- decoder window: 8 MiB;
- streaming I/O buffers: at most 256 KiB each;
- relevant automatic-backup files in the controlled directory: 32;
- quarantine sets: 3, never automatically deleted.

Declared lengths are never used for a proportional memory allocation. Expansion is
streamed to a new local staging file with an independent byte counter and free-space
preflight. Trailing bytes, concatenated frames, unknown required flags, duplicate
entry types, mismatched lengths, and checksum failures are rejected.

### 7.3 Compatibility

An older application rejects a newer required container, settings, or database
version. A newer application may migrate an explicitly supported old settings schema
and may migrate a database only in an isolated candidate. It never migrates the sole
backup in place. Unknown optional manifest metadata may be skipped only when its bit
is explicitly declared optional and its exact length is bounded.

## 8. Snapshot pipeline

### 8.1 Automatic and normal backup

1. Coalesce the request into one maintenance worker.
2. Check source identity, state health, output capacity, and free disk space.
3. Open the live database through a dedicated bounded snapshot boundary.
4. Use `rusqlite::backup::Backup` in finite page steps with cancellation/deadline
   checks and bounded retry on busy/locked results.
5. Finish into a new standalone SQLite staging file.
6. Open the candidate with defensive/trusted-schema-off/zero-mmap policy.
7. Require exact supported schema and bundled SQLite identity.
8. Run `PRAGMA integrity_check`, `PRAGMA foreign_key_check`, and TokenMaster semantic
   invariant checks.
9. Stream settings and database into a candidate package using Zstandard level 6,
   one compression thread, an 8 MiB window, content size, and frame checksum.
10. Flush, close, reopen, decode, hash, and revalidate the complete candidate package.
11. Publish it as a new unique backup file.
12. Only after publication, apply retention.

The source writer may continue between Online Backup page steps. Backup work never
runs on the Slint thread and never retains the whole database or archive in memory.

### 8.2 Maximum-compact manual export

Manual compact export first creates the same consistent online snapshot. It then runs
`VACUUM INTO` from that isolated snapshot into a second candidate, never against the
live archive. The compact candidate is verified and compressed with Zstandard level
19, one thread, long-distance matching disabled, and an 8 MiB window. Levels 20-22
are intentionally rejected because their memory/time cost conflicts with TokenMaster's
small-memory contract.

The preflight accounts for the raw snapshot, compact copy, package candidate, and a
fixed safety margin. Cancellation is accepted between phases and inside the bounded
copy/compression loops, but not after final publication starts.

### 8.3 Source-suspect rule

A failed snapshot candidate proves only that the candidate failed. TokenMaster deletes
only that unpublished candidate and retries once from a fresh Online Backup. Two
independent integrity/semantic failures against the same unchanged source identity
mark the active archive suspect, stop new mutation admission, preserve the last UI
snapshot, and request controlled recovery. This avoids both silent rotation of corrupt
data and recovery from a one-off staging/media fault.

## 9. Automatic backup policy and retention

Automatic backups are enabled by default after the first healthy publication.

Triggers are:

- before a supported database migration;
- after a successful migration;
- before full restore/import or destructive maintenance;
- after durable data changed and the application has been quiet for five minutes;
- at most once per six hours for ordinary periodic state;
- one coalesced catch-up after resume when a due interval was missed.

No backup is made from suspect state, before startup verification, on every SQLite
transaction, or synchronously during normal window close.

Default retention keeps:

- four newest restore points;
- seven daily representatives;
- four weekly representatives;
- at most fifteen retained verified backups;
- at most 2 GiB total compressed backup bytes by default, user-configurable from
  exactly 256 MiB through 64 GiB.

The newest two verified backups and the last pre-migration backup are protected. The
pre-migration point is unpinned only after a verified post-migration backup exists.
Retention is oldest-first and runs only after publishing a new valid backup. If the
byte budget cannot be satisfied without deleting protected backups, the new operation
fails before publication and reports `backup_capacity_exceeded`; it does not delete a
known-good point.

Disabling periodic backups disables only the quiet-time/six-hour schedule. It never
disables a healthy non-empty archive's mandatory pre-migration, pre-restore, or
pre-destructive-maintenance safety point. If such a mandatory point cannot be created
and reverified, the mutation is blocked. A definitively corrupt archive proceeds only
through complete-set quarantine; a brand-new empty installation is the sole no-prior-
backup exception. A post-migration backup failure keeps the pre-migration point pinned
and reports degraded protection rather than pretending migration rollback occurred.

Each backup is self-describing. A catalog is a disposable bounded cache rebuilt by
validating backup headers. More than 32 relevant files, unexpected links/types, or an
ambiguous duplicate identity fails automatic recovery closed and opens Data Health.

## 10. Run-state and startup validation

Before any SQLite open, startup publishes and rereads a new `run-*` generation with
`unclean`. If that durable write cannot be proved, startup enters safe mode and does
not open a writable archive.
Only after all runtimes, controller, backup worker, and settings writes have stopped
cleanly does shutdown publish `clean`.

Startup policy is:

- clean prior run: validate file identity, SQLite header/open, exact schema, and
  application invariants;
- unclean, missing, or invalid run marker: additionally run `quick_check` before
  starting asynchronous workers;
- pending recovery journal: resume the journal state machine before any SQLite open;
- explicit corruption or repeated source-suspect evidence: run full recovery;
- schema too new: safe mode with an upgrade-required result, never restore an older
  backup over newer truth;
- lease contention, permission failure, disk full, or transient I/O: no restore;
  return stable unavailable/busy state.

Periodic full integrity is performed on isolated snapshots, not on the live archive
at every launch.

## 11. Restore and recovery transaction

### 11.1 Preconditions

Restore first validates the package and expands its database into `staging/`. It opens
the candidate defensively, performs full structure/foreign-key/schema/semantic checks,
and if needed migrates only a disposable copy followed by the complete checks again.

For a running application, composition then:

1. closes new maintenance and UI mutation admission;
2. pauses and fully shuts down usage/nested-Git, quota, reminder, backup, and query
   owners so no SQLite handle remains;
3. acquires the existing fixed writer lease;
4. revalidates the active archive identity and candidate;
5. creates a bounded pre-restore backup when the current archive is healthy;
6. begins the durable restore journal.

### 11.2 Redundant journal

The recovery payload contains only a checked operation generation, opaque operation
ID, candidate digest, optional portable-settings digest/target generation, selected
data-only or data-plus-portable-settings mode, backup identity, reason code, attempt
count, and one state:

`prepared -> sidecars_quarantined -> main_replaced -> reopened_verified -> settings_published -> complete`

Every transition writes and verifies the inactive journal slot before filesystem
mutation. Transitions are idempotent and infer completion only from expected fixed
file names plus recorded digests. The journal contains no arbitrary path.

### 11.3 File promotion

1. Create a unique quarantine set with `create_new` semantics.
2. Move any current `-wal` and `-shm` into that set using same-volume write-through
   moves; record absence explicitly.
3. When the current main exists, use Windows `ReplaceFileW` to atomically replace
   `tokenmaster.sqlite3` with the verified candidate while writing the replaced main
   file into the same quarantine set. When the main is missing but durable prior-run
   artifacts prove this is a damaged installation, record its absence and use a
   same-volume write-through `MoveFileExW` candidate promotion. A directory with no
   prior durable TokenMaster artifacts is a brand-new installation and uses normal
   schema creation instead of recovery.
4. Reopen the new active file with the normal writable policy and repeat exact schema,
   integrity, foreign-key, and semantic checks.
5. For a manual full restore, apply the already validated portable settings only when
   the user selected **Data + portable settings**. **Data only** leaves both current
   portable and device-local settings untouched. Automatic recovery is always data
   only. Device-local settings are never restored from a package.
6. Publish and reread the exact settings target, or journal the explicit no-op for a
   data-only restore, then advance to `settings_published`.
7. Mark `complete`, release the lease, reconstruct application owners, force one full
   provider reconciliation, and publish a visible recovery receipt.

If replacement fails, the old main remains active and the quarantined sidecars are
restored under the still-held lease. If post-replacement validation fails, the same
journal performs a staged rollback from the quarantined main/WAL/SHM set. A crash at
any phase resumes before SQLite is opened. If portable settings cannot be published
and reread after the new database is verified, the database is rolled back and the
previous settings generation remains selected. If a crash occurs after settings were
durably published but before the journal advances, resume verifies their exact target
generation/digest and completes idempotently. Any conflicting settings generation, or
any state where neither forward completion nor rollback can be proved, retains every
artifact and enters safe mode.

Quarantine is never part of normal backup retention and is never automatically
deleted. At most three sets are allowed; a fourth recovery request stops for explicit
operator action rather than consuming disk indefinitely.

## 12. Automatic recovery selection

Automatic recovery is permitted only for an active archive that is definitively
corrupt, structurally invalid, or repeatedly fails semantic verification. It is not
permitted for busy, access denied, disk full, unsupported location, schema-too-new,
or ordinary provider unavailability.

Candidates are examined newest-first. Every candidate must:

- have been published as verified;
- pass the complete container and decompression limits now;
- pass full SQLite and application checks now;
- be compatible with the current installation and product format;
- not be the candidate that already caused the current crash loop.

After two failed launches of the same restored candidate, automatic selection stops
and enters safe mode. It never oscillates indefinitely among backups.

If no valid backup exists, TokenMaster quarantines the corrupt set, creates and
verifies a fresh empty archive through the normal schema constructor, and starts the
existing authoritative Codex JSONL full rebuild. Reconstructible usage returns through
normal publication. Non-reconstructible quota/reset/benefit/reminder/Git history is
marked lost/unavailable in a durable recovery receipt; an empty replacement is never
reported as equivalent prior truth.

SQLite `.recover` is not a 1.0 product feature. Safe mode may export the untouched
quarantine set for operator-owned forensic work, but TokenMaster never runs salvage,
overwrites active state from salvaged rows, or promotes rows that cannot pass the
normal schema and semantic invariants.

## 13. Failure decision matrix

| Failure | Automatic action | Preserved truth | Visible result |
| --- | --- | --- | --- |
| settings newest slot corrupt | load older valid slot | both slot files | settings reverted |
| both settings slots corrupt | load defaults | invalid slots | defaults plus warning |
| pre-open unclean marker cannot publish | no writable SQLite open | archive and control slots | safe mode |
| app/power loss with healthy WAL | SQLite recovery, then quick check | active set | recovered/healthy |
| one backup corrupt | skip it after bounded validation | corrupt backup | backup degraded |
| snapshot candidate fails once | discard unpublished candidate and retry once | active DB and old backups | retrying |
| snapshot fails twice on same source | pause writes and request recovery | active DB and old backups | data suspect |
| active database corrupt | restore newest reverified candidate, data only | full current set in quarantine | recovered; point age shown |
| no valid backup | quarantine and rebuild fresh DB from Codex | corrupt set | partial/lost domains explicit |
| active main missing with prior artifacts | journal recovery/promotion | sidecars and all prior artifacts | damaged state, never silent empty |
| no main and no prior artifacts | normal schema creation | new empty root | first install |
| schema newer than app | no replacement | newer DB | upgrade required |
| writer lease busy | no recovery | current DB | busy/retry |
| disk full or access denied | no replacement | current DB and backups | actionable error |
| restore crashes mid-phase | resume journal before SQLite open | candidate and quarantine | recovering |
| selected portable settings cannot publish | roll database back | old DB and settings generation | restore failed without partial commit |
| settings published before crash | verify exact generation/digest and finish | published DB and settings | recovery completes once |
| restored candidate crash-loops | stop after two failed launches | candidate and quarantine | safe mode |
| both recovery slots invalid with artifacts | no guessing | all artifacts | safe mode |
| quarantine cap reached | no fourth automatic recovery | three prior sets | operator action required |

## 14. Compression and optional encryption

The implementation pins `zstd = 0.13.3` without multithread support. Automatic level
6, normal manual level 12, and compact level 19 are explicit product constants.
Compression uses one streaming context and releases it after each operation. Decoder
window and expanded byte count are enforced independently.

Optional manual passphrase protection pins `age = 0.12.1` with default features off
passphrase/stream features. It uses the interoperable age v1 scrypt recipient, one
fixed work factor `log_n = 16`, and the same maximum on decrypt to cap attacker-chosen
CPU/RAM. A new passphrase must contain exactly 12 through 128 Unicode scalar values,
must match a separate confirmation field, and is neither trimmed nor normalized.
Passphrases never enter settings, logs, snapshots, errors, or backup manifests; both
UI fields are cleared immediately after conversion to a secret wrapper. Automatic
local backups remain unencrypted so startup can recover without storing a decryption
secret.

Encryption authenticates the portable export and protects it outside the data root.
SHA-256 inside an unencrypted automatic package detects accidental corruption but is
not claimed to resist a malicious user who can rewrite both payload and manifest.

## 15. Application and UI ownership

`tokenmaster-state` owns pure settings, package, backup-policy, catalog, and recovery
values plus the bounded maintenance implementation. `tokenmaster-store` owns SQLite
snapshot and candidate verification primitives. `tokenmaster-platform` owns controlled
file replacement, write-through move, free-space query, and native user-selected file
descriptors. `tokenmaster-app` alone sequences runtime shutdown/restart and restore.

`tokenmaster-product` receives copied fixed reliable-state health and recovery receipt
values. `tokenmaster-desktop` renders them and emits typed intents only. Neither crate
receives an archive path, file descriptor, backup key, SQLite connection, recovery
journal, or arbitrary filesystem operation.

Settings -> Data & Recovery provides:

- export/import settings with bounded preview;
- create normal or maximum-compact full backup;
- optionally password-protect a manual export;
- verify a selected package without restoring it;
- restore after a second explicit confirmation, with an explicit **Data only** or
  **Data + portable settings** choice;
- automatic schedule, retention, and byte-budget settings;
- last successful backup, verification, and recovery times;
- backup count/bytes and stable failure reason;
- recovery history and current Data Health;
- safe-mode restore, rebuild, retry, and export-quarantine choices.

Progress is phase-based and bounded. Cancel is available before publication or
replacement, not during the atomic promotion phase. The UI never displays a private
absolute path; native dialogs own user-selected descriptors.

## 16. Responsiveness and resource gates

- One maintenance worker and capacity-one latest request; no job per trigger.
- One active operation and one aggregate follow-up.
- No full-file `Vec`, archive history, progress-event queue, or per-backup thread.
- Automatic compression uses one thread and at most an 8 MiB window.
- UI callbacks perform only typed intent admission and snapshot replacement.
- Backup page steps yield between bounded chunks and honor cancellation/deadline.
- A 10,000-trigger burst retains one active plus one follow-up request.
- Repeated backup/verify/import/cancel cycles must return private memory, handles, and
  threads to the measured post-warm-up envelope.
- Automatic backup must not increase dashboard query p95 or input-to-paint p95 by more
  than 10 ms on the reference machine.
- Idle backup scheduling adds no UI timer and no wake more frequent than the existing
  due maintenance decision.
- Disk usage is bounded by staging preflight, retention, a hard relevant-file count,
  and the three-set quarantine stop condition.

## 17. Security and dependency boundary

Imported config, backup, encrypted envelope, SQLite candidate, catalog entry, and
recovery receipt are untrusted. Validation order is header -> declared bounds ->
encryption work bound -> streaming decode -> hashes -> SQLite defensive open -> full
integrity/FK -> exact schema -> semantic invariants.

The contour adds no HTTP, socket, shell, arbitrary SQL, generic archive extraction,
plugin loading, provider mutation, credential access, source-content persistence, or
LLM authority. File dialogs return sealed read/write capabilities to application
composition; UI and future CLI/MCP cannot supply an arbitrary internal data-root path.

Dependency closure, licenses, source language, binary strings, advisories, and release
size are audited after adding `zstd` and `age`. No external `zstd`, `age`, PowerShell,
or SQLite executable is invoked.

## 18. Required fault and adversarial evidence

Tests inject failure before and after every durable phase:

- every settings slot write/flush/reopen/publish boundary;
- Online Backup start, page step, finish, candidate reopen, each integrity layer,
  compression finish, package reopen, and retention deletion;
- byte flips and truncation at every fixed header/manifest/entry/footer region;
- false lengths, duplicate/unknown entries, trailing frames, oversized windows,
  decompression bombs, malicious age work factors, wrong passwords, and invalid UTF-8;
- SQLite header/page/index/schema/foreign-key/semantic corruption;
- hot WAL, missing/mismatched WAL/SHM, interrupted checkpoint, and main-only copy
  rejection;
- recovery slot corruption, every journal transition, replacement failure,
  post-replace reopen failure, rollback failure, and process restart at each phase;
- disk full, read-only/access-denied files, linked/reparse children, unsupported media,
  antivirus-style sharing failures, and checked ID/generation overflow;
- schema old/current/new, isolated migration rollback, no backup, multiple corrupt
  backups, quarantine cap, and restored-candidate crash loop;
- 10,000 coalesced triggers, cancellation, hibernation/resume, repeated operations,
  memory/handle/thread/USER/GDI return, and input-to-paint/query latency under backup.

No test uses or persists real user JSONL, credentials, paths in receipts, or provider
payloads. Synthetic fixtures contain canaries that the release/archive audits must not
find.

## 19. Explicit limits of the guarantee

The design protects application state against process crashes, power interruption,
partial writes, accidental file corruption, invalid imports, and recoverable local
filesystem failures under the documented Windows/filesystem guarantees. It cannot
self-recover from destruction of the complete disk/data root, a corrupted executable,
malicious code running as the same user, or hardware that falsely acknowledges durable
writes. A manually exported off-device `.tmbackup` is required for total-disk loss.

No backup, soak, hash, or SQLite check proves correctness against an unknown logical
bug. Historical restore points, semantic invariants, explicit recovery receipts, and
quarantine provide the operator path for that class of failure.

## 20. Implementation and acceptance order

1. Freeze requirements, formats, bounds, codes, and authority audit.
2. Add controlled platform atomic-file primitives.
3. Add `tokenmaster-state` and redundant settings/run/recovery slots.
4. Add SQLite Online Backup and complete candidate verification.
5. Add fixed `.tmconfig`/`.tmbackup` streaming codec and compression.
6. Add optional age passphrase wrapper.
7. Add catalog, policy, retention, and the capacity-one maintenance worker.
8. Add the idempotent restore journal, quarantine, rollback, and startup resume.
9. Integrate lease-preserving startup, pre-migration backup, and no-backup rebuild.
10. Add application service restart and typed manual operations.
11. Add product health, Settings/Data Health/safe-mode UI, and native dialogs.
12. Run fault, privacy, resource, latency, hibernation, workspace, and release audits.

P3-D.0 is implemented only when all tasks pass and the project-truth documents are
updated. The design and plan alone remain `planned` traceability evidence and do not
claim backup, restore, safe mode, encryption, release, or M0 acceptance.

## 21. Closure review and primary references

The second-pass review specifically checked WAL completeness, stable writer-lease
identity, old-version compatibility, Windows replacement semantics, crash windows
between WAL quarantine and main replacement, corrupt control files, catalog loss,
schema downgrade, logical corruption, disk exhaustion, quarantine growth, malicious
compression/encryption metadata, steady-state memory, and live UI isolation.

The review rejected active database generation paths because they would change the
proved lock identity and introduce split-truth compatibility risk. The fixed archive
plus redundant journal has a smaller state space: every pre-promotion failure leaves
the old main file, every promotion preserves the replaced main in quarantine, and
every interrupted phase resumes before SQLite open.

Primary behavior references:

- SQLite Online Backup API: <https://www.sqlite.org/backup.html>
- SQLite WAL persistence: <https://www.sqlite.org/wal.html>
- SQLite `VACUUM INTO`: <https://sqlite.org/lang_vacuum.html>
- SQLite integrity/foreign-key pragmas: <https://sqlite.org/pragma.html>
- SQLite untrusted database guidance: <https://www.sqlite.org/security.html>
- SQLite recovery limitations: <https://www.sqlite.org/recovery.html>
- Windows `ReplaceFileW`: <https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew>
- Windows `MoveFileExW`: <https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw>
- Zstandard API and memory guidance: <https://facebook.github.io/zstd/zstd_manual.html>
- age v1 Rust implementation: <https://docs.rs/age/0.12.1/age/>
