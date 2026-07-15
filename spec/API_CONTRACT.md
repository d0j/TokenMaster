# TokenMaster interface contract

Future interfaces are versioned, local-only, and bounded. Until implementation,
unlisted API, CLI, and MCP behaviors do not exist.

## CLI

The future CLI returns strict JSON for data commands and uses non-zero exits for
failure. Inputs use enumerated ranges, capped limits, and schema validation. Errors
use stable codes and bounded path-free descriptions.

## MCP

MCP uses stdio JSON-RPC. Every schema sets `additionalProperties: false`. It may expose
only bounded TokenMaster queries and an idempotent non-destructive refresh operation.
It MUST NOT expose arbitrary SQL, shell, HTTP, filesystem, credential, prompt,
response, or transcript operations.

## UI data boundary

The UI consumes immutable bounded snapshots. It receives stable data-quality and
freshness states and never directly receives source paths or raw source content.

Quota snapshots expose current window epochs and a bounded transition page. Full
weekly resets include before/after values, maximum pre-reset use, old/new reset times,
transition kind, evidence source, confidence, and an exact or bounded detection time.
CLI and MCP use the same fields and stable transition sequence so automation can react
idempotently. Unavailable provider capacity remains `null`/unavailable, not zero or an
estimate derived from local token usage.

## Provider plugin ABI

The future external-provider ABI is `tokenmaster:provider@1` expressed in WIT and
executed only by an isolated `tokenmaster-plugin-host`. A provider component may expose
bounded metadata, health, discovery, scan-page, and quota-page operations. It returns
provider-neutral observation drafts and opaque checkpoints, never canonical events,
fingerprints, replay dispositions, SQL, UI components, commands, or MCP tools.

Plugins receive no ambient WASI filesystem, network, environment, subprocess, or
stdio authority. Optional host capability imports provide scoped read-only filesystem,
allowlisted HTTPS, host-injected credential, and clock operations. All values and the
engine-to-host framed protocol use strict versioned schemas and hard byte/count/time
limits. The full package/runtime contract is recorded in
`docs/superpowers/specs/2026-07-14-tokenmaster-provider-plugin-system-design.md`.
