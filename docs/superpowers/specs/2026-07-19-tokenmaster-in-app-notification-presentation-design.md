# TokenMaster In-App Notification Presentation Design

Status: approved for autonomous execution by the operator on 2026-07-19. The operator
continued after the prior milestone named this exact app-owned notification bridge as
the next critical slice.

## 1. Goal

Close the crash gap between the existing durable reminder outbox and a visible
TokenMaster in-app notification. A bounded runtime batch must become one visible,
accessible Slint surface before its durable acknowledgement is written. Scheduling an
event-loop closure, publishing a product snapshot, or selecting the Notifications route
is not presentation proof.

This slice is deliberately narrower than notification policy. It does not edit reminder
profiles, snooze, implement quiet hours, schedule Windows notifications, add usage
threshold alerts, or activate a provider benefit.

## 2. Existing truth

- `BenefitReminderRuntime` retains at most one provider-neutral batch of 256 deliveries.
- `take_notifications` changes the process-local slot from ready to leased and returns a
  bounded copy. It does not change durable acknowledgement truth.
- `acknowledge_notifications` writes the immutable schema-v12 acknowledgement and keeps
  a failed acknowledgement leased for retry.
- `release_notifications` returns a leased batch to ready state.
- unacknowledged outbox rows replay after restart; acknowledged rows remain deduplicated.
- `tokenmaster-app` is the sole composition owner of reminder runtime and Slint.
- `tokenmaster-desktop` and the Notifications route have no runtime, store, SQL, delivery
  identity, acknowledgement, or provider-mutation authority.

## 3. Options considered

### A. Extend the product snapshot/event bridge

Rejected. Product generations are replaceable read truth; reminder deliveries are a
leased mutation protocol. Combining them would make an ignored/stale product snapshot
ambiguous delivery evidence and would tempt the frontend to acknowledge on ordinary
route refresh.

### B. Acknowledge directly in the Slint callback

Rejected. Runtime acknowledgement can acquire the writer lease, open SQLite, and fail
with contention. Performing that work on the UI event loop would violate the maximum-
responsiveness requirement and could visibly freeze the application.

### C. Extend the existing reminder worker with presentation commands

Rejected for this slice. It avoids one constant thread but changes the already-proved
P2 scheduler/backpressure state machine, mixes due-queue execution with a UI receipt,
and requires new retry scheduling semantics while `notification_pending` intentionally
blocks due work. The risk and regression surface outweigh the small fixed resource
saving.

### D. Add one app-owned capacity-one receipt worker

Selected. The application leases and maps the batch, Desktop schedules and applies only
safe presentation data, and a one-shot receipt wakes one constant application worker.
That worker performs durable acknowledgement outside the UI thread. It retains no
delivery copy, path, SQLite handle, history, or unbounded queue.

## 4. Ownership and components

### Desktop presentation value

`tokenmaster-desktop` owns a provider-neutral `DesktopInAppNotificationBatch` with:

- one through 256 rows;
- benefit kind, positive quantity, bounded localization key, lead seconds;
- due, expiry, and committed-delivery UTC milliseconds;
- no delivery/scope/account/workspace/window identity;
- no path, prompt, response, reasoning, command, credential, provider payload, receipt,
  activation capability, or runtime/store handle.

Construction validates the fixed bounds and time ordering. `Debug` reports only safe
kind/count/time facts and redacts the localization key.

### Desktop event-loop bridge

`DesktopBridgeFactory` creates a separate `DesktopInAppNotificationBridge` for each
application-bundle generation. It owns:

- one weak `MainWindow`;
- one monotonically checked presentation epoch;
- one scheduled flag;
- fixed saturating count/failure fields;
- no model queue, timer, worker, runtime, query, store, or strong window cycle.

`present(batch, receipt)` schedules exactly one event-loop closure. The closure verifies
that the bridge epoch is still active, upgrades the weak window, replaces the one
transient Slint model, makes the panel visible, and only then calls `receipt.presented()`.
If the bridge is closed/stale, the window is gone, presentation state is unavailable,
or scheduling fails, it calls/returns failure and never claims presentation.

The panel is persistent until the user dismisses it; there is no auto-hide timer or
animation. It exposes every row through one bounded scroll model, a truthful batch
count, an accessible group name, and a direct route-independent explanation. Dismissal
clears the transient model but does not grant acknowledgement authority.

### Application coordinator

`tokenmaster-app` owns `ReminderPresentationCoordinator`. Its runtime adapter wraps the
existing reminder owner behind one application-private `Arc<Mutex<_>>`; Desktop never
receives this adapter. `pump()` is invoked after runtime health publication on the
already existing lossy completion hint path:

1. take at most one runtime batch;
2. reject any non-in-app or invalid mapped value and release the lease;
3. drop the bounded runtime copy after building the Desktop batch;
4. schedule the Desktop presentation with a one-shot application receipt;
5. release immediately if event scheduling is rejected.

Generic usage/quota/Git completion hints may call `pump`, but the runtime slot makes all
calls after the first exact no-ops while a batch is leased or acknowledging. Ten
thousand hints therefore cannot queue or duplicate presentation work.

