# TokenMaster In-App Notification Presentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Present one existing leased expiry-reminder batch in Slint, acknowledge it
only after visible event-loop application, and release/retry safely without blocking
the UI or widening Desktop authority.

**Architecture:** Desktop owns only a bounded present-only value, a weak-window epoch
bridge, and one transient Slint model. Application owns runtime leasing plus one
capacity-one receipt worker; durable acknowledgement stays in the existing reminder
runtime/store and runs outside the UI thread. Every failure, stale callback, bundle
replacement, and shutdown path releases the lease or preserves it for bounded retry.

**Tech Stack:** Rust 1.97.0, Slint 1.17.1 with `winit-software`, bundled SQLite,
PowerShell/Pester deterministic source audits.

## Global Constraints

- Rust is exactly `1.97.0`; Slint is exactly `1.17.1`; the root Cargo workspace is the
  only workspace.
- `tokenmaster-desktop` must not depend on runtime, store in production, platform,
  provider, Codex, filesystem, process, network, browser, shell, or SQL authority.
- A batch contains `1..=256` rows; there is one scheduled presentation, one transient
  Slint model, one receipt action, and one app receipt worker when reminder runtime is
  available.
- Scheduling is not presentation. `Presented` is emitted only after the event-loop
  callback replaces the model and sets the visible panel state.
- Busy/StoreUnavailable acknowledgement retries after exactly 60 seconds off the UI
  thread; failed/cancelled/stale/closed presentation releases the lease.
- No delivery/scope/account/workspace/window identity, path, prompt, response, reasoning,
  command, source content, credential, provider payload, raw OS/SQLite error, receipt,
  or activation authority crosses Desktop/Slint.
- UI owns no timer, polling loop, queue, runtime handle, acknowledgement, or automatic
  dismissal.
- Settings editing, snooze, quiet hours, OS/tray delivery, usage alerts, activation,
  P4/P5/P6, M0, packaging, signing, soak, and release remain out of scope.
- TDD is mandatory: each behavior test must fail for the intended missing behavior
  before production code is added.

---

### Task 1: Present-only Desktop value, epoch bridge, and Slint panel

**Files:**
- Create: `crates/desktop/src/in_app_notification.rs`
- Create: `crates/desktop/ui/components/in-app-notification-panel.slint`
- Create: `crates/desktop/tests/in_app_notification_ui_contract.rs`
- Modify: `crates/desktop/src/bridge.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/ui/models.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/tests/ui_contract.rs`

**Interfaces:**
- Produces:
  `DesktopInAppNotification::new(kind, quantity, label_key, lead_seconds,
  due_at_ms, expiry_at_ms, delivered_at_ms) -> Result<Self, DesktopNotificationError>`.
- Produces:
  `DesktopInAppNotificationBatch::new(Vec<DesktopInAppNotification>) ->
  Result<Self, DesktopNotificationError>` with `MAX_DESKTOP_IN_APP_NOTIFICATIONS = 256`.
- Produces object-safe `DesktopNotificationPresentationReceipt::{presented, failed}`.
- Produces `DesktopInAppNotificationBridge::present(batch, receipt)` and fixed
  `DesktopNotificationBridgeSnapshot` health.
- Extends `DesktopBridgeFactory::in_app_notification_bridge()` with independent checked
  epochs; it does not reuse product-snapshot generation semantics.

- [x] **Step 1: Write RED value and bridge tests**

Add tests that compile against the wished-for public API and assert exact bounds,
event ordering, one in-flight presentation, stale/closed failure, and count-only Debug:

```rust
#[test]
fn scheduling_is_not_a_presentation_receipt() {
    let scheduler = Arc::new(ManualScheduler::default());
    let delivery = Arc::new(RecordingDelivery::default());
    let bridge = DesktopInAppNotificationBridge::with_parts_for_test(
        scheduler.clone(),
        delivery,
    );
    let receipt = Arc::new(RecordingReceipt::default());

    bridge
        .present(one_notification_batch(), receipt.clone())
        .expect("schedule one batch");

    assert_eq!(receipt.presented_count(), 0);
    assert_eq!(receipt.failed_count(), 0);
    scheduler.run_one();
    assert_eq!(receipt.presented_count(), 1);
}
```

