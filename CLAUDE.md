# TokenMaster

Working discipline that is not specific to TokenMaster lives in `~/.claude/CLAUDE.md` and is
not repeated here: two copies of a rule drift, and this repository has paid for that once.
`AGENTS.md` points here, so both Codex and Claude Code read one file.

## Product boundary

TokenMaster is the only product here: Rust 1.97, Slint 1.17, bundled SQLite. The root Cargo
workspace is the only workspace.

`docs/design/README.md` is the visual standard — the designer's handoff, authoritative, with
the exact hex values, type scale, paddings, card heights, radii and interaction rules measured
from the prototype. Every element in it is in scope, and where the product differs the product
is wrong. **Its numbers are mock data: the layout is the specification, the values come from
the archive.** `docs/UI_REFERENCE.md` does not re-transcribe it; it measures the distance from
it to the built product.

WhereMyTokens is the external UI reference, ccusage the usage-analysis one, both pinned in
`third_party/UPSTREAM.toml`. Studying them and reimplementing what you learn is the intended
way to work and needs no justification: palettes, spacing, type scale, chart weighting, axis
treatment, information hierarchy, empty and error states, wording, and which number belongs on
which screen are all fair to take. When the reference is better, match it.

Their **source** does not cross. Neither is vendored or a dependency; both are MIT, so copying
code takes on an obligation for nothing — TypeScript and React do not port into Rust and
Slint, and pixel-matching an Electron app in another layout engine is not the goal. The
product's claim is that it is not Electron.

## Source of truth

Read in this order before changing behaviour:

1. `spec/SPECIFICATION.md` — the normative contract
2. `spec/DATA_CONTRACT.md`, `spec/API_CONTRACT.md`, `spec/SECURITY.md`
3. `spec/DECISIONS.md` — ADRs
4. `spec/TRACEABILITY.md` — the sole requirement-status authority
5. `docs/RELEASE_EXIT_PLAN.md` — the delivery rail: phases, criteria, abort rules, and every
   open finding with its anchor

History is `git log` and `docs/CHANGELOG.md`.

**Do not add a narrative state or handoff document.** Four existed, absorbed more edits than
the most-edited source file, and contradicted each other; `docs/ROADMAP.md` was the fifth and
was deleted for declaring the exit plan authoritative and then carrying a rival phase
numbering. Requirement status is `spec/TRACEABILITY.md`, delivery order is
`docs/RELEASE_EXIT_PLAN.md`, everything else is `git log`.

`M0_ACCEPTANCE.md` records receipts from a probe binary since deleted. It measured a different
renderer and none of the ingestion, query or snapshot code, so it gates nothing.

## Product invariants

Binding regardless of what is being built.

- Never persist or expose prompts, responses, reasoning, commands, source contents,
  credentials, raw incomplete lines, or absolute user paths.
- Input lines, retained parser state, reader batches, checkpoints, chart points and UI lists
  are bounded. No production path allocates from an untrusted declared size.
- A missing value stays missing. Unavailable, partial and legitimate-zero are three different
  facts and stay distinguishable everywhere they are shown — text, chart, accessible label.
- CLI and MCP surfaces expose no arbitrary SQL, shell, HTTP, filesystem or credential
  operation.
- Every number the interface shows arrives by snapshot — `ProductSnapshot` → `ProductSection`
  → projection → Slint property — never recomputed beside it. Two screens that compute the
  same figure independently will disagree, and one of them has.

## Working

- Feature branch or isolated worktree. `main` accepts nothing else; see Landing.
- Follow `docs/RELEASE_EXIT_PLAN.md` in order, one phase at a time, and report against that
  phase's criterion with its measured number, including when it was missed.
- Prefer deleting code to adding it. Do not add a document, script or abstraction no phase
  asked for.
- In a tracked document, reference code by symbol, never by line number, and **never by commit
  hash**. Every `file:line` these documents once carried had drifted; and `main` is
  squash-only, so the squash orphaned every hash four documents had cited. Use
  `git log <ref> --grep` instead.
- A check that reads a script's own text is not that check. Assert against the artifact, and
  name the test for what it does.

## Verification

