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

The 1.0 package audit requires:

- exact clean commit, executable hash, package hash, and deterministic content list;
- signing identity and signature verification;
- Slint Royalty-free License 2.0 attribution in Help/About and the public download
  page, plus product license and generated third-party notices;
- SBOM, advisory audit, dependency/source/license policy, secret scan, immutable CI
  action references, and artifact provenance/attestation;
- clean-room launch, Windows interactive matrix, performance reference run, and
  uninterrupted release-candidate soak.

An automatic updater or installer is outside 1.0. Adding either requires a separately
approved signed-manifest, staged replacement, rollback, downgrade, and interrupted-
update contract; a download link or package script is not sufficient evidence.