The test matrix must include empty/256/257 rows, zero quantity, invalid localization
key, invalid time ordering, schedule rejection, bridge drop before callback, stale
epoch, closed window, duplicate `present`, and 10,000 rejected/coalesced attempts.

- [x] **Step 2: Run RED and record the intended failure**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop in_app_notification --locked
```

Expected: compilation fails because the new Desktop notification API does not exist.

- [x] **Step 3: Implement the bounded Desktop API and bridge**

Use these exact public shapes:

```rust
pub const MAX_DESKTOP_IN_APP_NOTIFICATIONS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopNotificationKind {
    BankedRateLimitReset,
    UsageCredit,
    TemporaryUsage,
    Unknown,
}

pub trait DesktopNotificationPresentationReceipt: Send + Sync + 'static {
    fn presented(&self);
    fn failed(&self);
}

pub struct DesktopInAppNotificationBridge {
    epoch: DesktopNotificationEpoch,
    inner: Arc<NotificationBridgeInner>,
}

impl DesktopInAppNotificationBridge {
    pub fn present(
        &self,
        batch: DesktopInAppNotificationBatch,
        receipt: Arc<dyn DesktopNotificationPresentationReceipt>,
    ) -> Result<(), DesktopNotificationError>;
}
```

Share the already proved `EventScheduler`/`SlintEventScheduler` machinery from
`bridge.rs` through `pub(crate)` visibility. Do not add another event-loop abstraction,
thread, timer, or queue.

- [x] **Step 4: Add the transient compiled Slint panel**

Add exactly one model struct:

```slint
export struct InAppNotificationRow {
    benefit-label: string,
    quantity-label: string,
    lead-label: string,
    due-label: string,
    expiry-label: string,
    delivered-label: string,
    accessible-label: string,
}
```

`InAppNotificationPanel` must be a persistent overlay with a bounded `ScrollView`, a
truthful count label, all row meanings, an accessible group label, and a Dismiss button.
It has no `Timer`, animation, auto-hide, callback except dismissal, or route mutation.
`DesktopShell` dismissal replaces the model with an empty `VecModel` and sets visibility
false.

- [x] **Step 5: Run GREEN Desktop tests and strict package checks**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop in_app_notification --locked
cargo +1.97.0 test -p tokenmaster-desktop --test in_app_notification_ui_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-desktop --all-targets --locked
```

Expected: all selected tests pass and Clippy emits zero warnings.

- [x] **Step 6: Commit the independently reviewable Desktop layer**

```powershell
git add -- crates/desktop/src/in_app_notification.rs crates/desktop/src/bridge.rs crates/desktop/src/lib.rs crates/desktop/src/ui.rs crates/desktop/ui/components/in-app-notification-panel.slint crates/desktop/ui/models.slint crates/desktop/ui/main.slint crates/desktop/tests/in_app_notification_ui_contract.rs crates/desktop/tests/ui_contract.rs
git diff --cached --check
git commit -m "feat(desktop): add in-app notification presenter"
```

---

### Task 2: App-owned lease, receipt worker, acknowledgement retry, and lifecycle

**Files:**
- Create: `crates/app/src/notification.rs`
- Create: `crates/app/src/notification_tests.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/application.rs`
- Modify: `crates/app/src/application_tests.rs`
- Modify: `crates/app/Cargo.toml`

**Interfaces:**
- Consumes `DesktopInAppNotificationBatch`, `DesktopInAppNotificationBridge`, and
  `DesktopNotificationPresentationReceipt` from Task 1.
- Produces app-private `ReminderPresentationPort` with `take`, `acknowledge`, and
  `release`; its real adapter is the only layer allowed to touch
  `BenefitReminderRuntime`.
- Produces app-private `ReminderPresentationCoordinator::{start,pump,shutdown}`.
- Refactors only the optional reminder owner to `Arc<Mutex<BenefitReminderRuntime>>`;
  quota/live runtime ownership remains unchanged.

- [x] **Step 1: Write RED coordinator/worker tests with deterministic fakes**

