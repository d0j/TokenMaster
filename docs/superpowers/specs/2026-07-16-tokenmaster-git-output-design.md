# TokenMaster Bounded Git Output Design

**Status:** accepted; implementation Tasks 1-8 complete on 2026-07-17

**Scope:** local read-only Git code-output metrics, transient repository association,
bounded incremental projection, immutable query snapshots, and usage/cost efficiency
join. P3 rendering, settings UI, CLI/MCP, remote hosting metadata, repository mutation,
and external provider packages remain later contours.

## 1. Outcome

TokenMaster adds the useful Code Output behavior of WhereMyTokens without inheriting
its repeated whole-history scans, raw-path cache, ambiguous author fallback, or
Electron/runtime overhead.

For repositories associated with normalized TokenMaster activity, the product exposes:

- commits;
- added, removed, and net text lines;
- product, test, documentation/specification, configuration/build,
  schema/migration, vendor/generated, asset, and other categories;
- bounded daily output;
- merge, binary, submodule, oversized, and omitted counters;
- explicit complete, partial, stale, and unavailable quality;
- optional cost per 100 added product-code lines only when exact compatible usage cost
  is available.

The first backend is the locally installed native Git executable. It is invoked
directly with an internal fixed argument grammar. No shell, repository hook, pager,
external diff, textconv filter, editor, credential helper, network operation, or
arbitrary user-supplied Git argument is allowed.

The domain, store, and query contracts are backend-neutral. A future pure-Rust backend
may replace the process adapter without changing SQLite, UI, CLI, or MCP schemas.

## 2. Why the native Git backend is selected

Three implementations were evaluated:

1. **Direct native Git process — selected.** Git already owns the exact commit graph,
   worktree, mailmap, rename, merge, and submodule semantics. A short-lived direct
   child adds no steady-state library memory and no large dependency tree. Missing or
   unsupported Git remains explicit `unavailable`.
2. **`gix` — deferred backend option.** It is pure Rust and portable, but its Git
   graph/diff surface would substantially increase dependencies, binary size, and
   implementation complexity for a feature that is idle most of the time. The backend
   boundary preserves this option if packaging evidence later justifies it.
3. **`git2`/libgit2 — rejected for 1.0.** It adds a bundled native C dependency,
   separate Git behavior, build/linking work, and permanent process memory without
   removing the need for TokenMaster-specific parsing, bounds, privacy, and cache
   semantics.

Git documentation confirms that `--no-ext-diff` disallows external diff drivers and
`--no-textconv` disallows text conversion filters. TokenMaster uses both. It also
disables paging and optional locks and never executes a command that can update refs,
the index, worktree, config, object database, remotes, or credentials.

## 3. Layering

### `tokenmaster-domain`

Adds only stable provider-neutral public facts:

- opaque 32-byte repository identity;
- Git output category and checked line counts;
- complete/partial/unavailable quality and stable reason codes;
- bounded daily and total metrics;
- repository activity association identity.

It owns no path, author email, Git object ID, branch name, command, process, parser,
filesystem, SQLite, clock, or UI capability.

### `tokenmaster-git`

Is the isolated Git subsystem. It owns:

- sealed transient repository paths and executable paths with redacted `Debug`;
- exact executable discovery/validation;
- fixed read-only command construction;
- bounded NUL-framed output parsing;
- author matching and mailmap policy;
- path classification without retaining paths;
- merge, rename, binary, and submodule interpretation;
- repository/ref fingerprints;
- incremental scan planning and deterministic aggregate batches.

The crate uses `std`, `sha2`, `thiserror`, and `tokenmaster-domain`. It does not depend
on SQLite, query, runtime, Slint, HTTP, an async runtime, or a shell crate.

### `tokenmaster-codex`

Produces an optional provider-neutral repository activity hint beside normal usage
facts when a valid absolute local `cwd` is present. The hint is transient:

- it is not serializable;
- it has redacted `Debug`;
- it is not written into parser resume state, checkpoints, SQLite, diagnostics, or
  canonical usage events;
- it is bounded to one latest hint per source batch.

The existing safe `ProjectAlias` remains the only project display metadata.

### `tokenmaster-store`

Schema v13 owns an independent Git projection:

- one Git publication state;
- one row per opaque repository identity;
- bounded ref-scan state;
- bounded daily/category aggregates;
- bounded repository-to-project association;
- exact snapshot revision and freshness;
- stable partial/unavailable reason counters.

It never stores repository paths, executable paths, emails, author names, branch/ref
names, commit IDs, file paths, file contents, diff content, command text, or raw Git
output.

One completed scan replaces or incrementally advances one repository projection in an
immediate transaction. Failed, cancelled, timed-out, truncated, or identity-mismatched
scans preserve the last complete projection and update only bounded health/freshness.

### `tokenmaster-query`

