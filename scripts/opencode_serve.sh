#!/usr/bin/env bash
# opencode_serve.sh — headless OpenCode-сервер для удалённого доступа CEO
#
# Запускает `opencode serve` в tmux-сессии (24/7).
# CEO отправляет команды агентам через REST API (см. scripts/ceo.sh).
#
# Использование:
#   ./scripts/opencode_serve.sh [start|stop|status|restart|password]
#
# Переменные окружения:
#   OPENCODE_SERVE_PORT      — порт сервера (по умолч. 4096)
#   OPENCODE_SERVE_HOST      — хост (по умолч. 0.0.0.0)
#   OPENCODE_SERVER_PASSWORD — пароль basic auth (автогенерация если не задан)
#   DEEPSEEK_API_KEY         — ключ DeepSeek (если нет файла .secret_deepseek_key)
#   LOG_DIR                  — директория логов (по умолч. logs/)
#
# Хранение секретов (не пушить в GitHub!):
#   .secret_deepseek_key     — DeepSeek API key (gitignored)
#   .serve_password          — пароль serve (автогенерация, gitignored)
#
# Пример:
#   ./scripts/opencode_serve.sh start
#   ./scripts/opencode_serve.sh status

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="${LOG_DIR:-$REPO_DIR/logs}"
SESSION_NAME="${SESSION_NAME:-waters-serve}"
WINDOW_NAME="serve"

PORT="${OPENCODE_SERVE_PORT:-4096}"
HOST="${OPENCODE_SERVE_HOST:-0.0.0.0}"

PASSWORD_FILE="$REPO_DIR/.serve_password"
PASSWORD="${OPENCODE_SERVER_PASSWORD:-}"

SECRET_KEY_FILE="$REPO_DIR/.secret_deepseek_key"

mkdir -p "$LOG_DIR"

generate_password() {
  if command -v openssl &>/dev/null; then
    openssl rand -base64 24 | tr -d '/+=' | cut -c1-20
  else
    tr -dc 'a-zA-Z0-9' < /dev/urandom | fold -w 20 | head -n1
  fi
}

get_password() {
  if [ -n "$PASSWORD" ]; then
    echo "$PASSWORD"
  elif [ -f "$PASSWORD_FILE" ]; then
    cat "$PASSWORD_FILE"
  else
    local new_pass
    new_pass=$(generate_password)
    echo "$new_pass" > "$PASSWORD_FILE"
    chmod 600 "$PASSWORD_FILE"
    echo "$new_pass"
  fi
}

# Загрузка API-ключей из локальных secret-файлов (gitignored)
load_secrets() {
  if [ -f "$SECRET_KEY_FILE" ]; then
    export DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-$(cat "$SECRET_KEY_FILE")}"
  fi
}

action="${1:-start}"

case "$action" in
  start)
    load_secrets
    PASS="$(get_password)"
    echo "Запуск OpenCode serve на $HOST:$PORT ..."

    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
      echo "Сессия $SESSION_NAME уже существует."
      echo "  Подключиться:  tmux attach -t $SESSION_NAME:$WINDOW_NAME"
      echo "  Перезапуск:    $0 restart"
      exit 0
    fi

    # Формируем окружение для serve: пароль + API-ключи из secret-файлов
    SERVE_ENV="OPENCODE_SERVER_PASSWORD='$PASS'"
    if [ -n "${DEEPSEEK_API_KEY:-}" ]; then
      SERVE_ENV="$SERVE_ENV DEEPSEEK_API_KEY='$DEEPSEEK_API_KEY'"
    fi

    tmux new-session -d -s "$SESSION_NAME" -n "$WINDOW_NAME" \
      "cd '$REPO_DIR' && $SERVE_ENV opencode serve --port '$PORT' --hostname '$HOST' 2>&1 | tee '$LOG_DIR/serve.log'; exec bash"

    echo "✓ OpenCode serve запущен: http://$HOST:$PORT"
    echo "  Сессия tmux:    $SESSION_NAME:$WINDOW_NAME"
    echo "  Лог:            tail -f $LOG_DIR/serve.log"
    echo "  Password файл:  $PASSWORD_FILE"
    echo ""
    echo "  Подключиться к логу:      tmux attach -t $SESSION_NAME:$WINDOW_NAME"
    echo "  Для CEO (см. ceo.sh):"
    echo "    export OPENCODE_SERVER=http://opencode:$PASS@$HOST:$PORT"
    echo "    ./scripts/ceo.sh sessions"
    ;;
  stop)
    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
      tmux kill-session -t "$SESSION_NAME"
      echo "✓ OpenCode serve остановлен"
    else
      echo "Сессия $SESSION_NAME не найдена"
    fi
    ;;
  restart)
    "$0" stop
    sleep 1
    "$0" start
    ;;
  status)
    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
      echo "✓ OpenCode serve: RUNNING"
      echo "  URL:            http://$HOST:$PORT"
      echo "  Password файл:  $PASSWORD_FILE"
      PASS="$(get_password)"
      echo "  Для CEO:        export OPENCODE_SERVER=http://opencode:$PASS@$HOST:$PORT"
      tmux list-windows -t "$SESSION_NAME" -F '  Окно: #W'
    else
      echo "✗ OpenCode serve: STOPPED"
      echo "  Запуск: $0 start"
    fi
    ;;
  password)
    PASS="$(get_password)"
    echo "$PASS"
    ;;
  url)
    PASS="$(get_password)"
    echo "http://opencode:$PASS@$HOST:$PORT"
    ;;
  *)
    echo "Использование: $0 [start|stop|status|restart|password|url]"
    exit 1
    ;;
esac
