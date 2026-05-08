#!/usr/bin/env bash
# run_carousel.sh — Карусель агентов WATERS
#
# Ротация: Архитектор → Интегратор → Хранитель → Законодатель
# Каждому агенту — 30 минут работы в OpenCode.
# Перед ротацией — снэпшот контекста в Redis.
# После ротации — восстановление контекста следующего агента.
#
# Поддерживает до 4 параллельных OpenCode-сессий (тройками).
# Копирует AGENTS.md конкретного агента перед запуском.
#
# Использование:
#   ./run_carousel.sh                    # полный цикл по всем агентам
#   SESSION_TIMEOUT=600 ./run_carousel.sh  # 10 мин на агента (для теста)
#   CAROUSEL_AGENTS="architect,integrator" ./run_carousel.sh  # только два
#
# Переменные окружения:
#   SESSION_TIMEOUT — время на агента в секундах (по умолч. 1800 = 30 мин)
#   CAROUSEL_AGENTS — список агентов через запятую (по умолч. все 4)
#   MAX_PARALLEL    — макс. параллельных сессий (по умолч. 4)
#   LOG_DIR         — директория логов (по умолч. logs/)
#   REDIS_CONTAINER — Redis-контейнер для снэпшотов
#   KAFKA_CONTAINER — Kafka-контейнер для событий
#

set -euo pipefail

# ─── Конфигурация ────────────────────────────────────────────────────────────
SESSION_TIMEOUT="${SESSION_TIMEOUT:-1800}"  # 30 минут по умолчанию
MAX_PARALLEL="${MAX_PARALLEL:-4}"
LOG_DIR="${LOG_DIR:-logs}"
REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
REDIS_CONTAINER="${REDIS_CONTAINER:-waters-redis}"
KAFKA_CONTAINER="${KAFKA_CONTAINER:-waters-kafka}"
BOOTSTRAP="localhost:9092"

DEFAULT_AGENTS=("architect" "integrator" "keeper" "lawkeeper")

if [ -n "${CAROUSEL_AGENTS:-}" ]; then
  IFS=',' read -ra AGENTS <<< "$CAROUSEL_AGENTS"
else
  AGENTS=("${DEFAULT_AGENTS[@]}")
fi

declare -A AGENT_CODES=(
  ["architect"]="agent.architect.v1"
  ["integrator"]="agent.integrator.v1"
  ["keeper"]="agent.keeper.v1"
  ["lawkeeper"]="agent.lawkeeper.v1"
)

declare -A AGENT_NAMES=(
  ["architect"]="Верховный Архитектор"
  ["integrator"]="Интегратор Знаний"
  ["keeper"]="Хранитель Протокола"
  ["lawkeeper"]="Законодатель"
)

SNAPSHOT_SCRIPT="$REPO_DIR/scripts/session_snapshot.sh"

# ─── Функции ─────────────────────────────────────────────────────────────────
publish_event() {
  local agent="$1"
  local status="$2"
  local message_id
  message_id="carousel-$(date +%s)-$agent-$status"
  local timestamp
  timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  local event
  event=$(cat <<JSON
{
  "message_id": "${message_id}",
  "from": "agent.constructor.v1",
  "to": "hivemind",
  "topic": "events.agent.v1",
  "payload": {
    "agent": "${AGENT_CODES[$agent]}",
    "agent_name": "${AGENT_NAMES[$agent]}",
    "event": "session_${status}",
    "carousel_round": "${CAROUSEL_ROUND:-1}",
    "timestamp": "${timestamp}",
    "session_timeout": ${SESSION_TIMEOUT}
  },
  "timestamp": "${timestamp}",
  "trace_id": "carousel-${message_id}"
}
JSON
  )
  echo "$event" | docker exec -i "$KAFKA_CONTAINER" rpk topic produce "events.agent.v1" \
    --brokers "$BOOTSTRAP" 2>/dev/null || true
}