Exposes immutable bounded Git output envelopes:

- at most 32 repositories;
- at most 400 daily points per request;
- totals plus category totals and omission counters;
- freshness, quality, stable warnings, and last trustworthy scan time;
- optional compatible usage/cost efficiency facts.

It performs no process execution, raw usage-event scan, repository traversal, or
per-visible-item query. Query values own their data and hold no SQLite transaction,
child process, path, or UI object.

### `tokenmaster-runtime`

Owns one Git refresh contour with:

- one existing constant-state scheduler/worker pair;
- one active repository scan;
- one aggregate follow-up;
- a capacity-one latest transient repository hint per bounded repository slot;
- one non-waiting shared writer-lease attempt after Git I/O;
- exact cancellation, deadline, pause/resume, shutdown, and child cleanup.

Git scanning never runs on the UI thread and never holds the SQLite writer lease.
Store publication occurs only after the child has exited and all output has passed
the bounded parser.

## 4. Repository association and privacy

The current usage contract intentionally persists only `ProjectAlias`, not `cwd`.
Therefore repository association must not be reconstructed from a label or guessed
from a path basename.

A valid Codex metadata/turn-context `cwd` creates a transient
`RepositoryActivityHint` containing:

- provider/profile/source/session scope;
- event time;
- safe project alias when available;
- a sealed absolute local candidate path.

The hint travels through a capacity-one transient method on the descriptor-bound
`SourceBatchReader`, not through `AdapterBatch` or `CanonicalBatch`. Providers without
repository activity use the default `None`. A consumer takes the value immediately
after the corresponding pull; the next pull may replace it.

The Git subsystem resolves the candidate with exact native Git read-only commands,
obtains the absolute common Git directory, and computes:

`repository_id = SHA256(domain, installation_salt, normalized_common_dir_identity)`.

The exact safe project alias from that same hint is separately derived as:

`project_key = SHA256(project-domain, installation_salt, safe_project_alias)`.

The query layer never receives the salt. A fixed store-owned matcher compares at most
32 project keys with at most 256 materialized safe usage aliases and returns only
candidate indices.

The installation salt is generated once and stored as opaque random bytes in the
local archive state. It prevents cross-installation correlation. The raw common-dir
path exists only during discovery/scanning.

All worktrees sharing the same common Git directory map to one repository. Multiple
sessions/projects may associate with that repository, but the store retains only
bounded opaque scope/project keys and timestamps.

Because an absolute path is never persisted, restart behavior is explicit:

- the last trustworthy Git projection remains queryable as aging/stale;
- a bounded startup recovery pass may rediscover candidates from recent active Codex
  source tails without retaining those tails;
- otherwise the next valid activity hint reactivates the repository;
- absence of a current locator never deletes historical metrics or fabricates zero.

Network/UNC/device namespaces, relative paths, traversal components, reparse-point
escapes, and paths outside the allowed local-root policy are rejected before Git
execution.

## 5. Author semantics

TokenMaster never falls back to counting every author.

Author selection order is:

1. explicit validated local TokenMaster author identities when configured later;
2. repository-local `user.email`;
3. global `user.email`.

If no valid email exists, the repository is `unavailable: author_identity_missing`.
Emails are read transiently, normalized as bounded UTF-8, hashed with the installation
salt, and discarded. They are not passed as visible process arguments.

The log stream includes both bounded raw and mailmapped author email fields.
TokenMaster hashes both and compares fingerprints in-process. A configured email
therefore still owns commits written with that exact raw email, while aliases declared
by the repository mailmap can also match a configured canonical email. Neither names
nor emails leave the parser.

## 6. Commit graph and diff semantics

### Reachability

- Scan all local branches only.
- A commit reachable from multiple branches is counted once.
- Remote-tracking branches, tags, reflogs, stashes, replace refs, and working-tree
  changes are excluded.
- Empty/unborn repositories produce a complete zero snapshot.
- Shallow repositories are partial with a stable `shallow_history` warning.

### Commit ownership

- Match the commit author, not committer.
- Count an owned commit even if it changes no text lines.
- Author timestamp is the immutable daily bucket authority.
- Invalid or overflowing timestamps make that commit omitted and the scan partial.

### Root and ordinary commits

- Root commits compare against the empty tree.
- Ordinary commits compare against their sole parent.

### Merge commits

- A merge commit is counted once when authored by the selected author.
- Its line output is defined as zero.
- Commits introduced by merged branches are still counted independently once through
  the all-local-branch reachability walk.
- Octopus merges use the same zero-line rule.

This avoids counting the merged side once in its original commits and a second time
in the merge result. Git cannot isolate only conflict-resolution lines from an
arbitrary historical merge without expensive reconstruction and version-sensitive
heuristics, so TokenMaster does not invent them. The separate merge count keeps that
activity visible.

### Renames and copies

