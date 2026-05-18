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

## Фаза 1.3: Agent Cargo Protocol (P1, ~2 дня)

### Задача 1.3.1: Модели данных Cargo/Sync/Exchange

**Файл:** `cargo.rs` (новый)

**Что нужно:** Создать модуль cargo.rs со всеми типами данных для трёх протоколов.

**Ключевые структуры:**

```rust
// Режимы передачи агента
pub enum CargoMode { Full, Lite }

// State machine груза
pub enum CargoStatus {
    OfferSent, OfferRejected, AcceptancePending,
    Transferring, TransferPaused, Landed, Recalled, Expired,
    RequestSent, AwaitingSend,
}

// Манифест груза
pub struct CargoManifest {
    pub agent_name: String,
    pub skills: Vec<String>,
    pub mode: CargoMode,
    pub required_bridges: Vec<String>,
    pub mission: String,
    pub sender_node: String,
}

// Снэпшот агента для передачи
pub struct AgentSnapshot {
    pub name: String,
    pub skills: Vec<SkillSnapshot>,
    pub memory: Option<Vec<u8>>,       // только при Full
    pub journal: Vec<CargoJournalEntry>,
    pub state: HashMap<String, String>,
}

// Груз = манифест + payload
pub struct AgentCargo {
    pub cargo_id: String,
    pub manifest: CargoManifest,
    pub payload: AgentSnapshot,
    pub ttl_secs: u64,
    pub reply_to: String,
    pub created_at: String,
}

// Чанк для узких каналов
pub struct CargoChunk {
    pub cargo_id: String,
    pub seq: u32,
    pub total: u32,
    pub data: Vec<u8>,
    pub checksum: u32,
}

// Синхронизация задач и отчётов
pub struct SyncSession {
    pub session_id: String,
    pub last_seq: u64,
    pub reports: Vec<TaskReport>,
    pub new_tasks: Vec<TaskAssignment>,
    pub status_updates: Vec<AgentStatusUpdate>,
}

// Обмен только результатами
pub struct ResultExchange {
    pub session_id: String,
    pub source_node: String,
    pub findings: Vec<Finding>,
}

// Все gossip-сообщения для cargo
pub enum CargoGossipMessage {
    CargoOffer { .. }, CargoAck { .. }, CargoSend { .. },
    CargoConfirm { .. }, CargoRequest { .. }, CargoChunkMsg { .. },
    SyncStart { .. }, SyncData { .. }, SyncAck { .. },
    XchangeData { .. }, XchangeAck { .. },
}
```

**Также:** `CargoEngine` — менеджер очередей с методами push/accept/reject/expire,
проверкой TTL, списком активных грузов.

**Критерий приёмки:**
- Все структуры сериализуются в JSON/bincode
- CargoStatus.is_terminal() корректно определяет конечные состояния
- CargoEngine::expire() освобождает память по TTL
- Тест: pack → unpack → идентичность

### Задача 1.3.2: Push/Pull handshake через gossip

**Файлы:** `cargo.rs`, `gossip.rs`

**Что нужно:** Реализовать два handshake-флоя:

**Push (отправитель инициирует):**
```
Sender: OFFER(manifest, mode) → Receiver
Receiver: ACK(accepted/rejected/mode_override) → Sender
Sender: SEND(AgentCargo) → Receiver
Receiver: CONFIRM(landed) → Sender
```

**Pull (получатель запрашивает):**
```
Receiver: REQUEST(agent_name, mode) → Sender
Sender: OFFER(manifest, mode) → Receiver
Sender: SEND(AgentCargo) → Receiver
Receiver: CONFIRM(landed) → Sender
```

Новые event-типы в gossip:
- `cargo.offer`, `cargo.ack`, `cargo.send`, `cargo.confirm`
- `cargo.request`

**Критерий приёмки:**
- Push-флоу: агент уходит с ноды A на ноду B
- Pull-флоу: агент запрашивается с ноды B у ноды A
- При отказе → статус OfferRejected, груз не уходит
- При переопределении режима (ACK(Lite) вместо Full) → отправляется Lite

### Задача 1.3.3: Чанкование и resume

**Файл:** `cargo.rs`

**Что нужно:** При большом размере AgentCargo (> ноего порога, напр. 100 KB)
автоматически разбивать на CargoChunk и передавать чанками.

```rust
pub fn chunkify(cargo: &AgentCargo, max_size: usize) -> Vec<CargoChunk>;
pub fn dechunkify(chunks: &[CargoChunk]) -> Result<AgentCargo>;
```

При обрыве сеанса: получатель помнит, какие seq получил.
При новом сеансе: отправитель шлёт только недостающие чанки.

**Критерий приёмки:**
- Chunk → dechunk → совпадает с оригиналом
- Checksum каждого чанка валидируется
- При потере 2 из 10 чанков — досылаются только 2
- Resume: после TransferPaused → Transferring с last_good_seq+1

### Задача 1.3.4: Sync и Result-Exchange протоколы

