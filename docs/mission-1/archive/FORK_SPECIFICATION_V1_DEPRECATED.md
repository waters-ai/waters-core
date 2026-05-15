# Спецификация форка DeepSeek-TUI → mission-control

**Статус:** Проектирование
**Дата:** 2026-05-15
**Автор:** Архитектор v1.0
**Для кого:** Конструктор Сети — исполнитель форка

---

## 1. Стратегия форка

### 1.1. Принцип: минимальный форк, максимальная конфигурация

НЕ форкаем то, что можно сконфигурировать. Форкаем ТОЛЬКО то, что нужно изменить.

| Компонент TUI | Действие | Обоснование |
|--------------|----------|-------------|
| RLM Pro (оркестратор) | **Оставить как есть** | Универсальный, skills-driven |
| FlashAgent (базовый) | **Оставить как есть** | Управляется SKILL.md |
| Durable Task Queue (RocksDB) | **Оставить как есть** | Работает, менять не нужно |
| HTTP Runtime API | **Оставить как есть** | Берём наш Mission API как route |
| MCP Client | **Оставить как есть** | Полностью устраивает |
| Skills System (SKILL.md) | **Оставить как есть** | Добавляем парсер TOML |
| Plan / Agent / YOLO | **Оставить как есть** | Добавляем Mission Mode |

**Меняем только:**
1. Добавляем модуль `mission_mode.rs` (новый режим работы 24/7)
2. Добавляем модуль `dtn.rs` (эмуляция задержек)
3. Добавляем Redis pub/sub интеграцию (вместо прямых вызовов между Flash)
4. Добавляем HTTP bridge → 238 (отправка findings)
5. Конвертеры SKILL.md ↔ TOML

### 1.2. Структура репозитория

```
mission-control/                    # Новый репозиторий (НЕ форк waters-core)
├── Cargo.toml                      # Зависимости: TUI (git = "..."), redis-rs, reqwest, ...
├── src/
│   ├── main.rs                     # Точка входа
│   ├── mission_mode.rs             # ★ РЕЖИМ МИССИИ (новый код)
│   ├── dtn.rs                      # ★ ЭМУЛЯЦИЯ ЗАДЕРЖЕК (новый код)
│   ├── redis_bus.rs                # ★ REDIS PUB/SUB (новый код)
│   ├── bridge.rs                   # ★ HTTP BRIDGE → 238 (новый код)
│   ├── skills/
│   │   ├── registry.rs             # Реестр скиллов (из TUI, с доработкой)
│   │   └── converters/
│   │       ├── from_tui.rs         # TUI .skill.md → TOML
│   │       └── from_opencode.rs    # OpenCode SKILL.md → TOML
│   └── api/
│       ├── mission.rs              # ★ Mission API (новые endpoint'ы)
│       └── agents.rs               # Управление агентами (из TUI)
├── skills/                         # Наши скиллы в TOML
│   ├── meteorite-search/
│   │   ├── skill.toml
│   │   └── INSTRUCTIONS.md
│   └── ...
├── mcp-servers/                    # Наши MCP-серверы
│   ├── mcp-nasa-fireball/
│   ├── mcp-spectra/
│   └── mcp-trajectory/
├── latency_plan.json               # DTN-план (настройка tc-netem)
└── docker-compose.yml              # Redis + Ollama + mission-control
```

## 2. Модуль mission_mode.rs (ключевой)

### 2.1. Режимы работы

```rust
enum MissionLevel {
    L0, // Полная мощность: DeepSeek API + sat-link
    L1, // Экономия: Ollama + sat-link
    L2, // Автономный: Ollama, нет sat-link
    L3, // Энергосбережение: только критические агенты
    L4, // Safe mode: только логирование
}
```

### 2.2. Логика автономии

```
// Псевдокод mission_mode.rs
Loop {
    heartbeat = send_heartbeat_to_238();

    match heartbeat {
        Ok(response) if response.orders.is_some() => {
            level = L0; // Есть связь, выполняем приказы
            execute_orders(response.orders);
        }
        Ok(_) => {
            level = L0; // Есть связь, работаем штатно
        }
        Err(timeout) if timeout > 30.seconds() => {
            level = L2; // Нет связи 30 секунд — автономный режим
            findings_queue.push(current_findings); // Копим находки
        }
        Err(timeout) if timeout > 300.seconds() => {
            level = L3; // 5 минут без связи — экономим энергию
            deactivate_non_critical_agents();
        }
    }

    // При восстановлении связи — отправляем всё накопленное
    if connection_restored && !findings_queue.is_empty() {
        bridge::send_batch(findings_queue.drain());
    }

    sleep(60.seconds()); // heartbeat раз в минуту
}
```

