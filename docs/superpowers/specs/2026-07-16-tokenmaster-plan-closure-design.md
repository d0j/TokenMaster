# TokenMaster Architecture and Release Plan Closure

Status: approved after critical self-review on 2026-07-16.

## Goal

Close every known product-plan ambiguity before P1-E implementation continues. This
document does not replace the normative specification. It records the reviewed
choices that are merged into the specification, security model, traceability, master
plan, roadmap, release boundary, and feature-parity ledger.

The acceptance meaning of "ideal plan" is precise: no known contradiction,
unbounded production path, unfrozen 1.0 platform decision, unowned requirement, or
feature-parity placeholder remains. It does not mean that future evidence cannot
discover a defect; every implementation and release claim still requires its tests
and receipts.

## Evidence reviewed

- all normative TokenMaster contracts in their repository-defined order;
- the current Rust workspace, dependency pins, Windows target configuration, CI, and
  implemented P0 through P1-D.6 runtime boundaries;
- the external references at the exact commits in `third_party/UPSTREAM.toml`, with
  their navigation, dashboard, settings, notification, report, filtering, pricing,
  output, offline, compact, and provider-adapter surfaces inspected read-only;
- current Rust, Slint, SQLite, Codex usage/reset, and MCP release information;
- M0 evidence rules and the missing interactive and uninterrupted-soak receipts.

The two external projects remain provenance and behavioral references only. Their
runtime stacks, source trees, assets, private integrations, and weaknesses are not
dependencies of TokenMaster.

## Options considered

### A. Documentation-only cleanup

Rename phases and expand a few roadmap sentences without freezing the release and
data-source decisions. This is small but leaves the same ambiguity for implementation
and acceptance. Rejected.

### B. Normative closure over the current native architecture

Keep Rust 1.97, Slint 1.17, bundled SQLite, the compiled-in Codex fast path, and the
approved provider-neutral boundaries. Freeze the Windows release lane, package,
license path, quota-source policy, pricing-update policy, supply-chain gates, delivery
order, and row-level parity ledger. Selected.

### C. Rewrite or framework switch

Restart in Electron, Tauri, React, Go, or another UI/runtime stack. This would discard
the already proven bounded reader, replay-safe accounting, transactional archive,
native lifecycle, and resource gates without solving any remaining product contract.
Rejected.

## Binding decisions

### 1. Product and stack

TokenMaster remains one original native Rust/Slint/SQLite product. The software Slint
renderer is the acceptance baseline; optional accelerated rendering may be evaluated
later but cannot be required for correctness. No webview, Node, Go, background daemon,
or in-process native plugin enters the 1.0 runtime.

### 2. Delivery order

The dependency order is:

1. P1 runtime publication and recovery;
2. P2 immutable query, analytics, pricing, quota, reset, and Git-output data;
3. P3 complete desktop information architecture over immutable snapshots;
4. P4 presentation, skins, layouts, localization, accessibility, and visible-paint
   performance;
5. P5 read-only automation through strict CLI JSON and a separate stdio MCP process;
6. P6 Windows integration, packaging, security evidence, interactive evidence, and
   release receipts;
7. 1.1 isolated external provider packages.

This puts the user-visible product before optional automation while keeping one query
truth for UI, CLI, and MCP. P5 may design against the already stable query schema; it
cannot delay P3/P4 or add authority to mutate provider state.

### 3. Canonical Windows release lane

The signed 1.0 artifact targets `x86_64-pc-windows-msvc`. The current GNU target
remains a supported development/M0 evidence lane until P6 migration evidence is
complete. P6 must remove the workspace-global forced target, make target selection
explicit in build scripts, and run a temporary dual-lane comparison covering tests,
startup, package size, private memory, handles/threads, rendering, tray behavior,
signing, and receipt identity. GNU evidence never silently substitutes for the MSVC
release artifact.

### 4. Distribution and updates

