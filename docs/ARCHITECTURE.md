# TokenMaster architecture

```text
Codex JSONL sources
  -> bounded discovery and streaming reader
  -> typed Codex decoder and provider-neutral ObservationDraft
  -> exclusive TokenMaster accounting canonicalizer
  -> replay classifier and revalidation/runtime engine
  -> transactional current/staging SQLite archive
  -> immutable query snapshots
  -> Slint desktop UI, future CLI, future MCP
```

The reader handles append, truncation, rewrite, incomplete tails, and bounded
oversized-line discard without retaining file content. Provider code cannot supply
fingerprints, replay signatures/evidence, event IDs, dispositions, or canonical
events. The accounting crate is their only constructor. The store persists only
path-private identities and approved usage metadata. Current-generation batches are
one SQLite transaction; staging promotion is a separate atomic boundary.

The UI receives bounded view models rather than owning archive state. Skin, layout,
and locale selection alter presentation state only, so switching remains immediate and
does not reparse sources or rebuild the archive.
