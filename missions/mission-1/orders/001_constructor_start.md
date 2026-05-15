{
  "order_id": "ord_arch_001",
  "type": "mission_start",
  "issued_by": "agent.architect.v1",
  "issued_at": "2026-05-15T18:00:00Z",
  "priority": "critical",
  "ttl_seconds": 604800,
  "payload": {
    "mission": "mission-1-meteorite",
    "action": "start_implementation",
    "spec_package": "docs/mission-control/CONSTRUCTOR_PACKAGE.md",
    "architecture": "Kafka-native Agent Runtime",
    "core_language": "Rust",
    "modules": [
      {"name": "kafka.rs", "layer": "5", "desc": "Kafka consumer/producer, 6 топиков"},
      {"name": "subagent.rs", "layer": "2", "desc": "SubAgent Manager, 5 ролей, lifecycle"},
      {"name": "autonomy.rs", "layer": "2", "desc": "Autonomy Engine L0-L4, offline queue, contact window"},
      {"name": "dtn.rs", "layer": "2", "desc": "DTN-эмуляция, tc-netem, latency_plan.json"},
      {"name": "api.rs", "layer": "4", "desc": "HTTP API (axum), /healthz, /api/v1/*"},
      {"name": "mcp.rs", "layer": "3", "desc": "MCP Client (stdio), запуск внешних серверов"}
    ],
    "edge_modules": [
      {"name": "rank_engine.py", "lang": "Python", "desc": "KPI → ранг, адаптация h2o-mining.space"},
      {"name": "mcp-nasa-fireball", "lang": "Python", "desc": "MCP-сервер NASA Fireball API"},
      {"name": "mcp-spectra", "lang": "Python", "desc": "MCP-сервер спектральных БД"},
      {"name": "mcp-trajectory", "lang": "Python", "desc": "MCP-сервер расчёта траекторий"}
    ],
    "server": {
      "provider": "Hetzner AX52",
      "cost": "€35/мес",
      "cpu": "8 vCPU",
      "ram": "32 GB",
      "ssd": "2×1 TB NVMe",
      "os": "Ubuntu 24.04 LTS"
    },
    "deadline": "2026-05-28"
  }
}
