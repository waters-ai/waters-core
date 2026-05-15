# Правила работы с Kafka для Конструктора

**Версия:** 2.0
**Дата:** 2026-05-15

---

## 1. Проблемы текущей конфигурации

### 1.1. `KAFKA_ADVERTISED_LISTENERS` сломает удалённое подключение

Было:
```
KAFKA_ADVERTISED_LISTENERS: "PLAINTEXT://localhost:9092"
```

Это работает ТОЛЬКО когда клиент на том же хосте. Если клиент:
- в другом Docker-контейнере — не работает
- на другой машине (миссионный сервер → 238) — не работает

### 1.2. `apache/kafka:latest` — плавающий образ

`latest` может измениться между запусками. Всегда фиксировать версию.

### 1.3. Нет `.env` с Kafka-конфигом

Переменные в `.env` не заданы — клиенты не знают куда подключаться.

---

## 2. Топология Kafka

```
ХАБ 238 (Kafka брокер)                        Миссионный сервер (клиент)
┌────────────────────────┐                    ┌────────────────────────┐
│  waters-kafka:9092     │◄── (через сеть) ──│  mission-core (Rust)   │
│                        │                    │  KAFKA_BROKER=         │
│  Топики:               │                    │  171.22.180.238:9092   │
│  mission.1.orders.v1   │                    │                        │
│  mission.1.findings.v1 │                    │  rank_engine.py        │
│  mission.1.agents.v1   │                    │  KAFKA_BROKER=         │
│  mission.1.ranks.v1    │                    │  171.22.180.238:9092   │
│  mission.1.heartbeat.v1│                    │                        │
│  mission.1.config.v1   │                    │  telegram_bot.py       │
└────────────────────────┘                    │  KAFKA_BROKER=         │
                                              │  171.22.180.238:9092   │
                                              └────────────────────────┘
```

---

## 3. Правильная конфигурация Kafka в docker-compose.yml

### 3.1. Исправленный блок Kafka (dual listeners)

```yaml
kafka:
  image: apache/kafka:4.0.0     # ← зафиксировать версию!
  container_name: waters-kafka
  ports:
    - "9092:9092"                # внешний listener
  volumes:
    - kafka-data:/var/lib/kafka/data
  environment:
    # KRaft mode (без Zookeeper)
    KAFKA_PROCESS_ROLES: "broker,controller"
    KAFKA_NODE_ID: "1"
    CLUSTER_ID: "waters-platform"

    # ДВА LISTENER'а: внутренний и внешний
    # Внутренний — для Docker-контейнеров (по имени сервиса)
    # Внешний — для удалённых клиентов (миссионный сервер)
    KAFKA_LISTENERS: "INTERNAL://0.0.0.0:9091,EXTERNAL://0.0.0.0:9092,CONTROLLER://0.0.0.0:9093"

    # ADVERTISED — что клиентам говорить
    KAFKA_ADVERTISED_LISTENERS: "INTERNAL://waters-kafka:9091,EXTERNAL://171.22.180.238:9092"

    # Маппинг имён listener'ов
    KAFKA_LISTENER_SECURITY_PROTOCOL_MAP: "INTERNAL:PLAINTEXT,EXTERNAL:PLAINTEXT,CONTROLLER:PLAINTEXT"
    KAFKA_INTER_BROKER_LISTENER_NAME: "INTERNAL"
    KAFKA_CONTROLLER_LISTENER_NAMES: "CONTROLLER"

    # Controller quorum — внутренний адрес
    KAFKA_CONTROLLER_QUORUM_VOTERS: "1@waters-kafka:9093"

    # Хранилище
    KAFKA_LOG_DIRS: "/var/lib/kafka/data"

    # Топики
    KAFKA_AUTO_CREATE_TOPICS_ENABLE: "true"
    KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: "1"
    KAFKA_GROUP_INITIAL_REBALANCE_DELAY_MS: "0"
    KAFKA_NUM_PARTITIONS: "3"
```

