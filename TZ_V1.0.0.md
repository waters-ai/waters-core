# WATERS Node v1.0.0 — Техническое Задание / Technical Task / 技术任务

**Версия:** 1.0.0 | **Дата:** 2026-05-18 | **Автор:** agent.architect.v1
**Статус:** Утверждено / Approved / 已批准
**Исполнитель:** agent.constructor.v1
**Репозиторий кода:** `waters-ai/waters-core`
**Репозиторий документации:** `waters-ai/waters-node`

---

## Содержание / Table of Contents / 目录

1. Executive Summary / Резюме / 概要
2. Модуль hybrid_llm / hybrid_llm Module / hybrid_llm 模块
3. Модуль distributed_inference / distributed_inference Module / distributed_inference 模块
4. Модуль code_review_pipeline / code_review_pipeline Module / code_review_pipeline 模块
5. Архитектурная схема / Architecture Diagram / 架构图
6. Потоки данных для 5 сценариев / Data Flows for 5 Scenarios / 5种场景的数据流
7. Модель безопасности v1.0.0 / Security Model v1.0.0 / 安全模型 v1.0.0
8. Рекомендации по бортовым LLM / Edge LLM Recommendations / 边缘LLM建议
9. План миграции v0.5.0 → v1.0.0 / Migration Plan / 迁移计划
10. Сравнение с аналогами / Competitive Comparison / 竞品对比
11. Задание Конструктору / Constructor Task / 构造器任务

---

## 1. Executive Summary / Резюме / 概要

### [RU]
WATERS Node v1.0.0 — мажорное обновление платформы, добавляющее три ключевых модуля поверх существующей архитектуры v0.5.0 (48 модулей Rust, P2P-рой, 6 LLM, A2A, MCP Store, голос, медиа-мосты, YASA-безопасность).

**Проблема:** v0.5.0 требует стабильного канала до LLM. Обрыв связи переводит ноду в L4 (safe mode). Для фермеров, дронов, туристов, умных домов с прерывистым интернетом это критично.

**Решение:**
1. **hybrid_llm** — бортовая GGUF-модель 1-3B для офлайн-режима. Прозрачная прослойка под LLM Router. Переключение L0-L4.
2. **distributed_inference** — распределение LLM-нагрузки на 6 нод P2P: кеш Redis, pipeline parallelism, федерация токенов, доменная специализация.
3. **code_review_pipeline** — 3 агента (Programmer, Security Reviewer, Onboard Manager) для безопасного самообучения.

**Ценность:** Raspberry Pi с GGUF работает без интернета. 6 фермеров с радиомостами получают общий LLM-пул. Дрон продолжает детекцию при потере 4G.

### [EN]
WATERS Node v1.0.0 is a major update adding three key modules to the existing v0.5.0 architecture (48 Rust modules, P2P swarm, 6 LLMs, A2A, MCP Store, voice, media bridges, YASA security).

**Problem:** v0.5.0 requires a stable LLM channel. Connection loss drops the node to L4 (safe mode). Critical for farmers, drones, tourists, smart homes with intermittent internet.

**Solution:**
1. **hybrid_llm** — onboard GGUF 1-3B model for offline mode. Transparent layer under LLM Router. L0-L4 switching.
2. **distributed_inference** — LLM load distribution across 6 P2P nodes: Redis cache, pipeline parallelism, token federation, domain specialization.
3. **code_review_pipeline** — 3 agents (Programmer, Security Reviewer, Onboard Manager) for safe self-improvement.

**Value:** Raspberry Pi with GGUF works without internet. 6 farmers with radio bridges share an LLM pool. Drones continue detection on 4G loss.

### [ZH]
WATERS Node v1.0.0 是一个主要更新，在现有 v0.5.0 架构（48个Rust模块、P2P群集、6个LLM、A2A、MCP商店、语音、媒体桥接、YASA安全）之上添加了三个关键模块。

**问题：** v0.5.0 需要稳定的LLM通道。连接断开会将节点降至L4（安全模式）。这对拥有间歇性互联网的农民、无人机、游客、智能家居至关重要。

**解决方案：**
1. **hybrid_llm** — 用于离线模式的板载 GGUF 1-3B 模型。LLM 路由器下的透明层。L0-L4 切换。
2. **distributed_inference** — 跨6个P2P节点的LLM负载分布：Redis缓存、流水线并行、令牌联盟、领域专业化。
3. **code_review_pipeline** — 3个智能体（程序员、安全审查员、板载管理器）实现安全的自我改进。

**价值：** 带GGUF的Raspberry Pi无需互联网即可工作。6个农民通过无线电桥共享LLM池。无人机在4G丢失时继续检测。

