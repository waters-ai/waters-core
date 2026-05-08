#!/usr/bin/env bash
# Запуск агента: Хранитель Протокола v1.0
# Использование:
#   ./run_keeper.sh          # обычный запуск
#   ./run_keeper.sh --tmux   # запуск в tmux-сессии (24/7)
set -euo pipefail

AGENT="keeper"
AGENT_FILE="agents/${AGENT}_AGENTS.md"
MODE="${1:-}"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

if [ "$MODE" = "--tmux" ]; then
    echo "🛡️ Запуск Хранителя Протокола в tmux-сессии..."
    exec "$(dirname "$0")/scripts/opencode_tmux.sh" "$AGENT" start
fi

echo "🛡️ Запуск Хранителя Протокола v1.0..."
echo "   AGENTS.md ← $AGENT_FILE"

cp "$AGENT_FILE" AGENTS.md
opencode
