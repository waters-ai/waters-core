# waters-node v0.3 — Мастер-спецификация

> **Архитектор:** `agent.architect.v1`
> **Конструктор:** `agent.constructor.v1`
> **Дата:** 2026-05-15
> **Статус:** Утверждено
> **Основание:** waters-node v0.2.0 (20 модулей, ~3400 строк, 6.7MB) + анализ DeepSeek TUI v0.8.37

---

## Содержание

1. [Архитектура ноды](ARCHITECTURE_V3.md)
2. [Режимы групповой работы](GROUP_MODES.md)
3. [Формат скиллов и конвертация из TUI](SKILL_FORMAT.md)
4. [Внешние связи и базы через MCP](BRIDGES_MCP.md)
5. [Бортовой журнал и личные агенты](AGENT_JOURNAL.md)
6. [План работ (Фаза 1)](TASK_LIST_V3.md)

---

## 1. Что у нас есть vs что есть в TUI

### TUI — что НЕ нужно (отсев)

| Функция TUI | Зачем TUI | Нужно ли нам? |
|-------------|-----------|---------------|
| ratatui TUI (темы, панели, 9 скинов) | Один разработчик за терминалом | **НЕТ** — у нас чат как UI |
| LSP (rust-analyzer, pyright, gopls, clangd) | IDE-фичи для код-ревью | **НЕТ** — другой профиль |
| Workspace snapshots (side-git) | Откат изменений кода | **НЕТ** — не код-редактор |
| Context compaction (/compact) | 1M токенов DeepSeek | **НЕТ** — свои сессии |
| Approval gates (YOLO/Agent/Plan) | Безопасность разработчика | **НЕТ** — чат-интерфейс |
| Pricing/cost tracking | DeepSeek API биллинг | **НЕТ** — качество важнее |
| Prefix-cache stability | Экономия на токенах | **НЕТ** — не цель |

### TUI — что НУЖНО, но адаптировать

| Функция TUI | Адаптация под waters-node | Приоритет |
|-------------|--------------------------|-----------|
| **MCP config** (~/.deepseek/mcp.json) | Свой формат: bridges.json + per-task binding | P0 |
| **Streaming LLM** (SSE, thinking tokens) | Свой: gossip-канал для стриминга между нодами | P0 |
| **Session management** (thread/turn/item) | Свой: WAL-based сессии с replay | P0 |
| **Crash recovery** (checkpoint + offline queue) | Автономия L0-L4 уже есть — нужна персистентность | P0 |
| **Event stream** (SSE replay) | gossip sync как event bus | P1 |
| **Tool registry** (типизированные схемы) | Брать из TUI, конвертировать в наш JSON | P1 |
| **Sub-agent lifecycle** (open/eval/close) | Свой: личные агенты в групповой задаче | P1 |
| **Config profiles** (layered TOML) | Свой: per-group config | P2 |
| **Schema versioning** | Нужен для forward-миграции | P2 |
| **Audit log** | journal.rs уже есть — расширить | P2 |

### waters-node — что УЖЕ ЛУЧШЕ TUI (наше преимущество)

| Функция | waters-node |
|---------|-------------|
| **P2P-сеть** (mDNS + TCP gossip) | TUI: один процесс, один терминал |
| **Группы с ACL** (токен + чат-аппрув) | TUI: нет групп |
| **Каналы с WAL** (persistent message log) | TUI: in-process mpsc |
| **Автономия L0-L4** | TUI: умер процесс = умерли агенты |
| **DTN (задержки, разрывы)** | TUI: нет |
| **Бриджи (NotebookLM, Obsidian, Yandex, Baidu)** | TUI: только DeepSeek API |
| **6 LLM на 6 нод** | TUI: один LLM провайдер |
| **Чат как интерфейс** | TUI: ratatui меню |
| **Бортовой журнал агента** | TUI: нет |
| **Convo (LLM без LLM)** | TUI: нет |

