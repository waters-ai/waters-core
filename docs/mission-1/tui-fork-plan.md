# План форка DeepSeek-TUI → mission-control

## Что берём из TUI (база)

| Компонент | Описание | Что делаем |
|-----------|----------|------------|
| **RLM (Pro + Flash)** | Иерархическая система Pro-оркестратор + N Flash-субагентов | Оставляем ядро, меняем типы агентов |
| **Durable Task Queue** | RocksDB — задачи переживают рестарт | Оставляем как есть |
| **HTTP Runtime API** | REST API для управления TUI | Дорабатываем — миссионные endpoint'ы |
| **MCP Client** | Подключение внешних MCP-серверов | Оставляем, расширяем registry |
| **Skills System** | SKILL.md → инструкции агентам | Перерабатываем под наш формат (TOML + YAML конвертеры) |
| **Plan / Agent / YOLO** | Три режима работы | Оставляем, добавляем Mission Mode |

## Что перерабатываем (наша надстройка)

### 1. Типизированные агенты вместо универсальных Flash

```
TUI: FlashAgent (универсальный, skill-driven)
     └── читает SKILL.md → действует

Наш: DataCollectorAgent / AnalyzerAgent / PatternMatcherAgent / CoordinatorAgent
     └── свой Rust-код + свой lifecycle + свой протокол
     └── общаются через Redis pub/sub (JSON)
     └── пишут в ChromaDB/LightRAG напрямую
```

### 2. Redis pub/sub как шина между агентами

```
TUI: Pro → Flash — прямой вызов (Rust fn)
     Flash → Flash — только через Pro

Наш: Pro → Flash — прямой вызов + дубль в Redis
     Flash → Flash — Redis pub/sub (JSON)
     Flash → всё — Redis каналы:
       - mission:state      — состояние миссии
       - mission:findings   — новые находки
       - mission:orders     — приказы
       - agent:heartbeat    — healthcheck агентов
```

### 3. Память (Memory Layer)

```
TUI: нет постоянной памяти
     └── только RocksDB очередь

Наш: тройной стек памяти:
     └── Redis (быстрое состояние)
     └── ChromaDB (векторная — аномалии, паттерны)
     └── LightRAG (графовая — связи метеоритов, траекторий, классификаций)
```

### 4. Mission Mode (новый режим)

```
Режимы TUI: Plan / Agent / YOLO
Наш режим:  Mission — RLM работает автономно 24/7
            └── не ждёт команд от человека
            └── сам принимает решения в рамках миссии
            └── отчитывается в центр (через bridge)
            └── сам себя восстанавливает (healthcheck → restart)
```

### 5. Адаптер/мост в центр

```
TUI → Центр 238:
  └── HTTP bridge (наш процесс)
  └── читает Redis миссии
  └── упаковывает findings в JSON
  └── шлёт HTTP POST на 238
  └── 238 кладёт в Kafka: mission.1.findings.v1

Центр 238 → TUI:
  └── Kafka: mission.1.orders.v1
  └── HTTP bridge на 238 читает Kafka
  └── кладёт orders в Redis миссии
```

## Структура форка

```
mission-control/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── rlm/
│   │   ├── mod.rs
│   │   ├── pro.rs              — RLM Pro (оркестратор)
│   │   ├── flash.rs            — базовый Flash-агент
│   │   ├── agents/
│   │   │   ├── collector.rs    — DataCollectorAgent
│   │   │   ├── analyzer.rs     — AnalyzerAgent
│   │   │   ├── matcher.rs      — PatternMatcherAgent
│   │   │   └── coordinator.rs  — CoordinatorAgent
│   │   └── mission.rs          — Mission Mode
│   ├── skills/
│   │   ├── registry.rs         — реестр скиллов
│   │   ├── store.rs            — загрузка/сохранение
│   │   └── converters/
│   │       ├── from_tui.rs     — TUI .skill.md → наш TOML
│   │       ├── from_opencode.rs
│   │       ├── to_tui.rs
│   │       └── to_opencode.rs
│   ├── mcp/
│   │   ├── client.rs           — MCP-клиент
│   │   ├── registry.rs         — реестр серверов
│   │   └── servers/
│   │       ├── nasa.rs
│   │       ├── spectra.rs
│   │       └── trajectory.rs
│   ├── memory/
│   │   ├── redis.rs            — Redis + pub/sub
│   │   ├── chroma.rs           — ChromaDB client
│   │   └── lightrag.rs         — LightRAG client
│   ├── bridge/
│   │   ├── mod.rs              — адаптер-мост в центр
│   │   ├── sender.rs           — findings → 238
│   │   └── receiver.rs         — orders от 238
│   └── api/
│       ├── mission.rs          — Mission API endpoint'ы
│       ├── agents.rs           — управление агентами
│       └── health.rs           — healthcheck
├── skills/                     — скиллы миссии (установленные)
│   ├── meteorite-search/
│   │   ├── skill.toml
│   │   ├── INSTRUCTIONS.md
│   │   └── mcp-requires.toml
│   └── ...
├── mcp-servers/                — MCP-серверы
│   ├── mcp-nasa/
│   └── ...
└── docker-compose.yml          — Kafka, Redis, ChromaDB, LightRAG
```

## Этапы реализации форка

| Фаза | Что делаем | Результат |
|------|------------|-----------|
| **0** | Форк TUI, структура проекта, Cargo.toml, базовый RLM | Компилируется, запускается 1 Flash |
| **1** | Redis pub/sub + типизированные агенты | 4 типа агентов общаются через Redis |
| **2** | MCP-клиент + первые серверы (NASA) | Агенты собирают реальные данные |
| **3** | Memory layer (ChromaDB + LightRAG) | Аномалии пишутся в вектора |
| **4** | Bridge → 238 | Findings уходят в центр |
| **5** | Mission Mode + автономность | Работает 24/7 без человека |
