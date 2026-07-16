# TokenMaster Bounded Git Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use
> `superpowers:executing-plans`, `superpowers:test-driven-development`,
> `superpowers:systematic-debugging`, and
> `superpowers:verification-before-completion`. Mark a checkbox only after its
> validator passes.

**Status:** in progress

**Goal:** derive bounded local Git output metrics for repositories associated with
normalized activity, preserve private incremental projections, and expose immutable
query snapshots without shell, path retention, file-content retention, or UI-thread
work.

**Design:**
`docs/superpowers/specs/2026-07-16-tokenmaster-git-output-design.md`

**Tech stack:** Rust 1.97, edition 2024, new isolated `tokenmaster-git` crate, direct
native Git process backend, bundled SQLite schema v13, existing synchronous query
facade, existing constant-state scheduler/worker, and synthetic local repositories.
No `gix`, `git2`, shell, HTTP client, async runtime, credential store, or repository
mutation API.

## Global constraints

- Work only on `cx/tokenmaster-product-architecture`; do not push, package, or claim a
  release.
- External references remain provenance-only at exact pins in
  `third_party/UPSTREAM.toml`; vendor no source.
- Never persist or expose repository/executable paths, author identity, refs, commit
  IDs, file paths, diff/blob content, command text, or raw Git output/error text.
- Query and UI code never start Git processes.
- Git I/O finishes before a non-waiting writer lease and SQLite open.
- Count no author when author identity is missing.
- Keep every collection, process stream, deadline, cache, page, and retained runtime
  object explicitly bounded.
- Use focused red/green tests and independently reviewable checkpoint commits.

---

### Task 1: Freeze provider-neutral Git output values

**Files:**

- Create: `crates/domain/src/git_output.rs`
- Modify: `crates/domain/src/lib.rs`
- Create: `crates/domain/tests/git_output_contract.rs`

**RED/GREEN:**

- [x] Add opaque redacted repository/activity identities.
- [x] Add category, quality, reason, warning, line-count, daily, total, and scan
  projection values.
- [x] Enforce checked counters, ordered unique days/categories, 400 daily points,
  32 repositories, coherent complete/partial/unavailable states, and stable codes.
- [x] Repeat validation during deserialization and reject unknown fields.
- [x] Prove values contain no path, email, branch/ref, commit/file, process, or command
  surface.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-domain --test git_output_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-domain --all-targets --locked
```

**Checkpoint commit:** `feat(domain): add git output contracts`

---

### Task 2: Add pure Git classification and stream contracts

**Files:**

- Create: `crates/git/Cargo.toml`
- Create: `crates/git/src/lib.rs`
- Create: `crates/git/src/identity.rs`
- Create: `crates/git/src/classify.rs`
- Create: `crates/git/src/aggregate.rs`
- Create: `crates/git/src/protocol.rs`
- Create: `crates/git/tests/classification_contract.rs`
- Create: `crates/git/tests/protocol_contract.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**RED/GREEN:**

- [x] Hash common-dir, author, ref-set, and frontier identities with domain-separated
  architecture-independent framing.
- [x] Classify rename destinations under the versioned precedence table without
  retaining paths.
- [x] Parse bounded NUL-framed commit/raw/numstat records incrementally.
- [x] Apply root, ordinary, first-parent merge, binary, and gitlink semantics.
- [x] Emit at most 256 aggregate records per batch with one commit accumulator and no
  whole-history vector.
- [x] Prove malformed/truncated/oversized/overflow input fails closed with stable
  path-free errors and redacted `Debug`.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-git --test classification_contract --locked
cargo +1.97.0 test -p tokenmaster-git --test protocol_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-git --all-targets --locked
```

**Checkpoint commit:** `feat(git): add bounded output core`

---

### Task 3: Add exact native read-only Git backend

**Files:**

- Create: `crates/git/src/command.rs`
- Create: `crates/git/src/discovery.rs`
- Create: `crates/git/src/process.rs`
- Create: `crates/git/src/scan.rs`
- Create: `crates/git/tests/process_contract.rs`
- Create: `crates/git/tests/synthetic_repository_contract.rs`
- Create: `crates/git/tests/process_resource_contract.rs`

**RED/GREEN:**

- [x] Validate explicit/automatic exact native Git discovery with bounded `PATH`.
- [x] Build only fixed version/discovery/config/ref/log commands with stdin null,
  hidden child, paging/locks/prompts/color/external diff/textconv disabled.
- [x] Read stdout/stderr concurrently under exact byte caps and kill/join on deadline,
  cancellation, parser fault, or drop.
- [x] Resolve common-dir identity, local heads, object format, shallow state, and
  author email without retaining raw values.
- [x] Cover root/ordinary/multiple branch/dedup/merge/octopus/rename/binary/submodule/
  worktree/mailmap/empty/missing-author/shallow fixtures.
- [x] Prove no shell, hook, pager, external diff, textconv, network, credential, editor,
  ref/index/worktree/config mutation, child leak, or unbounded retained output.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-git --test process_contract --locked
cargo +1.97.0 test -p tokenmaster-git --test synthetic_repository_contract --locked
cargo +1.97.0 test -p tokenmaster-git --test process_resource_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-git --all-targets --locked
```

