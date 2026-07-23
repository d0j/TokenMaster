# TokenMaster release audit boundary

Developer verification, benchmarks, and soak samples are evidence only for the exact
build they describe. A stable release requires separately verifiable package hashes,
commit identity, executable identity, interactive Windows evidence, signing evidence,
and an independent artifact review.

M0 package output, if its future receipt gates pass, is an architecture-proof artifact
and not a product release. Missing, stale, dirty-tree, mismatched-commit, or
mismatched-executable evidence fails closed.

M0 controller contracts must wait for terminal worker state, not a count of pollable
completion receipts: the shared worker intentionally retains only its latest result.
Any test that assumes every coalesced completion remains readable is invalid evidence.

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

The immutable CI action item is implemented: every current remote workflow action is
bound to a full reviewed commit, and the M0 verifier rejects mutable tags, branches,
expressions, abbreviated hashes, ambiguous references, and unsafe local-action paths.
The dependency-policy item is also implemented for the locked all-features canonical
MSVC graph: the exact reviewed `cargo-deny` 0.20.2 binary runs advisories, licenses,
and sources with no advisory ignores, reviewed license terms, and crates.io as the sole
registry. Pre/post state snapshots reject concurrent commit, worktree, tool, policy,
or lockfile drift. Transitive unmaintained findings remain upstream-visible under the
workspace-only policy and are not vulnerability claims. The receipt uses the current
fetched RustSec database and does not promise immutable historical replay.

The secret-scan item is implemented with the reviewed official Gitleaks 8.30.1
Windows x64 archive and executable pinned by SHA-256. It scans one clean commit as Git
history and the separately validated closed product ZIP at one archive level with
redacted, bounded temporary reports. The bounded receipt binds the same commit,
worktree, tool, and package before and after the scan and retains no findings, local
paths, command output, or source content.

The artifact-attestation producer path is implemented as a separate pinned Windows
workflow. It builds the canonical MSVC ZIP only from a `v*` tag or a default-branch
manual run, attests exactly that unsigned ZIP with OIDC, and then uploads the ZIP and
producer receipt. It has no OCI or artifact-metadata storage authority. This local
workflow is not a receipt: a trusted remote run, matching downloaded ZIP, and
independent `gh attestation verify` evidence remain required before attestation can be
marked complete.

These gates do not satisfy the remaining public-download attribution, remote
attestation verification, signing, interactive, performance, or soak items.

The current P2-B developer reference gate covers deterministic current and immutable-
legacy million-event fixtures, rebuild throughput and page p95, cold/cached/full
analytics, 32-scope and session-page latency, main+WAL+SHM amplification, privacy, and
resource plateaus. Its measurements are architecture evidence only. P6 must rerun the
same budgets against the exact clean MSVC release candidate and record machine,
commit, executable, and package identity; GNU developer numbers do not transfer.

An automatic updater or installer is outside 1.0. Adding either requires a separately
approved signed-manifest, staged replacement, rollback, downgrade, and interrupted-
update contract; a download link or package script is not sufficient evidence.
