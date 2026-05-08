#!/usr/bin/env bash
# ceo.sh — удалённое управление агентами WATERS через REST API OpenCode
#
# Требует: curl, jq
#
# Использование:
#   ./scripts/ceo.sh sessions                        # список сессий
#   ./scripts/ceo.sh create [agent]                  # создать сессию
#   ./scripts/ceo.sh init <session-id>               # инициализировать сессию
#   ./scripts/ceo.sh msg <session-id> "текст"        # отправить (синхронно)
#   ./scripts/ceo.sh tell <session-id> "текст"       # отправить (асинхронно)
#   ./scripts/ceo.sh status <session-id>             # статус сессии
#   ./scripts/ceo.sh abort <session-id>              # прервать сессию
#   ./scripts/ceo.sh log <session-id> [limit]        # последние сообщения
#   ./scripts/ceo.sh model <session-id> <model>      # сменить модель
#   ./scripts/ceo.sh export <session-id>             # экспорт сессии в JSON
#   ./scripts/ceo.sh help                            # справка
#
# Переменные окружения:
#   OPENCODE_SERVER — URL сервера (с паролем если нужен)
#                     По умолч.: http://opencode:$(cat .serve_password)@localhost:4096
#
# Пример:
#   export OPENCODE_SERVER=http://opencode:secret@192.168.1.100:4096
#   ./scripts/ceo.sh sessions
#   ./scripts/ceo.sh msg <id> "Продолжай анализ"
#
# Быстрый старт с локальным сервером:
#   ./scripts/opencode_serve.sh start
#   export OPENCODE_SERVER=http://opencode:$(./scripts/opencode_serve.sh password)@localhost:4096
#   ./scripts/ceo.sh sessions

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# ─── Конфигурация ────────────────────────────────────────────────────────────
PASSWORD_FILE="$REPO_DIR/.serve_password"
DEFAULT_PASS=""
if [ -f "$PASSWORD_FILE" ]; then
  DEFAULT_PASS="opencode:$(cat "$PASSWORD_FILE")"
fi
DEFAULT_SERVER="http://${DEFAULT_PASS}@localhost:4096"
SERVER="${OPENCODE_SERVER:-$DEFAULT_SERVER}"

# ─── Проверка зависимостей ────────────────────────────────────────────────────
for cmd in curl jq; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "Ошибка: требуется $cmd. Установите: apt install $cmd" >&2
    exit 1
  fi
done

# ─── Вспомогательные функции ──────────────────────────────────────────────────
api() {
  local method="$1" path="$2" body="${3:-}"
  shift 2
  if [ -n "$body" ]; then
    curl -s -X "$method" "$SERVER$path" \
      -H "Content-Type: application/json" \
      -d "$body" "$@"
  else
    curl -s -X "$method" "$SERVER$path" "$@"
  fi
}

die() {
  echo "Ошибка: $1" >&2
  exit 1
}

# ─── Команды ──────────────────────────────────────────────────────────────────
cmd_sessions() {
  echo "=== Сессии OpenCode ==="
  api GET /session | jq -r '.[] | "\(.id) | \(.title // "—") | \(.status // "unknown")"' 2>/dev/null \
    || echo "(нет сессий или сервер недоступен)"
}

cmd_create() {
  local agent="${1:-}"
  local title="${2:-}"
  if [ -z "$title" ]; then
    if [ -n "$agent" ]; then
      title="WATERS — ${agent}"
    else
      title="WATERS — $(date +%Y-%m-%d)"
    fi
  fi
  local body
  body=$(cat <<JSON
{ "title": "$title" }
JSON
  )
  local result
  result=$(api POST /session "$body") || die "Не удалось создать сессию"
  local id
  id=$(echo "$result" | jq -r '.id')
  echo "✓ Сессия создана: $id"
  echo "  title: $title"
  echo ""
  echo "  Инициализация: ./scripts/ceo.sh init $id"
  echo "  Отправить:     ./scripts/ceo.sh msg $id \"ваш запрос\""
  echo "  Статус:        ./scripts/ceo.sh status $id"
}

cmd_init() {
  local session_id="${1:-}"
  [ -z "$session_id" ] && die "Укажите ID сессии"
  echo "Инициализация сессии $session_id (создание AGENTS.md)..."
  api POST "/session/$session_id/init" '{}' || die "Ошибка инициализации"
  echo "✓ Сессия инициализирована"
}