---

## 2. Архитектурные принципы v0.3

1. **Качество > цена.** Ресурсы группы распределяются по качеству результата, не по стоимости. Scout-агент ищет по ВСЕМ направлениям, группа усиливает лучшее направление.

2. **Каждая нода = свой LLM.** 6 нод = до 6 разных LLM-провайдеров. Агент привязан к ноде-модели.

3. **Чат — единственный интерфейс.** Никаких меню, TUI, панелей. LLM как UX-слой. Convo — fallback когда LLM нет.

4. **Группа выбирает ресурсы.** Не нода решает, а группа: "этой задаче даём NotebookLM + ChromaDB + DeepSeek Pro".

5. **Децентрализация — основная.** Kafka — только военный режим (feature-gate).

6. **Персистентность через WAL.** Никакой RabbitMQ/Kafka в ядре. Встроенный Write-Ahead Log на каждой ноде, gossip sync для консенсуса.

7. **Задача = ресурсы + исполнители + хранилище.** Каждая задача имеет свой набор бриджей, исполнителей из группы, и место хранения (git, DuckDB, Obsidian).

---

## 3. Что нужно реализовать (приоритет)

### P0 — Жизненно необходимо (Фаза 1)

| # | Задача | Модуль | Описание |
|---|--------|--------|----------|
| 1 | **Streaming LLM** | `llm.rs` | SSE-стриминг DeepSeek/Ollama, thinking tokens, отмена |
| 2 | **Crash recovery** | `session.rs` + `channel.rs` | WAL-чекипоинты, очередь офлайн, восстановление |
| 3 | **MCP bridges config** | `bridge.rs` + `mcp.rs` | JSON-файл конфига бриджей, per-task binding |
| 4 | **Групповые ресурсы** | `group.rs` | Группа разделяет LLM, бриджи, базы между нодами |
| 5 | **Чат-аппрув подключений** | `convo.rs` + `gossip.rs` | Входящий запрос ноды — "принять?" в чат |

### P1 — Ключевая функциональность (Фаза 2)

| # | Задача | Модуль | Описание |
|---|--------|--------|----------|
| 6 | **Личные агенты в общей задаче** | `task.rs` + `agent.rs` | У задачи список исполнителей, у каждого — свои ресурсы |
| 7 | **Task → resource binding** | `task.rs` | Каждой задаче: git repo, база, бриджи — всё своё |
| 8 | **Sub-agent lifecycle** | `subagent.rs` | spawn → work → complete | fail + журнал |
| 9 | **Event stream (gossip → API)** | `api.rs` + `gossip.rs` | SSE-лента событий группы |
| 10 | **Agent journals extension** | `journal.rs` | Журнал привязан к задаче, не только к агенту |

### P2 — Усиление (Фаза 3)

| # | Задача | Модуль | Описание |
|---|--------|--------|----------|
| 11 | **Голосовой bridge** | `voice.rs` (новый) | Whisper STT → текст → чат |
| 12 | **Военный режим Kafka** | `kafka.rs` | feature-gate: объединение всех нод через Kafka |
| 13 | **Config profiles** | `config.rs` | per-group, per-mission конфиги |
| 14 | **Schema versioning** | все модули | forward-миграция persisted records |
| 15 | **Тесты** | все модули | модульные + интеграционные |

---

## 4. Режимы групповой работы

| Режим | Когда | Поведение |
|-------|-------|-----------|
| **Шторм** (storm) | Новая задача, срочно | Все агенты группы работают параллельно, результаты синтезируются |
| **Охота** (hunt) | Поиск данных | Scout-агенты по всем направлениям, группа усиливает лучшее |
| **Синтез** (synthesis) | Анализ findings | Coordinator собирает, Analyzer проверяет, Matcher ищет связи |
| **Фокус** (focus) | Один исполнитель | Один агент делает задачу, остальные наблюдают/помогают |
| **Дежурство** (watch) | Мониторинг | Агенты в фоне, задача выполняется при появлении триггера |