### 2.3. Recovery после падения

RocksDB хранит очередь задач. При рестарте RLM Pro:
1. Читает RocksDB — восстанавливает pending-задачи
2. Проверяет heartbeat к 238 — определяет уровень (L0 с联系+ или L2 без)
3. Продолжает с того же места (без потери контекста)

## 3. Модуль dtn.rs

### 3.1. Применение latency_plan.json

```json
{
  "plan_id": "latency_plan_v1",
  "description": "Эмуляция задержек для Миссии 1",
  "interface": "eth0",
  "default_delay_ms": 100,
  "default_jitter_ms": 20,
  "levels": {
    "lunar":  { "delay_ms": 650,  "jitter_ms": 50,  "loss": 0.001 },
    "mars":   { "delay_ms": 5000, "jitter_ms": 500, "loss": 0.01 },
    "field":  { "delay_ms": 100,  "jitter_ms": 10,  "loss": 0.001 }
  },
  "schedule": [
    { "time": "00:00", "level": "field", "duration_hours": 20 },
    { "time": "20:00", "level": "lunar", "duration_hours": 4 }
  ]
}
```

### 3.2. API dtn.rs

```rust
pub fn apply_latency(plan: &LatencyPlan) -> Result<(), DtnError>;
pub fn remove_latency(interface: &str) -> Result<(), DtnError>;
pub fn current_latency() -> Result<LatencyStatus, DtnError>;
// Вызов sudo tc qdisc ... через Command
```

## 4. Модуль redis_bus.rs

### 4.1. Каналы

| Канал | Назначение | Pub | Sub |
|-------|-----------|-----|-----|
| `agent:heartbeat` | Healthcheck | Все агенты | RLM Pro, CoordinatorAgent |
| `mission:state` | Состояние миссии | CoordinatorAgent | Все |
| `mission:findings` | Новые находки | AnalyzerAgent, PatternMatcherAgent | CoordinatorAgent → bridge |
| `mission:orders` | Приказы | RLM Pro | Все агенты |

### 4.2. Интеграция с FlashAgent

FlashAgent из TUI получает задачу от RLM Pro. В mission-control мы ДОБАВЛЯЕМ:
- После выполнения задачи FlashAgent публикует результат в `mission:findings`
- FlashAgent подписан на `mission:orders` — может получить новый приказ в реальном времени
- heartbeat раз в 10 секунд

**Всё это — в SKILL.md агента, не в коде FlashAgent.**

## 5. HTTP bridge → 238

### 5.1. Протокол

```rust
// bridge.rs
pub struct BridgeConfig {
    pub hub_url: String,          // "https://238.hub.waters"
    pub mission_id: String,       // "mission-1"
    pub findings_endpoint: String, // "/mission/1/findings"
    pub orders_endpoint: String,   // "/mission/1/orders"
    pub api_key: String,
    pub heartbeat_interval: u64,  // 60 секунд
}
```

### 5.2. Логика отправки

```
Queue (in-memory + RocksDB fallback)
  ↓
Sender thread (каждые N секунд)
  ├── heartbeat (healthcheck миссии)
  ├── findings (batch, если есть)
  └── flush на диск при failure
```

## 6. API endpoint'ы mission-control

### 6.1. Mission API (новые)

| Метод | Путь | Назначение |
|-------|------|------------|
| `GET` | `/api/v1/mission/status` | Состояние миссии (уровень, агенты, uptime) |
| `GET` | `/api/v1/mission/findings?since=<ts>` | Список находок |
| `POST` | `/api/v1/mission/order` | Принять приказ от 238 |
| `GET` | `/api/v1/mission/heartbeat` | Отдать heartbeat |
| `GET` | `/api/v1/agents` | Список агентов |
| `POST` | `/api/v1/agents/{id}/restart` | Перезапустить агента |
| `GET` | `/api/v1/dtn/status` | Статус DTN-эмуляции |
| `POST` | `/api/v1/dtn/apply` | Применить latency_plan |

### 6.2. Эти же endpoint'ы дублируются в HTTP Runtime API (из TUI)

## 7. Конвертеры скиллов

### 7.1. TUI .skill.md → TOML

```rust
// from_tui.rs
// Парсит .skill.md:
//   ---
//   name: "meteorite-search"
//   model: "deepseek-v4-flash"
//   ---
//   # Инструкция
//   ...
// → skill.toml + INSTRUCTIONS.md
```

