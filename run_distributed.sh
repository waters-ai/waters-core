#!/usr/bin/env bash
# run_distributed.sh — Запуск 3+3 агентов: 3 локально (238) + 3 на малом сервере (167)
#
# Использование:
#   ./run_distributed.sh              # запуск всех 6 агентов
#   ./run_distributed.sh attach       # подключиться к tmux-сессии
#   ./run_distributed.sh stop         # остановить всех агентов
#   ./run_distributed.sh status       # проверить статус

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
LOG_DIR="${LOG_DIR:-$REPO_DIR/logs}"
SESSION_NAME="${SESSION_NAME:-waters}"
REMOTE_HOST="waters-167"
REMOTE_DIR="/root/waters-core"

mkdir -p "$LOG_DIR"

LOCAL_AGENTS=("architect" "constructor" "integrator")
REMOTE_AGENTS=("director" "lawkeeper" "keeper")

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

start_agent_local() {
  local agent="$1"
  local title="${AGENT_TITLES[$agent]}"
  local icon="${AGENT_ICONS[$agent]}"
  local logfile="$LOG_DIR/${agent}_238.log"

  local agent_file="$REPO_DIR/agents/${agent}_AGENTS.md"
  if [ ! -f "$agent_file" ]; then
    echo "  ⚠ $agent: AGENTS.md не найден, пропуск"
    return 1
  fi

  if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    tmux new-session -d -s "$SESSION_NAME" -n "$agent" \
      "cd '$REPO_DIR' && cp '$agent_file' AGENTS.md && echo '$icon $title (238)' && opencode 2>&1 | tee '$logfile'; exec bash"
  else
    tmux new-window -t "$SESSION_NAME" -n "$agent" \
      "cd '$REPO_DIR' && cp '$agent_file' AGENTS.md && echo '$icon $title (238)' && opencode 2>&1 | tee '$logfile'; exec bash"
  fi
  echo "  $icon $title — локально (238)"
}

start_agent_remote() {
  local agent="$1"
  local title="${AGENT_TITLES[$agent]}"
  local icon="${AGENT_ICONS[$agent]}"
  local logfile="$LOG_DIR/${agent}_167.log"

  if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    tmux new-session -d -s "$SESSION_NAME" -n "$agent" \
      "ssh '$REMOTE_HOST' -t 'cd $REMOTE_DIR && cp agents/${agent}_AGENTS.md AGENTS.md && echo \"$icon $title (167)\" && opencode 2>&1 | tee logs/${agent}_167.log; exec bash' 2>&1 | tee '$logfile'; exec bash"
  else
    tmux new-window -t "$SESSION_NAME" -n "$agent" \
      "ssh '$REMOTE_HOST' -t 'cd $REMOTE_DIR && cp agents/${agent}_AGENTS.md AGENTS.md && echo \"$icon $title (167)\" && opencode 2>&1 | tee logs/${agent}_167.log; exec bash' 2>&1 | tee '$logfile'; exec bash"
  fi
  echo "  $icon $title — удалённо (167)"
}

stop_all() {
  echo "=== Остановка агентов ==="
  # Stop local agents
  for agent in "${LOCAL_AGENTS[@]}"; do
    pkill -f "run_${agent}.sh" 2>/dev/null || true
  done
  pkill -f "opencode.*AGENTS.md" 2>/dev/null || true
  # Stop remote agents
  ssh "$REMOTE_HOST" "pkill -f opencode 2>/dev/null; pkill -f run_.*\.sh 2>/dev/null; echo 'Remote agents stopped'" 2>/dev/null || true
  # Kill tmux
  tmux kill-session -t "$SESSION_NAME" 2>/dev/null || true
  echo "✅ Все агенты остановлены"
}

status_all() {
  echo "=== Статус агентов ==="
  for agent in "${LOCAL_AGENTS[@]}"; do
    if ps aux | grep -v grep | grep -q "opencode.*${agent}"; then
      echo "  ✅ $agent — локально (238): RUNNING"
    else
      echo "  ❌ $agent — локально (238): STOPPED"
    fi
  done
  for agent in "${REMOTE_AGENTS[@]}"; do
    if ssh "$REMOTE_HOST" "ps aux | grep -v grep | grep -q 'opencode.*${agent}'" 2>/dev/null; then
      echo "  ✅ $agent — удалённо (167): RUNNING"
    else
      echo "  ❌ $agent — удалённо (167): STOPPED"
    fi
  done
  echo ""
  if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    echo "  📺 tmux сессия '$SESSION_NAME' активна"
    echo "  Для подключения: tmux attach -t $SESSION_NAME"
  else
    echo "  📺 tmux сессия '$SESSION_NAME' не активна"
  fi
}

case "${1:-start}" in
  start)
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║      РАСПРЕДЕЛЁННЫЙ ЗАПУСК АГЕНТОВ WATERS v1.0                  ║"
    echo "╠══════════════════════════════════════════════════════════════════╣"
    echo "║  Локально (238): ${LOCAL_AGENTS[*]}"
    echo "║  Удалённо (167): ${REMOTE_AGENTS[*]}"
    echo "║  Сессия tmux: $SESSION_NAME"
    echo "║  Логи: $LOG_DIR/"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""

    # Ensure remote host is reachable
    if ! ssh -o ConnectTimeout=5 "$REMOTE_HOST" uptime 2>/dev/null; then
      echo "❌ Сервер 167 недоступен. Проверьте SSH-соединение."
      exit 1
    fi
    echo "✅ Сервер 167 доступен"

    # Ensure tunnels are alive
    if curl -s --max-time 2 http://localhost:11434/api/tags >/dev/null 2>&1; then
      echo "✅ Ollama tunnel (238→237) работает"
    else
      echo "⚠️  Ollama tunnel (238→237) не отвечает, запуск..."
      sudo systemctl restart ollama-tunnel.service 2>/dev/null || true
    fi

    if ssh "$REMOTE_HOST" "curl -s --max-time 2 http://localhost:11434/api/tags >/dev/null 2>&1"; then
      echo "✅ Ollama tunnel (167→237) работает"
    else
      echo "⚠️  Ollama tunnel (167→237) не отвечает, запуск..."
      ssh "$REMOTE_HOST" "ssh -o StrictHostKeyChecking=no -o ServerAliveInterval=30 -o ExitOnForwardFailure=yes -fNL 11434:localhost:11434 ubuntu@171.22.180.237" 2>/dev/null || true
    fi

    echo ""
    echo "━━━ Троица Структуры (238) ━━━"
    for agent in "${LOCAL_AGENTS[@]}"; do
      start_agent_local "$agent"
      sleep 2
    done

    echo ""
    echo "━━━ Троица Смысла (167) ━━━"
    for agent in "${REMOTE_AGENTS[@]}"; do
      start_agent_remote "$agent"
      sleep 2
    done

    echo ""
    echo "✅ Запущено $(( ${#LOCAL_AGENTS[@]} + ${#REMOTE_AGENTS[@]} )) агентов"
    echo "   Для подключения: tmux attach -t $SESSION_NAME"
    echo "   Для остановки:   ./run_distributed.sh stop"
    echo "   Для проверки:    ./run_distributed.sh status"
    ;;
  attach)
    tmux attach -t "$SESSION_NAME"
    ;;
  stop)
    stop_all
    ;;
  status)
    status_all
    ;;
  *)
    echo "Использование: $0 {start|attach|stop|status}"
    exit 1
    ;;
esac