---

## 2. Модуль hybrid_llm / hybrid_llm Module / hybrid_llm 模块

### 2.1 [RU] Назначение

Прозрачная прослойка между LLM Router (BridgePool) и агентами. Автоматически переключает источник LLM-ответа в зависимости от уровня автономии L0-L4.

### 2.1 [EN] Purpose

Transparent layer between LLM Router (BridgePool) and agents. Automatically switches LLM response source based on autonomy level L0-L4.

### 2.1 [ZH] 目的

LLM 路由器（BridgePool）和智能体之间的透明层。根据自主级别 L0-L4 自动切换 LLM 响应源。

### 2.2 [RU] Архитектура

**Файл:** `waters-node/src/hybrid_llm.rs` (новый модуль)

```rust
pub struct HybridLlm {
    remote: Arc<BridgePool>,         // существующий LLM Router
    edge: EdgeEngine,                 // бортовая GGUF
    switch: SwitchProtocol,           // логика L0-L4
    prefetch: PrefetchCache,          // упреждающая загрузка
    sync_queue: SyncQueue,            // очередь для L2-L3
}

pub enum QueryMode {
    Auto,        // определяет сам по L0-L4
    LocalOnly,   // только бортовая GGUF
    RemoteOnly,  // только внешняя LLM
    Distributed, // через distributed_inference
}
```

### 2.2 [EN] Architecture

**File:** `waters-node/src/hybrid_llm.rs` (new module)

```rust
pub struct HybridLlm {
    remote: Arc<BridgePool>,
    edge: EdgeEngine,
    switch: SwitchProtocol,
    prefetch: PrefetchCache,
    sync_queue: SyncQueue,
}
```

### 2.2 [ZH] 架构

**文件：** `waters-node/src/hybrid_llm.rs`（新模块）

### 2.3 [RU] Протокол переключения (SwitchProtocol)

Интеграция с существующим `autonomy.rs`:

| Уровень | hybrid_llm поведение |
|---------|----------------------|
| L0 | Remote (основная LLM), EdgeEngine валидирует ответ |
| L1 | Remote (Ollama local), EdgeEngine как fallback |
| L2 | EdgeEngine на простые запросы, сложные → SyncQueue |
| L3 | Только EdgeEngine + DTN-сжатый лог |
| L4 | Только SOS/маяк (safe mode) |

```rust
pub async fn resolve(&self, prompt: &str, level: AutonomyLevel) -> ResponseSource {
    match level {
        L0 => ResponseSource::Remote { validate: true },
        L1 => ResponseSource::Remote { validate: false },
        L2 => if self.is_simple_query(prompt) { ResponseSource::Local }
              else { ResponseSource::QueueForSync },
        L3 => ResponseSource::Local,
        L4 => ResponseSource::SosOnly,
    }
}
```

### 2.4 [RU] EdgeEngine — бортовая LLM

- Загрузка GGUF-модели 1-3B через крейт `llama-cpp-rs` или `candle`
- CPU-инференс, без GPU
- Ответ <500ms на Raspberry Pi 4
- Автоматическая загрузка модели при старте, если указан `edge_model` в конфиге

**Конфиг (новые поля):**
```toml
[edge]
model = "qwen2.5-1.5b.q4_k_m.gguf"  # путь к GGUF-файлу
cache_ttl = 300                        # TTL кеша бортовой LLM
max_tokens = 512
```

### 2.5 [RU] Упреждающая загрузка (PrefetchCache)

Анализирует активные задачи из `TaskManager` и предзагружает вероятные запросы:
```rust
pub async fn prefetch_for_scenario(&self, tasks: &[Task]) -> Result<()>
```

Хранит предзагруженные ответы в `KvStore::cache_db()` (Redis DB 15).

### 2.6 [RU] Очередь синхронизации (SyncQueue)

На базе существующего `OfflineQueue` (`offline.rs`). При L2-L3 сложные запросы:
1. `enqueue(prompt, edge_response)` → JSONL-файл
2. При восстановлении связи: `sync_after_reconnect()`
3. Remote LLM отвечает → сверка с edge_response
4. Расхождение → уведомление пользователя, коррекция

### 2.7 [RU] Формат DTN-пакета

```rust
pub struct DtnLlmPacket {
    magic: [u8; 4],    // b"WLLM"
    seq: u32,
    node_id: [u8; 16],
    payload: Vec<u8>,  // gzip(QueuedQuery)
    checksum: u32,
}
```

Пакет 256 байт — совместимость с существующим `dtn.rs`.

### 2.8 [RU] Интеграция с существующим кодом

