# TokenMaster Codex Quota Transport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:executing-plans and superpowers:test-driven-development. Track every task
> with checkbox syntax and verify before marking complete.

**Status:** approved and in progress on 2026-07-16

**Goal:** read official live Codex quota windows through one bounded short-lived local
app-server session and emit validated provider-neutral quota observations without
handling credentials or raw private endpoints.

**Design:** `docs/superpowers/specs/2026-07-16-tokenmaster-codex-quota-transport-design.md`

**Tech stack:** Rust 1.97, edition 2024, standard-library process/thread/channel I/O,
`serde_json = 1.0.150`, `sha2 = 0.11.0`, existing TokenMaster quota domain. No async
runtime, HTTP client, browser dependency, shell command construction, or new network
crate.

## Global constraints

- Work on `cx/tokenmaster-product-architecture`; do not push, package, or claim release.
- Use red/green TDD and commit independently reviewable checkpoints.
- The connector accepts an exact executable path; it never executes caller-provided
  arguments or shell text.
- Never print, persist, or return Codex home, email, raw frames, credentials, reset
  credit IDs, or inner process errors.
- Never hold a SQLite transaction, writer lease, UI callback, or query snapshot across
  app-server I/O.
- Keep benefit persistence/reminders/activation and UI outside this plan.
- Run the narrowest tests first, then the repository baseline.

---

### Task 1: Freeze strict Codex quota wire normalization

**Files:**

- Create: `crates/codex/src/quota/mod.rs`
- Create: `crates/codex/src/quota/wire.rs`
- Create: `crates/codex/src/quota/normalize.rs`
- Modify: `crates/codex/src/lib.rs`
- Create: `crates/codex/tests/quota_normalization_contract.rs`

**RED:**

- [x] Add current multi-bucket fixture with one legacy duplicate, primary/secondary
  windows, and reset credits.
- [x] Add legacy-only fallback.
- [x] Add account pseudonym/account-switch isolation.
- [x] Add ratio/time/duration/freshness/observation-ID exact vectors.
- [x] Add unknown field, invalid percent/time/count/string, bucket mismatch, window
  overflow, missing email, and clock overflow cases.
- [x] Prove public/debug/error surfaces contain no fixture email, Codex home, raw credit
  ID, or raw response.

**GREEN:**

- [x] Implement strict private wire structs.
- [x] Implement bounded provider string/count validation.
- [x] Implement domain-separated account/window/observation identities.
- [x] Normalize authoritative multi-bucket data without the legacy duplicate.
- [x] Emit immutable definition/sample pairs with explicit freshness and medium
  inference confidence.
- [x] Decode reset credits only transiently and return no benefit inventory yet.

**Verify:**

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test quota_normalization_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-codex --all-targets --locked
```

---

### Task 2: Add bounded short-lived app-server transport

**Files:**

- Create: `crates/codex/src/quota/transport.rs`
- Create: `crates/codex/src/bin/codex_app_server_fixture.rs`
- Create: `crates/codex/tests/quota_transport_contract.rs`
- Modify: `crates/codex/src/quota/mod.rs`
- Modify: `crates/codex/src/lib.rs`

**RED:**

- [x] Add fixture modes for success, stable JSON-RPC error, unsupported user-agent
  version, malformed/unknown/oversized frame, wrong/duplicate/out-of-order ID, early
  EOF, hang, and stderr noise.
- [x] Prove exact fixed argv and initialize/account/quota request shapes.
- [x] Prove success, every failure, and timeout terminate/reap the child and join the
  helper thread.
- [x] Prove command/path and fixture-private values are redacted.

**GREEN:**

- [x] Implement path-private exact native executable descriptor.
- [x] Implement bounded line reader and complete-output/frame caps.
- [x] Implement one helper thread and one monotonic deadline.
- [x] Parse only the tested stable protocol version and never set `experimentalApi`.
- [x] Map all transport failures to stable redacted codes.
- [x] Use hidden/no-console child creation on Windows.

**Verify:**

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test quota_transport_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-codex --all-targets --locked
```

---

### Task 3: Adversarial privacy and resource gate

**Files:**

- Create: `crates/codex/tests/quota_transport_adversarial_contract.rs`
- Create: `crates/codex/tests/quota_transport_resource_contract.rs`
- Create: `scripts/audit-codex-quota-transport.ps1`

**RED/GREEN:**

- [x] Fuzz-like bounded fixture matrix cannot escape strict schema/count/time limits.
- [x] Repeated success/error/timeout polls return process private memory, handles,
  threads, USER, and GDI counts to the documented Windows tolerance.
- [x] Source/dependency/release-library audit rejects browser, cookie, private endpoint,
  credential-file parsing, shell construction, listener/socket server, and raw payload
  persistence.
- [x] Confirm no task-owned fixture/app-server process remains after the tests.

**Verify:**

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test quota_transport_adversarial_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test quota_transport_resource_contract --locked
pwsh -NoProfile -File scripts\audit-codex-quota-transport.ps1 -RepositoryRoot (Get-Location).Path
```

---

### Task 4: Project truth and full verification

**Files:**

- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`

**Actions:**

- [ ] Record official app-server stable-surface boundary, exact version gate, process
  bounds, pseudonymous account limitation, and no-workspace-ID limitation.
- [ ] Mark the Codex quota source connector implemented only after Tasks 1-3 pass.
- [ ] Keep TM-FUNC-009 partial until runtime ingestion and UI exist.
- [ ] Record the next blocker as dedicated quota refresh scheduling/writer coordination;
  keep TM-DATA-009 benefit inventory/reminders separate.
- [ ] Run full baseline and inspect the final diff for private paths/data.

**Baseline:**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```
