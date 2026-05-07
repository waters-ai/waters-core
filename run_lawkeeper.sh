#!/usr/bin/env bash
# Запуск агента: Законодатель v1.0
set -euo pipefail

AGENT="lawkeeper"
AGENT_FILE="agents/${AGENT}_AGENTS.md"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

echo "⚖️ Запуск Законодателя v1.0..."
echo "   AGENTS.md ← $AGENT_FILE"

cp "$AGENT_FILE" AGENTS.md
opencode
