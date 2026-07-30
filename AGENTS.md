# TokenMaster agent instructions

## Product boundary

TokenMaster is the only product in this repository. It is a Rust 1.97, Slint 1.17,
and bundled SQLite application. The root Cargo workspace is the sole workspace.

`docs/UI_REFERENCE.md` is the visual standard: the owner's Dashboard mockup, transcribed.
Every visual element in it is in scope. Where the built product differs from it, the product
is wrong.

WhereMyTokens is the external UI/product reference. ccusage is the external
usage-analysis reference. Both are pinned in `third_party/UPSTREAM.toml`.

Read them. Studying a pinned reference and reimplementing what you learn is the
intended way to work, and it needs no justification: palettes, spacing and type
scales, chart weighting, axis and gridline treatment, information hierarchy,
empty and error states, wording, and which number belongs on which screen are all
fair to take. When the reference does something better, match it.

What does not cross: their source. Neither is a runtime dependency and neither is
vendored. Both are MIT, so copying code would carry a copyright and licence
obligation this repository does not take on, and there is nothing to gain —
TypeScript and React do not port into Rust and Slint, and pixel-matching an
Electron app in a different layout engine with a different font stack is not
achievable or desirable. The product's own claim is that it is not Electron.

## Source of truth

Read in this order before changing behaviour:

1. `spec/SPECIFICATION.md` — the normative product contract
2. `spec/DATA_CONTRACT.md`, `spec/API_CONTRACT.md`, `spec/SECURITY.md`
3. `spec/DECISIONS.md` — ADRs
4. `spec/TRACEABILITY.md` — the sole requirement-status authority
5. `docs/RELEASE_EXIT_PLAN.md` — the delivery rail: phases, criteria, abort rules

Project history lives in `git log` and `docs/CHANGELOG.md`. Do not reintroduce a
narrative state or handoff document: the four that existed absorbed more edits than
the most-edited source file and still contradicted each other.

`docs/ROADMAP.md` was the fifth and was deleted for the same reason. It ran 522 lines,
declared in its own opening line that the exit plan was the active rail, and then
carried its own phase numbering in which P6 meant signing rather than ingestion. It
called the secret scan green when nothing invoked the scanner, described a GNU release
lane removed in P0, and mandated a WebAssembly plugin host that was dropped by decision.
Requirement status belongs to `spec/TRACEABILITY.md`; delivery order belongs to
`docs/RELEASE_EXIT_PLAN.md`; everything else belongs to `git log`.

`M0_ACCEPTANCE.md` describes receipts produced by `tokenmaster-m0`, a probe binary
deleted in 2dd1f09. It measured a different renderer and none of the product's
ingestion, query or snapshot code, so it never could gate a product release and
does not now. Release gates live in `docs/RELEASE_EXIT_PLAN.md`.

## Product invariants

These are binding regardless of what is being built.

- Never persist or expose prompts, responses, reasoning, commands, source contents,
  credentials, raw incomplete lines, or absolute user paths.
- Input lines, retained parser state, reader batches, checkpoints, chart points and
  UI lists are bounded. No production path allocates from an untrusted declared size.
- A missing value stays missing. Unavailable, partial and legitimate-zero are three
  different facts and must remain distinguishable everywhere they are shown — text,
  chart, accessible label.
- CLI and MCP surfaces, when they exist, expose no arbitrary SQL, shell, HTTP,
  filesystem or credential operation.

## Working

- Work on a feature branch or an isolated worktree.
- Follow `docs/RELEASE_EXIT_PLAN.md` in its order. One phase at a time, and report
  against that phase's stated criterion with its measured number — including when it
  was missed. A reported miss is information; a quietly adjusted target is not.
- A behaviour change starts with a test that fails before it. A guard that stays
  green for both the correct and a plausible wrong implementation guards nothing.
- A defect found outside the current item is recorded and deferred, not fixed inline —
  unless it makes something this repository currently claims untrue. A red test, a gate
  that never runs, a document that misstates the state, and code no longer reachable are
  not deferrable: they are a false account of the present, and every later report
  inherits the falsehood. Fix those where they are found. Everything else gets a phase.
- Prefer deleting code to adding it when both reach the goal. Do not add a document,
  script, or abstraction that no phase asked for.
- Do not put a commit hash in a tracked document.
- In a tracked document, reference code by symbol rather than by line number. Every
  `file:line` reference these documents once carried had drifted, and two of them
  pointed at defects that were already fixed. A function or property name survives
  edits and is greppable. A commit message or a review comment is not a tracked
  document and may cite lines freely.
- A check that reads a script's own text is not that check. Assert against the artifact,
  and name the test for what it actually does.

## Verification

`rust-toolchain.toml` pins the exact toolchain and host triple, so no `+toolchain`
override belongs in a command or a script. Everything builds and tests
`x86_64-pc-windows-msvc`, which is the target that ships.

Run the narrowest relevant test first. The baseline quality gate is:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked
```

`pwsh -NoProfile -File scripts\verify-m0.ps1` runs that gate plus the clean-root,
immutable-action and dependency-policy checks and the eight Pester suites. The
dependency policy needs network access to the current RustSec database.
