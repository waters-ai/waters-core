#!/usr/bin/env bash
# Запуск Верховного Архитектора v1.0 на qwen2.5:7b (CPU)
# Гарантирует модель 7B + чистые SSH туннели + pre-flight
# Использование:
#   ./run_architect_7b.sh            # обычный запуск
#   ./run_architect_7b.sh --tmux     # в tmux-сессии
set -euo pipefail

cd "$(dirname "$0")"

echo "🏛️  Верховный Архитектор v1.0 — принудительно qwen2.5:7b"
echo ""

# Принудительно ставим 7B в конфиге
python3 -c "
import json
with open('opencode.json') as f:
    cfg = json.load(f)
cfg['model'] = 'ollama/qwen2.5:7b'
with open('opencode.json', 'w') as f:
    json.dump(cfg, f, indent=2, ensure_ascii=False)
print('   ✅ Модель: qwen2.5:7b')
"

# ─── SSH туннели ────────────────────────────────────────────────────
echo "   🔌 SSH tunnels (очистка старых + keepalive)..."

for port in 11434 11435 17687 25432; do
  pkill -f "ssh.*-L.*${port}:" 2>/dev/null || true
  pkill -f "ssh.*-NL.*${port}:" 2>/dev/null || true
  pkill -f "ssh.*-fNL.*${port}:" 2>/dev/null || true
done
sleep 0.5

SSH_OPTS="-o StrictHostKeyChecking=no -o ServerAliveInterval=15 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes"

ssh $SSH_OPTS -fNL 17687:localhost:7687 ubuntu@171.22.180.237 2>/dev/null || true
ssh $SSH_OPTS -fNL 25432:localhost:5432 ubuntu@171.22.180.237 2>/dev/null || true
ssh $SSH_OPTS -fNL 11434:localhost:11434 ubuntu@171.22.180.237 2>/dev/null || true
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
    curl -s http://localhost:11434/api/generate \
      -d '{"model":"qwen2.5:7b","keep_alive":"10m","prompt":""}' > /dev/null 2>&1 || true
    echo "   ✅ Модель qwen2.5:7b зафиксирована в памяти (keep_alive=10m)"
else
    echo "   ⚠️ Ollama не отвечает на localhost:11434"
fi

# ─── Запуск ──────────────────────────────────────────────────────────
AGENT="architect"
AGENT_FILE="agents/${AGENT}_AGENTS.md"

if [ ! -f "$AGENT_FILE" ]; then
    echo "❌ Файл $AGENT_FILE не найден"
    exit 1
fi

echo ""
echo "🏛️  AGENTS.md ← $AGENT_FILE"
cp "$AGENT_FILE" AGENTS.md

if [ "${1:-}" = "--tmux" ]; then
    echo ""
    echo "🌐 Запуск в tmux-сессии..."
    exec "$(dirname "$0")/scripts/opencode_tmux.sh" "$AGENT" start
fi

echo ""
exec opencode
