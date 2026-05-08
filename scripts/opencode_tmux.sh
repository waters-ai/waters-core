#!/usr/bin/env bash
# opencode_tmux.sh — запуск агента WATERS в tmux-сессии
#
# Сессия живёт 24/7 независимо от SSH.
# CEO подключается:  tmux attach -t waters:<agent>
# CEO отключается:   Ctrl+B, D (агент продолжает работу)
#
# Хранение секретов (не пушить в GitHub!):
#   .secret_deepseek_key     — DeepSeek API key (gitignored)
#
# Использование:
#   ./scripts/opencode_tmux.sh <agent>          # запустить агента
#   ./scripts/opencode_tmux.sh <agent> attach   # подключиться к запущенному
#   ./scripts/opencode_tmux.sh <agent> stop     # остановить агента
#   ./scripts/opencode_tmux.sh list             # список всех агентов
#
# Пример:
#   ./scripts/opencode_tmux.sh constructor
#   ./scripts/opencode_tmux.sh constructor attach
#   ./scripts/opencode_tmux.sh list

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="${LOG_DIR:-$REPO_DIR/logs}"
SESSION_NAME="${SESSION_NAME:-waters}"

mkdir -p "$LOG_DIR"

# Загрузка API-ключей из локальных secret-файлов и .env (gitignored)
if [ -f "$REPO_DIR/.env" ]; then
  set -a; source "$REPO_DIR/.env"; set +a
fi

SECRET_KEY_FILE="$REPO_DIR/.secret_deepseek_key"
if [ -f "$SECRET_KEY_FILE" ]; then
  export DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-$(cat "$SECRET_KEY_FILE")}"
fi

agent="${1:-}"
action="${2:-start}"

declare -A AGENT_TITLES=(
  ["architect"]="Архитектор"
  ["constructor"]="Конструктор"
  ["integrator"]="Интегратор"
  ["director"]="Директор"
  ["lawkeeper"]="Законодатель"
  ["keeper"]="Хранитель"
)

declare -A AGENT_ICONS=(
  ["architect"]="🏛️"
  ["constructor"]="🌐"
  ["integrator"]="📡"
  ["director"]="🙏"
  ["lawkeeper"]="⚖️"
  ["keeper"]="🛡️"
)

list_agents() {
  echo "Агенты в tmux-сессии '$SESSION_NAME':"
  echo ""
  if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    tmux list-windows -t "$SESSION_NAME" -F '#{window_index}: #{window_name}'
  else
    echo "  (сессия не запущена)"
  fi
  echo ""
  echo "Подключиться:  tmux attach -t $SESSION_NAME:<имя>"
  echo "Подключиться:  tmux attach -t $SESSION_NAME:<номер>"
}

if [ "$agent" = "list" ]; then
  list_agents
  exit 0
fi

if [ -z "$agent" ]; then
  echo "Использование: $0 <agent> [start|attach|stop]"
  echo "  или:         $0 list"
  echo "Агенты: ${!AGENT_TITLES[*]}"
  exit 1
fi

agent_file="$REPO_DIR/agents/${agent}_AGENTS.md"
window_name="$agent"
icon="${AGENT_ICONS[$agent]:-}"
title="${AGENT_TITLES[$agent]:-$agent}"

if [ ! -f "$agent_file" ]; then
  echo "Ошибка: файл $agent_file не найден"
  exit 1
fi

case "$action" in
  start)
    echo "${icon} Запуск $title в tmux-сессии $SESSION_NAME..."

    cp "$agent_file" "$REPO_DIR/AGENTS.md"

    # Передаём API-ключи в окружение tmux-сессии
    TMUX_ENV=""
    if [ -n "${DEEPSEEK_API_KEY:-}" ]; then
      TMUX_ENV="DEEPSEEK_API_KEY='$DEEPSEEK_API_KEY'"
    fi

    CMD="cd '$REPO_DIR' && ${TMUX_ENV} opencode 2>&1 | tee '$LOG_DIR/${agent}.log'; exec bash"

    if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
      tmux new-session -d -s "$SESSION_NAME" -n "$window_name" "$CMD"
    else
      if tmux list-windows -t "$SESSION_NAME" -F '#{window_name}' 2>/dev/null | grep -q "^$window_name$"; then
        echo "Окно $window_name уже существует. Подключитесь: tmux attach -t $SESSION_NAME:$window_name"
        exit 0
      fi
      tmux new-window -t "$SESSION_NAME" -n "$window_name" "$CMD"
    fi

    echo "${icon} $title запущен в tmux-сессии $SESSION_NAME:$window_name"
    echo ""
    echo "Подключиться:  tmux attach -t $SESSION_NAME:$window_name"
    echo "Отключиться:   Ctrl+B, D  (агент продолжит работу)"
    echo "Лог:           tail -f $LOG_DIR/${agent}.log"
    ;;
  attach)
    if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
      echo "Сессия $SESSION_NAME не найдена."
      echo "Запустите агента: $0 $agent start"
      exit 1
    fi
    if ! tmux list-windows -t "$SESSION_NAME" -F '#{window_name}' 2>/dev/null | grep -q "^$window_name$"; then
      echo "Окно $window_name не найдено."
      echo "Запустите агента: $0 $agent start"
      exit 1
    fi
    tmux attach-session -t "$SESSION_NAME:$window_name"
    ;;
  stop)
    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
      if tmux list-windows -t "$SESSION_NAME" -F '#{window_name}' 2>/dev/null | grep -q "^$window_name$"; then
        tmux kill-window -t "$SESSION_NAME:$window_name"
        echo "${icon} $title остановлен"
      else
        echo "Окно $window_name не найдено"
      fi
    else
      echo "Сессия $SESSION_NAME не найдена"
    fi
    ;;
  *)
    echo "Действие: start | attach | stop"
    exit 1
    ;;
esac
