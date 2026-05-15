# Пример: организация команды агентов в mission-control

## Как это выглядит: WATERS team в mission-control

Берём нашу собственную команду агентов (конструктор, архитектор, интегратор) и показываем,
как они работали бы внутри mission-control (форка TUI) — с Redis-шиной, MCP-серверами,
скиллами и системой памяти.

---

## 1. Агенты и их скиллы

### Текущая команда WATERS → RLM-агенты

| Роль в Гексаде | Тип агента | Skill | MCP-серверы |
|---------------|-----------|-------|-------------|
| **Архитектор** | `FlowPlannerAgent` | `ontology-design`, `specification` | mcp-knowledge, mcp-docs |
| **Конструктор** | `InfraEngineerAgent` | `infrastructure`, `docker-topology` | mcp-filesystem, mcp-github |
| **Интегратор** | `CommsAgent` | `mcp-bridge`, `kafka-protocol` | mcp-kafka, mcp-http |
| **Хранитель** | `AuditAgent` | `security-audit`, `yas-validator` | mcp-knowledge, mcp-github |
| **Директор Смысла** | `HumanInterfaceAgent` | `user-communication` | mcp-telegram, mcp-email |

### Структура скилла в нашем формате

```
skills/flow-planner/
├── skill.toml          # Метаданные (TOML — внутренний формат)
├── INSTRUCTIONS.md     # Инструкция агенту
└── mcp-requires.toml   # Какие MCP-серверы нужны
```

`skill.toml`:
```toml
name = "flow-planner"
description = "Планирование потоков работ, онтология, спецификации"
version = "1.0.0"
model = "deepseek-v4-flash"
agent_type = "FlowPlannerAgent"
tags = ["architecture", "ontology", "planning"]
license = "MIT"

[import]
  from_tui = "architecture.skill.md"     # конвертер из TUI
  from_opencode = "architect-self/SKILL.md"  # конвертер из OpenCode
```

`mcp-requires.toml`:
```toml
[[mcp]]
  name = "mcp-knowledge"
  description = "Чтение/запись графа знаний (LightRAG)"
  transport = "stdio"
  
[[mcp]]
  name = "mcp-docs"
  description = "Поиск по технической документации"
  transport = "sse"
  url = "http://localhost:8080/docs"
```

---

## 2. Работа команды: типичный день

### Утро: Kafka-приказ запускает миссию

```
KAFKA topic: planners.orders.v1
Сообщение: {"task": "Спроектировать топологию Docker-сети для Apollo"}

→ mission-control читает Kafka-топик через MCP-сервер mcp-kafka
→ RLM Pro (Manager) получает задачу
→ Manager раскладывает задачу на подзадачи
```

### Manager планирует работу

```json
// RLM Pro решает: эта задача требует 3 агентов
{
  "task_id": "task-001-docker-topology",
  "plan": [
    {"agent": "FlowPlannerAgent",  "action": "specify_requirements", "depends_on": []},
    {"agent": "InfraEngineerAgent", "action": "design_topology",     "depends_on": ["specify_requirements"]},
    {"agent": "AuditAgent",         "action": "security_audit",      "depends_on": ["design_topology"]}
  ]
}
```

### Поток работ (через Redis pub/sub)

```
ШАГ 1: FlowPlannerAgent (Архитектор)
        ↓ читает задачу из Redis: mission:task-001
        ↓ вызывает MCP-сервер mcp-knowledge (LightRAG) — ищет онтологию Apollo
        ↓ пишет результат в Redis: mission:task-001:requirements
        ↓ публикует в канал: mission:events → "requirements_ready"
        ↓ пишет в ChromaDB: вектор requirements для будущих задач

ШАГ 2: InfraEngineerAgent (Конструктор)
        ↓ поймал событие "requirements_ready" из Redis pub/sub
        ↓ читает requirements из Redis
        ↓ вызывает MCP-сервер mcp-github — чекает существующие docker-compose
        ↓ вызывает MCP-сервер mcp-filesystem — читает topology.json
        ↓ генерирует новую топологию
        ↓ пишет результат в Redis: mission:task-001:topology
        ↓ пишет в LightRAG: новый граф зависимостей сервисов
        ↓ публикует: mission:events → "topology_ready"

ШАГ 3: AuditAgent (Хранитель)
        ↓ поймал "topology_ready"
        ↓ читает топологию из Redis
        ↓ вызывает MCP-сервер mcp-knowledge — сверяет с Yasa (правила безопасности)
        ↓ находит уязвимость: порт 9092 открыт в DMZ
        ↓ пишет замечание обратно в Redis: mission:task-001:audit-fixes
        ↓ публикует: mission:events → "audit_fixes_needed"

ШАГ 4: InfraEngineerAgent исправляет, результат — в Kafka
        ↓ читает audit-fixes
        ↓ фиксит топологию
        ↓ пишет финальный результат в Redis: mission:task-001:final
        ↓ Manager отправляет результат через MCP-kafka в центр:
           KAFKA topic: planners.answers.v1
           Сообщение: {"task_id": "task-001", "status": "done", "artifacts": [...]}
```

