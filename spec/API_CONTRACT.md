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
