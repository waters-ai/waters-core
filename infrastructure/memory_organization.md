# Организация памяти Конструктора Сети

## Архитектура памяти

### Слои памяти

| Слой | Технология | Назначение | Retention |
|------|------------|------------|-----------|
| **Оперативная** | Redis | Состояние сервисов, сессии, кэш конфигов | До перезагрузки |
| **Векторная** | ChromaDB | Документация, runbook'и, how-to | Постоянно |
| **Графовая** | LightRAG | Топология, зависимости, связи компонентов | Постоянно |
| **Структурированная** | CozoDB | SLA, правила, ограничения, схемы | Постоянно |
| **Событийная** | Kafka/Redpanda | Логи событий, метрики, алерты | 30-90 дней |
| **Файловая** | ФС | Схемы JSON, docker-compose, скрипты | Постоянно |

## Стратегия наполнения памяти

### 1. ChromaDB (семантический поиск)

**Коллекция: `infrastructure_docs`**
- Документация по Docker, Kafka, Redis, ChromaDB и др.
- Runbook'и по устранению неполадок
- Чеклисты проверки состояния

**Коллекция: `migration_guides`**
- Инструкции Базовый → Оптимальный → Опережающий
- Сравнение технологий (Redis vs Sentinel vs Cluster)

### 2. LightRAG (граф знаний)

**Типы нод:**
- `component` — сервис (Redpanda, Redis, ChromaDB...)
- `scenario` — сценарий (basic, optimal, advanced)
- `dependency` — связь между компонентами
- `metric` — метрика (SLA, KPI, cost)

**Связи:**
- `depends_on` — компонент A зависит от B
- `implements` — сценарий реализует компонент
- `monitors` — метрика мониторит компонент

### 3. CozoDB (правила и ограничения)

**Таблица: `schemas`**
```cozo
schema_id, model, name, version, status
hivemind_military, military, "Военная модель", 1.0, active
hivemind_corporate, corporate, "Корпоративная модель", 1.0, active
```

**Таблица: `sla_rules`**
```cozo
component, metric, threshold, action
redis, uptime, 99.5, "alert"
chromadb, latency, 200ms, "scale"
```

**Таблица: `cost_limits`**
```cozo
scenario, max_usd_per_month, current_estimate
basic, 150, 125
optimal, 200, 180
advanced, 350, 320
```

### 4. Kafka (события)

**Топики для Конструктора:**
- `events.system.v1` — события инфраструктуры
- `metrics.raw.v1` — сырые метрики (CPU, RAM, диск)
- `metrics.kpi.v1` — агрегированные KPI
- `alerts.security.v1` — алерты безопасности

### 5. Файловая система

**Структура:**
```
infrastructure/
├── docker/
│   ├── topology_basic.json      # Базовый сценарий
│   ├── topology_optimal.json    # Оптимальный
│   └── topology_advanced.json   # Опережающий
├── kafka/
│   └── topics.json              # Список топиков и схем
├── monitoring/
│   └── sla.json                 # SLA-метрики
└── dtn/
    └── delays.json              # Эмуляция задержек
```

## Приоритеты заполнения (Спринт 1)

1. ✅ Создать 6 схем HiveMind (военная, корпоративная, научная, демократическая, монархическая, анархическая)
2. ✅ Создать 3 Docker-топологии
3. ⏳ Наполнить CozoDB правилами SLA
4. ⏳ Создать коллекции в ChromaDB
5. ⏳ Настроить граф связей в LightRAG

## Интеграция с Нервной системой

Конструктор читает:
- `planners.questions.v1` — вопросы от Архитектора
- `planners.answers.v1` — ответы Архитектора
- `events.system.v1` — системные события

Конструктор пишет:
- `planners.presentations.v1` — презентации инфраструктуры
- `metrics.raw.v1` — метрики компонентов
- `metrics.kpi.v1` — KPI инфраструктуры
- `events.system.v1` — события инфраструктуры
