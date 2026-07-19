# TokenMaster P3-D.7 Help/About Route Design

**Date:** 2026-07-19
**Status:** Approved for implementation
**Scope:** Replace the final data-independent P3-D placeholder with a truthful,
responsive, fixed-memory Help/About route.

## 1. Outcome

Help/About becomes an always-available product guide and attribution surface. It tells
the user what TokenMaster is, where its current facts come from, what it deliberately
does not retain, how to inspect data health, which automation capabilities are not yet
available, and which licenses/references apply.

This slice does not add diagnostics collection, support submission, runtime capability
probing, file or URL selection, a query, or another mutable owner.

## 2. Existing truth

- `ProductRoute::HelpAbout` already exists and is ready without an archive.
- Desktop already exposes the stable `help_about` route and top-level navigation item.
- The route currently falls through to the generic placeholder.
- TokenMaster is MIT licensed and the workspace package version is compile-time data.
- WhereMyTokens and ccusage are pinned external MIT references in
  `third_party/UPSTREAM.toml`; they are not runtime dependencies or vendored sources.
- Slint 1.17.1 is pinned. The selected distribution route is the Slint Royalty-free
  License 2.0 attribution route.
- P5 CLI/MCP and P6 generated notices/SBOM/signed MSVC package evidence are unfinished.

The official Slint Royalty-free License 2.0 attribution condition permits either the
standard `AboutSlint` widget in an accessible About surface or the Slint badge on the
public download page. TokenMaster will mount the pinned standard widget here and retain
the public-download attribution as a P6 package/release requirement.

## 3. Presentation contract

`HelpAboutView` is one static Slint component with:

1. an identity header containing `TokenMaster`, compile-time package version, and a
   concise local-first description;
2. six fixed semantic cards:
   - **Start here** — route orientation without implying every planned feature exists;
   - **Data sources and truth** — bounded local Codex usage plus separate official
     machine-interface quota evidence, with unavailable/stale truth;
   - **Privacy by design** — prompts, responses, reasoning, commands, source contents,
     credentials, raw paths, and private identity are not retained or exposed;
   - **Health and recovery** — Data Health and Settings are the existing local
     diagnostic/recovery surfaces; no support upload is claimed;
   - **Automation status** — P5 strict JSON CLI and stdio MCP are not available in the
     current build; no listener/browser/session automation is active;
   - **About and licenses** — TokenMaster MIT, both pinned MIT reference lineages, the
     standard Slint attribution widget, and no false SBOM/package claim.

The six cards are instantiated once. A position-only responsive layout places them in
two columns at 800 px or wider and one column below 800 px. The same text and accessible
meaning survive both layouts. A single scroll surface bounds the viewport.

## 4. Version and build truth

Rust sets one `help-product-version` property once during `DesktopShell` construction
from `env!("CARGO_PKG_VERSION")`. Snapshot application and route selection never set it.
The UI does not show a commit, executable hash, signing state, package target, SBOM,
release channel, or acceptance claim because those values belong to P6 receipts and
cannot be inferred from this library.

## 5. Attribution exception boundary

The view imports and mounts exactly one `AboutSlint` from `std-widgets.slint`. The
pinned standard component contains Slint's fixed `https://slint.dev` action. TokenMaster
source adds no `Platform.open-url`, arbitrary URL, browser/session state, network client,
callback, or URL property.

The deterministic audit must fail if:

- the standard widget/import is absent or duplicated;
- product source introduces an open-URL callback or arbitrary link;
- the attribution route is replaced with a hand-written lookalike;
- the visible route disappears from top-level navigation.

This narrow pinned attribution is not provider/network authority and does not weaken the
ban on browser-backed quota, private endpoints, credentials, or session reuse.

## 6. Runtime, memory, and privacy boundary

Help/About owns:

- zero product/query/runtime/store state;
- zero Slint list models;
- zero mutable collections;
- zero timers, animations, threads, workers, queues, caches, connections, polling, or
  retained snapshots;
- zero TokenMaster callbacks or commands;
- one compile-time version string and six fixed card instances.

It exposes no paths, provider/account/workspace/source/session IDs, credentials, raw
content, logs, SQL, environment values, current commit, or diagnostic payload.

## 7. Localization and accessibility

P3-D.7 supplies the complete English fallback used by the current P3 UI. It does not
invent an isolated Help-only locale owner. P4 migrates all UI strings together to the
approved English/Russian/pseudo catalogs and hot locale switching.

Every card is an accessible region whose label carries its full meaning. The root and
Slint attribution remain discoverable from the top-level Help/About route. Meaning does
not depend on color. Wide/narrow tests and the real headless accessibility tree are
required now; full keyboard/DPI/locale acceptance remains P4/P6.

## 8. Deferred capabilities

This slice does not implement:

- dynamic diagnostics, log collection/export, clipboard copy, support submission, or
  a support endpoint;
- executable automation examples, CLI, MCP, Hermes connection, or a local listener;
- live locale switching, skins, layout persistence, density, or scheme controls;
- generated dependency notices, SBOM, signed package identity, MSVC evidence, update,
  packaging, signing, M0 acceptance, or release acceptance.

## 9. Acceptance

P3-D.7 is complete only when tests and audits prove:

- the real Help/About view replaces the placeholder and remains ready without archive;
- exactly six fixed sections and one pinned standard `AboutSlint` render;
- version is compile-time-only and set once;
- wide/narrow content and accessibility are complete;
- required source/privacy/health/automation/license truth is visible;
- forbidden release, provider, privacy, URL, runtime, query, timer, model, and callback
  surfaces are absent;
- route switching does no work beyond visibility;
- focused Desktop tests, source/release audit, mutation suite, independent review, and
  the required workspace baseline pass.
