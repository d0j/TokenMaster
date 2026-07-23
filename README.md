# TokenMaster

TokenMaster is a Windows-first, portable, local-first usage monitor for Codex. It is
being built as an original Rust application with a responsive Slint desktop UI and a
bounded SQLite archive.

TokenMaster's original code is licensed under Apache-2.0. WhereMyTokens and ccusage
remain separately attributed external MIT references.

Global reminder settings synchronization is implemented: portable settings are the
desired-state authority, `N` maps to global profile revision `N + 1`, and startup,
explicit Save, and confirmed import share one retryable synchronizer. Settings edits
enable/disable five recommended and up to eight normalized custom leads. Per-scope
editing, snooze, quiet hours, OS/tray delivery, usage alerts, activation, P4/P5/P6,
M0 acceptance, package/signing/soak, and release remain incomplete.

WhereMyTokens is the UX and feature-breadth reference. ccusage is the usage-analysis,
model, pricing, and reporting reference. TokenMaster owns the implementation and uses
neither project as a runtime dependency.

## Current state

M0, the native architecture proof, has a working Rust/Slint/SQLite baseline with
three layouts, hot-switchable skins, English/Russian localization, tray lifecycle,
virtualized presentation models, and resource gates. It is not a product release or
an accepted interactive Windows validation.

The data/runtime foundation has bounded Codex discovery/parsing, replay-safe
accounting, strict SQLite schema v13, exact full rebuild, and a production incremental
tail refresh. Unchanged refreshes
read zero JSONL payload bytes; append resumes from the persisted checkpoint; new and
missing sources follow exact complete-scan authority; replacement, rewrite, and
truncation or a changed profile scope durably request a non-destructive full rebuild.
That rebuild safely recovers an unadmitted provisional source. The live runtime now
assembles startup recovery, the process-owned writer lease, the bounded worker,
scheduler and pathless watcher, incremental/rebuild selection, pause/resume, and
joined shutdown. P2 product data is complete: indexed immutable usage/cost analytics,
dynamic quota and full-reset history, expiring reset-benefit inventory with durable
reminder events, bounded Git output analytics, and one exact joined product status.
The constant-state product reducer retains one current snapshot, rejects stale async
work, copies only bounded runtime health, and derives fixed route readiness without
giving UI code SQLite or runtime ownership. P3 now includes the responsive Dashboard,
History, Sessions/detail, Models, Projects, Recent activity, Notifications expiry
center, Settings/Data Health foundation, and Help/About. Expiry reminders use a
separate app-owned bridge: a bounded notification becomes visible before durable
acknowledgement; confirmed release after a failed presentation preserves replay, and
the same single worker retries presentation without waiting for unrelated product
activity. A terminal acknowledgement error releases without an automatic presentation
loop. Global reminder settings synchronization/editing is implemented for the portable
global profile. The production shell now also has the bounded route palette, same-window
compact quota mode, fail-visible tray lifecycle, current-session single-instance Show,
fixed global `Ctrl+Alt+T` Show/restore/focus, and explicit current-user startup backed
only by the fixed HKCU Run value. Live Windows shell acceptance, OS/tray reminder
delivery, per-scope editing, snooze, quiet hours, usage alerts, benefit activation,
later-page Sessions/History controls, P4 presentation/localization, automation,
interactive evidence, packaging, signing, and release remain. The P3-E packaged shell
receipt contract and strict local preflight are fixed in `P3E_ACCEPTANCE.md`. Local P6
package provenance, immutable CI action binding, and the advisory/license/source
dependency policy are implemented; authenticated external interactive evidence, secret
scan, attestation, signing, and release remain absent.

## Build and verify

```powershell
cargo +1.97.0 test --workspace --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\verify-dependency-policy.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\verify-secret-scan.ps1 -RepositoryRoot (Get-Location).Path -PackagePath dist\TokenMaster-0.1.0-windows-x64-unsigned.zip
pwsh -NoProfile -File scripts\verify-m0.ps1 -RepositoryRoot (Get-Location).Path
```

The dependency command bootstraps the exact reviewed `cargo-deny` 0.20.2 Windows
binary and requires network access to the current RustSec database. The last command
also requires Pester 5.7.1 and a validated Windows GNU linker. Both record developer
evidence only; neither claims release acceptance.

## Run the M0 probe

```powershell
cargo +1.97.0 run -p tokenmaster-m0 --release
```

The software renderer is the default. `TOKENMASTER_RENDERER=femtovg` is retained only
for diagnostic comparison and cannot be the default renderer.

## Quality commitments

- Bounded, streaming source processing; no whole-history rescan on the fast path.
- No persistence or exposure of prompts, responses, reasoning, commands, source
  contents, credentials, or absolute user paths.
- Instant modular skin/layout/locale switching without rebuilding the archive.
- Pathless filesystem hints collapse into one fixed atomic aggregate and one bounded
  scheduler wake; periodic reconciliation remains the source of liveness.
- Startup recovery and every write run under the OS writer lease; shutdown stops
  admission and watcher ownership before joining scheduler and worker threads.
- Measured memory, CPU, handle, thread, USER, GDI, and latency gates.

See [the approved audit and master plan](docs/AUDIT_AND_MASTER_PLAN.md),
[architecture](docs/ARCHITECTURE.md), [feature matrix](docs/FEATURE_PARITY.md),
[roadmap](docs/ROADMAP.md), and [current handoff](docs/HANDOFF.md).