### 3.2. Порты назначения

| Listener | Порт | Назначение | Куда подключаться |
|----------|------|-----------|------------------|
| `INTERNAL` | 9091 | Docker-контейнеры на том же хосте | `waters-kafka:9091` |
| `EXTERNAL` | 9092 | Удалённые клиенты (миссионный сервер) | `171.22.180.238:9092` |
| `CONTROLLER` | 9093 | Внутренний KRaft consensus | только internal |

---

## 4. Куда и как подключаться

### 4.1. Docker-контейнеры на 238 (mcp-proxy, brave-search, ...)

Используют **INTERNAL** listener:

```
KAFKA_BOOTSTRAP: waters-kafka:9091
```

### 4.2. Миссионный сервер (Rust core, Python edge)

Используют **EXTERNAL** listener:

```
export KAFKA_BOOTSTRAP=171.22.180.238:9092
```

В `mission.toml`:
```toml
[kafka]
bootstrap_servers = "171.22.180.238:9092"
consumer_group = "mission-1-consumer"
```

### 4.3. Файл `.env` (корень проекта)

```bash
# Kafka — подключение с этого же хоста (Docker internal)
export REMOTE_KAFKA_BOOTSTRAP=171.22.180.238:9092
export KAFKA_BOOTSTRAP=waters-kafka:9091

# Redis
export REMOTE_REDIS_HOST=171.22.180.238
export REMOTE_REDIS_PORT=6379
```

---

## 5. 6 топиков миссии — правила создания

### 5.1. Команда создания топиков

```bash
# Создать все 6 топиков
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create \
  --topic mission.1.orders.v1 \
  --partitions 1 --replication-factor 1 --config retention.ms=604800000

docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create \
  --topic mission.1.findings.v1 \
  --partitions 3 --replication-factor 1 --config retention.ms=7776000000

docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create \
  --topic mission.1.agents.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact

docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create \
  --topic mission.1.ranks.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact

docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create \
  --topic mission.1.heartbeat.v1 \
  --partitions 1 --replication-factor 1 --config retention.ms=86400000

docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create \
  --topic mission.1.config.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact
```

### 5.2. Параметры топиков

| Топик | Партиции | Retention | Cleanup | Ключ |
|-------|----------|-----------|---------|------|
| `orders.v1` | 1 | 7 дней | delete | `order_<uuid>` |
| `findings.v1` | **3** | 90 дней | delete | `finding_<uuid>` |
| `agents.v1` | 1 | ∞ | **compact** | `agent_<id>` |
| `ranks.v1` | 1 | ∞ | **compact** | `rank_<agent_id>` |
| `heartbeat.v1` | 1 | 24 часа | delete | `mission_1` |
| `config.v1` | 1 | ∞ | **compact** | `<config_key>` |

### 5.3. Скрипт инициализации

Создать `scripts/init_kafka_topics.sh`:

```bash
#!/bin/bash
# Инициализация 6 топиков Миссии 1
# Запускать после старта Kafka: docker compose up -d kafka

KAFKA="docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092"

echo "=== Инициализация топиков Миссии 1 ==="

$KAFKA --create --topic mission.1.orders.v1 \
  --partitions 1 --replication-factor 1 --config retention.ms=604800000 \
  --if-not-exists && echo "✅ orders.v1"

$KAFKA --create --topic mission.1.findings.v1 \
  --partitions 3 --replication-factor 1 --config retention.ms=7776000000 \
  --if-not-exists && echo "✅ findings.v1"

$KAFKA --create --topic mission.1.agents.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact \
  --if-not-exists && echo "✅ agents.v1"

$KAFKA --create --topic mission.1.ranks.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact \
  --if-not-exists && echo "✅ ranks.v1"

$KAFKA --create --topic mission.1.heartbeat.v1 \
  --partitions 1 --replication-factor 1 --config retention.ms=86400000 \
  --if-not-exists && echo "✅ heartbeat.v1"

$KAFKA --create --topic mission.1.config.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact \
  --if-not-exists && echo "✅ config.v1"

echo ""
echo "=== Готово ==="
$KAFKA --list
```

