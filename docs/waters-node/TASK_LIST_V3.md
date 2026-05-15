# waters-node v0.3 — Поэтапное задание Конструктору

> **От:** Архитектор `agent.architect.v1`
> **Кому:** Конструктор Сети `agent.constructor.v1`
> **Дата:** 2026-05-15
> **Приоритет:** P0 → P1 → P2

---

## Общая цель

Превратить waters-node v0.2.0 (20 модулей, 3400 строк, P2P-сеть) в
**полноценную распределённую агентную систему** с:

- Streaming LLM (не блокирующий вызов)
- Crash recovery (WAL + checkpoint + offline queue)
- MCP bridges config (персистентный JSON-файл)
- Групповыми ресурсами (группа усиливает качественное направление)
- Чат-аппрув подключений (входящие запросы через чат)
- Личными агентами в общей задаче (каждый со своими ресурсами)

**Архитектурные документы:**
- `docs/waters-node/MASTER_SPEC_V3.md` — всё вместе
- `docs/waters-node/ARCHITECTURE_V3.md` — архитектура
- `docs/waters-node/GROUP_MODES.md` — режимы групп
- `docs/waters-node/SKILL_FORMAT.md` — скиллы
- `docs/waters-node/BRIDGES_MCP.md` — бриджи и базы
- `docs/waters-node/AGENT_JOURNAL.md` — журналы и личные агенты

---

## Фаза 1: Streaming + Crash Recovery (P0, ~2 дня)

### Задача 1.1: Streaming LLM

**Файл:** `llm.rs`

**Что сейчас:** Блокирующий `POST /chat/completions`, ждёт весь ответ.
**Что нужно:** SSE-стриминг с парсингом токенов.

```rust
// API:
pub async fn chat_stream(
    &self,
    messages: &[Message],
) -> Result<impl Stream<Item = LlmEvent>>;

pub enum LlmEvent {
    Token(String),           // обычный токен
    ReasoningToken(String),  // thinking блок DeepSeek
    ToolCall { name: String, args: Value },
    Done { usage: Usage },
    Error(String),
}
```

**Критерий приёмки:**
- `deepseek "напиши рассказ"` → токены появляются по одному
- DeepSeek reasoning_content парсится отдельно
- Можно отменить Ctrl+C (graceful shutdown)
- Ollama тоже стримит

### Задача 1.2: Crash Recovery

**Файлы:** `session.rs`, `channel.rs`

**Что сейчас:** Сессии сохраняются `.waters/sessions/*.json`, но нет checkpoint-ов.

**Что нужно:**
```rust
// 1. Checkpoint перед каждым шагом
pub fn save_checkpoint(state: &EngineState) -> Result<()>;
// → .waters/checkpoints/latest.json

// 2. Восстановление после падения
pub fn resume_from_checkpoint() -> Option<EngineState>;

// 3. Офлайн-очередь (для L2-L3)
pub fn enqueue_offline(event: &JournalEntry) -> Result<()>;
// → .waters/offline/queue.jsonl

pub fn flush_offline_queue() -> Result<Vec<JournalEntry>>;
// → отправка в gossip при L0
```

**Критерий приёмки:**
- `kill -9` ноды → при перезапуске всё восстанавливается
- Офлайн-события не теряются
- После flush очередь пуста

---

## Фаза 2: Bridges Config + Group Resources (P0, ~2 дня)

### Задача 2.1: MCP Bridges Config

**Файл:** `bridge.rs`

**Что сейчас:** Бриджи hardcoded в `bridge.rs`.
**Что нужно:** JSON-файл `.waters/bridges.json`:

```json
{
  "bridges": [
    {
      "name": "mcp-nasa-fireball",
      "type": "mcp",
      "transport": "stdio",
      "command": "python3",
      "args": ["mcp-servers/mcp-nasa-fireball/server.py"],
      "env": {"NASA_API_KEY": "xxx"},
      "healthcheck": true,
      "auto_connect": true
    },
    {
      "name": "notebooklm",
      "type": "personal",
      "config_keys": ["cookie"],
      "region": "all"
    }
  ],
  "mcp_servers": [
    {
      "name": "mcp-db-postgres",
      "command": "python3",
      "args": ["mcp-db-postgres/server.py"],
      "env": {"DATABASE_URL": "postgres://..."}
    }
  ]
}
```

**Критерий приёмки:**
- `/bridges` показывает ✅/⬜ статус
- `bridges.json` перечитывается при старте
- Нода публикует свои активные бриджи в gossip

### Задача 2.2: Group Resource Sharing

**Файл:** `group.rs`

**Что сейчас:** Группы есть, shared ресурсы объявлены, но не используются.
**Что нужно:**

```rust
pub struct GroupResources {
    pub llm_priority: HashMap<NodeId, u8>,    // чей LLM приоритетнее
    pub active_bridges: Vec<BridgeId>,         // какие бриджи активны
    pub resource_boost: Vec<ResourceBoost>,    // какие направления усилены
}

pub fn boost_direction(&mut self, node_id: &NodeId, bridge: &BridgeId, reason: &str);
// → увеличивает приоритет ноды/бриджа
// → публикует в gossip: "группа усилила scout-us"

pub fn evaluate_quality(&self, findings: &[Finding]) -> HashMap<DirectionId, f64>;
// → оценивает: какое направление даёт conf > 0.8
```

**Критерий приёмки:**
- Группа может сказать "усилить US-направление" → scout-us получает DeepSeek Pro
- Результаты оценок публикуются в групповой канал
- Любая нода в группе видит текущие бусты

---