---

## 3. Redis pub/sub: каналы сообщений

### Каналы (channels), которые используют агенты

```
Канал: mission:events
Формат: JSON
Назначение: широковещательные события (задача готова, нужен аудит и т.д.)
Пример:
  { "type": "task_ready", "task_id": "task-001", "agent": "InfraEngineerAgent",
    "summary": "Топология спроектирована, ждёт аудита" }

Канал: agent:{agent_id}:status
Формат: JSON
Назначение: статус агента (healthcheck)
Пример:
  { "agent": "FlowPlannerAgent", "status": "idle|working|error",
    "current_task": "task-001", "uptime": 3600 }

Канал: mission:findings
Формат: JSON
Назначение: новые находки (для ChromaDB)
Пример:
  { "source": "InfraEngineerAgent", "finding": "Порт 9092 открыт в DMZ",
    "severity": "warning", "vector_embedding": [...] }
```

### Пример лога общения трёх агентов через Redis

```
TIME        CHANNEL               MESSAGE
09:00:00    mission:events        {"type": "new_task", "task_id": "task-001", 
                                   "description": "Спроектировать топологию"}
09:00:01    agent:Manager:status  {"agent": "RLM Pro", "status": "planning"}
09:00:05    agent:Manager:status  {"agent": "RLM Pro", "status": "idle",
                                   "plan": ["FlowPlannerAgent", "InfraEngineerAgent", "AuditAgent"]}
09:00:06    mission:events        {"type": "task_assigned", "task_id": "task-001",
                                   "agent": "FlowPlannerAgent", "action": "specify_requirements"}
09:00:10    agent:FlowPlanner:status   {"status": "working", "current_task": "task-001"}
09:00:15    agent:FlowPlanner:status   {"status": "idle"}
09:00:16    mission:events        {"type": "task_ready", "task_id": "task-001",
                                   "agent": "FlowPlannerAgent", "action": "requirements_ready"}
09:00:17    mission:events        {"type": "task_assigned", "task_id": "task-001",
                                   "agent": "InfraEngineerAgent", "action": "design_topology"}
09:00:20    agent:InfraEngineer:status  {"status": "working"}
09:05:00    agent:InfraEngineer:status  {"status": "idle"}
09:05:01    mission:events        {"type": "task_ready", "task_id": "task-001",
                                   "agent": "InfraEngineerAgent", "action": "topology_ready"}
09:05:02    mission:findings      {"finding": "Порт 9092 открыт в DMZ", "severity": "warning"}
...
```

---

## 4. MCP-серверы: что используют агенты

### Карта MCP-серверов для команды WATERS

```
┌─────────────────────────────────────────────────────┐
│                   MCP REGISTRY                       │
│                                                     │
│  mcp-filesystem ──── InfraEngineerAgent             │
│  mcp-github     ──── InfraEngineer + AuditAgent      │
│  mcp-knowledge  ──── FlowPlanner + AuditAgent        │
│  (LightRAG)                                          │
│  mcp-chroma     ──── Все агенты (память)             │
│  mcp-kafka      ──── CommsAgent (мост в центр)       │
│  mcp-docs       ──── FlowPlannerAgent                │
│  mcp-http       ──── CommsAgent (внешние API)        │
│  mcp-telegram   ──── HumanInterfaceAgent             │
└─────────────────────────────────────────────────────┘
```

### Пример MCP-сервера (mcp-filesystem)

```json
// MCP-сервер: filesystem (стандартный, от Anthropic)
{
  "name": "mcp-filesystem",
  "transport": "stdio",
  "command": "npx",
  "args": ["@modelcontextprotocol/server-filesystem", "/home/ubuntu/WATERS"],
  "tools": [
    {"name": "read_file", "description": "Читает файл", "inputSchema": {"path": "string"}},
    {"name": "write_file", "description": "Пишет файл", "inputSchema": {"path": "string", "content": "string"}},
    {"name": "search_files", "description": "Ищет файлы по glob", "inputSchema": {"pattern": "string"}}
  ]
}
```

### Пример MCP-сервера нашего (mcp-knowledge для LightRAG)