Use a fake port and fake presenter, not a mocked database implementation. The port
models the externally observable lease protocol:

```rust
trait ReminderPresentationPort: Send + Sync + 'static {
    fn take(&self) -> Result<Option<DesktopInAppNotificationBatch>, PresentationFailure>;
    fn acknowledge(&self) -> Result<bool, PresentationFailure>;
    fn release(&self) -> Result<bool, PresentationFailure>;
}
```

Required tests:

```rust
#[test]
fn visible_receipt_precedes_acknowledgement() {
    let port = Arc::new(FakePort::with_one_batch());
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_millis(5),
    );

    coordinator.pump().expect("lease and schedule");
    assert_eq!(port.acknowledge_count(), 0);
    presenter.complete_presented();
    wait_until(|| port.acknowledge_count() == 1);
    coordinator.shutdown().expect("joined shutdown");
}
```

Also cover schedule failure/callback failure release, Busy then success retry, internal
failure release, one-shot contradictory receipt, 10,000 pump calls with one take, and
shutdown during scheduled/leased/retry states.

- [x] **Step 2: Run RED and record the intended failure**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-app notification --lib --locked
```

Expected: compilation fails because the app notification coordinator is absent.

- [x] **Step 3: Implement the capacity-one receipt worker**

Use one mutex/condition-variable state, one worker, and an injected retry duration for
tests. Production uses the exact constant:

```rust
const NOTIFICATION_ACK_RETRY: Duration = Duration::from_secs(60);

enum ReceiptAction {
    Presented,
    Failed,
}

struct ReceiptWorkerState {
    action: Option<ReceiptAction>,
    stopping: bool,
}
```

The worker never retains a batch. It retries only Busy/StoreUnavailable, releases on
failed presentation or terminal failure, and wakes immediately on shutdown. A receipt
has an `AtomicBool` one-shot guard.

- [x] **Step 4: Implement the real reminder adapter and mapping**

Add the direct `tokenmaster-domain` app dependency and exact kind/channel mapping:

```rust
match delivery.channel() {
    NotificationChannel::InApp => {}
    NotificationChannel::OsScheduled => return Err(PresentationFailure::InvalidData),
}

let kind = match delivery.kind() {
    BenefitKind::BankedRateLimitReset => DesktopNotificationKind::BankedRateLimitReset,
    BenefitKind::UsageCredit => DesktopNotificationKind::UsageCredit,
    BenefitKind::TemporaryUsage => DesktopNotificationKind::TemporaryUsage,
    BenefitKind::Unknown => DesktopNotificationKind::Unknown,
};
```

The adapter drops the returned runtime batch immediately after building one bounded
Desktop batch. It maps only stable error codes and exposes no path or inner error.

- [x] **Step 5: Integrate the coordinator into application bundle lifecycle**

Create the Desktop notification bridge and coordinator only when reminder runtime
starts. Before fallible controller publication, call `pump`; ordinary completion hints
are safe because the runtime lease makes duplicates no-ops. A released failed presentation
is re-pumped by the existing receipt worker, not by a second timer/thread. A terminal
acknowledgement error releases without automatic re-presentation. Shutdown order is:

1. invalidate/close Desktop notification bridge;
2. stop and join receipt worker, releasing outstanding lease;
3. pause controller/runtime admissions as already specified;
4. shutdown reminder runtime and remaining owners.

No bundle mutex may be held across worker join or event-loop execution.

- [x] **Step 6: Run GREEN app tests and real integration**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-app notification --lib --locked
cargo +1.97.0 test -p tokenmaster-app application --lib --locked
cargo +1.97.0 test -p tokenmaster-runtime --test reminder_runtime_contract --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-app --all-targets --locked
```

Expected: presentation tests pass, the existing runtime replay/ack/release suite remains
green, and no warning is emitted.

- [x] **Step 7: Commit the application composition layer**

```powershell
git add -- crates/app/Cargo.toml crates/app/src/lib.rs crates/app/src/notification.rs crates/app/src/notification_tests.rs crates/app/src/application.rs crates/app/src/application_tests.rs Cargo.lock
git diff --cached --check
git commit -m "feat(app): acknowledge visible expiry notifications"
```

