# 🌐 Центр управления миссиями — Mission Control Center / 任务控制中心

**Статус:** Доктрина. Фаза 0.
**Дата:** 08.05.2026
**Версия:** 1.0
**Автор:** Архитектор v1.0 / Конструктор Сети v1.0
**Связанные документы:** [HiveMind](hivemind_spec.md), [Гексада](hexad.md), [Нервная система](nervous_system.md)

---

## 1. Структура центра / Center Structure / 中心结构

### 1.1 Уровни управления / Management Layers / 管理层级

MCC построен по четырёхуровневой модели HiveMind:

| Уровень HiveMind | Название центра | Функция | Ответственный агент |
|:---|:---|:---|:---|
| **IV — Дух** | Этический совет | Верификация миссии, запрет на нарушение Ясы | Хранитель, Законодатель |
| **III — Воля** | Стратегический центр | Принятие решений, планирование миссии | Архитектор, Директор по Смыслу |
| **II — Право** | Тактический центр | Распределение ресурсов, тендеры | Аукционист (агент), Конструктор |
| **I — Сеть** | Операционный центр | Исполнение, телеметрия, связь | Навигатор, Геолог, Оператор дронов, Спектроскопист |

| HiveMind Level | Center Name | Function | Responsible Agent |
|:---|:---|:---|:---|
| **IV — Spirit** | Ethics Council | Mission verification, Yasa compliance | Keeper, Lawkeeper |
| **III — Will** | Strategic Center | Decision making, mission planning | Architect, Director of Meaning |
| **II — Law** | Tactical Center | Resource allocation, tenders | Auctioneer (agent), Constructor |
| **I — Network** | Operations Center | Execution, telemetry, communication | Navigator, Geologist, Drone Operator, Spectroscopist |

| HiveMind 层级 | 中心名称 | 功能 | 负责智能体 |
|:---|:---|:---|:---|
| **IV — 精神** | 伦理委员会 | 任务验证，雅萨合规 | 守护者，立法者 |
| **III — 意志** | 战略中心 | 决策制定，任务规划 | 架构师，意义总监 |
| **II — 法律** | 战术中心 | 资源分配，招标 | 拍卖师（智能体），构造师 |
| **I — 网络** | 运营中心 | 执行，遥测，通信 | 导航员，地质学家，无人机操作员，光谱学家 |

### 1.2 Агенты центра / Center Agents / 中心智能体

| Агент / Agent / 智能体 | Уровень / Level | Роль / Role / 角色 | Инструменты / Tools / 工具 |
|:---|:---|:---|:---|
| **Навигатор** | I | Орбитальная механика, SPICE-расчёты, траектории | SPICE kernels (de440), GMAT, Orekit |
| **Геолог** | I | Анализ грунта, геологические карты, бурение | USGS maps, RELAB, спектрометры |
| **Оператор дронов** | I | Телеметрия дронов, управление полётом, мониторинг | Drone telemetry, RTK GPS, DTN |
| **Спектроскопист** | I | Спектральный анализ, минералогия, химсостав | ECOSTRESS, HYPEX, LIBS |
| **Архитектор** | III | Онтология миссий, спецификации агентов | HiveMind, Kafka, Neo4j |
| **Конструктор** | II | Инфраструктура, Docker, сети, протоколы | Docker, Kafka, Redis, Qdrant |
| **Хранитель** | IV | Этический контроль, соблюдение Ясы | Yasa, Ethics DB |
| **Законодатель** | IV | Правовая верификация, контракты | Contract templates, SLA |

---

## 2. Потоки данных / Data Flows / 数据流

### 2.1 Архитектура потоков / Flow Architecture / 流架构