- Rename detection uses Git's bounded default similarity threshold.
- The destination path owns category classification.
- A pure rename normally contributes zero added/removed lines.
- Copy detection is disabled because exhaustive copy search can be unbounded and
  expensive; an ordinary copied file is classified from Git's text-line result.

### Binary and oversized files

- Binary `numstat` entries contribute zero text lines and increment `binary_files`.
- Oversized path/stat fields are discarded, increment omission counters, and make the
  scan partial.
- TokenMaster never reads blob contents itself and never retains patch text.

### Submodules

- Gitlink mode `160000` changes increment `submodule_changes`.
- They contribute zero text lines regardless of `numstat`.
- TokenMaster does not recurse into submodules automatically.
- A submodule may be scanned only as its own separately associated repository.

## 7. Category semantics

Classification is ASCII-case-insensitive after separator normalization and uses only
the destination path transiently. Precedence is:

1. `vendor_generated`;
2. `schema_migration`;
3. `test`;
4. `docs_spec`;
5. `config_build`;
6. `asset`;
7. `product_code`;
8. `other`.

Examples:

- vendor/generated: `vendor/`, `third_party/`, `node_modules/`, generated lock/build
  output;
- schema/migration: `migrations/`, `schema/`, recognized migration file names;
- test: `test/`, `tests/`, `spec/`, `__tests__/`, recognized test file names;
- docs/spec: `docs/`, Markdown/AsciiDoc/reStructuredText, normative specification
  files;
- config/build: CI, package/build manifests, Docker, workflow, and configuration
  extensions;
- asset: images, fonts, audio, video, archives, and opaque binary assets;
- product code: recognized programming/source/interface extensions outside excluded
  categories;
- other: text not covered above.

The category table is versioned. A version change invalidates cached category totals
and schedules a bounded rebuild rather than mixing definitions.

## 8. Exact process boundary

Discovery permits only a native executable named `git.exe` on Windows or `git` on
supported Unix platforms. Explicit configuration is authoritative; automatic
discovery examines a bounded process `PATH` snapshot and validates an absolute regular
native file under platform policy. Windows reparse points are rejected; conventional
Unix symlinks are canonicalized and the final same-named regular executable is
revalidated.

Every child uses:

- direct `std::process::Command`, never `cmd`, PowerShell, `sh`, or `bash`;
- stdin null;
- stdout/stderr pipes with hard byte caps;
- no inherited console;
- `GIT_OPTIONAL_LOCKS=0`;
- no prompt;
- no pager;
- no external diff;
- no textconv;
- merge diffs explicitly disabled;
- no color;
- no optional network or credential operation;
- inherited Git location/config/trace/askpass/SSH redirection removed;
- a monotonic deadline and kill/join cleanup.

Allowed command families are fixed:

- version/capability;
- repository root/common-dir/object-format/shallow discovery;
- local/global author email discovery;
- local-head ref fingerprint;
- bounded local-branch log/diff stream.

No caller can supply a subcommand, option, revision, ref, pathspec, environment name,
or config key.

Stderr is never surfaced verbatim. It is consumed under a small cap and mapped to a
stable path-free error code.

## 9. Bounded parser and scan limits

Initial hard limits:

- 32 associated repositories;
- 512 local branch refs per repository;
- 200,000 visited commits per authoritative rebuild;
- 4,096 changed paths per commit;
- 32 KiB per path field;
- 4 KiB per author field;
- 256 KiB for transient `.mailmap` hashing;
- 64 MiB total stdout per repository scan;
- 64 KiB stderr;
- 30 seconds per repository scan;
- 256 aggregate commit records per store batch;
- 400 daily points per query response;
- one active scan and one coalesced follow-up.

The parser consumes NUL-framed fields incrementally. It retains one commit accumulator,
one path-stat accumulator, fixed counters, one bounded batch, and no whole-history
vector. Any limit breach terminates the scan as partial or unavailable according to
whether a coherent prefix is safely publishable. An authoritative replacement
requires a complete stream and matching repository, author, local-ref, shallow/object
format, and mailmap identities before and after one shared scan deadline.

## 10. Incremental cache

The store never treats a timestamp alone as Git history authority.

For each repository it retains:

- hash algorithm;
- bounded sorted local-head fingerprint;
- bounded worktree-root `.mailmap` content fingerprint;
- category semantics version;
- author-set fingerprint;
- immutable daily/category aggregates;
- scan quality/freshness and counters.

Fast path:

1. discover current local-head fingerprint;
2. if unchanged, refresh freshness without walking history;
3. while the same process still owns a sealed bounded raw-head frontier, prove every
   prior head remains an ancestor of a current head, scan only newly reachable commits,
   and merge a bounded delta;
4. after restart, if the fingerprint changed, schedule an authoritative rebuild
   because TokenMaster deliberately persisted no raw commit ID;
