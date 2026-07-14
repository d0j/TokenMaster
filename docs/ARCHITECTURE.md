# TokenMaster architecture

```text
Codex JSONL sources
  -> bounded discovery and streaming reader
  -> typed parser and cumulative state
  -> revalidation/index orchestrator
  -> strict SQLite archive
  -> immutable query snapshots
  -> Slint desktop UI, future CLI, future MCP
```

The reader handles append, truncation, rewrite, incomplete tails, and bounded
oversized-line discard without retaining file content. The store persists only
path-private identities and approved usage metadata. Current-generation batches are
one SQLite transaction; future staging promotion will be a separate atomic boundary.

The UI receives bounded view models rather than owning archive state. Skin, layout,
and locale selection alter presentation state only, so switching remains immediate and
does not reparse sources or rebuild the archive.
