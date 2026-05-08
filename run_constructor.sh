#!/usr/bin/env bash
# Запуск агента: Конструктор Сети v1.0
# Использование:
#   ./run_constructor.sh          # обычный запуск
#   ./run_constructor.sh --tmux   # запуск в tmux-сессии (24/7)
set -euo pipefail

AGENT="constructor"
AGENT_FILE="agents/${AGENT}_AGENTS.md"
MODE="${1:-}"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

if [ "$MODE" = "--tmux" ]; then
    echo "🌐 Запуск Конструктора Сети v1.0 в tmux-сессии..."
    exec "$(dirname "$0")/scripts/opencode_tmux.sh" "$AGENT" start
fi

echo "🌐 Запуск Конструктора Сети v1.0..."
echo "   AGENTS.md ← $AGENT_FILE"

cp "$AGENT_FILE" AGENTS.md

# Загрузка секретов из .env
if [ -f "$(dirname "$0")/.env" ]; then
  set -a; source "$(dirname "$0")/.env"; set +a
fi

opencode
