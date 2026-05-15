# План запуска сервера Миссии 1

## Спецификация: Hetzner AX102

| Параметр | Значение |
|----------|----------|
| **Модель** | AX102 |
| **CPU** | 12 ядер AMD |
| **RAM** | 32 GB |
| **Диск** | 2 × 512 GB NVMe (RAID1 или раздельные) |
| **Сеть** | 1 Gbps |
| **Цена** | €35/мес |
| **Локация** | Хельсинки / Нюрнберг |

## Этапы развёртывания

### Шаг 1: Аренда и базовая настройка

```bash
# 1. Заказать через Hetzner Robot или Cloud Console
# 2. Установить Ubuntu 24.04 LTS
# 3. SSH-доступ по ключу
ssh root@<IP_сервера>

# 4. Базовая безопасность
ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw enable

# 5. Системные пакеты
apt update && apt upgrade -y
apt install -y docker.io docker-compose-v2 git curl htop iotop
```

### Шаг 2: Docker Compose (инфраструктура миссии)

```yaml
# docker-compose.yml
services:
  redis:
    image: redis:7-alpine
    ports: ["6379:6379"]
    command: redis-server --appendonly yes
    volumes: [redis-data:/data]
    restart: always

  chromadb:
    image: chromadb/chroma:latest
    ports: ["8000:8000"]
    volumes: [chroma-data:/chroma/chroma]
    environment: [ALLOW_RESET=true, IS_PERSISTENT=true]
    restart: always

  lightrag:
    build: ./docker/lightrag
    ports: ["8002:8000"]
    volumes: [lightrag-data:/data]
    restart: always

  ollama:
    image: ollama/ollama:latest
    ports: ["11434:11434"]
    volumes: [ollama-data:/root/.ollama]
    restart: always
    # После запуска: ollama pull deepseek-coder-v2:7b

volumes:
  redis-data:
  chroma-data:
  lightrag-data:
  ollama-data:
```

### Шаг 3: Деплой mission-control

```bash
# 1. Клонировать форк
git clone https://github.com/waters-ai/mission-control /opt/mission-control

# 2. Собрать Rust-бинар
cd /opt/mission-control
cargo build --release

# 3. Systemd-сервис
cat > /etc/systemd/system/mission-control.service << 'EOF'
[Unit]
Description=WATERS Mission 1 Control
After=redis.service docker.service

[Service]
User=root
WorkingDirectory=/opt/mission-control
ExecStart=/opt/mission-control/target/release/mission-control
Restart=always
RestartSec=10
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable mission-control
systemctl start mission-control
```

### Шаг 4: Bridge (связь с 238)

```bash
# Отдельный systemd-сервис для bridge
cat > /etc/systemd/system/mission-bridge.service << 'EOF'
[Unit]
Description=WATERS Mission 1 Bridge → Center 238
After=network.target mission-control.service

[Service]
ExecStart=/opt/mission-control/bridge/mission-bridge
Restart=always
RestartSec=30
Environment=CENTER_URL=https://238.bridge.waters

[Install]
WantedBy=multi-user.target
EOF
```

### Шаг 5: Healthcheck

```bash
# Healthcheck endpoint: http://localhost:8080/health
# Проверяет: Redis, ChromaDB, Ollama, RLM-агенты, диск, RAM

# Prometheus (опционально) — метрики
# Grafana — дашборд миссии
```

### Шаг 6: Запуск миссии

```bash
# 1. Установить скиллы миссии
mission-control install-skill skills/meteorite-search/

# 2. Проверить MCP-серверы
mission-control check-mcp

# 3. Запустить RLM
mission-control start-mission --mode autonomous

# 4. Проверить агентов
mission-control list-agents
# DataCollectorAgent ×4  [active]
# AnalyzerAgent     ×6  [active]
# PatternMatcher    ×4  [active]
# CoordinatorAgent  ×2  [active]
```

## Бюджет

| Статья | Стоимость |
|--------|-----------|
| Сервер AX102 | €35/мес |
| Домен (если нужен) | ~€10/год |
| DeepSeek API | по usage |
| **Итого** | **~€35-40/мес** |
