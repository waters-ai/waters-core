#!/usr/bin/env bash
# session_snapshot.sh — Сохранение/восстановление снэпшотов сессий агентов в Redis
#
# Карусельный механизм: перед ротацией агента сохраняем контекст,
# при следующем запуске — восстанавливаем.
#
# Использование:
#   ./scripts/session_snapshot.sh save <agent_id> [context_file]
#   ./scripts/session_snapshot.sh load <agent_id> [output_file]
#   ./scripts/session_snapshot.sh list
#
# Пример:
#   ./scripts/session_snapshot.sh save agent.architect.v1 /tmp/architect_context.json
#   ./scripts/session_snapshot.sh load agent.integrator.v1 /tmp/integrator_state.json
#
# Формат Redis-ключа: session:<agent_id>
# TTL: 7 дней (604800 секунд)
#

set -euo pipefail

REDIS_CONTAINER="${REDIS_CONTAINER:-waters-redis}"
REDIS_PORT="${REDIS_PORT:-6379}"
TTL_SECONDS=604800  # 7 дней

# ─── Команды ─────────────────────────────────────────────────────────────────
cmd_save() {
  local agent_id="$1"
  local context_file="${2:-/dev/stdin}"
  local key="session:${agent_id}"
  local timestamp

  timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  # Читаем контекст из файла или stdin
  local context
  context="$(cat "$context_file")"

  # Сохраняем в Redis с TTL
  docker exec -i "$REDIS_CONTAINER" redis-cli -p "$REDIS_PORT" SET "$key" "$context" EX "$TTL_SECONDS" > /dev/null

  # Сохраняем также временную метку
  docker exec -i "$REDIS_CONTAINER" redis-cli -p "$REDIS_PORT" SET "${key}:ts" "$timestamp" EX "$TTL_SECONDS" > /dev/null

  echo "{\"status\":\"saved\",\"agent\":\"${agent_id}\",\"timestamp\":\"${timestamp}\",\"ttl\":${TTL_SECONDS}}"
}

cmd_load() {
  local agent_id="$1"
  local output_file="${2:-/dev/stdout}"
  local key="session:${agent_id}"

  local context
  context="$(docker exec "$REDIS_CONTAINER" redis-cli -p "$REDIS_PORT" GET "$key" 2>/dev/null || true)"

  if [ -z "$context" ] || [ "$context" = "(nil)" ]; then
    echo "{\"status\":\"not_found\",\"agent\":\"${agent_id}\"}"
    return 1
  fi

  echo "$context" > "$output_file"
  echo "{\"status\":\"loaded\",\"agent\":\"${agent_id}\"}"
}

cmd_list() {
  docker exec "$REDIS_CONTAINER" redis-cli -p "$REDIS_PORT" KEYS "session:*" 2>/dev/null \
    | grep -v "^$" \
    | while read -r key; do
        local ttl
        ttl="$(docker exec "$REDIS_CONTAINER" redis-cli -p "$REDIS_PORT" TTL "$key" 2>/dev/null)"
        echo "  $key (TTL: ${ttl}s)"
      done
}

# ─── Main ────────────────────────────────────────────────────────────────────
case "${1:-help}" in
  save)
    if [ -z "${2:-}" ]; then
      echo "Usage: $0 save <agent_id> [context_file]"
      exit 1
    fi
    cmd_save "$2" "${3:-}"
    ;;
  load)
    if [ -z "${2:-}" ]; then
      echo "Usage: $0 load <agent_id> [output_file]"
      exit 1
    fi
    cmd_load "$2" "${3:-}"
    ;;
  list)
    cmd_list
    ;;
  help|*)
    echo "Session Snapshot — управление снэпшотами сессий агентов в Redis"
    echo ""
    echo "Usage:"
    echo "  $0 save <agent_id> [context_file]   — сохранить снэпшот"
    echo "  $0 load <agent_id> [output_file]    — восстановить снэпшот"
    echo "  $0 list                             — список сохранённых снэпшотов"
    echo ""
    echo "Ключи Redis: session:<agent_id>, session:<agent_id>:ts"
    echo "TTL: $((TTL_SECONDS / 86400)) дней"
    ;;
esac