The 1.0 distribution is a signed portable ZIP containing the executable, required
runtime files, licenses/notices, and verification metadata. The archive and executable
are hash-bound to a clean commit. There is no automatic updater in 1.0. An installer
or updater is future work and requires a signed manifest, staged replacement,
rollback, downgrade policy, and interrupted-update tests before activation.

### 5. Slint and third-party licensing

The distributed desktop binary uses the Slint Royalty-free License 2.0 path with the
required attribution in Help/About and on the public download page. `LICENSE`, a
generated third-party notice set, dependency license policy, and SBOM ship with the
artifact. A GPL build is not the default distribution route. A license-policy failure
blocks packaging.

### 6. Quota and banked-reset source policy

The built-in Codex 1.0 adapter may consume only a credential-free versioned local
format or a documented stable official machine interface. A user-facing dashboard,
slash command, browser page, cached cookie, or observed private endpoint is not an API
contract. TokenMaster never replays session traffic, scrapes browser state, stores
credentials, or fabricates limits from local token counts.

When no permitted live source exists, the product exposes an explicit unavailable or
stale state and may accept bounded manual banked-reset inventory. Reminder and history
features remain useful, but discovery and automatic activation remain unavailable.
Automatic activation additionally requires the already specified official,
idempotent, status-capable mutation contract; absence fails closed.

### 7. Pricing updates

The 1.0 binary embeds one pinned catalog with source/version metadata. Catalog changes
arrive through reviewed application releases, not network access on the hot path.
Bounded validated local overrides remain supported. Unknown models remain unknown;
their cost is never silently zero. A future signed catalog updater requires a separate
design and rollback/version policy.

### 8. Supply chain and CI

P6 requires exact Cargo locks plus all of the following reproducible gates:

- RustSec advisory audit;
- dependency/source/license policy validation;
- generated CycloneDX or SPDX SBOM;
- secret scan over source and package contents;
- GitHub Actions pinned by immutable commit SHA;
- artifact provenance/attestation and SHA-256 manifest;
- deterministic package-content audit and clean-room launch rehearsal.

Dependency automation may propose reviewed pull requests but cannot auto-merge into a
release. Tool names may change if an equivalent deterministic validator replaces
them; the evidence requirements may not be weakened.

### 9. Feature parity

`docs/FEATURE_PARITY.md` is the canonical behavioral ledger. Every reference feature
has an exact source family and pin, implement/adapt/reject decision, TokenMaster
improvement, requirement owner, delivery gate, validator, and status. "Full parity"
means every ledger row is implemented or deliberately rejected with rationale; it
does not mean source, pixel, protocol, or bug compatibility.

### 10. Performance evidence timing

Focused deterministic memory/handle/thread/CPU gates run throughout development.
The 24-hour M0 soak is run once against an otherwise frozen M0 candidate, and the
72-hour product soak against the frozen release candidate. A prior interrupted or
different-binary run is useful diagnostic evidence but is not an acceptance receipt.
Feature work is not paused for repeated day-long soaks before the candidate is frozen.

## Closure review

The selected design passes the following self-review:

- no stack rewrite is required and no obsolete runtime is reintroduced;
- UI precedes optional automation in the implementation rail;
- Codex local ingestion is replaceable without making plugins a 1.0 dependency;
- dynamic quota windows, full-reset epochs, banked-reset lots, expiry reminders, and
  activation authority are separate concepts;
- no private endpoint, browser session, credential, prompt, transcript, arbitrary
  SQL, shell, HTTP, or filesystem authority is introduced;
- release target, package shape, update policy, license route, pricing policy, and
  supply-chain evidence are explicit;
- every reference capability has a row-level disposition;
- every phase has an entry gate, exit evidence, and one downstream owner;
- long-run resource claims remain evidence-bound and cannot be inferred from unit
  tests or an interrupted soak;
- no tracked document contains the current commit hash.

No known blocking ambiguity remains after these decisions are merged. The approved
next implementation gate is P1-E immutable engine publication, generation ordering,
and suspend/resume race evidence.
