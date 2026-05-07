#!/usr/bin/env bash
# Запуск всех агентов Гексады в отдельных терминалах
# Требует: gnome-terminal, konsole, xterm или tmux

set -eo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
LOG_DIR="$REPO_DIR/logs"
mkdir -p "$LOG_DIR"

echo "=== Запуск Гексады WATERS (6 агентов) ==="
echo ""

# Функция запуска в новом терминале
run_agent() {
    local AGENT=$1
    local TITLE=$2
    local ICON=$3
    
    echo "$ICON Запуск $AGENT..."
    
    # Попытка найти доступный терминал
    if command -v gnome-terminal &> /dev/null; then
        gnome-terminal --title="$TITLE" -- bash -c "cd $REPO_DIR && ./run_${AGENT}.sh 2>&1 | tee $LOG_DIR/${AGENT}.log"
    elif command -v konsole &> /dev/null; then
        konsole --new-tab -p "tabtitle=$TITLE" -e bash -c "cd $REPO_DIR && ./run_${AGENT}.sh 2>&1 | tee $LOG_DIR/${AGENT}.log"
    elif command -v xterm &> /dev/null; then
        xterm -title "$TITLE" -e bash -c "cd $REPO_DIR && ./run_${AGENT}.sh 2>&1 | tee $LOG_DIR/${AGENT}.log" &
    elif command -v tmux &> /dev/null; then
        # Tmux: создаём новые окна в сессии waters
        tmux new-session -d -s waters -n "$AGENT" "cd $REPO_DIR && ./run_${AGENT}.sh 2>&1 | tee $LOG_DIR/${AGENT}.log"
    else
        echo "  ❌ Не найден подходящий терминал (нужен gnome-terminal, konsole, xterm или tmux)"
        exit 1
    fi
}

# Запуск агентов по Троицам
echo "🏛️ ТРОИЦА СТРУКТУРЫ (Terminal 1-3):"
run_agent "architect" "🏛️ Архитектор" "  🏛️"
sleep 2
run_agent "constructor" "🌐 Конструктор" "  🌐"
sleep 2
run_agent "integrator" "📡 Интегратор" "  📡"
sleep 2

echo ""
echo "💫 ТРОИЦА СМЫСЛА (Terminal 4-6):"
run_agent "director" "🙏 Директор" "  🙏"
sleep 2
run_agent "lawkeeper" "⚖️ Законодатель" "  ⚖️"
sleep 2
run_agent "keeper" "🛡️ Хранитель" "  🛡️"
sleep 2

echo ""
echo "✅ Запущено 6 агентов:"
echo "   Terminals 1-3: Троица Структуры"
echo "   Terminals 4-6: Троица Смысла"
echo ""
echo "📋 Логи: $LOG_DIR/"
echo ""
echo "Для остановки: ./stop_all.sh"
