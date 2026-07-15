# TokenMaster architecture

```text
Codex JSONL sources
  -> bounded native watcher paths reduced immediately to one pathless hint aggregate
  -> capacity-one scheduler wake plus mandatory periodic reconciliation
  -> bounded discovery and streaming reader
  -> typed Codex decoder and provider-neutral ObservationDraft/SessionRelationDraft
  -> exclusive TokenMaster accounting canonicalizer
  -> replay classifier and revalidation/runtime engine
  -> transactional current/staging SQLite archive
  -> immutable query snapshots
  -> Slint desktop UI, future CLI, future MCP

Built-in Codex quota/benefit adapter or future sandboxed read-only provider component
  -> immutable quota epochs and typed banked-reset/credit/temporary-use lots
  -> bounded query snapshots, expiry queue, reminders, and pure policy evaluation
  -> the same Slint UI and read-only CLI/MCP projections
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

The watcher is never source authority. Its callback discards `notify` event/error paths
before touching shared state; one atomic aggregate retains only dirty/force/urgency,
latest monotonic tick, health, lifecycle, and fixed counters. A 250 ms quiet window and
15 minute healthy or 60 second degraded poll trigger authoritative discovery. Missing
roots are not replaced by broad ancestor watches.

Provider-benefit inventory read does not imply activation authority. A future banked
reset mutation is a separate host-owned official capability with explicit local
policy, compare-and-swap admission, durable intent, provider idempotency/status, and
post-action inventory/quota reconciliation. Browser/session automation and generic
plugin/LLM mutation are outside the product boundary.
