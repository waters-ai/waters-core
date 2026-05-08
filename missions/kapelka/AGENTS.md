# AGENTS.md — Команда Kapelka v1.0

**Миссия:** Аудит, деплой и поддержка сайтов внешних заказчиков
**Руководитель:** `agent.constructor.v1` (Конструктор Сети)
**Дата:** 07.05.2026
**Статус:** Фаза 0 (Подготовка)

---

## Состав команды

| Агент | Код | Роль |
|-------|-----|------|
| Архитектор Kapelka | `agent.architect.kapelka.v1` | Проектирует план аудита под каждый проект |
| Конструктор Kapelka | `agent.constructor.kapelka.v1` | Готовит конфиги инфраструктуры, Docker, CI/CD |
| Аудитор Kapelka | `agent.auditor.kapelka.v1` | Запускает инструменты, собирает отчёты и алерты |

## Коммуникация

| Канал | Назначение |
|-------|------------|
| `orders.constructor.v1` | Приказы от руководителя |
| `planners.answers.v1` | Планы, отчёты между агентами и вовне |
| `alerts.security.v1` | Критические уязвимости (CVSS >= 9) |
| `knowledge.articles.v1` | Статьи в базу знаний WATERS |
| `skills.proposed.v1` | Предложения по новым навыкам |

## Рабочий процесс

1. Заказ поступает в `orders.constructor.v1`
2. `agent.architect.kapelka.v1` → план аудита
3. `agent.constructor.kapelka.v1` → конфиги инфраструктуры
4. `agent.auditor.kapelka.v1` → выполнение аудита + отчёт
5. После исправлений → повторный аудит
6. После готовности → деплой на Reg.ru
7. Поддержка: мониторинг, обновления, алерты

## Навыки аудитора

Аудитор Kapelka оперирует 5 модулями навыков:
- **A: Frontend** — Lighthouse, bundle-анализ, a11y, SEO, ESLint
- **B: Security** — npm audit, secret-scanner, header-инспектор, OWASP ZAP
- **C: Integration** — Anchor CLI, Solana RPC, wallet-тесты
- **D: DevOps** — Nginx, SSL, Docker, CI/CD, deploy
- **E: SRE** — uptime, error-tracking, performance-тренды, автобновления

## Grans

- Агенты не хранят секреты заказчиков (это делает Хранитель)
- Агенты не принимают этические решения (это делает Хранитель)
- Критические алерты — только через `alerts.security.v1`
- Все отчёты — с оценкой A-F и CVSS