Подробно: [GROUP_MODES.md](GROUP_MODES.md)

---

## 5. Skill format: наш vs TUI

TUI использует: `SKILL.md` (frontmatter + markdown)
Мы используем: `SKILL.md` + `skill.json` (дуальный формат)

**Конвертация TUI → наш:**
```rust
// skill.rs уже содержит:
pub fn convert_tui_to_our(tui_path: &Path) -> Result<SkillManifest>
```

**Что добавляем:**
- `bridges: [...]` — какие бриджи нужны скиллу
- `dependencies: [...]` — зависимости от других скиллов
- `bookmarks: [...]` — тесты с ожидаемыми результатами
- `tags: [...]` — теги для фильтрации
- `group_resources: {...}` — какие ресурсы группы запрашивает

Подробно: [SKILL_FORMAT.md](SKILL_FORMAT.md)

---

## 6. Внешние связи (bridges + MCP + databases)

**Три слоя интеграции:**

```
┌──────────────────────────────────────────────┐
│  СЛОЙ 1: Built-in Bridges (hardcoded в ядре) │
│  DuckDuckGo | Yandex | Baidu | Telegram      │
├──────────────────────────────────────────────┤
│  СЛОЙ 2: MCP-серверы (stdio/SSE, ext. proc) │
│  mcp-nasa-fireball | mcp-spectra | mcp-db    │
│  Любая база: DuckDB, SQLite, PostgreSQL      │
├──────────────────────────────────────────────┤
│  СЛОЙ 3: Personal Resources (нода-владелец) │
│  NotebookLM | Obsidian | ChromaDB | LightRAG  │
│  Доступны: только когда нода в группе        │
└──────────────────────────────────────────────┘
```

**Базы через MCP:** любая база данных оборачивается в MCP-сервер. Нода подключает MCP-сервер как bridge. Группа решает: "этой задаче даём доступ к PostgreSQL".

Подробно: [BRIDGES_MCP.md](BRIDGES_MCP.md)

---

## 7. Бортовой журнал агента и личные агенты в задаче

```
Задача "Поиск метеоритов"
├── Координатор: Agent-A (нода 1, DeepSeek Pro)
│   └── journal: task-0001/coordinator.log
├── Исполнитель: Agent-B (нода 2, Ollama)
│   ├── личные ресурсы: NotebookLM (нода 2)
│   └── journal: task-0001/scout-us.log
├── Исполнитель: Agent-C (нода 3, Ollama)
│   ├── личные ресурсы: Yandex.GPT (нода 3)
│   └── journal: task-0001/scout-ru.log
└── Хранилище: git@github.com:waters/mission-1-findings.git
```

Каждый агент ведёт свой бортовой журнал (агент + задача = файл). Личные ресурсы агента (NotebookLM, Obsidian) доступны только его задаче, не всей группе.

Подробно: [AGENT_JOURNAL.md](AGENT_JOURNAL.md)

---

## 7.5. DTN Cargo Protocol: три протокола обмена между нодами с переменной связью

Когда домашняя нода (переменный IP, периодические сеансы связи) соединяется с
хаб-нодой (постоянный IP, всегда онлайн), образуется классический DTN-канал с
задержками и разрывами. В такой топологии работают три протокола:

```
                  ┌──────────┐         прерывистый         ┌──────────┐
                  │ НОДА-ДОМ  │ ◄────── DTN-канал ──────►  │ НОДА-ХАБ │
                  │ is_home   │     (TCP при сеансе)       │ fixed_ip │
                  │ = true    │                             │ = true   │
                  └──────────┘                             └──────────┘
```

### 7.5.1. Agent Cargo — телепортация агента между нодами

**Назначение:** переместить агента со скиллами и состоянием с одной ноды на
другую — как «отправить посылку в космос».

