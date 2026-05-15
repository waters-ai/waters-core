#!/usr/bin/env bash
# Запуск агента: Конструктор Сети v1.0
# Использование:
#   ./run_constructor.sh          # обычный запуск (7B CPU)
#   ./run_constructor.sh --tmux   # запуск в tmux-сессии (24/7)
set -euo pipefail

AGENT="constructor"
AGENT_FILE="agents/${AGENT}_AGENTS.md"
MODE="${1:-}"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

echo "🌐 Запуск Конструктора Сети v1.0..."
echo "   AGENTS.md ← $AGENT_FILE"

cp "$AGENT_FILE" AGENTS.md

# Загрузка секретов из .env
if [ -f "$(dirname "$0")/.env" ]; then
  set -a; source "$(dirname "$0")/.env"; set +a
fi

# ─── SSH туннели ────────────────────────────────────────────────────
echo "   🔌 SSH tunnels (очистка старых + keepalive)..."

# Убить старые туннели на эти порты (чтобы не было дублей)
for port in 11434 11435 17687 25432; do
  pkill -f "ssh.*-L.*${port}:" 2>/dev/null || true
  pkill -f "ssh.*-NL.*${port}:" 2>/dev/null || true
  pkill -f "ssh.*-fNL.*${port}:" 2>/dev/null || true
done
sleep 0.5

SSH_OPTS="-o StrictHostKeyChecking=no -o ServerAliveInterval=15 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes"

# Neo4j (237) → localhost:17687
ssh $SSH_OPTS -fNL 17687:localhost:7687 ubuntu@171.22.180.237 2>/dev/null || true
# TimescaleDB (237) → localhost:25432
ssh $SSH_OPTS -fNL 25432:localhost:5432 ubuntu@171.22.180.237 2>/dev/null || true
# Ollama 7b (237) → localhost:11434
ssh $SSH_OPTS -fNL 11434:localhost:11434 ubuntu@171.22.180.237 2>/dev/null || true
# Ollama 14b (237) → localhost:11435
ssh $SSH_OPTS -fNL 11435:localhost:11435 ubuntu@171.22.180.237 2>/dev/null || true

echo "   ✅ Neo4j: localhost:17687 ← 237:7687"
echo "   ✅ TimescaleDB: localhost:25432 ← 237:5432"
echo "   ✅ Ollama 7b: localhost:11434 ← 237:11434"
echo "   ✅ Ollama 14b: localhost:11435 ← 237:11435"

# ─── Pre-flight: Ollama ─────────────────────────────────────────────
echo ""
echo "   🔄 Pre-flight: Ollama 7B..."
if curl -sf --max-time 5 http://localhost:11434/api/version > /dev/null 2>&1; then
    echo "   ✅ Ollama API доступна"
    # Keep model in memory (10 мин)
    curl -s http://localhost:11434/api/generate \
      -d '{"model":"qwen2.5:7b","keep_alive":"10m","prompt":""}' > /dev/null 2>&1 || true
    echo "   ✅ Модель qwen2.5:7b зафиксирована в памяти (keep_alive=10m)"
else
    echo "   ⚠️ Ollama не отвечает на localhost:11434"
fi

# ─── Startup Sequence ────────────────────────────────────────────────
echo ""
echo "   🔄 Startup Sequence (8 команд)..."
export REPO_DIR="$(dirname "$0")"
STARTUP_OUTPUT=$(python3 "$(dirname "$0")/scripts/constructor_startup.py" 2>/dev/null) || {
    echo "   ⚠️ Startup Sequence: некоторые сервисы недоступны"
    STARTUP_OUTPUT=""
}
if [ -n "$STARTUP_OUTPUT" ]; then
    echo "$STARTUP_OUTPUT" >> AGENTS.md
    echo "   ✅ Startup контекст добавлен в AGENTS.md"
fi

# ─── Запуск ──────────────────────────────────────────────────────────
if [ "$MODE" = "--tmux" ]; then
    echo ""
    echo "🌐 Запуск в tmux-сессии..."
    exec "$(dirname "$0")/scripts/opencode_tmux.sh" "$AGENT" start
fi

echo ""
exec opencode
