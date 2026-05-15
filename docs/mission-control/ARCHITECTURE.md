# Архитектура Mission Control

**Статус:** v1.0 • Фаза проектирования
**Форк:** DeepSeek TUI v0.8.37 (MIT) — адаптация для WATERS Mission 1

---

## 1. Обзор

Mission Control — это агентная система реального времени для автономных миссий. Она берёт концепцию **sub-agent'ов** из DeepSeek TUI, заменяет HTTP bridge на **Kafka**, добавляет **систему рангов** (адаптированную из [h2o-mining.space](https://h2o-mining.space)) и **DTN-эмуляцию** для реалистичных космических задержек.

```
┌──────────────────────────────────────────────────────────────────┐
│                      MISSION CONTROL                             │
│                                                                  │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌──────────┐  │
│  │ Kafka Layer│  │ Sub-agent  │  │   MCP      │  │  Rank    │  │
│  │ Consumer   │  │ Manager    │  │   Client   │  │  Engine  │  │
│  │ Producer   │  │ 5 ролей    │  │   Registry │  │  4 уровня│  │
│  │ Heartbeat  │  │ Lifecycle  │  │   Servers  │  │  KPI→rank│  │
│  └────────────┘  └────────────┘  └────────────┘  └──────────┘  │
│                                                                  │
│  ┌────────────┐  ┌────────────┐  ┌──────────────────────────┐   │
│  │ Autonomy   │  │   DTN      │  │  Config                  │   │
│  │ Engine L0-4│  │ tc-netem   │  │  mission.toml            │   │
│  │            │  │ latency_plan│  │  Kafka config            │   │
│  └────────────┘  └────────────┘  └──────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. Модули

### 2.1. Kafka Layer (`kafka_client.py`)

Два независимых потока:

```
Consumer Thread (orders):
  Kafka topic: mission.1.orders.v1
  → validate schema
  → deserialize order
  → push to Sub-agent Manager

Producer Thread (findings + heartbeat):
  Buffer: findings_queue (in-memory + Redis fallback)
  → batch отправка каждые N секунд
  → Kafka topics: findings.v1, agents.v1, heartbeat.v1, ranks.v1
```

**Параметры Kafka:**
- `bootstrap.servers`: 238.hub.waters:9092
- `group.id`: mission-1-consumer
- `auto.offset.reset`: earliest
- `enable.auto.commit`: true (auto.commit.interval.ms: 5000)

### 2.2. Sub-agent Manager (`subagent_manager.py`)

Управляет жизненным циклом sub-agent'ов:

```python
roles = {
    "explorer":    {"skill": "meteorite-explorer",   "max_concurrent": 4},
    "collector":   {"skill": "meteorite-collector",  "max_concurrent": 4},
    "analyzer":    {"skill": "meteorite-analyzer",   "max_concurrent": 2},
    "matcher":     {"skill": "meteorite-matcher",    "max_concurrent": 2},
    "coordinator": {"skill": "mission-coordinator",  "max_concurrent": 1},
}
```

**Lifecycle:**
```
PENDING ← order получен
  → ACTIVE ← sub-agent запущен
    → WORKING ← LLM вызывает MCP tools
      → COMPLETED → finding опубликован в Kafka
      → FAILED → retry (max 3) → ESCALATED
  → IDLE ← ожидание следующего order
```

### 2.3. MCP Client (`mcp_client.py`)

Заимствован из TUI, адаптирован под наш registry:

```python
mcp_servers = {
    "mcp-nasa-fireball": {
        "transport": "stdio",
        "command": "python3",
        "args": ["mcp-servers/mcp-nasa-fireball/server.py"]
    },
    "mcp-spectra": {
        "transport": "stdio",
        "command": "python3",
        "args": ["mcp-servers/mcp-spectra/server.py"]
    },
    "mcp-trajectory": {
        "transport": "stdio",
        "command": "python3",
        "args": ["mcp-servers/mcp-trajectory/server.py"]
    }
}
```

### 2.4. Rank Engine (`rank_engine.py`)

Адаптация MemRankConfig из Solana-контракта [h2o-mining.space](https://h2o-mining.space):

```python
class RankConfig:
    ranks = {
        1: {"name": "Bronze",  "min_findings": 10,  "min_confidence": 0.5,  "min_uptime_h": 0},
        2: {"name": "Silver",  "min_findings": 50,  "min_confidence": 0.7,  "min_uptime_h": 24},
        3: {"name": "Gold",    "min_findings": 200, "min_confidence": 0.85, "min_uptime_h": 168},
        4: {"name": "Platinum","min_findings": 1000,"min_confidence": 0.95, "min_uptime_h": 720},
    }
```

Триггер: каждые 10 findings → пересчёт KPI → публикация `mission.1.ranks.v1`.

### 2.5. Autonomy Engine (`autonomy.py`)

Определяет уровень миссии:

```python
def determine_level(kafka_connected, llm_type, energy_ok):
    if kafka_connected and llm_type == "deepseek":
        return L0  # Полная мощность
    if kafka_connected and llm_type == "ollama":
        return L1  # Экономия API
    if not kafka_connected and llm_type == "ollama":
        return L2  # Автономный
    if not kafka_connected and llm_type == "ollama_mini":
        return L3  # Энергосбережение
    return L4  # Safe mode
```

### 2.6. DTN Module (`dtn.sh`)

```bash
# Применение задержки (tc-netem)
dtn apply lunar   # 1.3s delay, 50ms jitter
dtn apply field   # 100ms delay, 10ms jitter
dtn remove        # сброс
```

---

## 3. Потоки данных

```
                    ═══════ KAFKA ═══════
                         │       ▲
                  orders │       │ findings
                         ▼       │
              ┌───────────────────────┐
              │   Coordinator Agent   │
              │   (1 instance)        │
              └───────┬───────────────┘
                      │ распределяет задачи
         ┌────────────┼────────────┐
         ▼            ▼            ▼
  ┌──────────┐ ┌──────────┐ ┌──────────┐
  │ Explorer │ │ Collector│ │ Analyzer │
  │ × N      │ │ × N      │ │ × N      │
  └────┬─────┘ └────┬─────┘ └────┬─────┘
       │            │            │
       ▼            ▼            ▼
  ┌───────────────────────────────────┐
  │          MCP-серверы              │
  │  mcp-nasa  mcp-spectra  mcp-traj  │
  └───────────────────────────────────┘
       │            │            │
       ▼            ▼            ▼
  ┌───────────────────────────────────┐
  │        LLM (DeepSeek / Ollama)    │
  └───────────────────────────────────┘
       │
       ▼
  ┌───────────────────────────────────┐
  │        Rank Engine                │
  │  KPI → проверка ранга → Kafka    │
  └───────────────────────────────────┘
```

---

## 4. Отличия от DeepSeek TUI

| Параметр | TUI (оригинал) | Mission Control (форк) |
|----------|---------------|----------------------|
| **Архитектура** | Одноагентный TUI | Swarm из N sub-agent'ов |
| **Коммуникация** | HTTP Runtime API | **Kafka** (6 топиков) |
| **Память** | memory.md (файл) | ChromaDB + LightRAG на 238 |
| **Брокер сообщений** | Нет | **Kafka** (orders/findings) |
| **LLM** | DeepSeek только | DeepSeek + Ollama fallback |
| **Автономия** | Нет | L0-L4 (встроенная) |
| **Ранги** | Нет | 4 личных + 4 командных |
| **DTN** | Нет | tc-netem (задержки) |
| **MCP** | Встроенный клиент | Клиент + кастомные серверы |
| **Деплой** | Рабочая станция | Выделенный сервер + Docker |
