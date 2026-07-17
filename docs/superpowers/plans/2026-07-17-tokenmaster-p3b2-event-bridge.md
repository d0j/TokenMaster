# TokenMaster P3-B.2 Slint Event-Loop Bridge Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` and complete
> each checkbox in order. Use `superpowers:test-driven-development` for every behavior
> change and `superpowers:verification-before-completion` before completion claims.

**Goal:** Deliver the controller's one latest immutable product snapshot to the Slint
UI through at most one queued event, with no polling, duplicate result slot, blocking
callback, thread, or ownership cycle.

**Architecture:** Expose a receiver view of the existing controller mailbox and one
idle-only weak notifier attachment. Add a bridge with one weak Slint handle, one
scheduled flag, fixed health counters, and one post-drain race recheck. Refactor the
shell's presentation state to a fallible `Arc<Mutex<_>>`; all model access remains on
the UI event thread.

**Tech stack:** Rust 1.97, Slint 1.17.1, existing engine/product/query controller,
headless Slint integration-test backend, Cargo workspace, PowerShell source audit.

---

### Task 1: Freeze mailbox and notifier attachment contracts

**Files:**
- Modify: `crates/desktop/src/controller.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/tests/controller_contract.rs`

- [x] **Step 1: Write failing public contracts**

Require `DesktopSnapshotReceiver`, `DesktopSnapshotNotifier`, one idle-only
`attach_snapshot_notifier`, and stable `busy`/`notifier_already_attached` failures.
The notifier must observe that the latest mailbox is populated before it is called,
including when it attaches after an idle publication.

- [x] **Step 2: Observe the intended compile failure**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test controller_contract --locked
```

Expected: FAIL because receiver/notifier APIs do not exist.

- [x] **Step 3: Implement the minimum shared mailbox API**

Keep one existing `Arc<Mutex<Option<Arc<ProductSnapshot>>>>`; expose only a cloneable
take/has receiver. Retain one optional notifier and invoke it after complete mailbox
replacement, outside all locks. Do not change query completion truth.

- [x] **Step 4: Run the focused controller contract**

Expected: PASS.

### Task 2: Make presentation state safely event-deliverable

**Files:**
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/tests/ui_contract.rs`

- [x] **Step 1: Write failing UI error/ownership tests**

Require `DesktopShell::apply_snapshot` to return a stable result, selection and stale
generation behavior to remain unchanged, and the shared state handle to be usable by
a `Send` delivery closure without making the strong `MainWindow` cross-thread.

- [x] **Step 2: Replace `Rc<RefCell<_>>` with `Arc<Mutex<_>>`**

Add stable `DesktopUiError`/code mapping. Keep state/model operations on the UI thread
and keep route callbacks query-free and non-blocking.

- [x] **Step 3: Run presentation and UI contracts**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
```

Expected: PASS.

### Task 3: Implement the capacity-one event gate

**Files:**
- Create: `crates/desktop/src/bridge.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/ui.rs`

- [x] **Step 1: Add red deterministic gate tests**

With a private manual scheduler, require 10,000 notifications to queue one task and
deliver only newest; a publication inside delivery must queue one follow-up; a failed
schedule must retain the snapshot and retry; close/drop must stop new scheduling.

- [x] **Step 2: Implement bridge core and fixed status**

Use one atomic scheduled flag, saturating atomic counters, weak notifier, shared
mailbox receiver, and post-drain recheck. No timer, loop thread, queue, or second
snapshot slot.

- [x] **Step 3: Add the Slint scheduler and weak-window delivery**

Call `slint::invoke_from_event_loop` exactly once in production source. Upgrade the
weak window inside the queued closure; apply only newer state and clean scheduled
state even if the component is gone.

- [x] **Step 4: Run package unit tests**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --lib --locked
```

Expected: PASS.

### Task 4: Prove a real headless Slint event loop

**Files:**
- Create: `crates/desktop/tests/bridge_event_loop_contract.rs`
- Modify: `crates/desktop/Cargo.toml`

- [x] **Step 1: Add the failing integration test**

Initialize Slint's integration testing backend with an event loop, create the real
generated window/shell, attach the bridge to a controller, submit one refresh, and
quit from a bounded observer after one delivered generation. Assert the generated
window property changed and all owned work joined.

- [x] **Step 2: Run the isolated event-loop test**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test bridge_event_loop_contract --locked
```

Expected: PASS without a visible/native window or external service.

### Task 5: Expand deterministic source boundaries

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`

- [x] **Step 1: Add failing audit fixtures**

Require rejection of a second `invoke_from_event_loop` site, any Slint/standard timer
or polling thread, strong `MainWindow` retention in the bridge, a second latest slot,
and all prior direct authority violations.

- [x] **Step 2: Update the audit**

Require seven Rust/five Slint files, one controller worker, one shared snapshot slot,
one event scheduling site, one weak window, zero UI queries, and zero bridge timer/
thread surfaces.

- [x] **Step 3: Run Pester and release audit**

```powershell
Invoke-Pester -Path scripts\tests\audit-desktop-shell.Tests.ps1 -Output Detailed
pwsh -NoProfile -File scripts\audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
```

Expected: PASS.

### Task 6: Synchronize project truth

**Files:**
- Modify: `spec/DECISIONS.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/CHANGELOG.md`

- [x] **Step 1: Record P3-B.2 honestly**

Record one shared mailbox, one event, race/cleanup behavior, tests, and P3-B.3 as next.
Do not claim production archive composition, visible dashboard payloads, benefit scope
discovery, P4 paint/resource gates, package, signing, or release.

- [x] **Step 2: Check documentation consistency**

```powershell
rg -n "P3-B\.2|event-loop bridge|P3-B\.3" spec docs
git diff --check
```

Expected: consistent current/remaining truth and clean diff.

### Task 7: Run the complete quality gate and checkpoint

**Files:** all files above.

- [ ] **Step 1: Run the baseline quality gate**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

- [ ] **Step 2: Inspect repository and process cleanliness**

Confirm only intentional files changed and no task-owned test, GUI, diagnostic,
watcher, timer, or temporary server process remains.
