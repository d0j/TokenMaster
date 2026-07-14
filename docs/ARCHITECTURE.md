# TokenMaster architecture

```text
Codex JSONL sources
  -> bounded discovery and streaming reader
  -> typed Codex decoder and provider-neutral ObservationDraft/SessionRelationDraft
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

The allocation-free accounting replay classifier is also store-independent. It
validates provider/profile/parent/ordinal scope and returns only typed disposition and
next-state values. Weak evidence and exhausted traversal budgets remain pending;
cycles and contradictory facts become conflict; proven divergence is irreversible.

Ancestry metadata may arrive after usage. The reader therefore emits a separate
bounded session-relation draft in addition to observation drafts; reconciliation can
apply it to earlier observations without retaining raw JSONL. Parser resume v2 stores
the next ordinal and bounded lineage state. Resume v1 fails closed because assigning
ordinal zero after prior emissions would create false identity collisions.

The UI receives bounded view models rather than owning archive state. Skin, layout,
and locale selection alter presentation state only, so switching remains immediate and
does not reparse sources or rebuild the archive.