`rust-toolchain.toml` pins the toolchain and host triple, so **no `+toolchain` override
belongs in any command or script** — it silently selects another toolchain, and the GNU one
cannot link this product. Everything targets `x86_64-pc-windows-msvc`.

Run the narrowest relevant test while working. **The gate is one command, and it takes no
arguments:**

```powershell
pwsh -NoProfile -File scripts\verify-m0.ps1
```

The stages are clean-root, immutable-actions, dependency-policy, **every** Pester suite in
`scripts/tests`, `fmt`, `clippy`, the SQLite million-row check, the workspace tests and the
release build. Anything less is a weaker claim wearing the same word. It needs network access
for the RustSec database.

The count used to be written here — "sixteen stages, eight Pester suites" — and adding the
ninth suite made both numbers wrong in a file that tells the next reader what green means. The
suites are enumerated from disk by the gate itself, so a number here was never the authority
and could only ever rot; `verify-m0.ps1` now also refuses a run in which any enumerated suite
recorded no passing stage.

**It stops at the first failing stage, so a red gate names one break and hides the rest.** Fix
and re-run until it is green rather than reading the first failure as the only one.

Read the run a push triggers; the required check is `verify`. A Codex bot reviews every pull
request, and its findings are worth reading — it caught a resource criterion that had been
widened past the leak it existed to detect.

## Traps

Each of these cost real time.

- **`expect` and `unwrap` are denied crate-wide** by `#![deny(clippy::unwrap_used,
  clippy::expect_used)]`, with `#[allow]` on specific items. In `src` use `?` or a named
  error; a test module inside `src` needs the attribute, and `clippy` is part of the gate, so
  this surfaces as a gate failure rather than a compile error.
- **`.gitattributes` pins line endings, and the reason is load-bearing.** SQL fixtures are
  embedded with `include_str!`, which keeps file bytes verbatim, and SQLite stores DDL text
  verbatim in `sqlite_schema`; the byte-exact schema guard compares against LF separators. A
  clone with `core.autocrlf=true` breaks those comparisons invisibly. Editors that write LF
  into a CRLF working copy produce `LF will be replaced by CRLF` warnings from git — those are
  normalization working, not a problem.
- **Localization is a closed set in both directions.** Eight fixed-size `[&str; N]` msgid
  arrays, literal totals asserted alongside them, and two catalogues — `ru` and `pseudo`. One
  new label moves an array size, the totals, and both `.po` files. A string emitted from Rust
  rather than through `@tr` escapes the contract by construction.
- **Packaging refuses a dirty tree**: `product packaging requires one clean commit`. Commit
  before building a package, not after.
- **`cargo test` runs test binaries one at a time**, so contention is inside a binary, never
  between them. A slow suite is its own parallel cases and the child processes they spawn.
- **Resource contracts are load-sensitive.** A red one is checked by re-running it alone
  before it is believed, and the reason is recorded beside its constants.
- **The gate fails if the working tree changes while it runs, anywhere.** The
  dependency-policy stage captures state before and after and compares `Commit` and `Dirty`
  among others, so an edit to a document no stage reads still throws `dependency policy inputs
  changed while the check was running`. "No stage reads `docs/`" is true and irrelevant: what
  is compared is whether the tree moved at all. Commit or stash first, then start the gate,
  then keep hands off until it answers.

## Landing work

`main` is protected, and the rules were learned by being refused. No direct push, no force
push, `enforce_admins` on. No merge commit — `required_linear_history`. A pull request is
required: no approving review needed, a green `verify` and every review conversation resolved.
**The only enabled merge method is squash.**

So a merge here is also a delete: squash lands one commit and the head branch is removed
automatically. Before merging a branch whose individual commits carry the measurements that
justified them, tag its head under `history/`. **Create the tag before the merge and push it
after**: a tag push is a `push` event, so pushing it first starts a fresh run of the required
check on a commit that had already passed, and the merge waits another hour for an answer it
already had. The local tag is what protects the commits in the meantime, and it is enough —
GitHub keeps the branch objects on the pull request's own refs until then.

A ruleset makes `history/` undeletable with no bypass actor, which is the only reason any of
this survives: the head branch is deleted automatically the instant a merge lands, twice
observed, and `history/product-architecture` holds the 675 commits that built this product
while `main` has none of them.
