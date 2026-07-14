## Scope

Describe the problem, the smallest implemented change, and explicit non-goals.

## Requirement and traceability

List affected `TM-*`, `FUNC-*`, `DATA-*`, `SEC-*`, `PERF-*`, `UI-*`, or `REL-*`
requirements and the traceability rows updated by this change.

## Verification

List exact commands and results. Separate focused RED/GREEN evidence from broader
workspace, oracle, documentation, and platform gates. State every unrun check.

## Memory and privacy impact

Describe retained state, hot-path allocations, cancellation/lifecycle ownership, and
all data crossing the changed boundary. Confirm that no prompts, responses, reasoning
text, tool arguments, command output, file contents, credentials, or absolute user
paths are stored or exposed.

## Open boundaries

State what remains unfinished or externally unverified. Do not convert scaffold,
developer smoke, interrupted soak, or unsigned build evidence into a release claim.

## Checklist

- [ ] I worked on a feature branch or isolated worktree.
- [ ] I added a focused failing test before each behavior change.
- [ ] Focused tests pass, followed by the complete relevant gates.
- [ ] Traceability, current state, project history, and affected security/operations docs are updated.
- [ ] Memory, privacy, timeout, input-bound, and rollback behavior remain fail-closed.
- [ ] Unverified platform, soak, packaging, signing, and release boundaries are explicit.
