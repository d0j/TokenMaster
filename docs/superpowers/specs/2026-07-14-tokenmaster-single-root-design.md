# TokenMaster Single-Root Design

## Goal

Make TokenMaster a self-contained, clean-history Rust desktop project whose only
product references are WhereMyTokens for information architecture and ccusage for
usage-analysis semantics. The repository has one Rust workspace at its root.

## Product boundary

TokenMaster is an original Windows-first, portable, local-first application. It
uses Rust 1.97, Slint 1.17, and bundled SQLite. It does not require a Go, Node, or
Electron runtime. No external repository is built, executed, or treated as a
runtime dependency.

WhereMyTokens defines the target breadth and quality of UI: quota-first hierarchy,
the complete board, supporting exploration views, compact access, and dense but
accessible dark information design. ccusage defines the target breadth and quality
of usage import, model normalization, token accounting, pricing, reports, and
history analysis. When the two references overlap, TokenMaster selects the safer,
more responsive, bounded implementation and records the resulting behavior in its
own contracts.

## Repository shape

The root contains exactly one Cargo workspace, its crates, scripts, reports,
documentation, CI configuration, project metadata, and MIT/provenance material.
There are no nested product workspaces. Generated reports, build products, local
data, and worktrees stay ignored.

## Retained provenance and licensing

`third_party/UPSTREAM.toml` pins each external reference by URL, commit, license,
and a short purpose statement. `third_party/licenses/` retains the exact MIT license
texts. TokenMaster does not vendor their application source or executable assets.

## Quality and privacy invariants

- Input processing is bounded and streaming; the fast path does not rescan whole
  histories or retain source content.
- Persistent data, diagnostics, UI, CLI, and future MCP surfaces never retain or
  expose prompts, responses, reasoning, commands, file contents, or credentials.
- The GUI consumes immutable, bounded snapshots and may switch layout, skin, and
  locale without rebuilding the data archive.
- A release remains blocked until the documented M0 and product-release evidence is
  present. Developer tests never imply an interactive Windows acceptance claim.

## Verification

The clean root must pass the locked Rust workspace tests, formatting, strict Clippy,
the M0 verification script, and a tracked-content audit that rejects a second product
workspace, non-Rust runtime manifests, and obsolete project identifiers. The audit
allows exact upstream names only inside the provenance, license, parity, and product
specification documents.
