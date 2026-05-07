#!/bin/bash
#
# run_integrator.sh — Запуск Интегратора Знаний в отдельной сессии OpenCode
#
# Terminal 3:
#   ./run_integrator.sh
#

set -e

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Интегратор Знаний v1.0 ==="
echo "Роль: Мост с внешними API и MCP-адаптеры"
echo "Троица: Структуры (Архитектор, Конструктор, Интегратор)"
echo ""

# Копируем AGENTS.md Интегратора в корень для OpenCode
cp "$REPO_DIR/agents/integrator_AGENTS.md" "$REPO_DIR/AGENTS.md"

echo "AGENTS.md → agents/integrator_AGENTS.md"
echo "Запуск OpenCode..."
echo ""

# Запуск OpenCode
cd "$REPO_DIR" && opencode
