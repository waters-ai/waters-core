#!/usr/bin/env bash
# Запуск агента: Интегратор Знаний v1.0
set -euo pipefail

AGENT="integrator"
AGENT_FILE="agents/${AGENT}_AGENTS.md"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

echo "🔬 Запуск Интегратора Знаний v1.0..."
echo "   AGENTS.md ← $AGENT_FILE"

cp "$AGENT_FILE" AGENTS.md
opencode