```
┌─────────────┐     ┌──────────┐     ┌──────────────┐     ┌──────────┐
│   Дроны      │────▶│  Kafka   │────▶│  TimescaleDB │────▶│ Neo4j    │
│   (сенсоры)  │     │ (33 топика)│    │ (телеметрия)  │     │ (граф)   │
└─────────────┘     └──────────┘     └──────────────┘     └──────────┘
       │                  │                  │                  │
       ▼                  ▼                  ▼                  ▼
┌─────────────┐     ┌──────────┐     ┌──────────────┐     ┌──────────┐
│  SPICE/Orekit│    │ Ollama   │     │  Аналитика   │     │ LightRAG │
│  (орбиты)    │    │ (LLM)    │     │  (агенты)    │     │ (знания) │
└─────────────┘     └──────────┘     └──────────────┘     └──────────┘
```

### 2.2 Каналы Kafka / Kafka Topics / Kafka 主题

| Топик / Topic / 主题 | Назначение / Purpose / 用途 | Формат / Format / 格式 |
|:---|:---|:---|
| `telemetry.raw.v1` | Сырая телеметрия с дронов | Avro / JSON |
| `telemetry.processed.v1` | Обработанная телеметрия (фильтр, агрегация) | JSON |
| `telemetry.alerts.v1` | Критические события (потеря связи, низкий заряд) | JSON |
| `orbits.calculated.v1` | Расчётные орбиты и траектории | JSON |
| `geology.samples.v1` | Пробы грунта, спектры, минералы | JSON |
| `planners.questions.v1` | Вопросы от агентов к планёрке | JSON |
| `planners.answers.v1` | Ответы и решения планёрки | JSON |
| `planners.meeting.v1` | События планёрки (созвон, голосование) | JSON |
| `planners.presentations.v1` | Презентации агентов на планёрке | JSON |
| `metrics.raw.v1` | Сырые метрики системы | JSON |
| `metrics.kpi.v1` | KPI миссии | JSON |
| `alerts.security.v1` | Оповещения безопасности | JSON |
| `knowledge.articles.v1` | Статьи базы знаний | JSON |
| `secrets.requests.v1` | Запросы секретов через Хранителя | JSON |

### 2.3 Маршруты данных / Data Routes / 数据路由

1. **Телеметрия**: Дрон → Kafka (`telemetry.raw.v1`) → TimescaleDB → Аналитика → Neo4j (граф состояний)
2. **Орбиты**: SPICE → Навигатор → Kafka (`orbits.calculated.v1`) → Neo4j → Оператор дронов
3. **Геология**: Спектрометр → Геолог → Kafka (`geology.samples.v1`) → Neo4j → LightRAG
4. **Решения**: Проблема → Kafka (`planners.questions.v1`) → Планёрка (HiveMind) → Kafka (`planners.answers.v1`) → Агенты

1. **Telemetry**: Drone → Kafka (`telemetry.raw.v1`) → TimescaleDB → Analytics → Neo4j (state graph)
2. **Orbits**: SPICE → Navigator → Kafka (`orbits.calculated.v1`) → Neo4j → Drone Operator
3. **Geology**: Spectrometer → Geologist → Kafka (`geology.samples.v1`) → Neo4j → LightRAG
4. **Decisions**: Problem → Kafka (`planners.questions.v1`) → Planning (HiveMind) → Kafka (`planners.answers.v1`) → Agents

1. **遥测**：无人机 → Kafka (`telemetry.raw.v1`) → TimescaleDB → 分析 → Neo4j（状态图）
2. **轨道**：SPICE → 导航员 → Kafka (`orbits.calculated.v1`) → Neo4j → 无人机操作员
3. **地质**：光谱仪 → 地质学家 → Kafka (`geology.samples.v1`) → Neo4j → LightRAG
4. **决策**：问题 → Kafka (`planners.questions.v1`) → 规划会议 (HiveMind) → Kafka (`planners.answers.v1`) → 智能体

---

## 3. Принятие решений через HiveMind / Decision Making via HiveMind / 通过 HiveMind 决策

### 3.1 Пять моделей координации / Five Coordination Models / 五种协调模式

| Модель / Model / 模式 | Когда применять / When / 何时使用 | Маршрут / Route / 路由 |
|:---|:---|:---|
| **Военная** | Авария, потеря дрона, критический отказ | III → I → отчёт |
| **Корпоративная** | Распределение ресурсов, тендер на бурение | II → I → II → I |
| **Научная** | Открытие минерала, спор о данных | III → I → II → III |
| **Демократическая** | Смена приоритетов миссии, маршрут | II → I → II → III |
| **Монархическая** | Миссия с сакральным заветом, смена Ясы | IV → III → I → III → IV |