### Receipt worker

One `tokenmaster-notification-receipt` thread owns a capacity-one state and a weakly
typed application runtime port. It retains neither the batch nor its visible values.

- `Presented` calls `acknowledge_notifications` off the UI thread.
- `Busy` and `StoreUnavailable` keep the batch leased and retry after exactly 60 seconds.
- an internal/closed/faulted acknowledgement failure releases the lease and records a
  stable app-private failure.
- `Failed` releases the lease without acknowledgement.
- duplicate or contradictory receipt completion is ignored by a one-shot atomic gate.
- shutdown wakes the worker immediately, releases any outstanding leased batch, joins
  the thread, and only then permits reminder-runtime pause/shutdown.

The retry wait uses one condition variable only while a presented batch remains
unacknowledged. It is not a UI timer, polling loop, or per-delivery allocation.

## 5. Lifecycle and race rules

- A bundle generation owns one presentation bridge and one receipt worker.
- Bundle replacement invalidates the old Desktop epoch before old callback execution.
- An old queued callback reports failure and cannot overwrite a newer bundle's panel.
- A successful callback submits exactly one `Presented` receipt after model replacement
  and visibility application.
- A failed callback, closed window, rejected event-loop schedule, invalid mapping,
  coordinator shutdown, or application cancellation releases the lease.
- Acknowledgement contention never causes a second visible presentation in the same
  process; the leased batch remains the sole retry authority.
- Process death before durable acknowledgement replays the outbox on restart.
- Process death after durable acknowledgement does not recreate the event.
- The application never holds its bundle mutex across worker join or Slint event-loop
  execution.

## 6. Resource limits

- runtime delivery batch: existing maximum 256;
- transient Desktop rows: maximum 256;
- Slint notification models: exactly one;
- scheduled presentation closures: maximum one per bundle;
- receipt actions: maximum one;
- receipt workers: exactly one when the reminder runtime is available, otherwise zero;
- retry state: one enum plus one deadline, no history;
- timers/threads in Desktop/Slint: zero;
- acknowledgement retry interval: 60 seconds;
- user-visible automatic lifetime: none; dismissal is explicit.

The implementation must prove repeated replacement and shutdown return to a fixed
thread/handle/model topology. P4/P6 retain physical-display, interactive accessibility,
and product-release resource evidence.

## 7. Error handling

- invalid or over-cap presentation input fails closed before event scheduling;
- a scheduling error returns a stable presentation code and releases the runtime lease;
- a stale epoch or missing window completes the receipt as failed;
- a poisoned internal mutex becomes a stable internal failure and releases where safe;
- acknowledgement Busy/StoreUnavailable is retryable without UI blocking;
- unrecoverable acknowledgement failure does not fabricate success;
- all errors and `Debug` remain path-free and content-free.

No error path logs or surfaces a delivery ID, provider/account/workspace identity,
archive path, SQLite/OS text, or localization payload beyond the already approved
bounded presentation key.

## 8. Testing and audit

### Desktop

- batch validation for zero, 256, and 257 rows plus invalid time/key/channel facts;
- scheduling is not presentation;
- event-loop application precedes `Presented` receipt;
- schedule rejection, window close, stale epoch, and bridge drop fail the receipt;
- only one closure/model exists and a second in-flight presentation is rejected;
- dismissal clears model/visibility without runtime callback;
- compiled wide/narrow panel keeps complete accessible row meaning.

### Application

- take -> schedule -> visible callback -> acknowledge;
- schedule failure/callback failure -> release -> re-take remains possible;
- acknowledgement Busy/StoreUnavailable retries without another presentation;
- one-shot receipt rejects presented/failed duplication;
- 10,000 pump calls retain one presentation;
- shutdown with scheduled, leased, or retrying work releases and joins;
- bundle replacement prevents stale callback acknowledgement/application;
- real reminder/store integration proves pre-ack restart replay and post-ack no replay.

### Deterministic gates

The Desktop/application audits must pin one app receipt worker, one transient model,
the 256-row bound, epoch invalidation, post-apply receipt ordering, off-UI
acknowledgement, failure release, 60-second retry, zero Desktop runtime/store imports,
zero UI timers/polling, and no private identity fields. Focused tests precede strict
package Clippy, source/release audits, the exact repository baseline, and an independent
read-only review.

## 9. Documentation truth

On completion, project truth may claim visible in-app expiry delivery with crash-safe
post-presentation acknowledgement. It must still say that reminder settings editing,
snooze, quiet hours, OS/tray delivery, usage alerts, activation, P4 localization/skins,
P5 CLI/MCP, M0, packaging, signing, soak, and release acceptance are unfinished.

## 10. Non-goals

This slice adds no notification history, automatic dismissal, OS notification, tray
balloon, sound, snooze, quiet-hour policy, usage threshold, provider action, activation,
settings schema field, arbitrary text/URL, query, SQLite API in Desktop, new product
snapshot section, CLI/MCP/plugin surface, packaging, signing, M0, or release evidence.