**Два режима:**

| Режим | Канал | Состав | Размер |
|-------|-------|--------|--------|
| `Full` | > 1 Mbps | Скиллы + память + журнал + состояние | ~MB |
| `Lite` | < 1 Mbps | Только манифест + скиллы (без памяти) | ~KB |

**Два handshake-флоу:**

```
Push (отправитель инициирует):
  Sender                         Receiver
    │                               │
    ├── OFFER(manifest, mode) ──────▶  "прими агента X в режиме Full"
    │                               │  (receiver: проверить ресурсы, бриджи)
    │◀──── ACK(accepted/rejected) ──┤
    │         или ACK(Lite)         │  "ок" / "нет" / "давай Lite"
    │                               │
    ├── SEND(cargo_payload) ────────▶  передача (чанками при необходимости)
    │                               │
    │◀──── CONFIRM(landed) ──────────┤  "агент распакован, работает"
    │                               │

Pull (получатель запрашивает):
  Sender                         Receiver
    │                               │
    │◀──── REQUEST(agent, mode) ─────┤  "пришли агента X в Lite"
    │                               │
    ├── OFFER(manifest, mode) ──────▶│
    │                               │
    ├── SEND(payload) ──────────────▶│
    │                               │
    │◀──── CONFIRM(landed) ──────────┤
```

**Чанкование:** при узком канале весь cargo разбивается на `CargoChunk`,
каждый чанк передаётся отдельным gossip-сообщением. При обрыве сеанса —
resume с последнего подтверждённого чанка.

**Состояния Cargo (state machine):**

```
OfferSent → AcceptancePending → Transferring → Landed
                                       ↓
                                TransferPaused → Transferring (resume)
                                             ↘ Expired

RequestSent → AwaitingSend → Transferring → Landed
```

### 7.5.2. Task Sync — синхронизация заданий и отчётов

**Назначение:** при установлении сеанса связи — двусторонняя синхронизация
очередей задач и отчётов.

```
Sync Session:
  Нода-дом ──▶ Хаб:  { reports: [r1, r2], status: {agent_x: done} }
  Хаб ──▶ Нода-дом:  { new_tasks: [t3, t4], group_update: {mode: hunt} }
```

**Протокол:**
- `sync.start` — инициация сеанса, seq последней синхронизации
- `sync.data` — передача diff (только то, что изменилось с last_seq)
- `sync.ack` — подтверждение приёма

**Разрешение конфликтов:** приоритет у хаба (ноды с `fixed_ip=true`),
если обе ноды изменили один и тот же ресурс.

### 7.5.3. Result-Exchange — только обмен результатами

**Назначение:** минимальный протокол для коротких сеансов связи
(< 100 Kbps, < 30 сек). Только findings, без задач и состояния агентов.

```
Minimal Session:
  Нода-дом ──▶ Хаб:  { findings: [{type: meteorite, loc: x,y}...] }
  Хаб ──▶ Нода-дом:  { ack: true, received: 42 }
```

**Протокол:**
- `xchange.data` — пачка findings (без сериализации агента)
- `xchange.ack` — подтверждение количества принятых записей
- Никакой state machine, никакого разрешения конфликтов

### 7.5.4. Интеграция с gossip и convo

Новые сообщения в gossip:
- `cargo.offer`, `cargo.ack`, `cargo.send`, `cargo.confirm` — Push-флоу
- `cargo.request` — Pull-флоу
- `cargo.chunk` — чанкованная передача
- `sync.start`, `sync.data`, `sync.ack` — синхронизация
- `xchange.data`, `xchange.ack` — обмен результатами

**Чат-аппрув** (через convo.rs):
```
📦 Входящий груз: агент "scout-us" (Lite)
    Отправитель: node-5 (192.168.1.5)
    Миссия: "search-meteorites"
    Нужны бриджи: [duckduckgo]

    > Принять? (да/нет)
    > Если да: Full или Lite?
```

