#!/bin/bash
# run_kapelka.sh — запуск всех агентов Kapelka
# Использует run_terminals.sh паттерн из WATERS core
#
# Режимы:
#   ./run_kapelka.sh          # Запуск всех агентов (фоновые процессы)
#   ./run_kapelka.sh stop     # Остановка всех агентов
#   ./run_kapelka.sh status   # Статус процессов

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGENTS_DIR="${SCRIPT_DIR}/agents"
LOG_DIR="${SCRIPT_DIR}/logs"
PID_DIR="${SCRIPT_DIR}/.pids"
KAPELKA_WORKSPACE="${SCRIPT_DIR}"

# === Настройки ===
KAPELKA_MODE="${KAPELKA_MODE:-file}"  # kafka или file
OLLAMA_HOST="${OLLAMA_HOST:-171.22.180.237}"
OLLAMA_PORT="${OLLAMA_PORT:-11434}"
OLLAMA_MODEL="${OLLAMA_MODEL:-qwen2.5:14b}"

mkdir -p "${LOG_DIR}" "${PID_DIR}"

export KAPELKA_MODE
export KAPELKA_WORKSPACE
export OLLAMA_HOST OLLAMA_PORT OLLAMA_MODEL

log() { echo "[kapelka] $(date '+%H:%M:%S') $*"; }

start_agent() {
    local name="$1"
    local script="$2"
    local pid_file="${PID_DIR}/${name}.pid"
    local log_file="${LOG_DIR}/${name}.log"

    # Проверка, не запущен ли уже
    if [ -f "${pid_file}" ]; then
        local old_pid
        old_pid=$(cat "${pid_file}")
        if kill -0 "${old_pid}" 2>/dev/null; then
            log "⚠️  ${name} уже запущен (PID ${old_pid})"
            return 0
        fi
        rm -f "${pid_file}"
    fi

    # Запуск
    nohup python3 "${script}" > "${log_file}" 2>&1 &
    local pid=$!
    echo "${pid}" > "${pid_file}"
    log "✅ ${name} запущен (PID ${pid})"
}

stop_agent() {
    local name="$1"
    local pid_file="${PID_DIR}/${name}.pid"

    if [ -f "${pid_file}" ]; then
        local pid
        pid=$(cat "${pid_file}")
        if kill -0 "${pid}" 2>/dev/null; then
            kill "${pid}" 2>/dev/null || true
            log "⏹  ${name} остановлен (PID ${pid})"
        else
            log "⚠️  ${name} не запущен"
        fi
        rm -f "${pid_file}"
    fi
}

status_agent() {
    local name="$1"
    local pid_file="${PID_DIR}/${name}.pid"

    if [ -f "${pid_file}" ]; then
        local pid
        pid=$(cat "${pid_file}")
        if kill -0 "${pid}" 2>/dev/null; then
            log "✅ ${name} работает (PID ${pid})"
        else
            log "❌ ${name} мёртв (PID ${pid} не найден)"
            rm -f "${pid_file}"
        fi
    else
        log "⬜ ${name} не запущен"
    fi
}

# === Команды ===
case "${1:-start}" in
    start)
        log "Запуск всех агентов Kapelka..."
        start_agent "architect"   "${AGENTS_DIR}/agent_architect.py"
        start_agent "constructor" "${AGENTS_DIR}/agent_constructor.py"
        start_agent "auditor"     "${AGENTS_DIR}/agent_auditor.py"
        log "Готово. Логи: ${LOG_DIR}"
        log "Для просмотра: tail -f ${LOG_DIR}/*.log"
        ;;

    stop)
        log "Остановка всех агентов..."
        stop_agent "architect"
        stop_agent "constructor"
        stop_agent "auditor"
        log "Все агенты остановлены"
        ;;

    restart)
        "${0}" stop
        sleep 1
        "${0}" start
        ;;

    status)
        log "Статус агентов:"
        status_agent "architect"
        status_agent "constructor"
        status_agent "auditor"
        ;;

    *)
        echo "Использование: $0 [start|stop|restart|status]"
        exit 1
        ;;
esac