### 7.2. OpenCode SKILL.md → TOML

```rust
// from_opencode.rs
// Парсит SKILL.md:
//   ---
//   name: architect-self
//   description: ...
//   ---
//   # Инструкция
//   ...
// → skill.toml + INSTRUCTIONS.md (+ checksum источника)
```

**Оба конвертера — read-only.** Они не пишут обратно. Оригинальные .md файлы остаются как primary source.

## 8. Этапы реализации форка

### Фаза 0: Скелет (1-2 дня)

| Задача | Файл | Оценка |
|--------|------|--------|
| Создать Cargo.toml, git init | `Cargo.toml` | 1ч |
| Добавить TUI как зависимость (git) | `Cargo.toml` | 1ч |
| main.rs: запуск TUI с нашими параметрами | `src/main.rs` | 2ч |
| Создать пустой mission_mode.rs | `src/mission_mode.rs` | 1ч |
| Убедиться что компилируется и запускается 1 Flash | — | 2ч |

**Итого:** ~7 часов

### Фаза 1: Redis bus + Mission Mode (2-3 дня)

| Задача | Файл | Оценка |
|--------|------|--------|
| redis_bus.rs: подключение к Redis, pub/sub | `src/redis_bus.rs` | 4ч |
| heartbeat агентов через Redis | `src/redis_bus.rs` | 2ч |
| mission_mode.rs: L0-L2, heartbeat к 238 | `src/mission_mode.rs` | 6ч |
| Интеграция с FlashAgent (SKILL.md через Redis) | — | 3ч |
| Тест: 4 агента общаются через Redis | — | 2ч |

**Итого:** ~17 часов

### Фаза 2: MCP + Bridge (2-3 дня)

| Задача | Файл | Оценка |
|--------|------|--------|
| bridge.rs: HTTP клиент к 238 | `src/bridge.rs` | 3ч |
| Очередь findings (in-memory + RocksDB) | `src/bridge.rs` | 4ч |
| MCP-сервер mcp-nasa-fireball | `mcp-servers/mcp-nasa-fireball/` | 4ч |
| MCP-сервер mcp-spectra | `mcp-servers/mcp-spectra/` | 3ч |
| MCP-сервер mcp-trajectory | `mcp-servers/mcp-trajectory/` | 4ч |
| Тест: сбор данных через MCP → findings → bridge | — | 2ч |

**Итого:** ~20 часов

### Фаза 3: DTN + Skills + Website (3-4 дня)

| Задача | Файл | Оценка |
|--------|------|--------|
| dtn.rs: tc-netem wrapper | `src/dtn.rs` | 3ч |
| latency_plan.json | `latency_plan.json` | 1ч |
| Конвертеры скиллов (TUI + OpenCode) | `src/skills/converters/` | 4ч |
| Skill: meteorite-search v1 | `skills/meteorite-search/` | 2ч |
| Mission API endpoint'ы | `src/api/mission.rs` | 4ч |
| Integration test: полный цикл | — | 4ч |
| Dockerfile + docker-compose | — | 2ч |

**Итого:** ~20 часов

### Фаза 4: Полировка (1-2 дня)

| Задача | Оценка |
|--------|--------|
| README.md, документация | 2ч |
| CI/CD (GitHub Actions) | 2ч |
| Тест на 238 в Docker | 3ч |
| Развёртывание на миссионном сервере | 3ч |
| Демо: полный цикл миссии | 2ч |

**Итого:** ~12 часов

### Общая оценка: ~76 часов (~10 рабочих дней для одного разработчика)

## 9. Критические решения по реализации

1. **TUI как зависимость (git), НЕ как подмодуль.** `Cargo.toml`:
   ```toml
   [dependencies]
   deepseek-tui = { git = "https://github.com/deepseek-ai/tui", branch = "main" }
   ```
   Никакого ручного копирования кода TUI. Только наши новые модули.

2. **SKILL.md — единственный способ управлять агентом.** Никаких жёстко закодированных типов агентов. Всё поведение — через инструкции.

3. **Никакого Neo4j, TimescaleDB, Qdrant.** Только Redis + RocksDB на миссионном сервере. ChromaDB + LightRAG на 238.

4. **Два конвертера скиллов — read-only.** Конвертеры не пишут обратно в исходный формат.

5. **RocksDB — fallback для bridge.** Если HTTP к 238 недоступен, findings пишутся в RocksDB и отправляются batch при восстановлении.
