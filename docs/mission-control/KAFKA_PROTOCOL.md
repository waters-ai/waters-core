# Kafka-протокол Mission Control

---

## 1. Топики

| Топик | Ключ | Retention | Партиции | Описание |
|-------|------|-----------|----------|----------|
| `mission.1.orders.v1` | `order_<uuid>` | 7 дней | 1 | Приказы с 238 на миссию |
| `mission.1.findings.v1` | `finding_<uuid>` | 90 дней | 3 | Находки с миссии на 238 |
| `mission.1.agents.v1` | `agent_<id>` | 7 дней (compact) | 1 | Статус sub-agent'ов |
| `mission.1.ranks.v1` | `rank_<agent_id>` | ∞ (log compact) | 1 | События рангов |
| `mission.1.heartbeat.v1` | `mission_1` | 24 часа | 1 | Healthcheck миссии |
| `mission.1.config.v1` | `<config_key>` | ∞ (log compact) | 1 | Конфигурация (имитация PDA) |

---

## 2. JSON-схемы

### 2.1. orders.v1

```json
{
  "order_id": "ord_001",
  "type": "search|recalibrate|pause|resume|shutdown|reconfigure",
  "issued_by": "agent.architect.v1",
  "issued_at": "2026-05-15T12:00:00Z",
  "payload": {},
  "priority": "low|normal|high|critical",
  "ttl_seconds": 3600
}
```

### 2.2. findings.v1

```json
{
  "finding_id": "f_002",
  "type": "meteorite_candidate|anomaly|spectrum|trajectory|status",
  "source_agent": "explorer.1",
  "source_skill": "meteorite-explorer",
  "found_at": "2026-05-15T12:34:56Z",
  "confidence": 0.87,
  "rank": 2,
  "data": {}
}
```

### 2.3. agents.v1

```json
{
  "agent_id": "explorer.1",
  "role": "explorer",
  "skill": "meteorite-explorer",
  "status": "idle|working|error|offline",
  "current_task": "ord_001",
  "findings_count": 47,
  "avg_confidence": 0.74,
  "uptime_seconds": 3600,
  "rank": 2,
  "level": "L0",
  "memory_usage_mb": 124
}
```

### 2.4. ranks.v1

```json
{
  "agent_id": "explorer.1",
  "event": "upgraded|demoted|unchanged",
  "type": "personal|team",
  "old_rank": 1,
  "new_rank": 2,
  "trigger": "findings_count >= 50 && avg_confidence >= 0.70",
  "metrics": {"findings": 52, "avg_confidence": 0.74, "uptime_hours": 12},
  "timestamp": "2026-05-15T12:00:00Z"
}
```

### 2.5. heartbeat.v1

```json
{
  "mission_id": "mission-1",
  "level": "L0|L1|L2|L3|L4",
  "agents_active": 12,
  "agents_total": 16,
  "findings_total": 47,
  "uptime_seconds": 3600,
  "memory_usage_pct": 0.6,
  "kafka_lag": 0,
  "llm": "deepseek-v4-flash",
  "last_order_processed": "ord_001",
  "errors_last_hour": 0
}
```

### 2.6. config.v1

```json
{
  "type": "rank_config|mission_config|dtn_config",
  "ranks": [ ... ],       // если type = rank_config
  "mission": { ... },      // если type = mission_config
  "dtn": { ... }           // если type = dtn_config
}
```

---

## 3. Consumer/Producer конфигурация

### Consumer (миссионный сервер)

```python
consumer_config = {
    "bootstrap.servers": "238.hub.waters:9092",
    "group.id": "mission-1-consumer",
    "auto.offset.reset": "earliest",
    "enable.auto.commit": True,
    "auto.commit.interval.ms": 5000,
    "max.poll.interval.ms": 300000,
}
```

### Producer (миссионный сервер)

```python
producer_config = {
    "bootstrap.servers": "238.hub.waters:9092",
    "acks": "all",          # гарантия доставки
    "retries": 3,
    "linger.ms": 100,       # batch для производительности
}
```
