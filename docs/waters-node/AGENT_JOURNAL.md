# waters-node v0.3 — Бортовой журнал и личные агенты в задаче

> **Версия:** 1.0
> **Дата:** 2026-05-15

---

## 1. Бортовой журнал агента (Agent Journal)

### Формат

Каждый агент ведёт свой журнал — JSON-файл с хронологией шагов.

```
.waters/logs/
├── <agent_id>/
│   ├── journal.log         # все шаги агента
│   ├── <task_id>.log       # шаги по конкретной задаче (P1)
│   └── errors.log          # только ошибки
```

### Формат записи

```json
{
  "timestamp": "2026-05-15T12:34:56Z",
  "agent_id": "explorer.1",
  "task_id": "task-0001",
  "event": "tool_call",
  "detail": {
    "tool": "web_search",
    "query": "meteorite fireball 2025",
    "result_count": 12,
    "duration_ms": 1450
  },
  "level": "info"
}
```

### Типы событий

| event | Когда | level |
|-------|-------|-------|
| `task_assigned` | Агенту назначена задача | info |
| `task_started` | Агент начал выполнение | info |
| `tool_call` | Агент вызвал инструмент | debug |
| `llm_call` | Агент обратился к LLM | debug |
| `finding` | Агент нашёл finding | info |
| `resource_request` | Агент запросил ресурс группы | info |
| `resource_granted` | Группа выдала ресурс | info |
| `resource_denied` | Группа отказала | warn |
| `error` | Ошибка выполнения | error |
| `task_completed` | Задача выполнена | info |
| `task_failed` | Задача провалена | error |

### Чат-команды

```
"журнал explorer.1"           → journal explorer.1
"последние 20 записей"        → /journal --tail 20
"ошибки за сегодня"           → /journal --level error --since today
"журнал по задаче task-0001"  → /journal --task task-0001
```

---

## 2. Личные агенты в общей задаче

### Концепция

```
Задача — общая (принадлежит группе).
Исполнители — личные агенты (принадлежат конкретным нодам).

Каждый агент вносит в задачу:
  — свои личные ресурсы (NotebookLM, Obsidian, ChromaDB)
  — свои бриджи (Yandex, Baidu, DuckDuckGo)
  — свой LLM (нода-владелец)
  — свой журнал
```

### Пример

```
Группа: "meteorite-hunt"
Задача: task-0001 "Найти метеориты за 24ч"

Исполнители:
┌─────────────────────────────────────────────────────────────┐
│ Agent-A (explorer.1)    │ нода 1 │ DeepSeek Pro            │
│ Личные ресурсы:         │ notebooklm, chromadb              │
│ Бриджи:                │ duckduckgo                        │
│ Журнал:                │ .waters/logs/explorer.1/task-0001 │
├─────────────────────────────────────────────────────────────┤
│ Agent-B (scout-us.2)    │ нода 2 │ Ollama qwen2.5:14b      │
│ Личные ресурсы:         │ obsidian                          │
│ Бриджи:                │ duckduckgo, bing                  │
│ Журнал:                │ .waters/logs/scout-us.2/task-0001 │
├─────────────────────────────────────────────────────────────┤
│ Agent-C (scout-ru.3)    │ нода 3 │ Ollama llama3:8b        │
│ Личные ресурсы:         │ yandex.gpt                        │
│ Бриджи:                │ yandex.search, yandex.gpt         │
│ Журнал:                │ .waters/logs/scout-ru.3/task-0001 │
├─────────────────────────────────────────────────────────────┤
│ Agent-D (scout-cn.4)    │ нода 4 │ DeepSeek Flash          │
│ Личные ресурсы:         │ lightrag                          │
│ Бриджи:                │ baidu.search                      │
│ Журнал:                │ .waters/logs/scout-cn.4/task-0001 │
└─────────────────────────────────────────────────────────────┘

Координатор: Agent-Coord (нода 1, DeepSeek Pro)
Ресурсы: группы → ChromaDB (общая), PostgreSQL (общая)
Хранилище: git@github.com:waters/mission-1-findings.git
```

### Как это реализовано (task.rs, расширение)

```rust
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub group: String,
    pub created_by: String,      // кто создал
    pub status: TaskStatus,

    // Исполнители
    pub executors: Vec<TaskExecutor>,

    // Ресурсы задачи
    pub resources: TaskResources,

    // Хранилище
    pub storage: Option<TaskStorage>,

    // Журнал
    pub journal_path: PathBuf,
}

pub struct TaskExecutor {
    pub agent_id: String,        // "explorer.1"
    pub node_id: String,         // "node-1"
    pub role_in_task: String,    // "explorer" | "scout-us" | "analyst"
    pub personal_resources: Vec<String>,  // ["notebooklm", "obsidian"]
    pub assigned_by: String,     // кто назначил
    pub status: ExecutorStatus,  // pending | working | done | failed
}

pub struct TaskResources {
    pub shared: Vec<String>,     // из группы: chromadb, postgres
    pub mcp: Vec<String>,        // mcp-nasa, mcp-db
    pub bridges: Vec<String>,    // duckduckgo, yandex
}

pub struct TaskStorage {
    pub r#type: StorageType,    // Git | Obsidian | DuckDB | Custom
    pub uri: String,            // git@github.com:... | /vault/path
    pub credentials: Option<String>,
}
```

---

## 3. Жизненный цикл задачи с личными агентами

```
1. СОЗДАНИЕ:
   Чат: "задача: найти метеориты за 24ч, исполнители: Explorer-A, Scout-US-B"
   → Engine создаёт Task { executors: [Agent-A, Agent-B] }

2. НАЗНАЧЕНИЕ:
   → Каждому агенту: task_assigned event
   → Каждый агент: проверяет доступность своих ресурсов
   → Если ресурсов нет → отказ + причина в журнал

3. ИСПОЛНЕНИЕ:
   → Каждый агент: работает со своими инструментами
   → Координатор: синтезирует результаты
   → Группа: может перераспределить ресурсы (режим Охота)

4. ЗАВЕРШЕНИЕ:
   → Все агенты: task_completed
   → Координатор: публикует finding в группу
   → Хранилище: результат сохраняется

5. ЖУРНАЛИРОВАНИЕ:
   → Каждый шаг каждого агента → journal.log
   → Можно прочитать журнал задачи: "журнал task-0001"
   → Можно прочитать журнал агента: "журнал explorer.1"
```

---

## 4. Чат-команды для работы с агентами и задачами

```bash
# Управление агентами
"создай агента Explorer-A со скиллом explorer на ноде 1"
"мои агенты"                           → /agent list mine
"агенты группы"                        → /agent list group
"поделись агентом Scout-US с группой"  → /agent share scout-us

# Управление задачами
"создай задачу: поиск метеоритов"      → /task create
"назначь Explorer-A и Scout-B"         → /task assign
"задаче task-0001 дай DuckDB"          → /task resource add duckdb
"хранилище: git@github.com:waters/db"  → /task storage set git
"статус задачи"                        → /task status
"отчёт"                                → /task report

# Ресурсы
"какие бриджи у ноды 2?"               → /bridges node-2
"подключи PostgreSQL"                   → /bridge add mcp-db-postgres
"дай Scout-US доступ к NotebookLM"     → /task resource boost scout-us notebooklm

# Журналы
"журнал Explorer-A"                    → /journal explorer-a
"журнал task-0001"                     → /journal --task task-0001
"ошибки scout-b"                       → /journal scout-b --level error
```
