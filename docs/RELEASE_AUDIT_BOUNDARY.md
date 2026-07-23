# TokenMaster release audit boundary

Developer verification, benchmarks, and soak samples are evidence only for the exact
build they describe. A stable release requires separately verifiable package hashes,
commit identity, executable identity, interactive Windows evidence, signing evidence,
and an independent artifact review.

M0 package output, if its future receipt gates pass, is an architecture-proof artifact
and not a product release. Missing, stale, dirty-tree, mismatched-commit, or
mismatched-executable evidence fails closed.

The canonical Windows 1.0 artifact is a signed `x86_64-pc-windows-msvc` portable ZIP.
The existing GNU target is a development and M0 evidence lane only until P6 completes
an explicit dual-lane functional/resource/package comparison. The release build does
not inherit a workspace-global target implicitly.

Before packaging, the executable must be the exact canonical MSVC release output and
pass executable inspection for x64 machine type, Windows GUI subsystem, and absence of
dynamic Visual C++/Universal CRT imports. This binary-portability receipt does not prove
package provenance, deterministic contents, signing, clean-room behavior, or release
acceptance.

The unsigned package producer now binds a clean commit to the repository-owned canonical
MSVC target, a closed nine-file portable stage, deterministic ZIP ordering/timestamps,
per-file SHA-256 manifest, generated dependency notices/license texts, and CycloneDX
SBOM. Its local extracted-package smoke is development evidence only. The ignored
artifact and receipt must be regenerated after every commit; neither is signing,
authenticated clean-room, interactive, attestation, RC, or stable-release evidence.

The 1.0 package audit requires:

- exact clean commit, executable hash, package hash, and deterministic content list;
- signing identity and signature verification;
- Slint Royalty-free License 2.0 attribution in Help/About and the public download
  page, plus product license and generated third-party notices;
- SBOM, advisory audit, dependency/source/license policy, secret scan, immutable CI
  action references, and artifact provenance/attestation;
- exact calendar dependency and data provenance: Jiff version, platform adapter,
  bundled IANA tzdb crate/version and IANA release (currently locked to Jiff 0.2.32,
  `jiff-tzdb-platform` 0.1.3, `jiff-tzdb` 0.1.8, and tzdb 2026c);
- clean-room launch, Windows interactive matrix, performance reference run, and
uninterrupted release-candidate soak.

The current P2-B developer reference gate covers deterministic current and immutable-
legacy million-event fixtures, rebuild throughput and page p95, cold/cached/full
analytics, 32-scope and session-page latency, main+WAL+SHM amplification, privacy, and
resource plateaus. Its measurements are architecture evidence only. P6 must rerun the
same budgets against the exact clean MSVC release candidate and record machine,
commit, executable, and package identity; GNU developer numbers do not transfer.

An automatic updater or installer is outside 1.0. Adding either requires a separately
approved signed-manifest, staged replacement, rollback, downgrade, and interrupted-
update contract; a download link or package script is not sufficient evidence.
