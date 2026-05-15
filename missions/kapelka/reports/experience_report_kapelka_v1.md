# Experience Report: Kapelka Audit v1

**Дата:** 2026-05-10
**Руководитель:** Конструктор Сети (agent.constructor.v1)
**Проект:** ISamFX/kapelka — React/Vite/Solana SPA
**Модель:** deepseek-v4-flash (sub-agent Task)

## Архитектура запуска

Вместо Python-демонов с Ollama (237 убит) — sub-agent Task tool.
Схема: Я (deepseek-v4-flash) → Task(architect) → Task(constructor) → Task(auditor)

Плюсы:
- Не нужен Kafka/Ollama/API-ключи
- Агенты имеют полный доступ к filesystem
- Каждый агент — изолированный контекст
- Модель везде одна (deepseek-v4-flash)

Минусы:
- Нет постоянного демона (polling)
- Ручной запуск цепочки

## Результаты аудита

**Общая оценка: D (52/100)**

| Категория | Оценка | Балл |
|-----------|--------|------|
| Frontend Code Quality | D | 55 |
| Security | D | 45 |
| Solana Integration | B | 78 |
| Deploy Readiness | F | 25 |

## Критические проблемы

1. **Хардкод Helius API key** — rpc.config.ts
2. **Нет Docker/Nginx/CI-CD** — deploy/* сгенерированы, не интегрированы
3. **skipPreflight:true во всех транзакциях** — mainnet небезопасно
4. **398+ console.log** — утечка PDA, sessionId, wallet hash
5. **process.env полифилл пустой** — dev-гейтинг сломан

## Сильные стороны проекта

- ScaledBatchedRpcFetcher с adaptive TTL, fallback, retry
- 62 аккаунта + 32 шарда с полной типизацией
- Чистая архитектура роутинга
- RPC Auditor с IndexedDB-хранением

## Сгенерированные артефакты

```
clients/IsamFX_kapelka/
├── client.json
├── reports/
│   ├── audit_plan.json (128 строк, 7 секций)
│   └── audit_report_summary.json (243 строки, полный отчёт)
└── deploy/
    ├── Dockerfile
    ├── docker-compose.yml
    ├── nginx.conf (SPA fallback, security headers, gzip, rate limit)
    ├── .env.production.example
    ├── deploy.sh (quick/full/ssl/rollback/check)
    └── .github/workflows/deploy.yml
```

## Уроки

1. Sub-agent Task tool эффективнее Python-демонов для разовых задач
2. Пустой process.env полифилл в Vite — частая ошибка, нужно документировать
3. Для деплоя Solana dApp нужен жёсткий CVSS-триаж
4. Token2022 + Phantom = known issue, нужен fallback display