| Файл | Изменения |
|------|-----------|
| `main.rs` | `HybridLlm` оборачивает `Arc<BridgePool>`. Все вызовы LLM через `HybridLlm::query()` |
| `bridge.rs` | Без изменений — HybridLlm вызывает BridgePool как RemoteRouter |
| `autonomy.rs` | Новый метод `can_use_edge()` |
| `offline.rs` | Без изменений — SyncQueue использует OfflineQueue |
| `store.rs` | Новая константа `EDGE_CACHE_DB = 14` |

---

## 3. Модуль distributed_inference / distributed_inference Module / distributed_inference 模块

### 3.1 [RU] Назначение

Распределение LLM-нагрузки между 6 нодами в P2P-группе. Четыре подрежима, автовыбор по условиям.

### 3.2 [RU] Подрежимы

#### 3.2.1 Распределённый кеш Redis (6×RAM)

**Файл:** `waters-node/src/distributed.rs` — `DistributedCache`

- hash(prompt) → key → gossip-запрос к соседям
- Поиск <200ms на 6 нодах
- До 80% повторных запросов без инференса
- Использует `KvStore::group_db()` (DB 1-6)

```rust
pub async fn lookup(&self, prompt: &str) -> Option<String> {
    let hash = blake3::hash(prompt.as_bytes());
    let key = format!("dllm:{}", hex::encode(&hash.as_bytes()[..4]));
    if let Some(val) = self.kv.get(&key)? { return Some(val) }
    self.gossip.broadcast(&GossipMessage::CacheRequest { key }).await;
    None
}
```

#### 3.2.2 Pipeline Parallelism (feature-gate)

- Модель разбивается по слоям (layer count / peer count)
- Каждая нода считает свой фрагмент
- Тензоры через `CargoEngine` (существующий `cargo.rs`)
- Формат: `P2pTensor { seq, source, dest, data, layer_idx }`
- Задержка не более ×3 от локального инференса
- **Отключён по умолчанию** — feature `pipeline-parallel`

#### 3.2.3 Федерация токенов (Token Federation)

- Единый пул токенов DeepSeek Flash API
- Распределение между нодами пропорционально задачам
- После распределения — полная автономия без API-запросов
- Модель: `TokenPool { total_tokens, allocated: HashMap<node_id, u64> }`

#### 3.2.4 Специализация по доменам

- Каждая нода держит свою .gguf модель
- `DomainRouter` мапит имя агента → нода через `ProfileConfig`

### 3.3 [RU] Автовыбор режима

```rust
pub fn select_mode(peer_count: usize, level: AutonomyLevel,
                   has_gguf: bool, has_tokens: bool) -> DistMode {
    match (peer_count, level, has_gguf, has_tokens) {
        (6, L0, _, true)  => TokenFederation,
        (6, L0, _, false) => PipelineParallel,
        (_, L0, false, _) => RedisCache,
        (_, L1, true, _)  => Local { fallback: true },
        (_, L2..=L4, true,_) => Local,
        _ => Off,
    }
}
```

### 3.4 [RU] Новые gossip-сообщения

```rust
pub enum DistGossipMessage {
    CacheRequest { key: String },
    CacheResponse { key: String, value: Option<String> },
    TensorChunk { tensor: P2pTensor },
    TokenAlloc { node_id: String, tokens: u64 },
}
```

---

## 4. Модуль code_review_pipeline / code_review_pipeline Module / code_review_pipeline 模块

### 4.1 [RU] Назначение

Безопасное самообучение ноды: 3 агента проверяют код перед внедрением. Каждый — `SubAgent`.

**Файл:** `waters-node/src/code_review.rs` (новый)

### 4.2 [RU] Роли

```rust
pub struct CodeReviewPipeline {
    programmer: SubAgent,    // пишет код + тесты
    security: SubAgent,      // YASA + OWASP Top 10
    onboard_mgr: SubAgent,   // CPU/RAM/compat
    sandbox: CodeSandbox,
    queue: Vec<PrEntry>,
}
```

#### 4.2.1 Programmer Agent
- Пишет код в temp-директорию
- Запускает `cargo test` в песочнице
- Готовит аудит-отчёт

#### 4.2.2 Security Reviewer Agent
- Выполняет `YasaAgent::screen_agent()`
- OWASP Top 10: inject, XSS, path traversal
- Анализ импортов: только аудированные крейты
- Вердикт: `Approve` / `Reject`

#### 4.2.3 Onboard Manager Agent
- Оценивает влияние на RAM/CPU по лимитам
- Проверяет совместимость с агентами
- Вердикт: `Approve` / `Reject`

### 4.3 [RU] Голосование