---

## 6. Consumer/Producer правила

### 6.1. Rust Core (mission-core)

```rust
// Consumer — читает orders
let consumer = StreamConsumer::create(
    ConsumerConfig::new()
        .set("bootstrap.servers", "171.22.180.238:9092")
        .set("group.id", "mission-1-consumer")
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "true")
        .set("auto.commit.interval.ms", "5000")
        .set("max.poll.interval.ms", "300000")
);

// Producer — пишет findings, agents, ranks, heartbeat
let producer = FutureProducer::create(
    ProducerConfig::new()
        .set("bootstrap.servers", "171.22.180.238:9092")
        .set("acks", "all")           // гарантия доставки
        .set("retries", "3")
        .set("linger.ms", "100")       // batch для производительности
);
```

### 6.2. Python Edge (rank_engine, telegram_bot)

```python
from confluent_kafka import Consumer, Producer

consumer = Consumer({
    'bootstrap.servers': '171.22.180.238:9092',
    'group.id': 'rank-engine-1',           # уникальный group.id!
    'auto.offset.reset': 'earliest',
    'enable.auto.commit': True,
})

producer = Producer({
    'bootstrap.servers': '171.22.180.238:9092',
    'acks': 'all',
})
```

### 6.3. Правило: group.id уникальный для каждого клиента

| Клиент | group.id |
|--------|----------|
| mission-core (Rust) | `mission-1-consumer` |
| rank_engine.py | `rank-engine-1` |
| telegram_bot.py | `telegram-bot-1` |
| website backend | `website-consumer-1` |

**Никогда не используйте один group.id для разных логических клиентов** — они начнут делить партиции и терять сообщения!

---

## 7. Проверка Kafka после запуска

```bash
# 1. Проверить что Kafka запущен
docker ps | grep waters-kafka

# 2. Проверить что Kafka отвечает
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --list

# 3. Отправить тестовое сообщение
echo '{"test": "hello"}' | \
  docker exec -i waters-kafka kafka-console-producer.sh \
  --bootstrap-server localhost:9092 --topic mission.1.heartbeat.v1

# 4. Прочитать тестовое сообщение
docker exec waters-kafka kafka-console-consumer.sh \
  --bootstrap-server localhost:9092 --topic mission.1.heartbeat.v1 \
  --from-beginning --max-messages 1

# 5. Проверить что внешний клиент может подключиться
# (с миссионного сервера)
kcat -b 171.22.180.238:9092 -L

# 6. Полный healthcheck
docker exec waters-kafka kafka-broker-api-versions.sh \
  --bootstrap-server localhost:9092
```

---

## 8. Типичные ошибки и их решения

| Симптом | Причина | Решение |
|---------|---------|--------|
| `Connection to node -1 refused` | `KAFKA_ADVERTISED_LISTENERS` неправильный | Установить `EXTERNAL://<IP_сервера>:9092` |
| `Failed to resolve waters-kafka:9091` | Внутренний listener не настроен | Проверить `INTERNAL://0.0.0.0:9091` в `KAFKA_LISTENERS` |
| `TimeoutException` | Kafka не успел стартовать | Подождать 30-60 сек, проверить `docker logs waters-kafka` |
| `UnknownTopicOrPartition` | Топик не создан | Запустить `scripts/init_kafka_topics.sh` |
| `Offset commit failed` | `group.id` конфликтует | Использовать уникальный `group.id` |
| `LeaderNotAvailable` | KRaft не завершил election | Подождать 10-15 сек после старта |
| `Message too large` | Размер сообщения > 1MB | Увеличить `KAFKA_MESSAGE_MAX_BYTES` |
