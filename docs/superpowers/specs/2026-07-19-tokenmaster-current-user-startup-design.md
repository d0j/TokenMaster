# TokenMaster current-user startup design

Date: 2026-07-19  
Status: approved for P3-E.5 implementation

## Goal

Add a responsive, opt-in Windows sign-in launch control without introducing a
second desired-state database, a shell/process capability, elevation, machine-wide
authority, or portable configuration state.

## Authority boundary

The sole source of truth is the fixed `REG_SZ` value `TokenMaster` below
`HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`. TokenMaster never
opens or writes `HKEY_LOCAL_MACHINE`, never invokes a shell, process, scheduled task,
service, installer, or elevation surface, and never accepts a caller-selected hive,
subkey, value name, command, argument, or executable path.

The adapter derives the path only from the running process, requires an absolute
ordinary non-reparse file, opens it, and obtains its path-private physical identity.
The stored command has one exact form: the quoted current executable path with no
arguments. A successful enable or repair is not published until an immediate readback
parses that exact form, reopens the target, and proves the same physical identity. A
successful disable is not published until readback proves the fixed value absent.

Registry data, executable paths, file identities, and raw operating-system errors do
not cross the platform boundary, appear in `Debug`, UI models, logs, receipts, config,
or backup metadata.

## State model

The path-free public observation is exactly one of:

- `Disabled`: the fixed value is absent;
- `EnabledVerified`: the value has the exact command shape and targets the running
  executable identity;
- `StaleRelocation`: the exact single-path shape names the same executable basename,
  but the target is absent or has a different physical identity;
- `Conflict`: the fixed value has a foreign type, malformed/argument-bearing command,
  or different executable basename;
- `AccessDenied`: the current-user value cannot be inspected or mutated because access
  is denied;
- `Unavailable`: executable verification, bounded registry decoding, or another
  platform operation cannot be proved.

`inspect` is read-only. It never repairs, deletes, creates, or normalizes state.
Startup inspection failure degrades only this Settings card and never blocks the main
application, current-session arbitration, data startup, or shutdown.

## Explicit actions

- `Enable`: idempotent for `EnabledVerified`; creates only from `Disabled`; rejects
  stale or conflicting state without mutation.
- `RepairStale`: idempotent for `EnabledVerified`; replaces only a freshly reread
  `StaleRelocation`; rejects disabled or conflicting state.
- `Disable`: idempotent for `Disabled`; deletes only a freshly reread
  `EnabledVerified` or `StaleRelocation`; rejects `Conflict` without mutation.

Every action performs one fresh observation before mutation and one verified readback
after mutation. The API does not promise cross-process compare-and-swap because the
Windows Run value has no such primitive; a readback mismatch fails closed as
`Unavailable`. No retries, polling, timer, worker, queue, cache, or retained path are
introduced.

## Application and UI composition

The application obtains one initial read-only observation after the existing early
current-session claim. The Desktop Settings surface receives only the path-free state.
Three typed intents (`Enable`, `RepairStale`, `Disable`) execute synchronously on the
Slint owner thread; each operation is a bounded current-user registry call and immediate
readback, not a long-running task. The presenter updates only this card.

The card uses explicit buttons instead of an ambiguous checkbox:

- Disabled: `Enable at sign-in`;
- Enabled: `Disable at sign-in`;
- Stale: `Repair registration` and `Remove old registration`;
- Conflict/access denied/unavailable: explanatory status with no destructive action.

All controls have stable accessible labels. Locale/theme modernization remains P4 and
must consume the same typed state rather than reimplementing registry logic.

## Failure and portability behavior

Non-Windows/headless builds return `Unavailable` and reject mutation. Access denial and
conflict are visible, stable, non-fatal states. Portable mode is not a special case:
the user may explicitly register that current executable, and relocation is then
reported rather than silently repaired.

Because registry state is the sole device-local source, no settings schema migration is
required. Existing portable candidate/export and backup paths cannot contain startup
state by construction and retain their current tests.

## Proof obligations

1. Pure state-machine tests cover all observations/actions, idempotence, stale repair,
   conflict non-mutation, access denial, failed readback, and bounded command parsing.
2. Windows source/mutation audit proves one fixed HKCU Run key/value, `REG_SZ`, no HKLM,
   shell/process/elevation, arbitrary registry input, retry, timer, polling, or path
   projection.
3. Desktop/app contracts prove three typed intents, visible degraded states, accessible
   explicit actions, and no expansion of the five-intent tray lifecycle contract.
4. Existing config/backup preservation tests and audits continue to prove portable-only
   export/import/restore.
5. Interactive acceptance later proves real enable/readback/launch/disable, relocation,
   denied ACL behavior, and resource return on the exact packaged executable.