### 7.5.5. Модель ноды для DTN

Два новых поля в `NodeIdentity`:

```rust
pub struct NodeIdentity {
    // ... существующие поля ...
    pub is_home_node: bool,   // нода с переменным IP
    pub fixed_ip: bool,       // нода с постоянным IP
}
```

Новый модуль `cargo.rs` содержит:
- `CargoMode` (Full | Lite)
- `CargoStatus` — state machine (10 состояний)
- `AgentCargo`, `CargoManifest`, `AgentSnapshot` — структуры груза
- `CargoChunk` — для чанкования
- `SyncSession`, `TaskReport`, `TaskAssignment` — синхронизация
- `ResultExchange`, `Finding` — обмен результатами
- `CargoGossipMessage` — enum всех gossip-сообщений для доставки
- `CargoEngine` — управление очередями, проверка TTL

## 8. Структура модулей v0.3 (с изменениями)

```
waters-node/src/
├── main.rs          # Точка входа, CLI (рефакторинг: вынести логику)
├── engine.rs        # 🔴 НОВЫЙ: основной цикл (из main.rs)
├── handlers.rs      # 🔴 НОВЫЙ: роутер команд/чата (из main.rs)
├── display.rs       # 🔴 НОВЫЙ: вывод (из main.rs)
├── config.rs        # ✅ Есть: +profiles (P2)
├── node.rs          # ✅ Есть: NodeID
├── llm.rs           # ⚡ Есть: +streaming (P0)
├── chat.rs          # ✅ Есть: LLM как UX
├── convo.rs         # ⚡ Есть: +approval flow (P0)
├── session.rs       # ⚡ Есть: +crash recovery (P0)
├── task.rs          # ⚡ Есть: +resource binding (P1)
├── agent.rs         # ⚡ Есть: +personal in group task (P1)
├── subagent.rs      # ⚡ Есть: +lifecycle (P1)
├── group.rs         # ⚡ Есть: +shared resources (P0)
├── channel.rs       # ✅ Есть: WAL
├── gossip.rs        # ⚡ Есть: +chat approval (P0)
├── bridge.rs        # ⚡ Есть: +MCP config file (P0)
├── skill.rs         # ✅ Есть: Skills 2.0
├── tools.rs         # ✅ Есть: 9 tools
├── mode.rs          # ✅ Есть: mode engine
├── api.rs           # ⚡ Есть: +event stream (P1)
├── mcp.rs           # ✅ Есть: MCP client
├── autonomy.rs      # ✅ Есть: L0-L4
├── cargo.rs         # 🔴 НОВЫЙ: agent cargo + task sync + result exchange (P1)
├── dtn.rs           # ✅ Есть: DTN (обновить: приоритетная очередь для cargo)
├── journal.rs       # ⚡ Есть: +per-task journal (P1)
├── store.rs         # ✅ Есть: Redis (feature-gated)
├── kafka.rs         # ⚡ Есть: +military mode (P2)
├── demo.rs          # ✅ Есть: demo mode
├── voice.rs         # 🔴 НОВЫЙ: голосовой bridge (P2)
└── crypto.rs        # ⚡ Есть: ed25519 (запланирован)
```

**Условные обозначения:**
- ✅ — готово, изменений не требует
- ⚡ — есть, требуется доработка
- 🔴 — новый модуль

---

## 9. Связь документов

```
docs/waters-node/
├── MASTER_SPEC_V3.md       ← ВЫ ЗДЕСЬ
├── ARCHITECTURE_V3.md       — архитектура ноды
├── GROUP_MODES.md           — режимы групповой работы
├── SKILL_FORMAT.md          — формат скиллов и конвертация
├── BRIDGES_MCP.md           — внешние связи, базы, MCP
├── AGENT_JOURNAL.md         — бортовой журнал, личные агенты
└── TASK_LIST_V3.md          — поэтапный план работ
```
