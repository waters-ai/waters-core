# Установка и запуск Mission Control

---

## 1. Требования к серверу

### Минимальные

| Параметр | Значение |
|----------|----------|
| CPU | 2 vCPU |
| RAM | 4 GB |
| SSD | 20 GB |
| ОС | Ubuntu 24.04 LTS |
| Docker | 27.x |
| Python | 3.12 |
| Rust | 2024 stable (для mcp-trajectory) |

### Рекомендуемые

| Параметр | Значение |
|----------|----------|
| CPU | 4 vCPU |
| RAM | 8 GB |
| SSD | 50 GB |

---

## 2. Установка

### 2.1. Через pip (ядро)

```bash
pip install mission-control
```

### 2.2. Из исходников

```bash
git clone https://github.com/waters-ai/mission-control.git
cd mission-control

# Установить Python-ядро
pip install -r requirements.txt

# Собрать MCP-сервер trajectory (Rust)
cd mcp-servers/mcp-trajectory && cargo build --release

# Вернуться
cd ../../

# Установить SKILL.md
mission-control install-skill skills/meteorite-explorer/
mission-control install-skill skills/meteorite-collector/
mission-control install-skill skills/meteorite-analyzer/
mission-control install-skill skills/mission-coordinator/
```

---

## 3. Настройка

### 3.1. Создать .env

```bash
cat > ~/.mission-control/.env << 'EOF'
DEEPSEEK_API_KEY=sk-...
NASA_API_KEY=...
KAFKA_BROKER=238.hub.waters:9092
MISSION_ID=mission-1
EOF
```

### 3.2. Настроить конфиг

```bash
cp config/mission.example.toml /etc/mission-control/mission.toml
cp config/mcp.example.json ~/.mission-control/mcp.json
```

### 3.3. Настроить DTN (опционально)

```bash
# Разрешить tc qdisc без sudo
sudo setcap cap_net_admin+ep $(which tc)

# Или через systemd: dtn.service (запускается от root)
```

---

## 4. Запуск

### 4.1. Запуск миссии

```bash
mission-control start
```

### 4.2. Запуск в Docker

```bash
docker-compose up -d
```

### 4.3. Проверка статуса

```bash
mission-control status
# Миссия: mission-1
# Уровень: L0
# Агентов: 12/16 активно
# Находок: 47
# Uptime: 3h 12m
```

---

## 5. Docker Compose

```yaml
services:
  redis:
    image: redis:7-alpine
    ports: ["6379:6379"]

  mission-control:
    build: .
    depends_on: [redis]
    env_file: .env
    volumes:
      - ./config:/etc/mission-control
      - ./skills:/opt/mission-control/skills
    network_mode: host  # для DTN (tc-netem)
```

---

## 6. Проверка после установки

```bash
# 1. Проверить конфигурацию
mission-control doctor

# 2. Проверить Kafka
mission-control doctor --kafka

# 3. Проверить MCP-серверы
mission-control doctor --mcp

# 4. Проверить ранги
mission-control doctor --ranks
```