### 3.2 Протокол планёрки / Planning Meeting Protocol / 规划会议协议

1. **Сбор вопросов**: агенты пишут в `planners.questions.v1` за 1 час до планёрки
2. **Планёрка**: HiveMind-модель выбирается автоматически по типу вопроса
3. **Голосование**: через Kafka (для демократической модели)
4. **Фиксация**: решение → `planners.answers.v1` → Neo4j (история решений)

1. **Question collection**: agents write to `planners.questions.v1` 1 hour before meeting
2. **Meeting**: HiveMind model auto-selected by question type
3. **Voting**: via Kafka (for democratic model)
4. **Recording**: decision → `planners.answers.v1` → Neo4j (decision history)

1. **问题收集**：智能体在会议前1小时向 `planners.questions.v1` 写入
2. **会议**：根据问题类型自动选择 HiveMind 模型
3. **投票**：通过 Kafka（民主模式）
4. **记录**：决策 → `planners.answers.v1` → Neo4j（决策历史）

---

## 4. Отказоустойчивость / Fault Tolerance / 容错

### 4.1 Сценарии отказов / Failure Scenarios / 故障场景

| Сценарий / Scenario / 场景 | Действие / Action / 操作 | Время реакции / Reaction / 反应时间 |
|:---|:---|:---|
| **Потеря связи с дроном** | Переход в автономный режим, возврат на базу по GPS | 5 сек |
| **Отказ Kafka-кластера** | Переключение на file-режим (буферизация на дроне) | 1 мин |
| **Потеря агента** | Перезапуск через Supervisor (tmux/systemd) | 30 сек |
| **Отказ TimescaleDB** | Переключение на read-only Neo4j, запись в локальный буфер | 2 мин |
| **Отказ SPICE-расчётов** | Использование кэшированных эфемерид (local cache) | 0 сек |
| **Потеря связи с Землёй** | Автономная миссия по заранее загруженному плану | N/A |

### 4.2 Автономный режим / Autonomous Mode / 自主模式

При потере связи с MCC:

1. Дрон переходит в режим **safe mode**: стабилизация, фиксация текущих координат
2. Все данные буферизируются локально (бортовой накопитель)
3. При восстановлении связи — дельта-синхронизация через DTN
4. Если связь не восстановилась за TTL — дрон возвращается на базу

1. Drone enters **safe mode**: stabilize, lock current coordinates
2. All data buffered locally (onboard storage)
3. On connection restore — delta sync via DTN
4. If connection not restored within TTL — drone returns to base

1. 无人机进入**安全模式**：稳定，锁定当前坐标
2. 所有数据本地缓冲（机载存储）
3. 连接恢复后——通过 DTN 进行增量同步
4. 如果在 TTL 内未恢复连接——无人机返回基地

### 4.3 Резервирование / Redundancy / 冗余

| Компонент / Component / 组件 | Основной / Primary / 主用 | Резервный / Backup / 备用 |
|:---|:---|:---|
| Брокер сообщений | Kafka (1 узел) | Файловая система (буферизация) |
| База телеметрии | TimescaleDB | CSV-логи на дроне |
| Графовая БД | Neo4j | LightRAG (встроенный) |
| Векторная БД | ChromaDB | Qdrant |
| LLM | DeepSeek V4 Flash (внешний API) | Ollama qwen2.5:14b (локальный) |

---

## 5. Интерфейсы MCC / MCC Interfaces / MCC 接口

### 5.1 Внутренние / Internal / 内部

| Интерфейс / Interface / 接口 | Протокол / Protocol / 协议 | Порт / Port / 端口 |
|:---|:---|:---|
| Kafka | TCP (SSL) | 9092 |
| TimescaleDB | PostgreSQL | 25432 (локальный SSH-туннель) |
| Neo4j | Bolt | 17687 (локальный SSH-туннель) |
| Ollama | HTTP REST | 11434 |
| MCP-bridge | STDIO (JSON-RPC) | — |

