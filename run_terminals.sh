#!/usr/bin/env bash
# run_terminals.sh — Запуск всех агентов Гексады в отдельных терминалах
#
# Поддерживает: gnome-terminal, konsole, xterm, tmux
# Можно запустить отдельных агентов через переменную TERMINAL_AGENTS
#
# Использование:
#   ./run_terminals.sh                          # все 6 агентов
#   TERMINAL_AGENTS="architect,constructor" ./run_terminals.sh
#   TERMINAL=tmux ./run_terminals.sh            # принудительно tmux
#   SESSION_NAME=waters ./run_terminals.sh      # имя tmux-сессии
#
# Переменные окружения:
#   TERMINAL         — принудительный выбор терминала
#   TERMINAL_AGENTS  — список агентов через запятую
#   SESSION_NAME     — имя tmux-сессии (по умолч. waters)
#   LOG_DIR          — директория логов (по умолч. logs/)
#

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
LOG_DIR="${LOG_DIR:-$REPO_DIR/logs}"
SESSION_NAME="${SESSION_NAME:-waters}"

mkdir -p "$LOG_DIR"

# ─── Агенты ───────────────────────────────────────────────────────────────────
declare -A AGENT_TITLES=(
  ["architect"]="Троица Структуры: Архитектор"
  ["constructor"]="Троица Структуры: Конструктор"
  ["integrator"]="Троица Структуры: Интегратор"
  ["director"]="Троица Смысла: Директор"
  ["lawkeeper"]="Троица Смысла: Законодатель"
  ["keeper"]="Троица Смысла: Хранитель"
)

declare -A AGENT_ICONS=(
  ["architect"]="🏛️"
  ["constructor"]="🌐"
  ["integrator"]="📡"
  ["director"]="🙏"
  ["lawkeeper"]="⚖️"
  ["keeper"]="🛡️"
)

DEFAULT_AGENTS=("architect" "constructor" "integrator" "director" "lawkeeper" "keeper")

if [ -n "${TERMINAL_AGENTS:-}" ]; then
  IFS=',' read -ra AGENTS <<< "$TERMINAL_AGENTS"
else
  AGENTS=("${DEFAULT_AGENTS[@]}")
fi

# ─── Определение доступного терминала ─────────────────────────────────────────
detect_terminal() {
  if [ -n "${TERMINAL:-}" ]; then
    echo "$TERMINAL"
    return
  fi

  if command -v gnome-terminal &> /dev/null; then
    echo "gnome-terminal"
  elif command -v konsole &> /dev/null; then
    echo "konsole"
  elif command -v xterm &> /dev/null; then
    echo "xterm"
  elif command -v tmux &> /dev/null; then
    echo "tmux"
  else
    echo "none"
  fi
}

# ─── Запуск в терминале ───────────────────────────────────────────────────────
run_in_terminal() {
  local agent="$1"
  local icon="${AGENT_ICONS[$agent]}"
  local title="${AGENT_TITLES[$agent]}"
  local term

  term="$(detect_terminal)"
  echo "$icon Запуск $agent ($title) через $term..."

  case "$term" in
    gnome-terminal)
      gnome-terminal --title="$title" -- bash -c \
        "cd '$REPO_DIR' && ./run_${agent}.sh 2>&1 | tee '$LOG_DIR/${agent}.log'; exec bash"
      ;;
    konsole)
      konsole --new-tab -p "tabtitle=$title" -e bash -c \
        "cd '$REPO_DIR' && ./run_${agent}.sh 2>&1 | tee '$LOG_DIR/${agent}.log'; exec bash"
      ;;
    xterm)
      xterm -title "$title" -e bash -c \
        "cd '$REPO_DIR' && ./run_${agent}.sh 2>&1 | tee '$LOG_DIR/${agent}.log'; exec bash" &
      ;;
    tmux)
      if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
        tmux new-session -d -s "$SESSION_NAME" -n "$agent" \
          "cd '$REPO_DIR' && ./run_${agent}.sh 2>&1 | tee '$LOG_DIR/${agent}.log'; exec bash"
      else
        tmux new-window -t "$SESSION_NAME" -n "$agent" \
          "cd '$REPO_DIR' && ./run_${agent}.sh 2>&1 | tee '$LOG_DIR/${agent}.log'; exec bash"
      fi
      ;;
    none)
      echo "  ❌ Не найден терминал (нужен gnome-terminal, konsole, xterm или tmux)"
      echo "  Установите tmux: sudo apt install tmux"
      return 1
      ;;
  esac
}

# ─── Main ──────────────────────────────────────────────────────────────────────
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║       ЗАПУСК АГЕНТОВ ГЕКСАДЫ WATERS v1.0                       ║"
echo "╠══════════════════════════════════════════════════════════════════╣"
echo "║  Агентов: ${#AGENTS[@]}"
echo "║  Терминал: $(detect_terminal)"
echo "║  Логи: ${LOG_DIR}/"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

echo "━━━ Троица Структуры ━━━"
run_in_terminal "architect"
sleep 2
run_in_terminal "constructor"
sleep 2
run_in_terminal "integrator"
sleep 2

echo ""
echo "━━━ Троица Смысла ━━━"
run_in_terminal "director"
sleep 2
run_in_terminal "lawkeeper"
sleep 2
run_in_terminal "keeper"
sleep 2

echo ""
echo "✅ Запущено ${#AGENTS[@]} агентов"
echo "   Логи: $LOG_DIR/"
echo "   Для остановки: ./stop_all.sh"

if [ "$(detect_terminal)" = "tmux" ]; then
  echo ""
  echo "   Для подключения к tmux: tmux attach -t $SESSION_NAME"
fi
