#!/usr/bin/env bash
# Запуск агента: Законодатель v1.0
# Использование:
#   ./run_lawkeeper.sh          # обычный запуск
#   ./run_lawkeeper.sh --tmux   # запуск в tmux-сессии (24/7)
set -euo pipefail

AGENT="lawkeeper"
AGENT_FILE="agents/${AGENT}_AGENTS.md"
MODE="${1:-}"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

if [ "$MODE" = "--tmux" ]; then
    echo "⚖️ Запуск Законодателя в tmux-сессии..."
    exec "$(dirname "$0")/scripts/opencode_tmux.sh" "$AGENT" start
fi

echo "⚖️ Запуск Законодателя v1.0..."
echo "   AGENTS.md ← $AGENT_FILE"

cp "$AGENT_FILE" AGENTS.md
opencode
