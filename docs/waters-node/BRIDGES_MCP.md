# waters-node v0.3 — Внешние связи: Bridges + MCP + Базы данных

> **Версия:** 1.0
> **Дата:** 2026-05-15

---

## 1. Три слоя интеграции

```
СЛОЙ 1: Built-in Bridges (hardcoded в ядре)
───────────────────────────────────────────────
  duckduckgo   — web search (free, region=all)
  yandex.search — RU search (api_key + user)
  yandex.gpt   — RU AI validation (api_key)
  baidu.search — CN search (api_key)
  telegram     — Telegram bot (token)

СЛОЙ 2: MCP-серверы (внешние процессы, stdio/SSE)
───────────────────────────────────────────────
  mcp-nasa-fireball  — NASA Fireball API
  mcp-spectra        — спектральные базы
  mcp-trajectory     — расчёт траекторий
  mcp-db-postgres    — PostgreSQL через MCP
  mcp-db-duckdb     — DuckDB через MCP
  mcp-db-sqlite     — SQLite через MCP
  mcp-my-custom     — любой свой MCP-сервер

СЛОЙ 3: Personal Resources (нода-владелец)
───────────────────────────────────────────────
  notebooklm  — Google NotebookLM (cookie)
  obsidian    — Obsidian vault (vault_path)
  chromadb    — векторная память (url)
  lightrag    — граф знаний (url)
  git         — git-репозиторий (url/key)
```

---

## 2. Как нода выбирает доступные соединения

### Принцип: нода сама решает, какие бриджи активны

```rust
pub struct Bridge {
    pub name: String,
    pub description: String,
    pub region: String,        // "ru" | "us" | "cn" | "all"
    pub r#type: BridgeType,    // Builtin | Mcp | Personal
    pub config_keys: Vec<String>,  // какие ключи нужны
    pub connected: bool,       // проверено при старте
    pub health: BridgeHealth,  // статус: ok / error / timeout
    pub latency_ms: u64,       // время ответа (для выбора)
}
```

### Алгоритм выбора при старте ноды:

```
1. Для каждого bridge в registry:
   a. Проверить config_keys (есть ли в env/config)
   b. Если есть ключи → попробовать подключиться (healthcheck)
   c. Если оk → connected=true, latency recorded
   d. Если нет → connected=false, visible=false

2. Нода публикует: "у меня активны [duckduckgo, yandex.search, notebooklm]"

3. Группа видит: "нода 2 может искать по RU и US"
```

---

## 3. Базы данных через MCP

Любая база данных оборачивается в MCP-сервер.

### MCP-DB протокол (стандартный JSON-RPC 2.0)

```json
// Запрос: выполнить SQL
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "mcp-db-query",
    "arguments": {
      "query": "SELECT * FROM findings WHERE confidence > 0.8",
      "params": [],
      "limit": 100
    }
  }
}

// Ответ
{
  "jsonrpc": "2.0",
  "result": {
    "content": [{
      "type": "text",
      "text": "[{\"id\": 42, \"type\": \"meteorite\", \"conf\": 0.92}]"
    }],
    "isError": false
  }
}
```

### Как нода подключает базу

```bash
# В config.toml ноды:
[mcp.servers.mcp-db-postgres]
command = "python3"
args = ["mcp-db-postgres/server.py"]
env = { DATABASE_URL = "postgres://..." }

[mcp.servers.mcp-db-duckdb]
command = "python3"
args = ["mcp-db-duckdb/server.py"]
env = { DUCKDB_PATH = "/data/mission.duckdb" }
```

### Как задача выбирает базу

```json
// В task.json:
{
  "id": "task-0001",
  "databases": [
    {"name": "mission-db", "type": "mcp-db-postgres", "url": "postgres://..."},
    {"name": "local-cache", "type": "mcp-db-duckdb", "path": "/data/cache.duckdb"}
  ]
}
```

---

## 4. Personal Resources (личные ресурсы ноды)

```
Каждая нода может иметь ЛИЧНЫЕ ресурсы:
  — NotebookLM (Google, нужна cookie)
  — Obsidian vault (локальный путь)
  — ChromaDB (своя инстанция)
  — LightRAG (своя инстанция)
  — Git-репозиторий (свой)

Эти ресурсы ДОСТУПНЫ ТОЛЬКО когда нода в группе.
Задача может попросить: "Agent-A, используй свой NotebookLM".
```

### Пример: задача использует Obsidian ноды-владельца

```json
{
  "task": "проанализировать метеориты",
  "agent": "Agent-C",
  "owner_node": "node-3",
  "resources": {
    "personal": ["obsidian"],  // Agent-C использует СВОЙ Obsidian
    "shared": ["chromadb"],    // из общих ресурсов группы
    "mcp": ["mcp-nasa"]       // из MCP-серверов
  }
}
```

---

## 5. Регистрация нового bridge/MCP-сервера

```bash
# Через чат (LLM как UX):
"подключи PostgreSQL базу данных"

# → нода создаёт MCP-сервер
# → проверяет connection
# → публикует в группу: "доступна база pg://mission-1"
```

Или вручную через config.toml ноды.

---

## 6. Таблица доступных интеграций (v0.3)

| Интеграция | Тип | Регион | Статус | config_keys |
|-----------|-----|--------|--------|-------------|
| DuckDuckGo | builtin | all | ✅ v0.2.0 | нет (free) |
| Yandex.Search | builtin | ru | ✅ v0.2.0 | api_key, user |
| Yandex.GPT | builtin | ru | ✅ v0.2.0 | api_key |
| Baidu.Search | builtin | cn | ✅ v0.2.0 | api_key |
| Telegram | builtin | all | ✅ v0.2.0 | token |
| NotebookLM | personal | all | ✅ v0.2.0 | cookie |
| Obsidian | personal | all | ✅ v0.2.0 | vault_path |
| ChromaDB | personal | all | ✅ v0.2.0 | url |
| LightRAG | personal | all | ✅ v0.2.0 | url |
| MCP (generic) | mcp | all | ✅ v0.2.0 | command+args |
| MCP-DB PostgreSQL | mcp | all | 🔴 P1 | DATABASE_URL |
| MCP-DB DuckDB | mcp | all | 🔴 P1 | DUCKDB_PATH |
| MCP-DB SQLite | mcp | all | 🔴 P1 | path |
| MCP-NASA Fireball | mcp | all | 🔴 P1 | api_key |
| MCP-Spectra | mcp | all | 🔴 P2 | api_key |
| MCP-Trajectory | mcp | all | 🔴 P2 | api_key |
| Voice (Whisper) | builtin | all | 🔴 P2 | model_path |