---

### Task 3: Adversarial audit, project truth, independent review, and baseline

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify: `scripts/audit-application-composition.ps1`
- Modify: `scripts/tests/audit-application-composition.Tests.ps1`
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`
- Modify: this plan

**Interfaces:**
- Consumes the exact Desktop/app behavior from Tasks 1-2.
- Produces computed audit receipts for one app receipt worker, one transient model,
  256 rows, post-apply receipt ordering, failure release, 60-second retry, checked epoch,
  and zero Desktop runtime/store/SQL/polling authority.
- Produces ADR-073 with the selected app-owned receipt-worker decision.

- [x] **Step 1: Write RED source-audit mutations**

Mutate each required anchor independently and require audit failure:

- remove the 256 cap;
- add a second model or receipt worker;
- move `presented()` before model/visibility application;
- remove schedule/callback/shutdown release;
- accept `Err`/`false` release or clear local backpressure first;
- remove panic rollback/outer-mutex fallback release;
- remove same-worker re-pump or immediate receipt wake;
- re-present a terminal acknowledgement error;
- invoke a receipt before clearing Desktop bridge-busy state;
- omit the visible benefit label from accessibility text;
- change retry from 60 seconds;
- remove epoch invalidation;
- add Desktop runtime/store dependency/import;
- add UI timer/polling/auto-hide;
- expose delivery/private identity;
- acknowledge on route selection or product projection.

- [x] **Step 2: Run RED audits**

Run:

```powershell
Invoke-Pester -Path scripts\tests\audit-desktop-shell.Tests.ps1,scripts\tests\audit-application-composition.Tests.ps1 -Output Detailed
```

Expected: new mutation cases fail until production audits enforce their anchors.

- [x] **Step 3: Implement computed audits and run GREEN**

The audit output must compute counts rather than merely grep for a comment. Run:

```powershell
pwsh -NoProfile -File scripts\audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path
Invoke-Pester -Path scripts\tests\audit-desktop-shell.Tests.ps1,scripts\tests\audit-application-composition.Tests.ps1 -Output Detailed
```

Expected: both audits and every mutation pass, with no `testResults.xml` retained.

- [x] **Step 4: Synchronize normative and operational documents**

Record visible in-app expiry presentation and post-apply durable acknowledgement as
implemented. Keep settings editing, snooze, quiet hours, OS/tray delivery, usage alerts,
activation, P4/P5/P6, M0, package/signing/soak/release explicitly incomplete. Do not
write the current commit hash into tracked documents.

- [x] **Step 5: Run focused composition and independent critical review**

Run:

```powershell
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-desktop -p tokenmaster-app --all-targets --locked
cargo +1.97.0 test -p tokenmaster-desktop --locked
cargo +1.97.0 test -p tokenmaster-app --locked
cargo +1.97.0 test -p tokenmaster-runtime --test reminder_runtime_contract --locked
```

Then dispatch one read-only Sol High review for correctness, lifecycle/deadlock,
false-ack/crash/retry semantics, stale callback, shutdown/resource return, privacy,
accessibility, and audit sufficiency. Fix every Critical/Important finding under a new
RED test and re-run the reviewer.

Result: the repeated read-only Sol High review is READY with Critical 0, Important 0,
and Minor 0 after the terminal-acknowledgement RED regression and mutation passed.

- [x] **Step 6: Run the exact repository baseline**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Expected: every command exits zero. This proves developer closure only, not M0,
interactive Windows, soak, package, signing, or release acceptance.

Result: the exact sequence passed in 1014.4 seconds, including `TM-CLEAN-PASS`,
workspace Clippy with warnings denied, and every workspace test. This remains developer
evidence only.

- [x] **Step 7: Commit synchronized evidence and verify clean state**

```powershell
git add -- <explicit lifecycle code and test paths>
git diff --cached --check
git commit -m "fix(app): harden notification receipt lifecycle"
git add -- <explicit audit, specification, and documentation paths>
git diff --cached --check
git commit -m "docs: close in-app notification presentation"
git status --short
```

Result: lifecycle implementation/tests and synchronized evidence are recorded as two
reviewable commits; final status is empty.
