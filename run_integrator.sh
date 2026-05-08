#!/usr/bin/env bash
# run_integrator.sh — Запуск Интегратора Знаний
# Использование:
#   ./run_integrator.sh          # обычный запуск
#   ./run_integrator.sh --tmux   # запуск в tmux-сессии (24/7)

set -euo pipefail

AGENT="integrator"
AGENT_FILE="agents/${AGENT}_AGENTS.md"
MODE="${1:-}"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

if [ "$MODE" = "--tmux" ]; then
    echo "📡 Запуск Интегратора Знаний в tmux-сессии..."
    exec "$(dirname "$0")/scripts/opencode_tmux.sh" "$AGENT" start
fi

echo "=== Интегратор Знаний v1.0 ==="
echo "Роль: Мост с внешними API и MCP-адаптеры"
echo "Троица: Структуры (Архитектор, Конструктор, Интегратор)"
echo ""

cp "$AGENT_FILE" AGENTS.md
opencode

echo "AGENTS.md → agents/integrator_AGENTS.md"
echo "Запуск OpenCode..."
echo ""

# Запуск OpenCode
cd "$REPO_DIR" && opencode