cmd_msg() {
  local session_id="${1:-}" text="${2:-}"
  [ -z "$session_id" ] && die "Укажите ID сессии"
  [ -z "$text" ] && die "Укажите текст сообщения"
  local body
  body=$(cat <<JSON
{
  "parts": [
    { "type": "text", "text": "$text" }
  ]
}
JSON
  )
  echo "Отправка сообщения в $session_id (ожидание ответа)..."
  api POST "/session/$session_id/message" "$body" | jq -r '.parts[]? | select(.type == "text") | .text // .'
}

cmd_tell() {
  local session_id="${1:-}" text="${2:-}"
  [ -z "$session_id" ] && die "Укажите ID сессии"
  [ -z "$text" ] && die "Укажите текст сообщения"
  local body
  body=$(cat <<JSON
{
  "parts": [
    { "type": "text", "text": "$text" }
  ]
}
JSON
  )
  echo "Отправка сообщения в $session_id (асинхронно)..."
  api POST "/session/$session_id/prompt_async" "$body"
  echo "✓ Сообщение отправлено. Проверить статус: ./scripts/ceo.sh status $session_id"
}

cmd_status() {
  local session_id="${1:-}"
  [ -z "$session_id" ] && die "Укажите ID сессии"
  api GET "/session/$session_id" | jq '{
    id: .id,
    title: .title,
    status: .status,
    created: .created,
    messages: (.messages | length)
  }'
}

cmd_abort() {
  local session_id="${1:-}"
  [ -z "$session_id" ] && die "Укажите ID сессии"
  echo "Прерывание сессии $session_id..."
  api POST "/session/$session_id/abort" || echo "✓ Сессия прервана"
}

cmd_log() {
  local session_id="${1:-}" limit="${2:-10}"
  [ -z "$session_id" ] && die "Укажите ID сессии"
  echo "=== Последние сообщения сессии $session_id ==="
  api GET "/session/$session_id/message?limit=$limit" | jq -r '
    .[] | "[" + (.info.role // "?") + "] " +
    (.parts[]? | select(.type == "text") | .text // "")
  ' 2>/dev/null || echo "(нет сообщений)"
}

cmd_export() {
  local session_id="${1:-}"
  [ -z "$session_id" ] && die "Укажите ID сессии"
  api GET "/session/$session_id" | jq '.'
}

cmd_help() {
  cat <<HELP
Использование: $0 <команда> [аргументы]

Команды:
  sessions                    список всех сессий
  create [agent]              создать новую сессию
  init <session-id>           инициализировать сессию (AGENTS.md)
  msg <session-id> "текст"    отправить сообщение (дождаться ответа)
  tell <session-id> "текст"   отправить сообщение (асинхронно)
  status <session-id>         статус сессии
  abort <session-id>          прервать выполнение
  log <session-id> [limit]    последние сообщения (по умолч. 10)
  export <session-id>         экспорт сессии в JSON
  help                        эта справка

Переменные окружения:
  OPENCODE_SERVER    URL OpenCode-сервера

Пример:
  export OPENCODE_SERVER=http://opencode:password@192.168.1.100:4096
  $0 sessions
  $0 msg <id> "Продолжай анализ архитектуры"

Модели (для init):
  ollama/qwen2.5:14b     Qwen 2.5 14B (сбалансированная)
  ollama/qwen2.5:7b      Qwen 2.5 7B (быстрая, CPU)
  deepseek/deepseek-v4-flash  DeepSeek V4 Flash (внешняя)
HELP
}

# ─── Main ─────────────────────────────────────────────────────────────────────
cmd="${1:-help}"
shift 2>/dev/null || true

case "$cmd" in
  sessions|list)   cmd_sessions "$@" ;;
  create|new)      cmd_create "$@" ;;
  init)            cmd_init "$@" ;;
  msg|message)     cmd_msg "$@" ;;
  tell|async)      cmd_tell "$@" ;;
  status|state)    cmd_status "$@" ;;
  abort|stop)      cmd_abort "$@" ;;
  log|history)     cmd_log "$@" ;;
  export|dump)     cmd_export "$@" ;;
  help|--help|-h)  cmd_help ;;
  *)
    echo "Неизвестная команда: $cmd"
    cmd_help
    exit 1
    ;;
esac
