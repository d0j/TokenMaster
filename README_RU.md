# TokenMaster

TokenMaster — нативный локальный монитор использования Codex для Windows. Это
самостоятельный проект на Rust, Slint и SQLite: без Electron, без фонового сервиса и
без зависимости от Go или Node.js во время работы.

Оригинальный код TokenMaster распространяется по Apache-2.0. WhereMyTokens и ccusage
остаются отдельно атрибутированными внешними MIT-референсами.

WhereMyTokens задаёт полноту интерфейса и сценариев пользователя. ccusage задаёт
полноту импорта, аналитики токенов, моделей, стоимости и отчётов. TokenMaster не
запускает и не встраивает эти проекты: реализация, контракты безопасности и критерии
качества принадлежат самому TokenMaster.

M0 доказал нативный стек, мгновенную смену layout/skin/locale, виртуализированные
модели, tray lifecycle и измеряемые ограничения ресурсов. Контур данных уже включает
ограниченное обнаружение Codex-источников, потоковый JSONL reader, replay-safe
accounting, production incremental refresh и строгую мигрируемую SQLite-схему на
`USAGE_SCHEMA_VERSION`. P2-A даёт неизменяемые ограниченные activity-запросы. В P2-B
готовы provider-self-contained canonical events, транзакционные UTC/session rollups и
возобновляемая постраничная перестройка агрегатов. Далее — фиксированные aggregate
queries, календарь/timezone, pricing, quota/reset data, полный UI, automation и
release evidence.

```powershell
cargo test --workspace --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\verify-secret-scan.ps1 -RepositoryRoot (Get-Location).Path -PackagePath dist\TokenMaster-0.1.0-windows-x64-unsigned.zip
pwsh -NoProfile -File scripts\verify-m0.ps1 -RepositoryRoot (Get-Location).Path
```

Не добавляйте сюда переопределение `+toolchain`: `rust-toolchain.toml` уже прибивает
`1.97.0-x86_64-pc-windows-msvc`, а голое `cargo +1.97.0` отменяет пин и достраивает
версию хостом по умолчанию — на хосте `windows-gnu` это выберет GNU-тулчейн без
`dlltool.exe`, и сборка упадёт.

Последняя команда создаёт только developer evidence. M0 и продуктовый релиз не
приняты, пока нет отдельных интерактивных Windows и непрерывных soak receipts.

Подробности: [архитектура](docs/ARCHITECTURE.md),
[матрица функциональности](docs/FEATURE_PARITY.md)
и [план выхода к релизу](docs/RELEASE_EXIT_PLAN.md).