**Checkpoint commit:** `feat(git): add read-only native backend`

---

### Task 4: Add transient repository activity hints

**Files:**

- Create: `crates/platform/src/local_directory.rs`
- Create: `crates/provider/src/repository.rs`
- Create: `crates/provider/tests/repository_activity_contract.rs`
- Create: `crates/runtime/tests/repository_hint_contract.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/src/lease.rs`
- Modify: `crates/git/Cargo.toml`
- Modify: `crates/git/src/command.rs`
- Modify: `crates/git/tests/process_contract.rs`
- Modify: `crates/provider/Cargo.toml`
- Modify: `crates/provider/src/capability.rs`
- Modify: `crates/provider/src/lib.rs`
- Modify: `crates/engine/Cargo.toml`
- Modify: `crates/engine/src/ports.rs`
- Modify: `crates/codex/src/parser/effects.rs`
- Modify: `crates/codex/src/parser/state.rs`
- Modify: `crates/codex/src/parser/value.rs`
- Modify: `crates/codex/src/parser/mod.rs`
- Modify: `crates/codex/src/provider.rs`
- Modify: `crates/codex/src/reader/mod.rs`
- Modify: `crates/runtime/src/codex_adapter.rs`
- Modify: `crates/codex/tests/parser_state_contract.rs`
- Modify: `crates/codex/tests/pipeline_contract.rs`
- Modify: `crates/codex/tests/source_discovery_contract.rs`
- Modify: `crates/engine/tests/port_traits_contract.rs`

**RED/GREEN:**

- [x] Add one latest bounded provider-neutral transient repository hint per source
  batch.
- [x] Reject relative/network/device/traversal/reparse-unsafe candidate paths.
- [x] Keep the sealed path non-serializable and redacted and exclude it from parser
  resume/checkpoint/canonical events.
- [x] Preserve safe project alias and exact provider/profile/source/session/time
  association.
- [x] Coalesce repeated metadata/turn-context hints without a path/event history.
- [x] Prove old checkpoints remain compatible and no path reaches store/query/errors/
  diagnostics/`Debug`.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test parser_state_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test pipeline_contract --locked
cargo +1.97.0 test -p tokenmaster-provider --test repository_activity_contract --locked
cargo +1.97.0 test -p tokenmaster-engine --test port_traits_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test repository_hint_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-platform -p tokenmaster-provider -p tokenmaster-codex -p tokenmaster-engine -p tokenmaster-runtime --all-targets --locked
```

**Checkpoint commit:** `feat(codex): emit private repository hints`

---

### Task 5: Add strict schema-v13 incremental Git projection

**Files:**

- Create: `crates/store/src/usage/git_schema.rs`
- Create: `crates/store/src/usage/git_types.rs`
- Create: `crates/store/src/usage/git_write.rs`
- Create: `crates/store/src/usage/git_query.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/usage/schema.rs`
- Modify: `crates/store/src/usage/migration.rs`
- Modify: `crates/store/src/lib.rs`
- Modify: `crates/store/Cargo.toml`
- Create: `crates/store/tests/git_schema_contract.rs`
- Create: `crates/store/tests/git_projection_contract.rs`
- Create: `crates/store/tests/git_incremental_contract.rs`
- Create: `crates/store/tests/git_query_contract.rs`

**RED/GREEN:**

- [ ] Add installation salt, independent Git publication state, repository,
  association, salted ref fingerprint, daily/category, and bounded health objects.
- [ ] Migrate exact v12 transactionally to v13 and validate fresh/v13 archives.
- [ ] Publish complete rebuild or a same-process proven append-only delta atomically
  after exact before/after ref fingerprints.
- [ ] Refresh unchanged scans without history traversal or aggregate mutation.
- [ ] Invalidate restart-with-changed-refs, force-push/deletion/author/mailmap/category/
  object/shallow changes and preserve prior projection stale until rebuild.
- [ ] Add bounded read captures for 32 repositories and 400 daily points.
- [ ] Fault-test every schema/projection/frontier/publication boundary and prove no
  usage/price/quota/benefit/reminder regression.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test git_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test git_projection_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test git_incremental_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test git_query_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-store --all-targets --locked
```

**Checkpoint commit:** `feat(store): add incremental git projection`

