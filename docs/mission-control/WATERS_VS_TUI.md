# WATERS vs TUI: улучшения в коммуникации, автономии и сеансовой связи

**Версия:** 1.0
**Дата:** 2026-05-15

---

## 1. Проблема TUI, которую мы решаем

TUI спроектирован для одного разработчика за одним терминалом с мгновенной связью.
Вся архитектура завязана на **in-process каналы (mpsc)** — пока процесс жив, агенты общаются.
Процесс умер — всё потеряно.

Наша среда принципиально другая:
- Агенты на **разных серверах** (238 + миссионный сервер + будущие)
- Связь **непостоянная** — часы/дни без контакта
- Задержки **измеряются минутами**, не миллисекундами
- Сообщения **не должны теряться** при падении процесса

---

## 2. Агентская коммуникация: TUI vs WATERS

### TUI (in-process)

```
          ┌─── Engine (один процесс) ─────────────────────┐
          │                                               │
User → Engine ──(mpsc)──► SubAgent Manager
          │                    │
          │              ┌─────┴─────┐
          │         (mpsc)│          │(mpsc)
          │         ┌────▼──┐  ┌────▼──┐
          │         │Agent A│  │Agent B│
          │         │       │  │       │
          │         │Mailbox│  │Mailbox│
          │         └──┬────┘  └──┬────┘
          │            │(direct)  │
          │         ┌──▼──────────▼──┐
          │         │   Tool Calls   │
          │         └────────────────┘
          └──────────────────────────────────────────────

Проблемы:
- Все агенты в одном процессе → умер процесс = умерли все
- Общение только через mpsc → нет replay, нет истории
- Нет разделения ответственности → agent A не может работать без engine
- Нет очередей → при перегрузке сообщения теряются
```

### WATERS (Kafka-native)

```
          ┌─────── Agent Runtime A (миссионный сервер 1) ──────┐
          │                                                     │
          │  ┌────────────────────────────────────────────┐     │
          │  │  Kafka Consumer Group: "mission-1"         │     │
          │  │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────────┐ │     │
          │  │  │expl.1│ │coll.2│ │anl.3 │ │coord.1   │ │     │
          │  │  └──┬───┘ └──┬───┘ └──┬───┘ └─────┬────┘ │     │
          │  │     │         │         │           │      │     │
          │  │     └────┬────┴─────────┴───┬───────┘      │     │
          │  │          │                  │              │     │
          │  │     ┌────▼──────────────────▼────┐         │     │
          │  │     │     Kafka topics:          │         │     │
          │  │     │  findings.v1  agents.v1    │         │     │
          │  │     │  ranks.v1    heartbeat.v1 │         │     │
          │  │     └──────────────┬─────────────┘         │     │
          │  └────────────────────┼────────────────────────┘     │
          └───────────────────────┼──────────────────────────────┘
                                  │
                    ══════════════╪══════════════
                    │    Kafka    │   Cluster    │
                    │             │   (238 ХАБ)  │
                    ══════════════╪══════════════
                                  │
          ┌───────────────────────┼──────────────────────────────┐
          │  Agent Runtime B      │  (миссионный сервер 2)       │
          │  ┌────────────────────┴────────────────────────────┐ │
          │  │  Kafka Consumer Group: "mission-1"              │ │
          │  │  (те же топики, свои partition'ы)              │ │
          │  │  ┌──────┐ ┌──────┐                               │ │
          │  │  │exp.2 │ │col.3 │                               │ │
          │  │  └──────┘ └──────┘                               │ │
          │  └─────────────────────────────────────────────────┘ │
          └──────────────────────────────────────────────────────┘

Преимущества:
- Агенты на разных серверах ↔ разделены физически
- Kafka хранит историю ↔ replay при падении
- Consumer groups ↔ агенты восстанавливаются там же, где упали
- Каждый finding — immutable record ↔ аудит, traceability
- Асинхронность ↔ агент не блокируется ожиданием другого
```