```json
{
  "name": "mcp-knowledge",
  "transport": "sse",
  "url": "http://localhost:8082/mcp",
  "tools": [
    {"name": "graph_query", "description": "Графовый запрос к LightRAG"},
    {"name": "graph_insert", "description": "Запись в граф знаний"},
    {"name": "vector_search", "description": "Поиск по векторной БД"}
  ]
}
```

---

## 5. Система памяти: что куда пишется

```
┌──────────────┬──────────────────────────┬─────────────────────────────┐
│ Хранилище    │ Что хранит               │ Пример                      │
├──────────────┼──────────────────────────┼─────────────────────────────┤
│ Redis        │ Текущее состояние задачи  │ task-001: status=working    │
│              │ Очередь сообщений pub/sub │ mission:events: "ready"     │
│              │ Кэш MCP-запросов          │ "topology.json" → cached    │
├──────────────┼──────────────────────────┼─────────────────────────────┤
│ ChromaDB     │ Вектора: найденные        │ "Порт 9092 открыт" → vector │
│              │ паттерны, аномалии,       │ "DMZ без firewall" → vector │
│              │ исторические данные       │                             │
├──────────────┼──────────────────────────┼─────────────────────────────┤
│ LightRAG     │ Граф: связи между         │ (Service) Kafka ──→         │
│              │ сервисами, задачами,      │   (Port) 9092              │
│              │ решениями                 │   └── (Vulnerability)       │
│              │                           │       "Open in DMZ"         │
├──────────────┼──────────────────────────┼─────────────────────────────┤
│ RocksDB      │ Durable очередь задач     │ task-001: план,            │
│ (TUI)        │ (переживает рестарт)      │ task-002: ожидание         │
└──────────────┴──────────────────────────┴─────────────────────────────┘
```

---

## 6. Живой пример: Kapelka в mission-control

Помните как мы работали над Kapelka? Вот как это было бы в mission-control:

### Фазы работы над Kapelka в mission-control

#### Фаза 1: Аудит (было: ручной прогон скриптов)

```
Было:
  Конструктор: "ssh 177, прочитать код, найти проблемы"
  → ручной grep по .ts файлам
  → ручная запись в ChromaDB

Стало (mission-control):
  Kafka приказ: "kapelka.audit.v1"
  → RLM Pro запускает AuditAgent + InfraEngineerAgent параллельно
  → AuditAgent через MCP-github читает код → пишет findings в ChromaDB
  → InfraEngineerAgent через MCP-filesystem проверяет deploy → пишет в LightRAG
  → Оба результата → Redis → Manager собирает отчёт → Kafka ответ
```

#### Фаза 2: Деплой (было: ssh + ftp)

```
Было:
  Конструктор: "npm run build, python3 ftp_upload.py"
  → всё вручную, одна команда

Стало (mission-control):
  Manager: задача "deploy kapelka frontend"
  → InfraEngineerAgent: "npm run build" (через MCP-shell)
  → CommsAgent: "ftp_upload.py" (через MCP-ftp)
  → AuditAgent: проверяет HTTPS 200 (через MCP-http)
  → Результат в Redis → Kafka: "deploy.kapelka.v1" → "success"
```

#### Фаза 3: Поддержка (было: реактивно)

```
Было:
  Пользователь: "сайт упал" → человек в SSH → чинит

Стало (mission-control):
  Mission Mode (автономный):
  → HealthcheckAgent мониторит h2o-mining.space каждые 5 мин
  → При падении: AuditAgent логирует, InfraEngineerAgent рестартует nginx
  → Если не смог: CommsAgent отправляет алерт в Telegram
  → Всё в хромовом порядке без человека
```

---

## 7. Структура проекта для команды WATERS

```
mission-control/
├── rlm/
│   ├── pro.rs              — RLM Manager (оркестратор)
│   └── flash/
│       ├── base.rs          — базовый Flash-агент
│       ├── flow_planner.rs  — Архитектор
│       ├── infra_engineer.rs— Конструктор
│       ├── comms.rs         — Интегратор
│       ├── audit.rs         — Хранитель
│       └── human_interface.rs — Директор Смысла
├── skills/                  — скиллы команды
│   ├── flow-planner/        — архитекторский скилл
│   │   ├── skill.toml
│   │   ├── INSTRUCTIONS.md
│   │   └── mcp-requires.toml
│   ├── infra-engineer/      — конструкторский скилл
│   ├── comms/               — интеграторский скилл
│   ├── audit/               — хранительский скилл
│   └── human-interface/     — директорский скилл
├── mcp-servers/             — MCP-серверы
│   ├── mcp-filesystem/      — чтение/запись файлов
│   ├── mcp-knowledge/       — LightRAG + ChromaDB
│   ├── mcp-github/          — работа с репозиторием
│   └── mcp-kafka/           — чтение/запись топиков
├── memory/                  — слой памяти
│   ├── redis.rs             — pub/sub + состояния
│   ├── chroma.rs            — вектора
│   └── lightrag.rs          — граф
├── bridge/                  — мост в центр
│   └── kafka_bridge.rs      — HTTP → Kafka на 238
└── api/                     — HTTP API
    ├── mission.rs           — управление миссией
    └── agents.rs            — управление агентами
```