---

### Task 6: Add immutable Git query snapshots and efficiency join

**Files:**

- Create: `crates/query/src/git_output.rs`
- Modify: `crates/query/src/service.rs`
- Modify: `crates/query/src/lib.rs`
- Modify: `crates/query/src/calendar.rs`
- Create: `crates/query/tests/git_output_contract.rs`
- Create: `crates/query/tests/git_efficiency_contract.rs`
- Create: `crates/query/tests/git_scale_contract.rs`

**RED/GREEN:**

- [ ] Map store captures into independent immutable Git envelopes with checked
  process-local snapshot generation.
- [ ] Preserve complete/partial/stale/unavailable reasons and exact omission counters.
- [ ] Expose bounded totals/categories/daily points with no path/ref/commit/file data.
- [ ] Join fixed-point cost per 100 product-code lines only for exact compatible
  repository/project/range/freshness/quality evidence.
- [ ] Keep zero lines, unknown cost, conflicts, partial history, ambiguous association,
  and mismatched boundaries explicitly unavailable.
- [ ] Prove 32 repositories, 400 points, concurrent snapshot isolation, no raw event
  scan, read deadline cleanup, restart, corruption rejection, and resource return.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-query --test git_output_contract --locked
cargo +1.97.0 test -p tokenmaster-query --test git_efficiency_contract --locked
cargo +1.97.0 test -p tokenmaster-query --test git_scale_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-query --all-targets --locked
```

**Checkpoint commit:** `feat(query): expose immutable git output`

---

### Task 7: Add bounded Git runtime publication

**Files:**

- Create: `crates/runtime/src/git/mod.rs`
- Create: `crates/runtime/src/git/config.rs`
- Create: `crates/runtime/src/git/execution.rs`
- Create: `crates/runtime/src/git/health.rs`
- Create: `crates/runtime/src/git/runtime.rs`
- Modify: `crates/runtime/src/lib.rs`
- Modify: `crates/runtime/Cargo.toml`
- Create: `crates/runtime/tests/git_runtime_contract.rs`
- Create: `crates/runtime/tests/git_runtime_resource_contract.rs`

**RED/GREEN:**

- [ ] Reuse one constant-state scheduler/worker and retain at most 32 latest transient
  repository hints, one active scan, and one aggregate follow-up.
- [ ] Complete Git discovery/scan/child cleanup before one non-waiting writer lease and
  one store open.
- [ ] Publish unchanged, incremental, rebuild, partial, unavailable, cancelled, and
  stale outcomes truthfully with count-only health.
- [ ] Coalesce activity/manual/resume/clock hints; never scan on UI/query threads.
- [ ] Pause closes admission and cancels the exact child; resume forces rediscovery;
  shutdown/`Drop` join all owned threads/processes.
- [ ] Prove contention-before-SQLite, stale-result rejection, no sibling-runtime fault,
  fixed memory/handles/threads/USER/GDI, and no task-owned child after every path.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test git_runtime_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test git_runtime_resource_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-runtime --all-targets --locked
```

**Checkpoint commit:** `feat(runtime): publish bounded git output`

---

### Task 8: Close authority, documentation, and full quality gates

**Files:**

- Create: `scripts/audit-git-output.ps1`
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: this plan and the design status

**RED/GREEN:**

- [ ] Audit four production boundaries for forbidden shell/network/credential/mutation/
  raw-path/raw-author/raw-output surface and exact lease/I/O/query ordering.
- [ ] Scan release binaries for forbidden private fixture markers and command strings.
- [ ] Confirm no vendored upstream source or foreign production language returned.
- [ ] Run focused Git/store/query/runtime suites, clean-root, format, strict locked
  workspace Clippy, complete locked workspace tests/doctests, dependency review, and
  task-owned process audit.
- [ ] Record only measured evidence and leave P3/P5/M0/package/release unclaimed.

**Final validators:**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
pwsh -NoProfile -File scripts\audit-git-output.ps1 -RepositoryRoot (Get-Location).Path
git diff --check
git status --short
```

**Checkpoint commit:** `docs(git): close bounded output contour`

---

## Stop conditions

Stop and preserve the last passing checkpoint if:

- exact Git behavior cannot be obtained without shell, external diff/textconv, hooks,
  credentials, network, or repository mutation;
- repository association would require persisting an absolute path;
- a process/parser/store/query collection lacks a hard bound;
- incremental authority cannot prove ancestry/ref identity and would publish rewritten
  history as append-only;
- a failed scan would overwrite the last trustworthy projection;
- resource use grows with refresh cycles after bounded warm-up;
- exact Rust 1.97 or bundled SQLite compatibility cannot be preserved.

No P2-E completion claim is allowed before Task 8 passes.