---

## 3. Получение заданий от внешних источников

### TUI

```
Источник заданий           Способ получения
─────────────────         ─────────────────
TUI command line           ✅  прямой ввод
Feishu/Lark bot            ✅  через bridge к localhost API
GitHub Issues              ✅  cron агенты на Cloudflare
Telegram                   ❌  нет
Voice                      ❌  нет
HTTP API (remote)          ❌  localhost-only
Другие сервисы             ❌  нет
```

### WATERS

```
Источник заданий           Канал                    Время жизни
─────────────────         ─────────────────────    ────────────
Telegram бот              → Kafka: orders.v1       ✅  живёт в Kafka, не теряется
Web UI (mission.waters.ai)→ Kafka: orders.v1        ✅  живёт в Kafka
Voice (Whisper → STT)    → Kafka: orders.v1        ✅  живёт в Kafka
HTTP API (публичный)      → Kafka: orders.v1       ✅  rate-limited, auth
Kafka cron (scheduler)    → Kafka: orders.v1        ✅  расписание
Другая миссия             → Kafka: orders.v1        ✅  перекрёстные миссии
238 ХАБ (OpenCode агенты) → Kafka: orders.v1        ✅  штатный канал
```

**Ключевое отличие:** любой внешний источник пишет в тот же Kafka topic `orders.v1`.
Никакой магии, никаких дополнительных bridge'ей для каждого нового источника.
Добавить новый канал = написать producer для одного Kafka топика.

---

## 4. Автономная работа (наше главное преимущество)

### TUI

```
Состояние сети       Что происходит
────────────────     ─────────────────────
Есть интернет        ✅  работает
Нет интернета        ❌  НЕ РАБОТАЕТ (DeepSeek API недоступен)
LLM недоступна       ❌  НЕ РАБОТАЕТ (нет fallback)
Процесс упал         ❌  потеря контекста (если не сохранена сессия)
```

### WATERS — L0-L4

```
УРОВЕНЬ   KAFKA      LLM              АГЕНТЫ             НАХОДКИ
L0        ✅ Online  DeepSeek API     Все активны        real-time в Kafka
L1        ✅ Online  Ollama local     Все активны        real-time в Kafka
L2        ❌ Offline Ollama local     Все активны        локальная очередь (Redis)
L3        ❌ Offline Ollama урезан.  Критические только  локальная очередь
L4        ❌ Offline НЕТ             STOP               логи на диск
```

**Автономия ≠ остановка.** Агенты продолжают работать, findings копятся локально.
При восстановлении связи — burst-синхронизация всего накопленного за период.

### Сеансовый режим работы

Для лунных/марсианских миссий, где связь возможна только в окна:

```
┌──────────────────────────────────────────────────────────────┐
│  Сеансовый режим (Contact Window Mode)                        │
│                                                               │
│  ОКНО СВЯЗИ 1                    ОКНО СВЯЗИ 2                │
│  [08:00-08:15]                   [16:00-16:15]               │
│       │                                │                    │
│       ▼                                ▼                    │
│  ┌──────────────┐               ┌──────────────┐            │
│  │ Получить     │               │ Получить     │            │
│  │ orders       │               │ orders       │            │
│  │ Отправить    │               │ Отправить    │            │
│  │ findings     │               │ findings     │            │
│  │ (burst)      │               │ (burst)      │            │
│  └──────┬───────┘               └──────┬───────┘            │
│         │                              │                     │
│         ▼                              ▼                     │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  МЕЖДУ ОКНАМИ: Автономный режим (L2)                 │   │
│  │  ─────────────────────────────────────────────       │   │
│  │  • Агенты работают 24/7                              │   │
│  │  • Findings копятся в Redis                         │   │
│  │  • Rank Engine считает KPI локально                  │   │
│  │  • Autonomy Level: L2                                │   │
│  │  • При наступлении окна — burst-отправка             │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

---

## 5. Сеансовая отчётность (Contact Window Protocol)

### Как это работает

Каждое окно связи — это сеанс. Внутри сеанса:

```
┌───── МИССИЯ ───────────────────────────────── 238 ХАБ ──────┐
│                                                               │
│ 1. HANDSHAKE                                                  │
│    ─────────────────                                          │
│    "Я — Миссия 1, уровень L2,                                │
│     12/16 агентов, 47 новых findings"                        │
│    ◄──────────────────────────────►                           │
│    "Принято, жду findings"                                    │
│                                                               │
│ 2. BURST UPLOAD (высокий приоритет)                          │
│    ───────────────────────────────                            │
│    [finding_048] [finding_049] ... [finding_094]             │
│    ◄──────────────────────────────►                           │
│    "94/94 подтверждено"                                       │
│                                                               │
│ 3. BURST DOWNLOAD (orders)                                   │
│    ─────────────────────────────                              │
│    [order_012] [order_013] [order_014]                        │
│    ◄──────────────────────────────►                           │
│    "3/3 принято"                                              │
│                                                               │
│ 4. STATE SYNC                                                │
│    ──────────                                                 │
│    Конфиг рангов → миссия                                     │
│    Статус ХАБа → миссия                                       │
│    ◄──────────────────────────────►                           │
│                                                               │
│ 5. TEARDOWN                                                   │
│    ──────────                                                 │
│    "Сеанс завершён. Следующее окно: 16:00 UTC"               │
│    ◄──────────────────────────────►                           │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

### Протокол сеанса (один Kafka message)

```json
{
  "session_id": "contact_20260515_0800",
  "mission_id": "mission-1",
  "type": "handshake|burst_up|burst_down|state_sync|teardown",
  "timestamp": "2026-05-15T08:00:00Z",
  "sequence": 1,
  "total_messages": 5,
  "payload": {
    "findings_batch": ["f_048", "f_049", "..."],
    "orders_batch": ["ord_012", "ord_013"],
    "agent_status": {"active": 12, "total": 16, "uptime_h": 48},
    "kpi_snapshot": {"findings_total": 94, "avg_confidence": 0.71}
  },
  "checksum": "sha256:<hash>"
}
```

### Очередь для burst (Redis → Kafka)

Когда Kafka недоступен (L2-L3), findings пишутся в Redis с структурированной очередью:

```
Redis keys:
  mission:1:findings:queue        → List (JSON findings, ordered by time)
  mission:1:findings:queue:count  → Integer (счётчик для быстрой проверки)
  mission:1:heartbeat:last        → String (последний heartbeat перед потерей)
  mission:1:session:last_contact  → String (время последнего успешного сеанса)

При восстановлении:
  1. `BLPOP` всю очередь
  2. Упаковать в batch (до 100 findings за один сеанс)
  3. Отправить в Kafka
  4. Если batch не влез — разбить на несколько сеансов
```

---

## 6. Что улучшили относительно TUI — сводная таблица

| Аспект | TUI | WATERS | Тип улучшения |
|--------|-----|--------|--------------|
| **Канал общения агентов** | mpsc in-process (1 процесс) | Kafka topics (N серверов) | **Архитектурный прорыв** |
| **Выживаемость при падении** | Теряет контекст | Kafka replay + consumer group | **Принципиально лучше** |
| **История сообщений** | Нет, только session.json | Kafka log с retention | **Принципиально лучше** |
| **Внешние источники** | CLI + Feishu/Lark (2) | Telegram + Web + Voice + API + Cron + ... (∞) | **Масштабируемость** |
| **Добавление нового канала** | Писать bridge к localhost API | Написать Kafka producer (3 строки) | **Проще на порядок** |
| **Автономия без связи** | ❌ Не работает | ✅ L0-L4 | **Наше эксклюзивное** |
| **Задержки (DTN)** | ❌ Не поддерживает | ✅ tc-netem + burst sync | **Наше эксклюзивное** |
| **Сеансовый режим** | ❌ Нет | ✅ Handshake → Burst → Sync | **Наше эксклюзивное** |
| **Multi-миссия** | ❌ 1 агент, 1 сессия | ✅ Mиссии общаются через Kafka | **Архитектурный прорыв** |
| **Мониторинг** | stdout логи | Kafka → mission.waters.ai | **Принципиально лучше** |
| **Multi-user** | localhost-only | JWT + Kafka auth | **Масштабируемость** |
| **Capacity планирование** | Context slack profiling | Kafka partitions + consumer lag | **Надёжность** |