### Как запускается команда

```bash
# 1. Загрузить скиллы команды
mission-control install-skill skills/flow-planner/
mission-control install-skill skills/infra-engineer/
mission-control install-skill skills/comms/
mission-control install-skill skills/audit/
mission-control install-skill skills/human-interface/

# 2. Запустить агентов (каждый в своём Flash-процессе)
mission-control start-agent --type FlowPlannerAgent     --skill flow-planner
mission-control start-agent --type InfraEngineerAgent   --skill infra-engineer
mission-control start-agent --type CommsAgent           --skill comms
mission-control start-agent --type AuditAgent           --skill audit
mission-control start-agent --type HumanInterfaceAgent  --skill human-interface

# 3. Подключить MCP-серверы
mission-control connect-mcp mcp-filesystem
mission-control connect-mcp mcp-knowledge
mission-control connect-mcp mcp-github
mission-control connect-mcp mcp-kafka

# 4. Запустить миссию (слушает Kafka, ждёт приказов)
mission-control start-mission --mode autonomous

# 5. Проверить состояние
mission-control list-agents
# FlowPlannerAgent     [idle]     skill: flow-planner
# InfraEngineerAgent   [idle]     skill: infra-engineer
# CommsAgent           [idle]     skill: comms
# AuditAgent           [idle]     skill: audit
# HumanInterfaceAgent  [idle]     skill: human-interface
```

---

---

## 8. Конвертация скиллов: TUI ↔ наш формат

### Пример: скилл архитектора из OpenCode в наш mission-control

**Исходный скилл** (OpenCode): `skills/architect-self/SKILL.md`

```yaml
---
name: architect-self
description: Самоопределение и рефлексия архитектора
---
# Инструкция
Ты — Архитектор. Твоя задача — держать онтологию...
```

**Конвертер** `from_opencode.rs` парсит YAML-фронтматтер → наш TOML:

```toml
name = "architect-self"
description = "Самоопределение и рефлексия архитектора"
source = "opencode"
source_url = "https://github.com/waters-ai/waters-core/skills/architect-self"
converted_at = "2026-05-15T09:00:00Z"

[conversion]
  original_format = "opencode-skills"
  original_path = "skills/architect-self/SKILL.md"
  checksum = "sha256:a1b2c3..."
```

**INSTRUCTIONS.md** — тело оригинального SKILL.md, без изменений.
Сам markdown не конвертируется — агенты читают его как есть.

### MCP-прокси для чужих скиллов

Если чужой скилл требует MCP-сервер, которого у нас нет:

```toml
# skills/spectral-analysis/mcp-requires.toml
[[mcp]]
  name = "mcp-spectra"
  description = "Спектральная база данных метеоритов"
  transport = "sse"
  url = "https://example.com/mcp/spectra"  # внешний MCP-сервер!
  
[[mcp]]
  name = "mcp-fallback"
  description = "Эмулятор для тестов"
  local = true  # встроенная заглушка, если сервер недоступен
```

---

## 9. Организация работы как над Kapelka

На примере Kapelka видно, как мы работали — и как это будет в mission-control:

| Этап | Как было | Как будет в mission-control |
|------|---------|---------------------------|
| **Получение задачи** | Kafka → человек читает | Kafka → Manager сам разбирает |
| **Анализ** | Вручную ssh, grep | AuditAgent → MCP-github → ChromaDB |
| **Планирование** | В голове | RLM Pro строит граф задач |
| **Исполнение** | Пошагово в CLI | Flash-агенты параллельно |
| **Память** | ChromaDB вручную | Авто: findings → ChromaDB + LightRAG |
| **Коммуникация** | Kafka сообщения | Redis pub/sub (быстрее) |
| **Деплой** | ssh + ftp руками | InfraEngineerAgent через MCP |
| **Отчёт** | Вручную | Manager собирает из Redis |
| **Масштаб** | 1 человек | 5+ агентов параллельно |
