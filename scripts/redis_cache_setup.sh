#!/bin/bash
#
# redis_cache_setup.sh — Настройка Redis-кэша для WATERS
# Настраивает eviction policy, maxmemory, TTL и структуры ключей
#
# Usage:
#   bash scripts/redis_cache_setup.sh [REDIS_HOST] [REDIS_PORT]
#

set -e

REDIS_HOST="${1:-localhost}"
REDIS_PORT="${2:-6379}"
REDIS_CLI="redis-cli -h $REDIS_HOST -p $REDIS_PORT"

echo "=== Настройка Redis-кэша WATERS ==="
echo "Host: $REDIS_HOST:$REDIS_PORT"
echo ""

# Проверка соединения
echo "[1/8] Проверка соединения..."
if ! $REDIS_CLI ping > /dev/null 2>&1; then
    echo "ERROR: Не удалось подключиться к Redis на $REDIS_HOST:$REDIS_PORT"
    exit 1
fi
echo "  Redis отвечает: $($REDIS_CLI ping)"

# Maxmemory и eviction policy
echo ""
echo "[2/8] Установка eviction policy: allkeys-lru..."
$REDIS_CLI CONFIG SET maxmemory-policy allkeys-lru

echo "[3/8] Установка maxmemory: 256mb..."
$REDIS_CLI CONFIG SET maxmemory 256mb

# TTL по умолчанию для разных типов данных
echo ""
echo "[4/8] Установка TTL для конфигурационных ключей..."

# Инструкции по кэшированию — сохраняем как справочные ключи
$REDIS_CLI SET "cache:rules:ai_ttl" "3600"
$REDIS_CLI SET "cache:rules:search_ttl" "21600"
$REDIS_CLI SET "cache:rules:config_ttl" "86400"
$REDIS_CLI SET "cache:rules:schema_ttl" "604800"
$REDIS_CLI SET "cache:rules:default_ttl" "86400"
$REDIS_CLI SET "cache:rules:max_value_size" "524288"

# Метрики — инициализация
echo ""
echo "[5/8] Инициализация счётчиков метрик..."
$REDIS_CLI SETNX "metrics:cache_hit" "0"
$REDIS_CLI SETNX "metrics:cache_miss" "0"
$REDIS_CLI SETNX "metrics:tokens_saved" "0"
$REDIS_CLI SETNX "metrics:eviction_count" "0"
$REDIS_CLI SETNX "metrics:total_keys" "0"

# Паттерны ключей — справочная информация
echo ""
echo "[6/8] Регистрация паттернов ключей..."
$REDIS_CLI SET "cache:patterns:ai" "cache:ai:<sha256[:16]>"
$REDIS_CLI SET "cache:patterns:search" "cache:search:<provider>:<sha256[:16]>"
$REDIS_CLI SET "cache:patterns:chromadb" "cache:chromadb:<sha256[:16]>"
$REDIS_CLI SET "cache:patterns:schema" "cache:schema:<name>"
$REDIS_CLI SET "cache:patterns:config" "cache:config:<component>"

# Предзагрузка схем HiveMind в кэш (если файлы доступны)
echo ""
echo "[7/8] Предзагрузка схем в кэш..."
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SCHEMA_DIR="$SCRIPT_DIR/../schemas"

for schema_file in "$SCHEMA_DIR"/hivemind_*.json; do
    if [ -f "$schema_file" ]; then
        name=$(basename "$schema_file" .json)
        echo "  Загрузка схемы: $name"
        $REDIS_CLI SETEX "cache:schema:$name" 604800 "$(cat "$schema_file")"
    fi
done

# Предзагрузка топологий Docker
for topo_file in "$SCRIPT_DIR/../infrastructure/docker/topology.json"; do
    if [ -f "$topo_file" ]; then
        echo "  Загрузка топологии: docker"
        $REDIS_CLI SETEX "cache:config:docker_topology" 86400 "$(cat "$topo_file")"
    fi
done

# Итоговая статистика
echo ""
echo "[8/8] Итоговая конфигурация:"
echo ""
echo "  maxmemory-policy: $($REDIS_CLI CONFIG GET maxmemory-policy | tail -1)"
echo "  maxmemory: $($REDIS_CLI CONFIG GET maxmemory | tail -1)"
echo "  keys в кэше: $($REDIS_CLI DBSIZE | awk '{print $2}')"
echo "  использовано памяти: $($REDIS_CLI INFO memory | grep used_memory_human | tr -d '\r')"
echo ""

# Выводим текущие ключи
echo "  Ключи:"
$REDIS_CLI KEYS "cache:*" | while read -r key; do
    ttl=$($REDIS_CLI TTL "$key")
    echo "    $key (TTL: ${ttl}s)"
done

echo ""
echo "=== Настройка Redis-кэша завершена ==="