---

## 7. Как это реализовать: модули ядра

### Rust core: 4 модуля, ~1500 строк

```rust
// 1. kafka.rs — асинхронный consumer/producer
pub struct KafkaClient {
    consumer: StreamConsumer,
    producer: FutureProducer,
    group_id: String,
}

impl KafkaClient {
    pub async fn consume_orders(&self) -> Result<Order>;
    pub async fn publish_finding(&self, finding: Finding) -> Result<()>;
    pub async fn publish_agents(&self, agents: Vec<AgentStatus>) -> Result<()>;
    pub async fn publish_heartbeat(&self, hb: Heartbeat) -> Result<()>;
    pub async fn publish_ranks(&self, rank_event: RankEvent) -> Result<()>;
}

// 2. subagent.rs — lifecycle + mailbox → Kafka
pub struct SubAgentManager {
    agent_type: AgentRole,
    skill_path: PathBuf,
    mcp_clients: Vec<McpClient>,
    kafka: KafkaClient,
}

impl SubAgentManager {
    pub async fn spawn(order: Order) -> Result<AgentHandle>;
    pub async fn process_turn(handle: AgentHandle) -> Result<Finding>;
}

// 3. autonomy.rs — L0-L4 + сеансовый режим
pub struct AutonomyEngine {
    level: AutonomyLevel,
    kafka: KafkaClient,
    offline_queue: RedisClient,
    contact_window: Option<ContactWindow>,
}

impl AutonomyEngine {
    pub async fn determine_level() -> AutonomyLevel;
    pub async fn handle_offline_queue() -> Result<()>; // burst sync
    pub async fn run_contact_window() -> Result<()>;    // сеансовый протокол
}

// 4. dtn.rs — эмуляция задержек (tc-netem wrapper)
pub struct DtnEngine {
    interface: String,
    profile: DtnProfile,
}

impl DtnEngine {
    pub fn apply(profile: DtnProfile) -> Result<()>;
    pub fn remove() -> Result<()>;
    pub fn schedule(latency_plan: LatencyPlan) -> TaskHandle;
}
```

### Python edge: ранги + MCP-серверы + сайт

```
rank_engine.py     — читает findings.v1 → KPI → публикует ranks.v1
mcp-nasa/          — MCP сервер для NASA Fireball API
mcp-spectra/       — MCP сервер для спектральных баз
mcp-trajectory/    — MCP сервер для расчёта траекторий
telegram_bot.py    — Telegram → orders.v1
website/           — FastAPI + React (Kafka → WS)
```

---

## 8. Вывод

TUI хорош для одного разработчика. Мы строим **распределённую систему для колонизации космоса**. Наш подход:

- **Kafka вместо in-process** — развязали агентов, дали историю, дали replay
- **Автономия L0-L4** — система работает без связи неделями, не теряя данных
- **Сеансовый протокол** — для связи с задержками и окнами
- **Любой источник заданий** — Telegram, голос, веб, API — всё через Kafka
- **Масштабирование** — добавить миссию = поднять ещё один Agent Runtime с тем же Kafka

Это не форк TUI. Это **новая архитектура**, вдохновлённая TUI, но перестроенная под распределённую, автономную, отложенную связь.
