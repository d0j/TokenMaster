# Security policy

## Supported status

TokenMaster is pre-alpha and not released. There is no supported public TokenMaster
binary yet. The repository still treats privacy, local data, and release-supply-chain
issues as security-sensitive.

## Private vulnerability reporting

Use GitHub's [private vulnerability reporting](https://github.com/d0j/TokenMaster/security/advisories/new)
for suspected vulnerabilities. Do not open a public issue, discussion, or pull request
for an unpatched vulnerability.

Include only what is necessary:

- affected version or commit;
- impact and realistic attack boundary;
- minimal reproduction steps;
- stable, path-free error codes or diagnostics; and
- a safe way to confirm the fix.

Do not include credentials, tokens, private account identifiers, prompts, responses,
reasoning text, tool arguments, commands, command output, file contents, raw JSONL, or
absolute user paths. If sensitive evidence is essential, describe its shape first and
wait for a private maintainer response before attaching it.

## Scope priorities

High-priority reports include local-data disclosure, secret exposure, unsafe archive
or path handling, loopback authentication bypass, arbitrary CLI/MCP system access,
release identity/hash bypass, and unbounded resource behavior triggered by untrusted
input.

Public feature requests, non-sensitive bugs, and performance ideas belong in the
normal issue forms.
