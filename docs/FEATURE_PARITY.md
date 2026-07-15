# TokenMaster feature matrix

The matrix is a requirements guide, not a copied implementation or compatibility
claim. Each TokenMaster behavior must be specified, tested, bounded, and privacy-safe.

| Reference area | Target capability | TokenMaster status |
| --- | --- | --- |
| WhereMyTokens: quota-first board | Plan usage, code output, trend, sessions, activity, model usage | Planned product board; M0 presentation primitives exist; weekly full-reset epochs add exact before/after and early/repeated-reset history |
| WhereMyTokens: exploration views | History, sessions, models, projects, activity, health, settings, help, compact access | Planned |
| WhereMyTokens: interaction | Dense dark information design, responsive layouts, keyboard/accessibility, tray access | M0 layouts, skins, localization, tray contracts implemented |
| ccusage: source handling | Codex history discovery, active/archive precedence, incremental update | M1 discovery and bounded reader implemented |
| ccusage: usage semantics | Token fields, cumulative deltas, model normalization, session identity | M1 parser/domain implemented |
| ccusage: analytics | Cost, reports, model/session/project breakdowns, periods | Planned after staging and index completion |
| TokenMaster improvement | Bounded memory, path-private storage, transactional SQLite, immutable UI snapshots | Core contracts and current-generation ingest implemented |
