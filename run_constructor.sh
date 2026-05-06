#!/usr/bin/env bash
# Запуск агента: Конструктор Сети v1.0
set -euo pipefail

AGENT="constructor"
AGENT_FILE="agents/${AGENT}_AGENTS.md"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

echo "🌐 Запуск Конструктора Сети v1.0..."
echo "   AGENTS.md ← $AGENT_FILE"

cp "$AGENT_FILE" AGENTS.md
opencode