5. otherwise schedule an authoritative rebuild.

Force-push, branch deletion, rewritten ancestry, changed author configuration,
mailmap/category version change, object-format change, shallow-boundary change, or
cache inconsistency invalidates incremental authority. The prior snapshot remains
visible as stale until a complete rebuild publishes atomically.

No per-commit history is retained indefinitely. The durable projection stores daily
and category aggregates plus only salted fingerprints. Raw Git object IDs may exist
only in the bounded process-lifetime scan frontier and are dropped on pause, shutdown,
identity change, or runtime replacement. This deliberately trades occasional
background rebuild CPU after restart for zero persisted commit IDs.

## 11. Usage and cost efficiency join

Git output and usage events are independent evidence streams.

The query layer may expose cost per 100 added product-code lines only when:

- the requested UTC time range and project/repository association are exact;
- Git quality for that range is complete;
- usage cost is available and non-conflicting;
- neither side is stale beyond the accepted query policy;
- added product-code lines are greater than zero.

The value is fixed-point:

`cost_per_100_added_micros = round_half_up(cost_usd_micros * 100 / added_lines)`.

Missing cost, partial Git history, ambiguous association, zero added lines, mismatched
time boundaries, or incompatible dataset freshness remains explicitly unavailable.
TokenMaster never treats total token usage as code output and never attributes all
project cost to a repository by basename guess.

Git parser/projection day indices are UTC calendar days. The public Git range therefore
uses and labels UTC half-open dates; it never relabels these buckets as local civil
days. Usage evidence is resolved from the same UTC range plan. One facade call shares
one maximum two-second read budget across Git, materialized usage/price, and project
matching. Failure of the optional usage side disables efficiency without hiding
independent Git facts.

## 12. Query/UI contract

The P2 query snapshot is designed for the later P3 Code Output card:

- headline added/removed/net and commit count;
- range selector: today, 7 days, 30 days, and bounded custom/all-time projection;
- daily chart;
- category breakdown;
- cost per 100 added product-code lines when valid;
- visible freshness/quality and omission reasons;
- repository grouping by safe alias plus opaque identity.

The UI receives no path, branch, author, commit, file, command, or Git stderr. It does
not start scans during paint. Refresh requests are asynchronous hints and an older
snapshot may never replace a newer one.

## 13. Failure truth

Stable unavailable reasons include:

- `git_not_found`;
- `git_not_native`;
- `repository_not_found`;
- `repository_path_rejected`;
- `author_identity_missing`;
- `unsupported_git_version`;
- `unsupported_object_format`;
- `too_many_repositories`;
- `too_many_refs`;
- `history_limit_exceeded`;
- `output_limit_exceeded`;
- `deadline_exceeded`;
- `process_failed`;
- `history_changed_during_scan`;
- `cache_incompatible`;
- `store_unavailable`.

Partial warnings include:

- `shallow_history`;
- `binary_files_omitted`;
- `submodule_lines_omitted`;
- `oversized_fields_omitted`;
- `invalid_commit_omitted`;
- `daily_history_truncated`;
- `incremental_rebuild_pending`;
- `association_incomplete`.

Stable codes are English ASCII. Localized text belongs to P3/P4.

## 14. Acceptance

P2-E is complete only when all of the following pass:

- synthetic repositories for root, ordinary, merge, octopus, rename, binary,
  submodule, worktree, multiple local branches, duplicate reachability, branch delete,
  force-push, shallow history, empty/unborn repository, missing author, and mailmap;
- exact author filtering with no count-all fallback;
- unchanged zero-history-read path;
- append-only incremental path and rewrite-triggered rebuild;
- strict schema-v13 fresh/migration/rollback/corruption contracts;
- immutable 32-repository/400-point query snapshots;
- compatible and unavailable efficiency joins;
- cancellation/deadline/child cleanup;
- path/email/ref/commit/file/stderr/command privacy scans;
- no shell/network/credential/mutation dependency or source surface;
- repeated refresh/drop resource plateau;
- clean-root, formatting, strict workspace Clippy, and complete workspace tests.

P2-E completion does not claim the P3 card, P5 CLI/MCP surface, M0 acceptance,
packaging, signing, or release.

## 15. Rejected shortcuts

- Persisting raw/encrypted/obfuscated repository paths.
- Guessing a repository from `ProjectAlias`.
- Counting every author when `user.email` is missing.
- Running `git` from query or UI code.
- Shell command strings or user-supplied Git arguments.
- One `git show` child per commit.
- Repeated full-history scans on every dashboard refresh.
- Retaining commit IDs, file paths, raw Git output, or diff/blob contents.
- Recursing into submodules.
- Treating binary/submodule changes as text lines.
- Publishing a prefix as authoritative after timeout, output truncation, or ref change.
- Adding `gix`/`git2` before measured packaging/resource evidence requires it.
