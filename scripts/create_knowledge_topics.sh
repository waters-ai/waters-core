#!/bin/bash
#
# create_knowledge_topics.sh — Создание Kafka-топиков для системы знаний
# Используется контейнер waters-heart с Kafka
#

set -e

BOOTSTRAP="localhost:9092"
PARTITIONS=1
REPLICATION=1
CONTAINER="waters-heart"
KAFKA_BIN="/opt/kafka/bin/kafka-topics.sh"

echo "=== Создание топиков для системы знаний WATERS ==="

echo "[1/2] Создание knowledge.articles.v1..."
sudo docker exec "$CONTAINER" "$KAFKA_BIN" \
  --create \
  --topic knowledge.articles.v1 \
  --bootstrap-server "$BOOTSTRAP" \
  --partitions "$PARTITIONS" \
  --replication-factor "$REPLICATION"

echo "[2/2] Создание knowledge.graph.v1..."
sudo docker exec "$CONTAINER" "$KAFKA_BIN" \
  --create \
  --topic knowledge.graph.v1 \
  --bootstrap-server "$BOOTSTRAP" \
  --partitions "$PARTITIONS" \
  --replication-factor "$REPLICATION"

echo "=== Топики созданы ==="
echo ""
echo "Проверить:"
echo "  sudo docker exec $CONTAINER $KAFKA_BIN --list --bootstrap-server $BOOTSTRAP"
