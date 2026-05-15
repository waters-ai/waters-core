# Mission Control — автономная агентная система поиска метеоритов

> **Форк:** DeepSeek TUI (v0.8.37, MIT) → WATERS Mission Control v1.0
> **Оригинал:** [github.com/Hmbown/DeepSeek-TUI](https://github.com/Hmbown/DeepSeek-TUI) (29.6k ⭐)
> **Платформа:** [WATERS](https://github.com/waters-ai/waters-core) — открытая платформа для автономных ИИ-агентов

---

## Что это?

**Mission Control** — агентная система для автономного поиска, классификации и трекинга метеоритов. Запускается на выделенном сервере, управляется через Kafka с центрального хаба 238, работает в пяти уровнях автономии (L0-L4) и возвращает findings на хаб для глобальной памяти.

Это не просто форк DeepSeek TUI. Это **адаптация**:

| Что взяли от TUI | Что изменили | Что добавили |
|-----------------|-------------|--------------|
| Sub-agents (7 ролей) | Одноагентный цикл → Swarm | Kafka как единственная шина |
| MCP-клиент | HTTP bridge → Kafka bridge | Система рангов (адаптация из [h2o-mining.space](https://h2o-mining.space)) |
| SKILL.md система | Memory.md → ChromaDB/LightRAG | DTN-эмуляция (tc-netem) |
| Tool surface | Config → Kafka config | Autonomy Engine L0-L4 |
| Архитектура Rust | Rust → Python (ядро) | Rank Engine (KPI → ранг) |

### Ключевые особенности

- **Kafka-native архитектура** — 6 топиков (orders, findings, agents, ranks, heartbeat, config). Никакого HTTP bridge
- **Автономия L0-L4** — от полной мощности до safe mode. Работает без связи с центром
- **Система рангов** — 4 личных + 4 командных ранга (адаптированы из [h2o-mining.space](https://h2o-mining.space)). Каждый finding повышает ранг агента
- **MCP для всего** — единый протокол для NASA, спектров, траекторий, памяти
- **DTN-эмуляция** — tc-netem имитирует задержки Земля-Луна (1.3s), Земля-Марс (5-20min)

---

## Архитектура

```
                    ═══════ KAFKA (6 топиков) ═══════
                    │                                  │
  ┌─────────────────┴────────────────────┐   ┌─────────┴──────────┐
  │         238 ХАБ                       │   │ Миссионный сервер   │
  │  OpenCode (Архитектор, Конструктор)  │   │                     │
  │  ChromaDB + LightRAG (память)        │   │ Agent Runtime (Python)
  │  Redis (глобальное состояние)        │◄──►│   ├── Sub-agents ×N │
  │  Kafka (orders→ findings←)           │   │   ├── MCP-клиент    │
  └──────────────────────────────────────┘   │   ├── Rank Engine   │
                                             │   ├── Autonomy Eng  │
                                             │   └── DTN (tc-netem)│
                                             └─────────────────────┘
```

---

## Быстрый старт

```bash
# 1. Установить Agent Runtime
pip install -r requirements.txt

# 2. Настроить Kafka-конфиг
export KAFKA_BROKER="238.hub.waters:9092"
export MISSION_ID="mission-1"

# 3. Загрузить SKILL.md для sub-agent'ов
mission-control install-skill skills/meteorite-explorer/
mission-control install-skill skills/meteorite-collector/
mission-control install-skill skills/meteorite-analyzer/
mission-control install-skill skills/mission-coordinator/

# 4. Запустить миссию
mission-control start --mode autonomous
```

---

## Режимы работы

| Режим | Kafka | LLM | Поведение |
|-------|-------|-----|-----------|
| **L0** | ✅ Online | DeepSeek API | Полная мощность |
| **L1** | ✅ Online | Ollama (local) | Экономия API |
| **L2** | ❌ Offline | Ollama (local) | Автономный, очередь findings |
| **L3** | ❌ Offline | Ollama (урезанная) | Энергосбережение |
| **L4** | ❌ Offline + нет энергии | — | Safe mode, только логи |

---

## Система рангов

```
BRONZE (1) → SILVER (2) → GOLD (3) → PLATINUM (4)
  ↑              ↑             ↑              ↑
  10 findings   50 findings   200 findings   1000 findings
  conf≥50%      conf≥70%      conf≥85%       conf≥95%
```

Личные ранги определяют привилегии агента. Командные ранги — приоритет доступа к 238.

---

## MCP-серверы

| Сервер | Данные | Протокол |
|--------|--------|----------|
| `mcp-nasa-fireball` | NASA Fireball API | stdio |
| `mcp-spectra` | Спектральные базы | stdio |
| `mcp-trajectory` | Расчёт траекторий | stdio |
| `mcp-knowledge` (на 238) | ChromaDB + LightRAG | SSE |

---

## Документация

| Документ | Описание |
|----------|----------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Полная архитектура системы |
| [AGENTS.md](AGENTS.md) | Sub-agent система, роли, lifecycle |
| [MCP.md](MCP.md) | MCP-серверы и протокол |
| [RANKS.md](RANKS.md) | Система рангов (личные + командные) |
| [MODES.md](MODES.md) | Уровни автономии L0-L4 |
| [CONFIGURATION.md](CONFIGURATION.md) | Конфигурация миссии |
| [KAFKA_PROTOCOL.md](KAFKA_PROTOCOL.md) | Kafka-протокол, 6 топиков, JSON-схемы |
| [INSTALL.md](INSTALL.md) | Установка и запуск |
| [WEBSITE.md](WEBSITE.md) | Сайт виртуальной миссии |

---

## Лицензия

[MIT](../../LICENSE) — форк DeepSeek TUI (MIT). WATERS надстройка — MIT.

---

*"Вода принимает форму. Миссия находит цель."*
