# TokenMaster

TokenMaster — нативный локальный монитор использования Codex для Windows. Это
самостоятельный проект на Rust, Slint и SQLite: без Electron, без фонового сервиса и
без зависимости от Go или Node.js во время работы.

WhereMyTokens задаёт полноту интерфейса и сценариев пользователя. ccusage задаёт
полноту импорта, аналитики токенов, моделей, стоимости и отчётов. TokenMaster не
запускает и не встраивает эти проекты: реализация, контракты безопасности и критерии
качества принадлежат самому TokenMaster.

Сейчас готов M0 — доказательство стека с мгновенной сменой layout/skin/locale,
виртуализированными моделями, tray lifecycle и измеряемыми ограничениями ресурсов.
M1 реализует ограниченное обнаружение Codex-источников, потоковый JSONL reader,
checkpoint, строгую SQLite-схему и атомарный append. Теперь Codex выдаёт только
provider-neutral observation/session-relation drafts, а fingerprint, replay identity
и canonical event создаёт исключительно `tokenmaster-accounting`. Следующий участок —
недеструктивное versioned-хранилище результатов уже готового ограниченного
replay-классификатора и его staging-интеграция.

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