## Фаза 3: Chat Approval + Task Binding (P1, ~1 день)

### Задача 3.1: Chat Approval для входящих подключений

**Файлы:** `convo.rs`, `gossip.rs`

**Что сейчас:** Gossip принимает подключения по токену автоматически.
**Что нужно:** Когда нода X стучится в группу — владельцу ноды приходит в чат:

```
🔔 Нода 192.168.1.5 (node-5) запрашивает доступ к группе "meteorite-hunt"
   Токен: ******** (совпадает)
   
   > Принять? (да/нет)
   > Если да: назначить роль (explorer / collector / observer)
```

**Критерий приёмки:**
- Входящее подключение → сообщение в чат
- Ответ "да" → handshake завершается
- Ответ "нет" → access_denied
- Можно настроить auto-approve для доверенных IP

### Задача 3.2: Per-Task Resource Binding

**Файл:** `task.rs`

**Что сейчас:** У задачи есть поля, но нет привязки ресурсов.
**Что нужно:**

```rust
pub fn bind_resource(&mut self, task_id: &str, resource: ResourceBinding);
// → "задаче task-0001 даём DuckDB + NotebookLM"

pub struct ResourceBinding {
    pub bridge: String,        // "notebooklm"
    pub source: String,        // "personal" | "shared" | "mcp"
    pub owner_node: Option<String>, // чей ресурс (для personal)
    pub purpose: String,       // "анализ спектров"
    pub boost: bool,           // усиление?
}
```

**Критерий приёмки:**
- "задаче task-0001 дай DuckDB и NotebookLM" → ресурсы привязаны
- Агент задачи видит только свои ресурсы
- Ресурсы освобождаются при завершении задачи

---

## Фаза 4: Personal Agents + Journal (P1, ~2 дня)

### Задача 4.1: Личные агенты в общей задаче

**Файлы:** `agent.rs`, `task.rs`

**Что сейчас:** Агенты есть, но личные/общие не влияют на исполнение.
**Что нужно:**

```rust
pub struct Agent {
    pub name: String,
    pub role: String,
    pub agent_type: String,     // "personal" | "shared"
    pub owner_node: String,
    pub personal_resources: Vec<String>,  // ["notebooklm", "obsidian"]
    pub active_skill: Option<String>,
    pub status: AgentStatus,
}
```

При создании задачи:
```rust
// Чат: "назначаю Explorer-A (нода 1) и Scout-B (нода 2)"
// → task.executors.push(TaskExecutor {
//     agent_id: "explorer-a",
//     node_id: "node-1",
//     personal_resources: agent.personal_resources,
//     ...
// })
```

**Критерий приёмки:**
- Agent-A (нода 1) использует NotebookLM ноды 1
- Agent-B (нода 2) использует Obsidian ноды 2
- Ресурсы не пересекаются

### Задача 4.2: Agent Journal Extension

**Файл:** `journal.rs`

**Что сейчас:** Один файл `.waters/logs/<agent_id>.log` на агента.
**Что нужно:**

```rust
// Per-task journal
pub fn log_task(agent_id: &str, task_id: &str, event: &JournalEntry);
// → .waters/logs/<agent_id>/<task_id>.log

// Read with filters
pub fn read_task_log(agent_id: &str, task_id: &str, level: Option<&str>) -> Vec<JournalEntry>;
pub fn read_errors(agent_id: &str, since: &DateTime<Utc>) -> Vec<JournalEntry>;
```

**Критерий приёмки:**
- "журнал explorer-a по задаче task-0001" → показывает только записи этой задачи
- "ошибки scout-b за сегодня" → только error level

---

## Фаза 5: Event Stream + Военный режим (P2, ~2 дня)

### Задача 5.1: Event Stream через gossip

**Файлы:** `api.rs`, `gossip.rs`

**Что нужно:**
- SSE-эндпоинт `GET /api/v1/events?since_seq=N`
- gossip синхронизирует события между нодами
- Клиент (Web UI) подписывается на SSE

### Задача 5.2: Kafka Military Mode

**Файл:** `kafka.rs`

**Что нужно:**
- feature-gate `kafka-transport`
- При включении: все ноды подключаются к Kafka
- Топики: `mission.1.orders.v1`, `mission.1.findings.v1`
- Централизованное управление роем

---

## Порядок выполнения

```
День 1-2:  Задача 1.1 (Streaming) + Задача 1.2 (Crash Recovery)
День 3-4:  Задача 2.1 (Bridges Config) + Задача 2.2 (Group Resources)
День 5:    Задача 3.1 (Chat Approval) + Задача 3.2 (Task Binding)
День 6-7:  Задача 4.1 (Personal Agents) + Задача 4.2 (Journal)
День 8-9:  Задача 5.1 (Event Stream) + Задача 5.2 (Kafka)
```

Каждый день:
1. Читать архитектурные документы (`docs/waters-node/`)
2. Писать код
3. Тестировать (ручные тесты, модульные где возможно)
4. Писать в бортовой журнал

---

## Критерии готовности v0.3

- [ ] Streaming LLM: токены приходят по одному, отмена работает
- [ ] Crash recovery: kill -9 не теряет данные
- [ ] Bridges config: JSON-файл, статус ✅/⬜ в `/bridges`
- [ ] Group resources: группа усиливает качественное направление
- [ ] Chat approval: входящие подключения через чат
- [ ] Per-task resources: у каждой задачи свои бриджи и базы
- [ ] Personal agents: агент использует личные ресурсы ноды
- [ ] Agent journal: per-task, фильтры по level
- [ ] Event stream: SSE через gossip
- [ ] Kafka military mode: feature-gate работает
