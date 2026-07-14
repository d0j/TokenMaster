# Contributing to TokenMaster

Use a feature branch or isolated worktree. Start with the applicable requirement in
`spec/`, add a focused failing test for behavior changes, implement the smallest safe
change, and run the affected test plus the workspace gate.

Keep every surface local-first and privacy-safe: never add persisted prompts,
responses, reasoning, commands, source contents, credentials, or absolute user paths.
Do not add unbounded collection growth or whole-history work to a UI hot path.

Before a pull request, update `spec/TRACEABILITY.md`, `docs/CURRENT_STATE.md`,
`docs/PROJECT_HISTORY.md`, and the relevant contract or operational guide. State
unverified Windows, packaging, or release boundaries explicitly.