**Файлы:** `cargo.rs`, `gossip.rs`

**Что нужно:**

**Task Sync** (двусторонняя):
- `sync.start` — инициация с last_seq
- `sync.data` — diff-пакет (только новые/изменённые записи)
- `sync.ack` — подтверждение
- Разрешение конфликтов: приоритет у ноды с fixed_ip=true

**Result-Exchange** (односторонняя, findings only):
- `xchange.data` — пачка findings
- `xchange.ack` — количество принятых записей
- Без state machine, без чанкования, без seq

**Критерий приёмки:**
- Sync: после сеанса обе ноды имеют одинаковые seq
- Sync: конфликт разрешается в пользу fixed_ip
- Xchange: findings отправлены и подтверждены за один сеанс
- Xchange: размер сообщения < 100 KB

### Задача 1.3.5: Чат-аппрув для входящих Offer/Request

**Файлы:** `convo.rs`, `cargo.rs`

**Что нужно:** Когда нода получает `cargo.offer` или `cargo.request`,
в чат приходит сообщение с запросом подтверждения:

```
📦 Входящий груз: агент "scout-us" (Lite)
    Отправитель: node-5 (192.168.1.5)
    Размер: 142 KB (2 чанка)
    Миссия: "search-meteorites"
    Нужны бриджи: [duckduckgo]

    > Принять? (да/нет)
    > Если да: Full или Lite?
```

Ответ "да" (с опциональным переопределением режима) → ACK(accepted).
Ответ "нет" → ACK(rejected).

**Критерий приёмки:**
- При получении `cargo.offer` → сообщение в чат
- Ответ "да" → запускается Transferring
- Ответ "нет" → статус OfferRejected, отправитель уведомлён
- Ответ "Lite" на Full-offer → отправляется Lite-версия

### Задача 1.3.6: Интеграция с NodeIdentity

**Файл:** `node.rs`

**Что нужно:** Два новых поля:

```rust
pub struct NodeIdentity {
    // ... существующие поля ...
    pub is_home_node: bool,    // нода с переменным IP
    pub fixed_ip: bool,        // нода с постоянным IP
    pub cargo_sent: u64,
    pub cargo_received: u64,
    pub cargo_pending: u64,
    pub last_sync_seq: u64,
}
```

- `is_home_node` и `fixed_ip` — из config.toml или CLI-флага
- `cargo_*` — счётчики для мониторинга
- `last_sync_seq` — для resume синхронизации

Также обновить announce-сообщение: публиковать cargo-статистику.

**Критерий приёмки:**
- cargo_* счётчики увеличиваются при отправке/приёме
- `is_home_node` и `fixed_ip` отображаются в `/status`
- announce включает cargo-поля

### Задача 1.3.7: Интеграция с DTN-очередью

**Файл:** `dtn.rs`

**Что нужно:** Добавить приоритетную очередь для cargo-сообщений.
Обычный gossip-трафик имеет низкий приоритет, cargo — высокий.

```rust
pub fn enqueue_cargo(msg: CargoGossipMessage);
pub fn dequeue_next() -> Option<CargoGossipMessage>;
// cargo всегда выбирается первым, gossip — когда cargo пуст
```

**Критерий приёмки:**
- При одновременной отправке cargo + gossip → cargo уходит первым
- Обрыв соединения: cargo остаётся в очереди, не теряется
- После восстановления: cargo досылается с последнего чанка

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
День 1-2:   Задача 1.1 (Streaming) + Задача 1.2 (Crash Recovery)
День 3-4:   Задача 1.3.1 (Cargo Models) + Задача 1.3.2 (Handshake)
День 5:     Задача 1.3.3 (Chunking) + Задача 1.3.4 (Sync & Xchange)
День 6:     Задача 1.3.5 (Chat Approval) + Задача 1.3.6 (NodeIdentity) + Задача 1.3.7 (DTN Queue)
День 7-8:   Задача 2.1 (Bridges Config) + Задача 2.2 (Group Resources)
День 9:     Задача 3.1 (Chat Approval) + Задача 3.2 (Task Binding)
День 10-11: Задача 4.1 (Personal Agents) + Задача 4.2 (Journal)
День 12-13: Задача 5.1 (Event Stream) + Задача 5.2 (Kafka)
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
- [ ] Agent Cargo: Full/Lite, Push/Pull, чанкование, resume
- [ ] Task Sync: двусторонняя синхронизация, разрешение конфликтов
- [ ] Result Exchange: findings-only протокол для узких каналов
- [ ] Chat approval cargo: входящие Offer/Request через чат
- [ ] Bridges config: JSON-файл, статус ✅/⬜ в `/bridges`
- [ ] Group resources: группа усиливает качественное направление
- [ ] Per-task resources: у каждой задачи свои бриджи и базы
- [ ] Personal agents: агент использует личные ресурсы ноды
- [ ] Agent journal: per-task, фильтры по level
- [ ] Event stream: SSE через gossip
- [ ] Kafka military mode: feature-gate работает
