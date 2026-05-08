#!/usr/bin/env bash
# Остановка всех агентов Гексады и восстановление AGENTS.md

set -e

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Остановка Гексады WATERS ==="
echo ""

# Восстановление оригинального AGENTS.md (дашборда)
echo "🔄 Восстановление AGENTS.md (дашборд Гексады)..."
git -C "$REPO_DIR" checkout AGENTS.md 2>/dev/null || true

# Убиваем локальные процессы opencode
echo "🛑 Остановка локальных агентов (238)..."
pkill -f "opencode" || echo "  Процессы opencode не найдены"

# Остановка удалённых агентов на 167
echo "🛑 Остановка удалённых агентов (167)..."
ssh -o ConnectTimeout=5 waters-167 "pkill -f opencode 2>/dev/null; echo '  Удалённые агенты остановлены'" 2>/dev/null || echo "  Сервер 167 недоступен"

# Убиваем tmux сессию
tmux kill-session -t waters 2>/dev/null || true

# Убиваем терминалы (если запускались через gnome-terminal)
if command -v gnome-terminal &> /dev/null; then
    pkill -f "gnome-terminal" || true
fi

echo ""
echo "✅ Гексада остановлена"
echo "📋 AGENTS.md восстановлен (дашборд)"
