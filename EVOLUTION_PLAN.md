# EVOLUTION PLAN — WATERS Node Self-Improvement Roadmap

**Версия:** v0.5.0-alpha → v1.0.0
**Дата:** 2026-05-17
**Вектор:** Качество → Стабильность → Экосистема → Масштаб

---

## Фаза 1: Качество кода (сейчас)

### Цель
```
warnings = 0
tests ≥ 50
unwrap = 0
```

### Задачи для TUI-агентов

| № | Задача | Агент | Приоритет |
|---|--------|-------|-----------|
| 1 | Убрать unwrap() в security.rs → обработка ошибок | implementer | P0 |
| 2 | Убрать unwrap() в media_bridge.rs → обработка | implementer | P0 |
| 3 | Убрать unwrap() в cron.rs → безопасные тесты | implementer | P1 |
| 4 | Убрать unwrap() в store.rs (тесты) → ok().unwrap() | implementer | P1 |
| 5 | Добавить тесты для bridge.rs (основной транспорт) | verifier | P0 |
| 6 | Добавить тесты для handlers.rs (все команды) | verifier | P1 |
| 7 | Добавить тесты для subagent.rs (lifecycle) | verifier | P1 |
| 8 | Добавить тесты для api.rs (REST endpoints) | verifier | P1 |
| 9 | Добавить тесты для group_chat.rs, gossip.rs | verifier | P2 |
| 10 | Добавить тесты для skill.rs, security.rs, tunnel.rs | verifier | P2 |

### Команды для запуска
```
/self improve        — выполнить цикл
/self status         — текущий прогресс
/diagnose            — проверка результатов
```

---

## Фаза 2: Стабильность (после Фазы 1)

### Цель
```
uptime > 99.5%
recovery < 10s
auto-backup
```

### Задачи
| № | Задача | Агент |
|---|--------|-------|
| 1 | Системные логи — структурированный JSON | implementer |
| 2 | Graceful degradation при потере Redis | implementer |
| 3 | Health endpoint — расширенные метрики | implementer |
| 4 | Auto-backup .waters/ → .waters/backups/ | implementer |
| 5 | Тесты на восстановление после падения | verifier |

---

## Фаза 3: Экосистема (после Фазы 2)

### Цель
```
skills ≥ 50
platforms ≥ 10
plugins ≥ 3
```

### Задачи
| № | Задача | Агент |
|---|--------|-------|
| 1 | MCP registry — реальный fetch с huggingface | implementer |
| 2 | Signal, Line, Slack chat transports | implementer |
| 3 | Plugin system — ctx.llm + tool_override | implementer |
| 4 | Skill self-improvement — полный цикл | implementer |
| 5 | Конвертация TUI → waters format | implementer |

---

## Фаза 4: Масштаб (после Фазы 3)

### Цель
```
peers ≥ 6
DTN failover < 1s
fork auto-deploy
```

### Задачи
| № | Задача | Агент |
|---|--------|-------|
| 1 | P2P mesh — полная сетка 6 нод | implementer |
| 2 | DTN — production-ready | implementer |
| 3 | Fork auto-deploy на GitHub | implementer |
| 4 | Синхронизация контактов между нодами | implementer |

---

## Метрики прогресса

```
Фаза 1: warnings=0, tests≥50, unwrap=0
Фаза 2: uptime≥99.5%, recovery<10s, backup ежедневно
Фаза 3: skills≥50, platforms≥10, plugins≥3
Фаза 4: peers≥6, DTN≤1s failover, forks≥3
```

## Статус сейчас

```
✅ v0.5.0-alpha — 12MB, 44 модуля, 29 тестов
✅ Нода на VPS — жива, получает команды (2 ноды в P2P mesh)
✅ TUI-агенты (7 шт) — вшиты в бинарник
✅ Self-improve цикл — diagnose → plan → implement → review → verify → deploy
✅ Fork на GitHub — форки создаются в waters-ai/*

📌 Текущая фаза: 1 — Качество кода
📊 36 unwrap, 38 modules без тестов, 154 warnings
```
