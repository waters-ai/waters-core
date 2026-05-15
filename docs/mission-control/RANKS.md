# Система рангов Mission Control

**Источник:** [h2o-mining.space](https://h2o-mining.space) (Kapelka Solana contract) — адаптирована для WATERS Mission 1

---

## 1. Концепция

Ранги — это **KPI-система** для агентов миссии, адаптированная из Solana-контракта Kapelka (h2o-mining.space). Вместо Solana PDA используем Kafka-топики. Вместо токенов WATER — метрики миссии (findings, confidence, uptime).

```
Solana (h2o-mining):              Mission Control (адаптация):
  MemRankConfig PDA               → mission.1.config.v1 (rank_config)
  User PDA offset 219             → mission.1.agents.v1 (agent.rank)
  update_mem_rank инструкция      → Rank Engine (Python)
  bonus_fund токены               → привилегии (priority, access)
  team_operations INIT/BATCH/...  → mission.1.ranks.v1 (team_rank)
```

---

## 2. Личные ранги (Personal Ranks)

### 2.1. Уровни

| Ранг | Название | Findings | Conf. | Uptime | Привилегия |
|------|----------|----------|-------|--------|------------|
| 0 | Новобранец | 0 | — | — | Базовые MCP |
| 1 | **Bronze** | ≥10 | ≥50% | — | explorer + collector |
| 2 | **Silver** | ≥50 | ≥70% | ≥24h | + analyzer |
| 3 | **Gold** | ≥200 | ≥85% | ≥168h | + matcher, управление sub-agent'ами |
| 4 | **Platinum** | ≥1000 | ≥95% | ≥720h | Полный доступ, mission override |

### 2.2. Конфигурация (Kafka: `mission.1.config.v1`)

```json
{
  "type": "rank_config",
  "ranks": [
    {"level": 1, "name": "Bronze",  "min_findings": 10,  "min_confidence": 0.50, "min_uptime_hours": 0},
    {"level": 2, "name": "Silver",  "min_findings": 50,  "min_confidence": 0.70, "min_uptime_hours": 24},
    {"level": 3, "name": "Gold",    "min_findings": 200, "min_confidence": 0.85, "min_uptime_hours": 168},
    {"level": 4, "name": "Platinum","min_findings": 1000,"min_confidence": 0.95, "min_uptime_hours": 720}
  ]
}
```

### 2.3. Триггер апгрейда

Как в Kapelka: проверка при каждом значимом событии.

```
Каждые 10 findings → Rank Engine:
  1. Суммирует KPI агента (findings, avg_confidence, uptime)
  2. Проверяет условия текущего ранга +1
  3. Если выполнены → апгрейд
  4. Публикует mission.1.ranks.v1
```

---

## 3. Командные ранги (Team Ranks)

### 3.1. Уровни

| Ранг | Агентов | Findings | Точность | Привилегия |
|------|---------|----------|---------|------------|
| 1 | ≥2 | — | — | Базовый heartbeat к 238 |
| 2 | ≥3 | ≥100 | ≥70% | Приоритетный канал |
| 3 | ≥5 | ≥500 | ≥85% | Полный дуплекс |
| 4 | ≥8 | ≥2000 | ≥95% | Автономный L0 |

### 3.2. Процесс (адаптация team_operations из Kapelka)

```
INIT (sub_tag 0)       → начать пересчёт командных KPI
BATCH (sub_tag 1-250)  → пачки данных по агентам
FINALIZE (sub_tag 251) → завершить вычисления
APPLY_RANK (sub_tag 252) → применить командный ранг
DISTRIBUTE (sub_tag 253) → распределить привилегии
```

---

## 4. Администрирование рангов

| Действие | Kafka топик | Формат |
|----------|------------|--------|
| Изменить конфиг рангов | `mission.1.config.v1` | JSON c type="rank_config" |
| Просмотреть ранг агента | `mission.1.agents.v1` | `agents.{id}.rank` |
| История апгрейдов | `mission.1.ranks.v1` | JSON c event="upgraded|demoted" |
| Командный ранг | `mission.1.ranks.v1` | JSON c type="team" |

---

## 5. Сравнение с h2o-mining.space

| Параметр | [h2o-mining.space](https://h2o-mining.space) (Solana) | Mission Control (Kafka) |
|----------|--------------------------|------------------------|
| Хранение | on-chain (PDA) | off-chain (Kafka) |
| Валидация | контракт (Rust) | Rank Engine (Python) |
| Токены | WATER (Solana SPL) | Нет токенов |
| Газ | SOL комиссия | Нет |
| Прозрачность | Explorer | Kafka consumer |
| Скорость | ~400ms | ~1ms |
