# TokenMaster M0 acceptance

**Status: SUFFICIENT FOR BOUNDED M1 DEVELOPMENT; NOT M0/RELEASE-ACCEPTED — automated
developer gates pass; interactive and uninterrupted 24-hour evidence is missing.**

## Verified locally

- Rust 1.97.0 locked workspace builds on Windows GNU.
- Clippy with `-D warnings`, focused tests, full workspace tests, and docs audit pass.
- Bundled SQLite runtime is exactly 3.53.2.
- Explicit one-million-row store test passes with 256-row keyset pages.
- Three layouts, three themes, en/ru, pseudo-locale, and one-window state switches pass.
- Native tray close keeps the exact process alive; the smoke harness cleans its PID.
- Software structured stress passes for 10K skins, 10K routes, and 1M rows plus 10K
  skins. The 1M case ended at 69.85 MiB with zero retained sampled bytes/objects.
- A 0.01-hour soak harness smoke passes and cleans the process.
- A clean-commit long run reached about nine wall-clock hours and 896 samples. The same
  process survived and resumed after Windows low power with stable one-hour resource
  windows.

## Rejected path

- FemtoVG retained 437,600,256 private bytes in the first tray smoke, above the 64 MiB
  empty hard limit. It is diagnostic-only and cannot satisfy M0.

## Required before M0 acceptance

- 24-hour software soak receipt named `reports/soak-24h.json`.
- Interactive Windows 10/11 report named `reports/interactive-m0.json`.
- Keyboard and screen-reader inspection.
- 100%, 150%, and 200% DPI plus mixed-monitor checks.
- Tray Show/Hide/Quit menu clicks, Explorer restart, lock/unlock, sleep/resume.
- Measured input-to-paint/theme/layout paint percentiles; headless callback time is not
  paint latency.

Both required receipts must contain the current Git commit, `dirty: false`, and the
SHA-256 of the tested `tokenmaster-m0.exe`. Missing or mismatched identity fields fail
packaging even when `result` says `pass`.

The long run's 5,588.6-second sample gap exceeded the 75-second hard limit, so it is
not a 24-hour pass and produced no `soak-24h.json`.

Only after both external JSON receipts say `result: pass` may `package-m0.ps1` create a
non-release architecture-proof ZIP. ADR-011 permits bounded M1 development while this
boundary remains open; it does not authorize M0 acceptance, packaging, or release.
