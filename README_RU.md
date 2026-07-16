# TokenMaster

TokenMaster — нативный локальный монитор использования Codex для Windows. Это
самостоятельный проект на Rust, Slint и SQLite: без Electron, без фонового сервиса и
без зависимости от Go или Node.js во время работы.

WhereMyTokens задаёт полноту интерфейса и сценариев пользователя. ccusage задаёт
полноту импорта, аналитики токенов, моделей, стоимости и отчётов. TokenMaster не
запускает и не встраивает эти проекты: реализация, контракты безопасности и критерии
качества принадлежат самому TokenMaster.

M0 доказал нативный стек, мгновенную смену layout/skin/locale, виртуализированные
модели, tray lifecycle и измеряемые ограничения ресурсов. Контур данных уже включает
ограниченное обнаружение Codex-источников, потоковый JSONL reader, replay-safe
accounting, production incremental refresh и строгую SQLite-схему v8. P2-A даёт
неизменяемые ограниченные activity-запросы. В P2-B готовы provider-self-contained
canonical events, транзакционные UTC/session rollups и возобновляемая постраничная
перестройка агрегатов. Далее — фиксированные aggregate queries, календарь/timezone,
pricing, quota/reset data, полный UI, automation и release evidence.

```powershell
cargo +1.97.0 test --workspace --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\verify-m0.ps1 -RepositoryRoot (Get-Location).Path
```

Последняя команда создаёт только developer evidence. M0 и продуктовый релиз не
приняты, пока нет отдельных интерактивных Windows и непрерывных soak receipts.

Подробности: [утверждённый аудит и master plan](docs/AUDIT_AND_MASTER_PLAN.md),
[архитектура](docs/ARCHITECTURE.md),
[матрица функциональности](docs/FEATURE_PARITY.md),
[roadmap](docs/ROADMAP.md) и [handoff](docs/HANDOFF.md).
