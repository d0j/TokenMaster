# TokenMaster P3-D.0 developer acceptance

## Boundary

This contract accepts only the P3-D.0 Reliable State implementation. It is developer
evidence for bounded backup, recovery, and UI responsiveness on the measured Windows
machine. It is not M0 acceptance, interactive Windows/DPI/accessibility evidence, a
release-candidate soak, packaging, signing, or product-release authorization.

The receipt fails closed unless every identity field and every gate below is present
and passing. A headless software-rendered frame is a real Slint paint measurement, but
it is not proof of physical-display presentation or OS input latency.

## Frozen format and toolchain identity

The receipt MUST record these exact versions from the tested tree:

| Field | Required value |
|---|---:|
| Rust toolchain | `1.97.0` |
| Slint | `1.17.1` |
| SQLite | `3.53.2` |
| database schema | `13` |
| package container | `1` (`TMPKG001`) |
| package manifest | `1` (`TMMNF001`) |
| settings schema | `1` |
| P3-D.0 evidence schema | `1` |
| Zstd crate | `0.13.3` |
| age crate | `0.12.1` |

The authoritative values remain the source constants and `Cargo.lock`. This table is
an acceptance pin, not a second implementation authority.

## Preconditions

Run on Windows 11 x64 from the repository root with the release profile, one test
thread, no unrelated load, and enough free disk for the deterministic fixtures. Before
recording a receipt:

1. `git rev-parse HEAD` MUST return a full 40-character commit ID.
2. `git status --porcelain=v1 --untracked-files=all` MUST be empty, so `dirty=false`.
3. `cargo +1.97.0 build -p tokenmaster-app --release --locked` MUST pass.
4. The SHA-256 of the exact built `TokenMaster.exe` MUST be recorded after the build.
5. The target triple, Windows build, CPU model/logical count, and physical-memory size
   MUST be recorded without usernames, absolute paths, machine names, or other private
   data.
6. Every command below MUST run against the same commit and executable identity.

If the tree changes, a command is retried against another binary, or an identity cannot
be established, discard the receipt and start a new run.

## Mandatory commands

```powershell
$arguments = @('+1.97.0', 'test', '-p', 'tokenmaster-state', '--test', 'backup_performance_contract', '--release', '--locked', '--', '--ignored', '--nocapture', '--test-threads=1')
& cargo @arguments

$arguments = @('+1.97.0', 'test', '-p', 'tokenmaster-state', '--test', 'recovery_resource_contract', '--release', '--locked', '--', '--nocapture', '--test-threads=1')
& cargo @arguments

$arguments = @('+1.97.0', 'test', '-p', 'tokenmaster-app', '--test', 'backup_ui_latency_contract', '--release', '--locked', '--', '--nocapture', '--test-threads=1')
& cargo @arguments
```

The first command MUST execute both ignored contracts. Skipped, filtered, retried, or
non-release tests do not satisfy this contract. The ordinary debug workspace suite
compiles the recovery-resource and UI targets but explicitly skips their measurements;
only the mandatory `--release` commands above can pass their P3-D.0 gates.

## Gates