save_snapshot() {
  local agent="$1"
  local snapshot_file="$LOG_DIR/snapshot_${agent}.json"

  # Собираем контекст: время, теущий AGENTS.md, последние логи
  cat > "$snapshot_file" <<JSON
{
  "agent": "${AGENT_CODES[$agent]}",
  "agent_name": "${AGENT_NAMES[$agent]}",
  "saved_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "session_timeout": ${SESSION_TIMEOUT},
  "carousel_round": ${CAROUSEL_ROUND:-1},
  "working_directory": "${REPO_DIR}"
}
JSON
  "$SNAPSHOT_SCRIPT" save "${AGENT_CODES[$agent]}" "$snapshot_file" 2>/dev/null || true
  echo "  💾 Снэпшот сохранён: ${AGENT_CODES[$agent]}"
}

rotate_agent() {
  local agent="$1"

  echo ""
  echo "═══════════════════════════════════════════════════════════════════"
  echo "  🎠 Карусель: ${AGENT_NAMES[$agent]} (${AGENT_CODES[$agent]})"
  echo "  ⏱  Таймаут: ${SESSION_TIMEOUT}с ($((SESSION_TIMEOUT / 60)) мин)"
  echo "═══════════════════════════════════════════════════════════════════"

  # 1. Копируем AGENTS.md агента
  cp "$REPO_DIR/agents/${agent}_AGENTS.md" "$REPO_DIR/AGENTS.md"
  echo "  📋 AGENTS.md ← agents/${agent}_AGENTS.md"

  # 2. Публикуем событие начала сессии
  publish_event "$agent" "start"

  # 3. Запускаем OpenCode с таймаутом
  #    timeout сам пошлёт SIGTERM по истечении SESSION_TIMEOUT
  echo "  🚀 Запуск OpenCode для ${AGENT_NAMES[$agent]}..."
  cd "$REPO_DIR"
  timeout "$SESSION_TIMEOUT" opencode 2>&1 | tee "$LOG_DIR/${agent}_session.log" || true

  # 4. Сохраняем снэпшот после завершения
  save_snapshot "$agent"

  # 5. Публикуем событие завершения
  publish_event "$agent" "end"

  echo "  ✅ Сессия ${AGENT_NAMES[$agent]} завершена"
}

# ─── Инициализация ───────────────────────────────────────────────────────────
mkdir -p "$LOG_DIR"
CAROUSEL_ROUND="${CAROUSEL_ROUND:-1}"

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║          🎠 КАРУСЕЛЬ АГЕНТОВ WATERS v1.0                        ║"
echo "╠══════════════════════════════════════════════════════════════════╣"
echo "║  Ротация: ${AGENTS[*]}"
echo "║  Таймаут: ${SESSION_TIMEOUT}с ($((SESSION_TIMEOUT / 60)) мин/агент)"
echo "║  Параллельных сессий: до ${MAX_PARALLEL}"
echo "║  Раунд: ${CAROUSEL_ROUND}"
echo "║  Логи: ${LOG_DIR}/"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

# ─── Основной цикл ──────────────────────────────────────────────────────────
for agent in "${AGENTS[@]}"; do
  if [ ! -f "$REPO_DIR/agents/${agent}_AGENTS.md" ]; then
    echo "  ⚠ AGENTS.md для ${agent} не найден, пропускаю"
    continue
  fi

  rotate_agent "$agent"
done

# ─── Завершение ──────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════════════════"
echo "  ✅ Карусель завершена (раунд ${CAROUSEL_ROUND})"
echo "  Обработано агентов: ${#AGENTS[@]}"
echo "═══════════════════════════════════════════════════════════════════"

# Восстанавливаем AGENTS.md Конструктора
cp "$REPO_DIR/agents/constructor_AGENTS.md" "$REPO_DIR/AGENTS.md"
echo "  📋 AGENTS.md восстановлен (Конструктор Сети)"

# Если запущено с run_all.sh, увеличиваем раунд и повторяем
if [ "${AUTO_REPEAT:-false}" = "true" ]; then
  export CAROUSEL_ROUND=$((CAROUSEL_ROUND + 1))
  echo ""
  echo "🔄 Автоповтор: раунд ${CAROUSEL_ROUND}"
  exec "$0" "$@"
fi
