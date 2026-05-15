# Пакет для Конструктора Сети — реализация mission-control

**Версия:** 2.0
**Дата:** 2026-05-15
**Назначение:** единый документ для старта разработки

---

## 1. Миссионный сервер — спецификация

### 1.1. Требования к железу

| Параметр | Минимум | Рекомендуется |
|----------|---------|---------------|
| CPU | 4 vCPU | 8 vCPU |
| RAM | 8 GB | 16 GB |
| SSD | 50 GB | 100 GB NVMe |
| ОС | Ubuntu 24.04 LTS | Ubuntu 24.04 LTS |
| Docker | 27.x | 27.x |
| Python | 3.12 | 3.12 |
| Rust | 2024 stable | 2024 stable |
| Сеть | 1 Gbps | 1 Gbps |

### 1.2. Провайдер

[Hetzner AX52](https://www.hetzner.com/) — €35/мес:
- 8 vCPU
- 32 GB RAM
- 2 × 1 TB NVMe
- 1 Gbps

### 1.3. Что ставится на сервер

```
mission-server/
├── mission-core/         ← Rust: ядро (сборка в статический бинарник)
│   └── target/release/mission-core   ← единственный бинарник (~15MB)
├── mission-edge/         ← Python: ранги, MCP-серверы
├── config/
│   ├── mission.toml      ← конфиг миссии
│   ├── mcp.json          ← MCP-серверы
│   └── latency_plan.json ← DTN-профили
├── .env                  ← API-ключи (не в git)
├── skills/               ← SKILL.md для sub-agent'ов
├── docker-compose.yml    ← Redis + (опционально)
└── dtn.sh               ← tc-netem wrapper
```

---

## 2. Архитектура — 6 слоёв (что строим)

```
┌────────────────────────────────────────────────────────────────┐
│  СЛОЙ 6: Интерфейсы (Python, отдельные процессы)              │
│  Telegram Bot | Voice Gateway | Web UI | Public API            │
│  ─── все пишут/читают Kafka ───                                │
├────────────────────────────────────────────────────────────────┤
│  СЛОЙ 5: Kafka (готовая инфраструктура на 238)                │
│  6 топиков: orders | findings | agents | ranks | heartbeat | cfg│
├────────────────────────────────────────────────────────────────┤
│  СЛОЙ 4: HTTP API (Rust, axum, порт 8787)                    │
│  /healthz | /api/v1/mission/* | /api/v1/admin/*               │
├────────────────────────────────────────────────────────────────┤
│  СЛОЙ 3: MCP Client (Rust, stdio/SSE)                        │
│  mcp-nasa | mcp-spectra | mcp-trajectory                       │
├────────────────────────────────────────────────────────────────┤
│  СЛОЙ 2: Agent Runtime Core (Rust) ← **СТРОИМ СЕЙЧАС**       │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ SubAgent Manager  │ Rank Engine │ Autonomy L0-L4 │ DTN  │  │
│  └─────────────────────────────────────────────────────────┘  │
├────────────────────────────────────────────────────────────────┤
│  СЛОЙ 1: Инфраструктура (Docker/system)                      │
│  Redis (кэш) | Ollama (fallback) | tc-netem (DTN)             │
└────────────────────────────────────────────────────────────────┘
```

---

## 3. Rust Core — 4 модуля (что конкретно писать)

### 3.1. `src/kafka.rs` — Kafka слой

```rust
// Зависимости: rdkafka, tokio, serde, serde_json
pub struct KafkaClient {
    consumer: StreamConsumer,      // orders topic
    producer: FutureProducer,      // findings, agents, ranks, heartbeat
    group_id: String,              // "mission-1-consumer"
}

// —— Consumer ——
pub async fn consume_orders(&self) -> Result<Order>;
// Читает mission.1.orders.v1, десериализует в Order

// —— Producers ——
pub async fn publish_finding(&self, f: Finding) -> Result<()>;
pub async fn publish_agents(&self, agents: Vec<AgentStatus>) -> Result<()>;
pub async fn publish_heartbeat(&self, hb: Heartbeat) -> Result<()>;
pub async fn publish_ranks(&self, event: RankEvent) -> Result<()>;

// Типы данных (соответствуют JSON-схемам из schemas/mission_*.v1.json)
pub struct Order {
    pub order_id: String,
    pub type: OrderType,        // Search | Recalibrate | Pause | Resume | Shutdown | Reconfigure
    pub issued_by: String,
    pub issued_at: DateTime<Utc>,
    pub payload: serde_json::Value,
    pub priority: Priority,     // Low | Normal | High | Critical
    pub ttl_seconds: u64,
}

pub struct Finding {
    pub finding_id: String,
    pub r#type: FindingType,   // MeteoriteCandidate | Anomaly | Spectrum | Trajectory | Status
    pub source_agent: String,   // "explorer.1"
    pub found_at: DateTime<Utc>,
    pub confidence: f64,
    pub rank: u8,
    pub data: serde_json::Value,
}

pub struct Heartbeat {
    pub mission_id: String,
    pub level: AutonomyLevel,   // L0 | L1 | L2 | L3 | L4
    pub agents_active: u32,
    pub agents_total: u32,
    pub findings_total: u64,
    pub uptime_seconds: u64,
    pub memory_usage_pct: f64,
}
```

**Что берём из TUI:** ничего — Kafka свой, в TUI его нет.

### 3.2. `src/subagent.rs` — SubAgent Manager

```rust
pub enum AgentRole {
    Explorer,
    Collector,
    Analyzer,
    Matcher,
    Coordinator,
}

pub enum AgentStatus {
    Idle,
    Working { current_task: String },
    Error { reason: String },
    Offline,
}

pub struct SubAgentManager {
    agents: HashMap<String, SubAgentInstance>,
    max_concurrent: u8,              // default 10
    mcp_client: McpClient,           // слой 3
    kafka: Arc<KafkaClient>,         // слой 5
    redis: RedisClient,              // слой 1
}

impl SubAgentManager {
    // Получить order из Kafka → найти/создать sub-agent → дать задачу
    pub async fn dispatch_order(&mut self, order: Order) -> Result<()>;

    // Запустить sub-agent с ролью и SKILL.md
    pub async fn spawn(
        &mut self,
        role: AgentRole,
        skill_path: &Path,
        task: &str,
    ) -> Result<AgentHandle>;

    // Получить результат работы sub-agent'а
    pub async fn collect_finding(&mut self, handle: AgentHandle) -> Result<Finding>;

    // Heartbeat — обновить статус всех sub-agent'ов
    pub async fn heartbeat(&self) -> Vec<AgentStatus>;
}
```

**Что берём из TUI:**
- Концепцию lifecycle: `Pending → Active → Working → Completed | Failed`
- Концепцию Mailbox (structured events) — адаптируем в Kafka-топики
- Концепцию fork_context (prefix cache для повторных поисков)

### 3.3. `src/autonomy.rs` — Autonomy Engine

```rust
pub enum AutonomyLevel { L0, L1, L2, L3, L4 }

pub struct AutonomyEngine {
    level: AutonomyLevel,
    kafka: Arc<KafkaClient>,
    redis: RedisClient,           // очередь offline findings
    last_contact: DateTime<Utc>,  // когда последний раз был heartbeat к 238
}

impl AutonomyEngine {
    // Определить текущий уровень на основе состояния Kafka и LLM
    pub async fn determine_level(&self) -> AutonomyLevel;

    // Переключиться на другой уровень
    pub async fn switch_to(&mut self, level: AutonomyLevel);

    // При L2-L3: сохранить finding в Redis (Kafka недоступен)
    pub async fn enqueue_offline(&self, finding: Finding);

    // При восстановлении L0: отправить все накопленные findings
    pub async fn flush_offline_queue(&self) -> Result<()>;

    // Сеансовый протокол: Handshake → Burst → Sync → Teardown
    pub async fn run_contact_window(&self, window: ContactWindow) -> Result<()>;
}
```

**Что берём из TUI:** ничего — в TUI нет автономии. Наша разработка.

### 3.4. `src/dtn.rs` — DTN-эмуляция

```rust
pub struct DtnEngine {
    interface: String,     // eth0
    active_profile: DtnProfile,
}

pub struct DtnProfile {
    pub name: String,       // "field" | "lunar" | "mars" | "deep"
    pub delay_ms: u64,
    pub jitter_ms: u64,
    pub loss: f64,
}

impl DtnEngine {
    // Применить профиль задержки через tc-netem
    pub fn apply(profile: &DtnProfile) -> Result<()>;

    // Сбросить все задержки
    pub fn remove() -> Result<()>;

    // Запустить планировщик по latency_plan.json
    pub async fn schedule(plan_path: &Path) -> Result<JoinHandle<()>>;
}
```

**Что берём из TUI:** ничего — в TUI нет DTN. Наша разработка.

---

## 4. Python Edge — что писать на Python

| Модуль | Назначение | Когда |
|--------|-----------|-------|
| `rank_engine.py` | Читает findings.v1 → считает KPI → публикует ranks.v1 | Фаза 2 |
| `mcp-nasa-fireball/server.py` | MCP-сервер для NASA API | Фаза 1 |
| `mcp-spectra/server.py` | MCP-сервер для спектральных БД | Фаза 1 |
| `mcp-trajectory/server.py` | MCP-сервер для расчёта траекторий | Фаза 1 |
| `telegram_bot.py` | Telegram → Kafka orders.v1 | Фаза 4 |
| `website/` | FastAPI + React (Kafka → WS) | Фаза 4 |

---

## 5. API контракты между слоями

### 5.1. Слой 2 (Core) → Слой 5 (Kafka)

```
Core читает:     mission.1.orders.v1     (приказы)
                 mission.1.config.v1     (конфигурация)

Core пишет:      mission.1.findings.v1   (находки)
                 mission.1.agents.v1     (статус агентов)
                 mission.1.heartbeat.v1  (healthcheck)
                 mission.1.ranks.v1      (события рангов — через Rank Engine)
```

### 5.2. Слой 2 (Core) → Слой 3 (MCP)

```
Core → MCP-сервер: stdio JSON-RPC 2.0
  tools: search_fireballs, get_fireball_detail, classify_meteorite,
         search_spectra_db, calculate_trajectory, estimate_landing_zone
```

### 5.3. Слой 2 (Core) → Слой 4 (HTTP API)

```
HTTP API → Core (через tokio mpsc / Arc<Mutex<SubAgentManager>>):
  GET  /healthz                         → healthcheck
  GET  /api/v1/mission/status           → статус миссии
  GET  /api/v1/mission/agents           → список sub-agent'ов
  POST /api/v1/mission/order            → отправить order (альтернатива Kafka)
  POST /api/v1/admin/reconfigure        → переконфигурировать
  GET  /api/v1/admin/log                → последние N строк журнала
```

### 5.4. Слой 2 (Core) → Слой 1 (Redis)

```
Core → Redis:
  SET mission:1:level "L0"             — уровень автономии
  RPUSH mission:1:offline:queue {...}   — очередь findings при L2-L3
  GET mission:1:offline:queue:count     — размер очереди
  SET mission:1:last_contact "..."      — время последнего сеанса
```

---

## 6. Структура Rust-проекта

```
mission-core/
├── Cargo.toml
├── src/
│   ├── main.rs              # точка входа, tokio runtime
│   ├── kafka.rs             # Kafka consumer + producer
│   ├── subagent.rs          # SubAgent Manager
│   ├── autonomy.rs          # Autonomy Engine (L0-L4)
│   ├── dtn.rs               # DTN-эмуляция
│   ├── mcp.rs               # MCP Client (stdio)
│   ├── api.rs               # HTTP API (axum)
│   ├── config.rs            # Чтение mission.toml
│   ├── types.rs             # Все типы (Order, Finding, Agent, etc.)
│   └── utils.rs             # Логирование, ошибки
└── Cargo.lock
```

**Зависимости Cargo.toml:**

```toml
[package]
name = "mission-core"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rdkafka = { version = "0.37", features = ["tokio"] }
redis = { version = "0.27", features = ["tokio-comp"] }
axum = "0.8"
tower-http = { version = "0.6", features = ["cors"] }
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = { version = "0.4", features = ["serde"] }
toml = "0.8"
anyhow = "1"
thiserror = "2"
reqwest = { version = "0.12", features = ["json", "rustls"] }
uuid = { version = "1", features = ["v4"] }
```

---

## 7. Запуск и деплой

```bash
# === На миссионном сервере ===

# 1. Собрать бинарник
cargo build --release
# → target/release/mission-core (~15MB)

# 2. Скопировать на сервер
scp target/release/mission-core user@mission-server:/opt/mission-control/
scp -r config/ user@mission-server:/opt/mission-control/
scp -r skills/ user@mission-server:/opt/mission-control/

# 3. Запустить
cd /opt/mission-control
./mission-core --config config/mission.toml

# 4. Или через Docker
docker build -t mission-core .
docker run -d --network host -v $(pwd)/config:/config mission-core
```

---

## 8. Фазы реализации (для Конструктора)

| Фаза | Что делать | Оценка |
|------|-----------|--------|
| **P0** | `main.rs` + `kafka.rs` + `types.rs` — Kafka consumer читает orders, producer пишет heartbeat | 1 день |
| **P0** | `subagent.rs` — spawn sub-process, lifecycle (Pending→Active→Working→Completed→Idle) | 1 день |
| **P1** | `autonomy.rs` — L0-L4, offline queue (Redis), flush при восстановлении | 1 день |
| **P1** | `api.rs` — axum HTTP API: /healthz, /api/v1/mission/* | 0.5 дня |
| **P1** | `mcp.rs` — MCP Client (stdio), запуск mcp-nasa-fireball как subprocess | 0.5 дня |
| **P2** | `config.rs` — чтение mission.toml, mcp.json, latency_plan.json | 0.5 дня |
| **P2** | `dtn.rs` — tc-netem wrapper, планировщик по расписанию | 0.5 дня |
| **P3** | Интеграционный тест: order → sub-agent → MCP → finding → Kafka → rank | 1 день |
| **P3** | Dockerfile + systemd service | 0.5 дня |

**Итого на ядро:** ~6 дней чистого кода

**Параллельно (Python):**
| **P1** | `mcp-nasa-fireball/server.py` | 0.5 дня (Интегратор) |
| **P2** | `rank_engine.py` — Kafka consumer → KPI → ranks.v1 | 0.5 дня |
| **P3** | `mcp-spectra/server.py` + `mcp-trajectory/server.py` | 1 день (Интегратор) |

---

## 9. Ссылки на документацию

| Что нужно | Где |
|-----------|-----|
| JSON-схемы Kafka-топиков | `schemas/mission_*.v1.json` |
| Описание протокола Kafka | `docs/mission-control/KAFKA_PROTOCOL.md` |
| Sub-agent роли и SKILL.md | `docs/mission-control/AGENTS.md` |
| MCP-серверы | `docs/mission-control/MCP.md` |
| Уровни автономии L0-L4 | `docs/mission-control/MODES.md` |
| Система рангов | `docs/mission-control/RANKS.md` |
| Конфигурация миссии | `docs/mission-control/CONFIGURATION.md` |
| Ранги (reference) | [h2o-mining.space](https://h2o-mining.space) |
| TUI под капотом | `docs/deepseek-tui/` (18 файлов) |
| Сравнение с аналогами | `docs/mission-control/WATERS_VS_TUI.md` |
| Доктрина миссии | `doctrine/mission_system.md` |
| Бортовой журнал | `doctrine/onboard_logs.md` |
