# Sub-agent система Mission Control

**Источник:** DeepSeek TUI SUBAGENTS.md (7 ролей) — адаптировано под WATERS Mission 1

---

## 1. Роли sub-agent'ов

В отличие от TUI, где sub-agents — временные фоновые задачи, в Mission Control каждый sub-agent — **постоянный участник роя** со своим SKILL.md и жизненным циклом.

### 1.1. Роли миссии

| Роль | Назначение | SKILL | Max |
|------|-----------|-------|-----|
| **explorer** | Поиск данных: NASA fireball, спектры | `meteorite-explorer` | 4 |
| **collector** | Сбор деталей: масса, тип, координаты | `meteorite-collector` | 4 |
| **analyzer** | Классификация: тип, траектория, датировка | `meteorite-analyzer` | 2 |
| **matcher** | Поиск аномалий, корреляций, паттернов | `meteorite-matcher` | 2 |
| **coordinator** | Оркестрация, Kafka bridge, heartbeat | `mission-coordinator` | 1 |

### 1.2. SKILL.md метеоритной миссии

Каждый SKILL.md содержит:

```markdown
---
name: meteorite-explorer
role: explore
model: deepseek-v4-flash
instructions: |
  Ты — разведчик данных миссии WATERS Mission 1.
  Используй MCP-сервер mcp-nasa-fireball для поиска болидов.
  Параметры поиска передаются через order.payload.
  Возвращай JSON-массив кандидатов с полями:
  - event_id, date, coordinates (lat/lon), energy (kT),
    velocity (km/s), altitude (km), confidence (0.0-1.0)
---

# Инструкция explorer

Твоя задача — найти необработанные записи о метеоритах/болидах
из NASA Fireball API за указанный период и регион.
```

## 2. Жизненный цикл

```
         ┌─────────────────┐
         │   PENDING       │ ← order получен из Kafka
         └────────┬────────┘
                  │
         ┌────────▼────────┐
         │   ACTIVE        │ ← sub-agent запущен
         └────────┬────────┘
                  │
         ┌────────▼────────┐
         │   WORKING       │ ← LLM вызывает MCP tools
         └────────┬────────┘
              ┌───┴───┐
              │       │
     ┌────────▼┐ ┌────▼──────┐
     │COMPLETED│ │  FAILED   │ ← retry max 3
     └────┬────┘ └────┬──────┘
          │           │ если всё
     ┌────▼────┐ ┌────▼────────┐
     │  IDLE   │ │ ESCALATED   │ ← координатору
     └─────────┘ └─────────────┘
```

## 3. Output contract

Каждый sub-agent возвращает finding в формате:

```json
{
  "finding_id": "uuid",
  "type": "meteorite_candidate|anomaly|spectrum|trajectory",
  "source_agent": "explorer.1",
  "confidence": 0.87,
  "data": {
    "coordinates": {"lat": 48.8566, "lon": 2.3522},
    "mass_kg": 2.3,
    "classification": "H5 chondrite",
    "trajectory": [{...}],
    "spectrum_url": "https://..."
  },
  "rank": 2,
  "timestamp": "2026-05-15T12:34:56Z"
}
```

## 4. Алиасы

Внутренние имена (case-insensitive):
- `explorer` → `e`, `exp`, `scout`
- `collector` → `c`, `col`, `gatherer`
- `analyzer` → `a`, `anl`, `classifier`
- `matcher` → `m`, `mtc`, `pattern`
- `coordinator` → `coord`, `orchestrator`

## 5. Concurrency

| Параметр | Значение |
|----------|----------|
| Max concurrent sub-agents | 10 (настраивается) |
| Heartbeat interval | 60 секунд |
| Retention finding queue | 1000 записей (Redis) |
| Max retries on failure | 3 |
| Timeout per sub-agent | 300 секунд |
