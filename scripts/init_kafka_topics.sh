#!/bin/bash
# init_kafka_topics.sh — Инициализация 6 топиков Миссии 1
# Запускать: docker compose up -d kafka && sleep 15 && ./scripts/init_kafka_topics.sh

KAFKA="docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092"

echo "=== Инициализация топиков Миссии 1 ==="
echo ""

$KAFKA --create --topic mission.1.orders.v1 \
  --partitions 1 --replication-factor 1 --config retention.ms=604800000 \
  --if-not-exists && echo "✅ mission.1.orders.v1"

$KAFKA --create --topic mission.1.findings.v1 \
  --partitions 3 --replication-factor 1 --config retention.ms=7776000000 \
  --if-not-exists && echo "✅ mission.1.findings.v1"

$KAFKA --create --topic mission.1.agents.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact \
  --if-not-exists && echo "✅ mission.1.agents.v1"

$KAFKA --create --topic mission.1.ranks.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact \
  --if-not-exists && echo "✅ mission.1.ranks.v1"

$KAFKA --create --topic mission.1.heartbeat.v1 \
  --partitions 1 --replication-factor 1 --config retention.ms=86400000 \
  --if-not-exists && echo "✅ mission.1.heartbeat.v1"

$KAFKA --create --topic mission.1.config.v1 \
  --partitions 1 --replication-factor 1 --config cleanup.policy=compact \
  --if-not-exists && echo "✅ mission.1.config.v1"

echo ""
echo "=== Топики ==="
$KAFKA --list