| ID | Required evidence | Passing rule |
|---|---|---|
| `P3D0-PERF-01` | Automatic, normal, and compact end-to-end snapshot/package/verify measurements for deterministic small and large schema-13 fixtures | Every mode passes its declared throughput floor and verification |
| `P3D0-PERF-02` | Compression thread and decoder-window observations | No compression-created thread; one sampler thread is the only allowed measurement delta; decoder window is at most 8 MiB |
| `P3D0-PERF-03` | Small/large private-memory high water, including a 96 MiB deterministic large fixture | Every run remains within a fixed 64 MiB private-growth envelope and the large database exceeds measured growth by more than 16 MiB; no database-sized allocation |
| `P3D0-SCHED-01` | 10,000 maintenance triggers | One active operation and at most one aggregate follow-up; no request-sized queue |
| `P3D0-RES-01` | 64 warm-up plus 256 backup/package/verify/import-inspect-cancel/retention cycles and 16 complete isolated restore cycles | Final private memory is within 16 MiB, handles within one, and threads/USER/GDI at or below the all-contours post-warm-up baseline; zero child processes |
| `P3D0-RES-02` | At least 16 forced cancellations after a recovery candidate and its source reader are acquired, followed by staging recovery | No child process; staging returns to zero; no leaked worker thread or file/process handle beyond the same return envelope |
| `P3D0-UI-01` | 40 post-warm-up cached Dashboard query samples with and without one identity-tracked in-progress automatic backup cycle spanning the complete loaded sample window | Backup p95 increase is at most 10 ms |
| `P3D0-UI-02` | 40 post-warm-up route-input-to-software-paint samples under the same spanning automatic backup cycle | Backup p95 increase is at most 10 ms and the exact overlapping cycle completes during joined shutdown |
| `P3D0-SCHED-02` | Resume after a missed deadline | Exactly one due catch-up, no schedule burst, then the ordinary next deadline |
| `P3D0-DISK-01` | Repeated retention and staging observations | At most 15 retained points; verification staging returns to zero; every measured cycle returns to the exact filled daily/weekly-tier byte plateau |
| `P3D0-CRYPT-01` | Real compact package plus age manual encryption | Temporary high water is measured; the stage is discarded; resources return to the same envelope |

The resource envelope is intentionally measured after warm-up so one-time runtime and
Windows API initialization cannot be misreported as a leak. The one-handle allowance
covers stable process-global measurement state only; monotonic growth across checkpoints
or any child process is a failure.

## Deterministic fixture identity

`backup_performance_contract` emits the exact length and SHA-256 of both
`schema13-freelist-v1` fixtures. A valid receipt MUST copy those values from the same
run and MUST reject duplicate kinds, missing sizes, zero lengths, malformed hashes, or
hash changes between command attempts. Fixture files and absolute temporary paths MUST
not be retained in the receipt.

## Receipt

Generated evidence belongs under ignored `reports/` and is disposable developer
output. The canonical filename is `reports/p3d0-developer-evidence.json`. It MUST be a
single strict JSON document with no comments and at least these fields:

```json
{
  "schema": "tokenmaster.p3d0.acceptance.v1",
  "result": "pass",
  "commit": "<40 lowercase hexadecimal characters>",
  "dirty": false,
  "application_executable_sha256": "<64 lowercase hexadecimal characters>",
  "target": "<exact Rust target triple>",
  "environment": {
    "windows_build": "<non-private value>",
    "cpu_model": "<non-private value>",
    "logical_processors": 1,
    "physical_memory_bytes": 1
  },
  "versions": {
    "rust": "1.97.0",
    "slint": "1.17.1",
    "sqlite": "3.53.2",
    "database_schema": 13,
    "package_container": 1,
    "package_manifest": 1,
    "settings_schema": 1,
    "evidence_schema": 1,
    "zstd_crate": "0.13.3",
    "age_crate": "0.12.1"
  },
  "fixtures": [],
  "commands": [],
  "duration_ms": 1,
  "metrics": {},
  "gates": []
}
```

Each command record MUST contain the exact argument array, exit code, duration, and
the parsed schema-tagged JSON line emitted by its test. Each gate record MUST contain
one unique ID from the table, `result`, measured values, limits, and source command.
`result=pass` is valid only when all eleven IDs are present exactly once and pass.

The receipt MUST contain no prompts, responses, reasoning, commands observed from user
sessions, source contents, credentials, raw incomplete lines, usernames, absolute user
paths, machine names, or provider-private data.

## Acceptance decision

P3-D.0 may be marked complete only after this receipt passes on a clean exact commit
and Task 18 closes documentation, traceability, focused audits, the full locked
workspace suite/doctests, strict Clippy, and the release build. Even then, M0 and the
product remain unaccepted until their separate interactive, soak, packaging, signing,
and release requirements pass.

The tracked repository contains this contract and the executable gates, not a mutable
golden measurement. A valid local completion run writes the ignored receipt only after
the documentation closure commit, validates all eleven unique gate IDs, and rereads the
commit, worktree, executable hash, versions, fixtures, and parsed command results before
returning `pass`. Deleting or regenerating `reports/` cannot change implementation or
release truth; absence of that local receipt means only that P3-D.0 acceptance has not
been reproduced on that machine.
