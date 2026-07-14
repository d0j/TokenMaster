# TokenMaster Single-Root Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a single-root, clean-history TokenMaster repository containing only the active Rust product, its neutral documentation, and required MIT provenance.

**Architecture:** Promote the active Rust workspace to repository root on an orphan branch, then reconstruct the product contracts, CI, and handoff around TokenMaster. WhereMyTokens and ccusage remain pinned external references, not dependencies or copied implementations. A content audit proves the root contains no second product workspace or obsolete identifier.

**Tech Stack:** Rust 1.97.0, edition 2024, Slint 1.17.1, bundled SQLite 3.53.2, PowerShell, GitHub Actions, MIT provenance records.

## Global Constraints

- Keep exactly one Rust workspace at repository root.
- Retain only TokenMaster source, tests, scripts, documentation, branding, MIT license, and external-reference provenance.
- Do not vendor or execute external application source.
- Do not retain prompts, responses, reasoning, commands, source-file contents, or credentials.
- Never claim M0 acceptance, package, signing, or release without the documented receipt gates.
- All user-facing project documentation is English except the Russian README; source identifiers remain English.

---

### Task 1: Create the clean history and root workspace

**Files:**
- Create: root `Cargo.toml`, `Cargo.lock`, and `rust-toolchain.toml`
- Create: root `.cargo/`, `crates/`, and `scripts/`
- Create: root `M0_ACCEPTANCE.md` and `.gitignore`

- [x] **Step 1: Verify the active workspace before promotion**

Run: `cargo +1.97.0 test --workspace --locked`

Expected: exit code 0.

- [x] **Step 2: Create an orphan branch without inherited tracked content**

Run: `git switch --orphan cx/tokenmaster-clean-history`

Expected: the branch has no parent commit and every prior file is untracked.

- [x] **Step 3: Promote only the active workspace to root**

Move the listed files and directories with PowerShell `Move-Item`. Do not move
`target/` or `reports/`; they are generated local output.

- [x] **Step 4: Stage only the promoted root workspace and required project files**

Run: `git add -- Cargo.toml Cargo.lock rust-toolchain.toml .cargo crates scripts M0_ACCEPTANCE.md .gitignore`

Expected: `git diff --cached --name-only` contains no nested product path.

### Task 2: Rebuild the TokenMaster project surface

**Files:**
- Create: `README.md`
- Create: `README_RU.md`
- Create: `AGENTS.md`
- Create: `CONTRIBUTING.md`
- Create: `docs/CURRENT_STATE.md`
- Create: `docs/PROJECT_HISTORY.md`
- Create: `docs/HANDOFF.md`
- Create: `docs/ROADMAP.md`
- Create: `docs/FEATURE_PARITY.md`
- Create: `docs/ARCHITECTURE.md`
- Create: `docs/CHANGELOG.md`
- Create: `spec/SPECIFICATION.md`
- Create: `spec/DATA_CONTRACT.md`
- Create: `spec/API_CONTRACT.md`
- Create: `spec/SECURITY.md`
- Create: `spec/TRACEABILITY.md`
- Create: `spec/DECISIONS.md`

- [x] **Step 1: Define the reference hierarchy**

State in the README, feature matrix, specification, and decisions that
WhereMyTokens is the UI/product reference, ccusage is the usage-analysis reference,
and TokenMaster owns all implementation and final behavior decisions.

- [x] **Step 2: Record exact current implementation truth**

Document the implemented M0 proof and completed M1 reader/store tasks, then state
that staging generations and scan epochs are the next unimplemented slice. Do not
record a commit hash in tracked documents.

- [x] **Step 3: Define the privacy, bounds, and release gates**

Carry forward only the TokenMaster contracts for bounded JSONL processing, path-private
storage, snapshot UI, SQLite configuration, and M0 evidence boundaries.

### Task 3: Rebuild CI, licensing, and provenance

**Files:**
- Modify: `.github/workflows/tokenmaster-m0-windows.yml`
- Retain: `.github/ISSUE_TEMPLATE/`
- Retain: `.github/PULL_REQUEST_TEMPLATE.md`
- Retain: `.github/SECURITY.md`
- Retain: `LICENSE`
- Retain: `third_party/UPSTREAM.toml`
- Retain: `third_party/licenses/ccusage-MIT.txt`
- Retain: `third_party/licenses/WhereMyTokens-MIT.txt`
- Retain: `docs/assets/github/`

- [x] **Step 1: Point CI at the root workspace**

Replace nested paths in the workflow with root paths. The verification command is
`./scripts/verify-m0.ps1 -RepositoryRoot $PWD.Path`; receipts are under `reports/`.

- [x] **Step 2: Verify provenance language is precise**

Keep exact upstream URL, commit, MIT license, and non-vendoring statement. Do not
claim compatibility, copied code, or external-runtime dependency.

### Task 4: Add and execute the clean-root audit

**Files:**
- Create: `scripts/audit-clean-root.ps1`
- Create: `scripts/tests/audit-clean-root.Tests.ps1`

- [x] **Step 1: Write the failing audit test**

The Pester test creates an isolated fixture containing a forbidden second
`Cargo.toml`, invokes `audit-clean-root.ps1`, and asserts a non-zero exit code with
the stable marker `TM-CLEAN-SECOND-WORKSPACE`.

- [x] **Step 2: Run the Pester test to verify it fails**

Run: `Invoke-Pester .\scripts\tests\audit-clean-root.Tests.ps1 -CI`

Expected: failure because the audit script does not exist.

- [x] **Step 3: Implement the minimal audit**

The script accepts only `-RepositoryRoot`, validates that it is an absolute existing
directory, allows the root `Cargo.toml` only, rejects nested Rust/Go/Node manifests,
rejects forbidden project identifiers outside the explicit provenance and license
allowlist, and returns stable markers without exposing arbitrary paths.

- [x] **Step 4: Run the focused test and repository audit**

Run: `Invoke-Pester .\scripts\tests\audit-clean-root.Tests.ps1 -CI`

Expected: all tests pass.

Run: `pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path`

Expected: exit code 0 and one `TM-CLEAN-PASS` line.

### Task 5: Verify and record the clean root

**Files:**
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/HANDOFF.md`
- Modify: `spec/TRACEABILITY.md`

- [x] **Step 1: Run source and policy audits**

Run: `git ls-files | Select-String -Pattern '(^|/)(portable|apps|tokenmaster)/|^go\.mod$|^package\.json$'`

Expected: no output.

Run: `pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path`

Expected: no output.

- [x] **Step 2: Run the root workspace quality gate**

Run: `cargo +1.97.0 fmt --manifest-path Cargo.toml --all -- --check`

Expected: exit code 0.

Run: `$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --manifest-path Cargo.toml --workspace --all-targets --locked`

Expected: exit code 0.

Run: `cargo +1.97.0 test --manifest-path Cargo.toml --workspace --locked`

Expected: exit code 0.

- [x] **Step 3: Run M0 script evidence and inspect the staged tree**

Run: `pwsh -NoProfile -File scripts\verify-m0.ps1 -RepositoryRoot (Get-Location).Path`

Expected: exit code 0 without a release or acceptance claim.

Run: `git diff --cached --check; git status --short`

Expected: staged source contains only the clean TokenMaster root and ignored local
build/report output remains untracked.

- [x] **Step 4: Commit the initial TokenMaster root**

Run: `git commit -m "chore: establish clean TokenMaster root"`

Expected: a root commit with no parent.
