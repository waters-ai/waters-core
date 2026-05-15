# Конфигурация Mission Control

**Источник:** DeepSeek TUI CONFIGURATION.md — адаптировано для WATERS Mission 1

---

## 1. Конфигурационные файлы

| Файл | Назначение | Lokacija |
|------|-----------|----------|
| `mission.toml` | Конфигурация миссии | `/etc/mission-control/mission.toml` |
| `mcp.json` | MCP-серверы | `~/.mission-control/mcp.json` |
| `latency_plan.json` | DTN-профили | `/etc/mission-control/latency_plan.json` |
| `.env` | Секреты (API-ключи) | `~/.mission-control/.env` |

---

## 2. mission.toml

```toml
[mission]
id = "mission-1"
name = "Meteorite Search Earth"
mode = "autonomous"  # autonomous | manual | test
level = "L0"         # L0 | L1 | L2 | L3 | L4

[kafka]
bootstrap_servers = "238.hub.waters:9092"
consumer_group = "mission-1-consumer"
auto_offset_reset = "earliest"
findings_batch_size = 10
findings_batch_interval_sec = 60
heartbeat_interval_sec = 60

[agents]
max_concurrent = 10
default_llm = "deepseek-v4-flash"
fallback_llm = "ollama/qwen2.5:7b"
findings_queue_size = 1000
retry_max = 3
timeout_sec = 300

[ranks]
enabled = true
config_topic = "mission.1.config.v1"
check_interval_findings = 10

[dtn]
enabled = true
interface = "eth0"
default_profile = "field"

[redis]
host = "localhost"
port = 6379
db = 0
findings_queue_key = "mission:1:findings:queue"
```

---

## 3. mcp.json

```json
{
  "mcpServers": {
    "mcp-nasa-fireball": {
      "command": "python3",
      "args": ["mcp-servers/mcp-nasa-fireball/server.py"],
      "env": {
        "NASA_API_KEY": "{env:NASA_API_KEY}"
      },
      "timeout": 30000
    },
    "mcp-spectra": {
      "command": "python3",
      "args": ["mcp-servers/mcp-spectra/server.py"],
      "timeout": 15000
    },
    "mcp-trajectory": {
      "command": "python3",
      "args": ["mcp-servers/mcp-trajectory/server.py"],
      "timeout": 60000
    }
  }
}
```

---

## 4. latency_plan.json

```json
{
  "interface": "eth0",
  "profiles": {
    "field":  { "delay_ms": 100,  "jitter_ms": 10,  "loss": 0.001 },
    "lunar":  { "delay_ms": 650,  "jitter_ms": 50,  "loss": 0.001 },
    "mars":   { "delay_ms": 5000, "jitter_ms": 500, "loss": 0.01 },
    "deep":   { "delay_ms": 60000, "jitter_ms": 5000, "loss": 0.05 }
  },
  "schedule": [
    { "time": "00:00", "profile": "field", "duration_h": 20 },
    { "time": "20:00", "profile": "lunar", "duration_h": 4 }
  ]
}
```

---

## 5. Переменные окружения

| Переменная | Назначение | По умолчанию |
|-----------|-----------|-------------|
| `MISSION_ID` | ID миссии | `mission-1` |
| `KAFKA_BROKER` | Kafka bootstrap | `localhost:9092` |
| `DEEPSEEK_API_KEY` | DeepSeek API ключ | — |
| `NASA_API_KEY` | NASA API ключ | — |
| `OLLAMA_HOST` | Ollama хост | `localhost` |
| `OLLAMA_MODEL` | Ollama модель | `qwen2.5:7b` |
| `REDIS_HOST` | Redis хост | `localhost` |
| `MISSION_LOG_LEVEL` | Уровень логирования | `INFO` |
