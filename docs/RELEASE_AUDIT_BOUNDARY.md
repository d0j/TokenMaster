# TokenMaster release audit boundary

Developer verification, benchmarks, and soak samples are evidence only for the exact
build they describe. A stable release requires separately verifiable package hashes,
commit identity, executable identity, interactive Windows evidence, signing evidence,
and an independent artifact review.

M0 package output, if its future receipt gates pass, is an architecture-proof artifact
and not a product release. Missing, stale, dirty-tree, mismatched-commit, or
mismatched-executable evidence fails closed.
