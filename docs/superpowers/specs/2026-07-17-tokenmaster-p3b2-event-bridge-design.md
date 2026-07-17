# TokenMaster P3-B.2 Slint Event-Loop Bridge Design

Status: approved for execution from the approved P3 architecture, completed P3-B.1
controller contract, and the operator's explicit autonomous `go` instruction.
Date: 2026-07-17.

## 1. Decision

P3-B.2 delivers completed controller snapshots to the existing `DesktopShell` through
one capacity-one event-loop gate. The controller's P3-B.1 latest-snapshot mailbox
remains the only retained result slot. A weak notifier schedules at most one Slint
event; the event takes the newest snapshot, applies it on the UI thread, clears the
scheduled flag, and schedules one more event only if a publication raced with the
drain.

```text
controller worker
    |
    | replace the one latest snapshot, then notify
    v
shared latest mailbox -----> bridge scheduled flag (0/1)
                                  |
                                  v
                         invoke_from_event_loop
                                  |
                                  v
                   weak window + shared DesktopState
                                  |
                                  v
                       one complete model replacement
```

The production executable is not yet given an archive root or live-runtime owner.
P3-B.2 implements and proves the delivery mechanism; P3-B.3 performs production
composition after the data-root policy is approved.

## 2. Options considered

### A. Poll the controller from a Slint timer

Rejected. Polling adds a permanent timer and wakeups even when data is unchanged,
creates shutdown ordering, and makes responsiveness depend on an interval. Reducing
the interval increases idle CPU; increasing it adds visible latency.

### B. Queue one event for every completed publication

Rejected. A busy or suspended UI could accumulate closures and old snapshots. The
event queue would become hidden unbounded history and eventually increase latency and
memory despite the controller's capacity-one result slot.

### C. Share the latest mailbox and coalesce event scheduling

Selected. One atomic scheduled flag admits a single event. Repeated notifications
while it is set are counted and coalesced. The scheduled event takes the newest
snapshot only, so older unpublished values are released. A post-drain recheck closes
the producer/consumer race without polling.

## 3. Ownership graph

`DesktopController` retains:

- the existing single latest-snapshot mailbox;
- at most one attached `DesktopSnapshotNotifier` trait object;
- its existing worker, source, reducer, and completion slot.

`DesktopSnapshotBridge` retains:

- a cloneable receiver view of the same mailbox, not a second snapshot slot;
- one `slint::Weak<MainWindow>`;
- one `Arc<Mutex<DesktopState>>` shared with `DesktopShell`;
- one scheduler object;
- fixed atomics for scheduled/closed state and bounded counters.

The controller holds a notifier that contains only `std::sync::Weak` back to the
bridge. The bridge holds no controller and only a mailbox receiver. A scheduled event
temporarily owns one `Arc` to bridge state. This graph contains no strong cycle.

`DesktopShell` continues to own the strong `MainWindow`. Its presentation state moves
from `Rc<RefCell<_>>` to `Arc<Mutex<_>>` only so a `Send` event closure can carry the
state handle to the UI thread. All actual state/model access still occurs on the UI
thread. Lock poisoning becomes a stable explicit UI/bridge failure, never an unwrap or
silent fallback.

## 4. Controller attachment

The controller exposes a cloneable `DesktopSnapshotReceiver` and permits one notifier
attachment while no refresh is active or pending. A second attachment fails with
`notifier_already_attached`; attachment during work fails with `busy`. No detach or
notifier replacement is allowed in P3-B.2.

After a complete attempt, the worker:

1. clones the attached notifier under its short fixed lock;
2. replaces the one mailbox snapshot under its separate short lock;
3. calls `snapshot_ready()` outside both locks;
4. reports the existing truthful refresh completion.

No notifier exists by default, preserving P3-B.1 tests and headless consumers. A
notifier schedules delivery only; it cannot read queries, mutate the reducer, block
the worker, or claim action authority.

## 5. Race algorithm

