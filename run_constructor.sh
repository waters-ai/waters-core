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

# SSH туннели к удалённым серверам
echo "   🔌 SSH tunnels..."
# Neo4j (237) → localhost:17687
ssh -o StrictHostKeyChecking=no -o ExitOnForwardFailure=yes \
  -fNL 17687:localhost:7687 ubuntu@171.22.180.237 2>/dev/null || true
# TimescaleDB (237) → localhost:25432
ssh -o StrictHostKeyChecking=no -o ExitOnForwardFailure=yes \
  -fNL 25432:localhost:5432 ubuntu@171.22.180.237 2>/dev/null || true
echo "   ✅ Neo4j: localhost:17687 ← 237:7687"
echo "   ✅ TimescaleDB: localhost:25432 ← 237:5432"

opencode