### 5.2 Внешние / External / 外部

| Интерфейс | Протокол | Назначение |
|:---|:---|:---|
| OpenCode CLI | TUI (Terminal) | Управление агентами |
| GitHub API | HTTPS (REST) | Версионирование, CI/CD |
| NASA SPICE | FTP/HTTP (kernels) | Эфемериды |

---

## 6. Метрики центра / Center KPIs / 中心 KPI

| Метрика / Metric / 指标 | Цель / Target / 目标 | Измерение / Measurement / 测量 |
|:---|:---|:---|
| Время принятия решения | < 30 сек (авто), < 5 мин (планёрка) | Kafka latency |
| Задержка телеметрии | < 1 сек (Земля), < 5 сек (эмулированная Луна) | TimescaleDB |
| Доступность центра | > 99.5% | Uptime Kuma |
| Время восстановления | < 1 мин (авто), < 5 мин (ручное) | Системные логи |
| Полнота данных | 100% (нет потерь телеметрии) | Kafka offset |

---

---

## 7. Движок миссий — mission-control (форк DeepSeek-TUI)

**Статус:** Проектирование. Фаза 1.
**Дата:** 15.05.2026
**Источник:** `docs/mission-1/`

### Решение: создать форк DeepSeek-TUI как движок миссий

DeepSeek-TUI (MIT-лицензия) взят за основу для создания собственного движка `mission-control`.

**Что берём из TUI без изменений:**
- RLM (Pro + Flash) — иерархическая система агентов
- Durable Task Queue (RocksDB)
- HTTP Runtime API
- MCP Client (подключение внешних MCP-серверов)

**Что перерабатываем:**
- Типизированные агенты (DataCollectorAgent, AnalyzerAgent, PatternMatcherAgent, CoordinatorAgent) вместо универсальных Flash
- Redis pub/sub как шина между агентами (JSON)
- Memory Layer: ChromaDB (вектора) + LightRAG (граф) + Redis (состояние)
- Mission Mode (автономная работа 24/7 без человека)
- Адаптер-мост HTTP → Kafka для связи с центром 238

**Skills-система:**
- Внутренний формат: TOML (skill.toml) + INSTRUCTIONS.md
- Конвертеры: TUI .skill.md → наш формат, OpenCode SKILL.md → наш формат
- И наоборот: экспорт нашего скилла в TUI/OpenCode

### Архитектура связи

```
МИССИЯ (TUI-форк)                    ЦЕНТР 238 (Kafka)
┌─────────────────┐                  ┌──────────────────┐
│  RLM Pro        │  HTTP bridge     │  Kafka broker    │
│   ├─ Flash × N  │◄───────────────► │  (20+ топиков)   │
│   ├─ Redis      │  findings/orders │  OpenCode агенты │
│   ├─ ChromaDB   │                  │  MCP адаптеры    │
│   └─ Ollama     │                  │  Global ChromaDB │
└─────────────────┘                  └──────────────────┘
```

### Серверная инфраструктура

| Узел | Сервер | Роль |
|------|--------|------|
| **Центр 238** | Hetzner AX52 (8 ядер, 32GB, 1TB) | Kafka, OpenCode, глоб. БД |
| **Миссия 1** | Hetzner AX102 (12 ядер, 32GB, 1TB) | TUI-форк, RLM, Ollama, Redis, ChromaDB |

### Документация

Полное описание: `docs/mission-1/` (6 файлов):
- `README.md` — обзор миссии
- `tui-fork-plan.md` — план форка TUI
- `architecture.md` — архитектура и потоки данных
- `server-plan.md` — развёртывание сервера
- `project-analysis.md` — сравнение аналогов, апгрейд 238
- `team-workflow-example.md` — пример работы команды агентов

---

*MCC — нервный центр платформы WATERS. Без него миссия слепа.*
*MCC is the nerve center of the WATERS platform. Without it, the mission is blind.*
*MCC 是 WATERS 平台的神经中枢。没有它，任务就是盲目的。*