```rust
pub fn decide(security: &Verdict, onboard: &Verdict) -> Decision {
    match (security, onboard) {
        (Approve, Approve) => AutoDeploy,
        (Approve, Reject) | (Reject, Approve) => EscalateToOwner,
        (Reject, Reject) => Blocked,
    }
}
```

### 4.4 [RU] Интеграция с self_deploy.rs

```rust
pub fn deploy_with_review(&self, pipeline: &CodeReviewPipeline) -> Result<DeployResult, String>
```

### 4.5 [RU] CodeSandbox

```rust
pub struct CodeSandbox {
    temp_dir: TempDir,     // tempfile
    timeout: Duration,     // 30s
    max_memory: u64,       // 256MB
}
```

Режимы: `unshare` (Linux, root) | `docker` | `none` (отладка)

---

## 5. Архитектурная схема / Architecture Diagram / 架构图

### [RU]

```
┌──────────────────────────────────────────────────────────┐
│                    waters-node v1.0.0                     │
├──────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌───────────┐  │
│  │BridgePool│  │GossipEng │  │AgentMgr │  │A2AAdapter │  │
│  │(LLM,chat,│  │mDNS+TCP  │  │(10/node)│  │(Google A2A)│  │
│  │ MCP,voice│  │DTN+Cargo │  │SubAgents│  │           │  │
│  └────┬─────┘  └──────────┘  └─────────┘  └───────────┘  │
│       │                                                   │
│  ┌────▼─────────────────────────────────────────┐        │
│  │          HybridLlm (прослойка)               │        │
│  │  RemoteRouter → BridgePool                   │        │
│  │  EdgeEngine  → llama-cpp-rs (GGUF 1-3B)    │        │
│  │  SwitchProtocol → AutonomyEngine (L0-L4)    │        │
│  │  PrefetchCache → KvStore (DB 14-15)         │        │
│  │  SyncQueue → OfflineQueue (JSONL)           │        │
│  └──────────────────────────────────────────────┘        │
│                                                           │
│  ┌──────────────────────────────────────────────┐        │
│  │         DistributedInference                 │        │
│  │  DistributedCache | Pipeline | TokenFed |    │        │
│  │  DomainRouter (автовыбор)                    │        │
│  └──────────────────────────────────────────────┘        │
│                                                           │
│  ┌──────────────────────────────────────────────┐        │
│  │         CodeReviewPipeline                   │        │
│  │  Programmer | SecurityReviewer | OnboardMgr  │        │
│  │  CodeSandbox (unshare/docker)                │        │
│  └──────────────────────────────────────────────┘        │
│                                                           │
│  YASA | MCP Store | Voice | Media | Tamagotchi | Cron    │
└──────────────────────────────────────────────────────────┘
```

---

## 6. Потоки данных / Data Flows / 数据流

### 6.1 [RU] Сценарий 1: Фермер

```
Online: Camera → crop-monitor → hybrid_llm → RemoteRouter → DeepSeek
Offline (L2): Camera → hybrid_llm → EdgeEngine (GGUF 1-3B) → <500ms
Reconnect: SyncQueue.flush() → remote → compare → log correction
```

### 6.2 [RU] Сценарий 2: Дрон

```
Online: Camera → delivery-coordinator → Remote → JetPack
Offline (L2): avoidance → EdgeEngine (1B nav) → 200ms
Reconnect: gossip sync → findings.public
```

### 6.3 [RU] Сценарий 3: Кооператив

```
5 хозяйств, радиомост: DistributedCache → miss → PipelineParallel → нода агрономии
Market-pricer: TokenFed через спутник раз/сутки
```

### 6.4 [RU] Сценарий 4: Дом

```
Offline: elderly-monitor пульс>120 → EdgeEngine → DTN(256B) → relay → Telegram
Online: SyncQueue.flush() → remote → сравнение → лог
```

### 6.5 [RU] Сценарий 5: Туризм

```
Группа А (L4): hike-buddy → EdgeEngine, DTN beacon каждые 5 мин
Группа Б (L2): EdgeEngine + OfflineQueue
Сближение: gossip → DTN-обмен
```

---

## 7. Модель безопасности v1.0.0 / Security Model / 安全模型

### 7.1 [RU] YASA-CMD-6

```rust
("YASA-CMD-6", "Проверяй бортовую LLM — ответ может быть неточным без связи")
```

### 7.2 [RU] EdgeGuard

- Ответ GGUF не содержит: rm, dd, mkfs, API-ключи
- Prompt-инъекция → блокировка

### 7.3 [RU] CodeSandbox изоляция

