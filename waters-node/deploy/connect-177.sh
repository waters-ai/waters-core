#!/bin/bash
# WATERS — подключение к мастер-ноде 177
# Запуск: ./connect-177.sh [порт]

PORT=${1:-42069}
MASTER="87.242.102.177"
REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"
DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:?Ошибка: установи DEEPSEEK_API_KEY}"

echo "═══ WATERS v0.4 — подключение к $MASTER:$PORT ═══"
echo ""

# Запускаем ноду с авто-подключением к мастеру
exec ./waters-node \
  --port $PORT \
  --connect "$MASTER:$PORT"
