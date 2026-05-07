#!/usr/bin/env bash
# Остановка всех агентов Гексады и восстановление AGENTS.md

set -e

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Остановка Гексады WATERS ==="
echo ""

# Восстановление оригинального AGENTS.md (дашборда)
echo "🔄 Восстановление AGENTS.md (дашборд Гексады)..."
git -C "$REPO_DIR" checkout AGENTS.md

# Убиваем все процессы opencode (осторожно!)
echo "🛑 Поиск процессов opencode..."
pkill -f "opencode" || echo "  Процессы opencode не найдены"

# Убиваем терминалы (если запускались через gnome-terminal)
if command -v gnome-terminal &> /dev/null; then
    pkill -f "gnome-terminal" || true
fi

echo ""
echo "✅ Гексада остановлена"
echo "📋 AGENTS.md восстановлен (дашборд)"