| Режим | Требования | default |
|-------|-----------|---------|
| unshare | Linux root | да |
| docker | Docker | нет |
| none | — | отладка |

### 7.4 [RU] Rate Limit

```rust
EdgeRateLimiter { max_per_minute: 100 }  // на RPi4
```

---

## 8. Рекомендации по бортовым LLM / Edge LLM Recommendations / 边缘LLM建议

### [RU]

| Сектор | Модель GGUF | RAM | Устройство |
|--------|------------|-----|-----------|
| Агрономия | qwen2.5-3b-q4_k_m | 4GB | RPi4/Jetson |
| Доставка | tinylama-1.1b-q4_k_m | 2GB | Jetson Nano |
| Дом | qwen2.5-3b-q4_k_m | 4GB | RPi4/ПК |
| Туризм | tinylama-1.1b-q4_k_m | 2GB | Смартфон |
| Медицина | qwen2.5-3b-q4_k_m | 4GB | RPi4 |
| Fallback | qwen2.5-1.5b-q4_k_m | 2GB | Любое |

---

## 9. План миграции / Migration Plan / 迁移计划

### [RU]

| Фаза | Задачи | Дни |
|------|--------|-----|
| 1. Фундамент | EdgeEngine, SwitchProtocol, LlmCache, SyncQueue | 5 |
| 2. Распределённый | DistributedCache, DomainRouter, TokenPool, Pipeline | 4 |
| 3. Безопасность | CodeSandbox, CodeReviewPipeline, EdgeGuard | 3 |
| 4. Интеграция | main.rs wiring, тесты 5 сценариев, доки | 3 |
| **Итого** | | **15** |

### [RU] Риски

| Риск | Смягчение |
|------|----------|
| llama-cpp-rs не на ARM | fallback на candle |
| Pipeline >×3 latency | feature-gate, off по умолчанию |
| CodeSandbox без root | Docker fallback |
| GGUF 3B >500ms RPi4 | 1.5B или TinyLlama 1.1B |

---

## 10. Сравнение с аналогами / Competitive Comparison / 竞品对比

### [RU]

| Критерий | John Deere AI | DJI FlightHub | Home Assistant | Waters v1.0.0 |
|----------|--------------|---------------|----------------|--------------|
| Цена | $1200/год | $800/год | Бесплатно | MIT |
| Офлайн | Нет | Нет | Частично | L0-L4+DTN |
| Бортовая LLM | Нет | Нет | Нет | GGUF 1-3B |
| P2P-рой | Нет | Нет | Нет | 6 mesh |
| LLM-роутинг | Нет | Нет | local | hybrid+distr |
| Self-update safe | — | — | — | 3-агента |
| Железо | JD only | DJI only | Любое | RPi4, Jetson |

**Преимущество:** Единственная платформа с работающими агентами при полном обрыве связи (L4 → SOS).

---

## 11. Задание Конструктору / Constructor Task / 构造器任务

### 11.1 [RU] Порядок

1. **Фаза 1 (5д):** `hybrid_llm.rs` — EdgeEngine (llama-cpp-rs/candle GGUF), SwitchProtocol (L0-L4), LlmCache (Redis DB 14-15), SyncQueue (OfflineQueue)
2. **Фаза 2 (4д):** `distributed.rs` — DistributedCache (gossip), DomainRouter (ProfileConfig), TokenPool (DeepSeek Flash)
3. **Фаза 3 (3д):** `code_review.rs` — CodeSandbox, 3 SubAgent-а, CodeReviewPipeline
4. **Фаза 4 (3д):** Интеграция в main.rs, 5 тестов сценариев, документация

### 11.2 [RU] Что НЕ трогать

`a2a.rs`, `mcp_store.rs`, `media_bridge.rs`, голос, `tamagotchi.rs`, `yasa_agent.rs` (только +CMD-6)

### 11.3 [RU] Критерии приёмки

1. EdgeEngine <500ms RPi4, модель 1.5B GGUF
2. L0→L2 переключение <100ms
3. DistributedCache поиск 6 нод <200ms
4. PipelineLatency ≤ ×3 локального
5. 3 агента голосуют, 2/3 Approve → AutoDeploy
6. CodeSandbox блокирует rm -rf /
7. Бинарник <20MB (GGUF отдельно)
8. Все 36 тестов проходят

### 11.4 [RU] Формат отчёта

После каждой фазы: коммит(ы) в `waters-ai/waters-core`, diffstat, тесты, отклонения.

---

*Документ создан: agent.architect.v1 | Утверждено: 2026-05-18 | Формат: YASA Technical v2*
*Три языка: RU (основной), EN (технический), ZH (международный)*