Publication always replaces the mailbox before notification. Notification performs a
compare-and-set from unscheduled to scheduled. If already scheduled, it returns after
incrementing one saturating coalesced counter.

The event-loop closure performs one bounded drain:

1. stop immediately if the bridge is closed;
2. take the current latest snapshot, if any;
3. upgrade the weak window on the event-loop thread;
4. apply only a newer product generation through `DesktopState`;
5. clear the scheduled flag;
6. recheck the mailbox and schedule once if a snapshot arrived during steps 2-5.

Every interleaving is covered:

- publication before the take is included in that drain;
- publication after the take sees scheduled=true and is found by the recheck;
- publication after clear schedules itself; a simultaneous recheck merely coalesces;
- multiple publications replace one slot and one queued event observes only newest;
- scheduler failure clears scheduled and leaves the snapshot for an explicit retry.

The bridge uses `slint::invoke_from_event_loop`, not
`Weak::upgrade_in_event_loop`. The latter skips the supplied closure when the component
was destroyed, which would skip scheduled-flag cleanup. The selected closure always
runs when dequeued, performs its own weak upgrade, and closes cleanly if the window is
gone.

## 6. Lifecycle and failure truth

`DesktopBridgeSnapshot` exposes only fixed values: phase, scheduled flag, delivered,
ignored/coalesced, scheduling-failure counts, last delivered generation, and one
stable failure code. Counters saturate rather than overflow or allocate.

- `NoEventLoopProvider` is retryable: scheduled resets, the latest snapshot remains,
  and a later notification may retry.
- `EventLoopTerminated` closes the bridge and releases pending delivery.
- a dropped/closed window closes the bridge when the queued event runs;
- state-lock poison faults the bridge with `state_unavailable` and exposes no inner
  panic or model data;
- an equal/older generation is counted as ignored and never replaces visible state;
- dropping the bridge closes future scheduling; controller notification uses a weak
  reference and becomes a no-op.

Bridge failure does not rewrite the completed query result or reducer truth. The
controller keeps its one current reducer snapshot, and the bridge's stable health
shows that visible delivery did not occur.

## 7. Boundedness and performance

- one latest snapshot mailbox total across controller and bridge;
- zero polling timers and zero additional threads;
- at most one queued event owned by this bridge;
- one weak Slint handle and one shared presentation state;
- one fixed 11-row model replacement per accepted delivered generation;
- no query, SQLite, provider, filesystem, process, or network operation on the event
  loop;
- locks cover only one pointer/trait-object take or bounded presentation replacement;
- 10,000 notifications retain one event and deliver only the newest snapshot.

P3-B.2 does not claim visible-paint latency or long-run GUI resource acceptance; those
remain P4/P6 gates. It does prove constant queue/slot/thread topology.

## 8. Verification

P3-B.2 is complete only when tests prove:

- one notifier attaches only while idle and is called after mailbox publication;
- 10,000 notifications create one scheduled task and deliver only the newest
  generation;
- publication during delivery schedules exactly one follow-up and loses no newest
  snapshot;
- scheduler failure retains the snapshot and a later retry succeeds;
- stale/equal generations remain ignored by the existing desktop state;
- bridge drop/window close and controller shutdown leave no strong cycle or task-owned
  thread;
- a real headless Slint integration event loop applies a controller snapshot to the
  generated `MainWindow` and exits deterministically;
- the source audit rejects polling timers, a second event-loop schedule site, strong
  window retention, UI queries, or additional workers/result slots;
- clean-root, format, warnings-as-errors locked Clippy, and complete locked workspace
  tests pass.

## 9. Closure review

The design was checked against the specification, security/data/API contracts,
P3/P3-B.1 designs, current state, handoff, roadmap, Slint 1.17.1 `Weak` and event-loop
implementation, and the existing controller/shell ownership. It adds no data source,
runtime, timer, thread, route payload, identity, or action authority. The critical
destroyed-window cleanup race is handled explicitly, and the same capacity-one
mailbox remains the sole snapshot retention boundary.

No implementation ambiguity remains for P3-B.2.
