#!/usr/bin/env bash
# create_remote_topics.sh — Создание Kafka-топиков на сервере 171.22.180.238
#
# Создаёт все топики для Гексады WATERS на удалённом сервере.
# Использует Redpanda rpk через docker exec (как и локальные скрипты).
#
# Топики разделены на:
#   planners.*  — планёрки и обсуждения
#   tasks.*     — задачи и контроль выполнения
#   events.*    — события системы и агентов
#   alerts.*    — оповещения безопасности и миссий
#   skills.*    — управление скиллами
#   knowledge.* — база знаний и граф
#   orders.*    — приказы Конструктора (новый)
#   secrets.*   — шифрованный обмен (новый)
#   metrics.*   — метрики и KPI
#   oversight.* — надзор Законодателя
#   external.*  — внешние запросы
#   security.*  — аудит безопасности
#   future_scans.* — эхолокация будущего
#
# Использование:
#   SSH_KEY=/path/to/key ./scripts/create_remote_topics.sh
#   или: export SSH_KEY=~/.ssh/id_rsa && ./scripts/create_remote_topics.sh
#
# Переменные окружения:
#   SSH_KEY      — путь к SSH-ключу (по умолчанию ~/.ssh/id_rsa)
#   REMOTE_HOST  — удалённый хост (по умолчанию 171.22.180.238)
#   REMOTE_USER  — пользователь SSH (по умолчанию root)
#   KAFKA_CONTAINER — имя Kafka-контейнера на удалённом сервере (по умолч. waters-redpanda)
#

set -euo pipefail

# ─── Конфигурация ────────────────────────────────────────────────────────────
SSH_KEY="${SSH_KEY:-$HOME/.ssh/id_rsa}"
REMOTE_HOST="${REMOTE_HOST:-171.22.180.238}"
REMOTE_USER="${REMOTE_USER:-root}"
KAFKA_CONTAINER="${KAFKA_CONTAINER:-waters-kafka}"
BOOTSTRAP="${BOOTSTRAP:-localhost:9092}"

PARTITIONS=1
REPLICATION=1

# ─── Полный список топиков HiveMind ──────────────────────────────────────────
TOPICS=(
  # Планёрки (30 дней, кроме decisions — вечно)
  planners.meeting.v1
  planners.questions.v1
  planners.answers.v1
  planners.proscons.v1
  planners.presentations.v1
  planners.decisions.v1
  planners.interpretations.v1

  # Задачи
  tasks.assigned.v1
  tasks.completed.v1
  tasks.overdue.v1

  # События
  events.system.v1
  events.agent.v1

  # Оповещения
  alerts.security.v1
  alerts.mission.v1

  # Скиллы
  skills.proposed.v1
  skills.approved.v1
  skills.rejected.v1
  skills.incident.v1

  # Знания
  knowledge.articles.v1
  knowledge.graph.v1

  # Метрики
  metrics.raw.v1
  metrics.kpi.v1

  # Внешние запросы
  external.requests.v1
  external.responses.v1

  # ─── НОВЫЕ ТОПИКИ ──────────────────────────────────────────────────────
  # Приказы Конструктора
  orders.constructor.v1

  # Шифрованный обмен секретами (Хранитель)
  secrets.requests.v1
  secrets.responses.v1

  # Надзор Законодателя
  oversight.compliance.v1
  oversight.reports.v1

  # Аудит безопасности
  security.audit.v1

  # Эхолокация будущего
  future_scans.v1
  future_scans.proposals.v1

  # События внешних AI
  events.external_ai.v1
)

# ─── Функции ─────────────────────────────────────────────────────────────────
create_topic() {
  local topic="$1"
  echo "  ➜ Создание топика: $topic"
  ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new \
    "${REMOTE_USER}@${REMOTE_HOST}" \
    "docker exec $KAFKA_CONTAINER rpk topic create $topic \
      --brokers $BOOTSTRAP \
      --partitions $PARTITIONS \
      --replication-factor $REPLICATION" 2>/dev/null || \
    echo "    ⚠ Топик $topic уже существует или ошибка создания"
}

# ─── Исполнение ─────────────────────────────────────────────────────────────
echo "═══════════════════════════════════════════════════════════════════"
echo "  Создание Kafka-топиков WATERS на $REMOTE_HOST"
echo "═══════════════════════════════════════════════════════════════════"
echo ""
echo "Контейнер: $KAFKA_CONTAINER"
echo "Всего топиков: ${#TOPICS[@]}"
echo ""

# Проверка SSH-доступа
echo "[0/4] Проверка SSH-доступа к $REMOTE_HOST..."
if ! ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new \
     -o ConnectTimeout=5 \
     "${REMOTE_USER}@${REMOTE_HOST}" "docker ps --format '{{.Names}}' | grep -q $KAFKA_CONTAINER"; then
  echo "❌ Не удалось подключиться к $REMOTE_HOST или контейнер $KAFKA_CONTAINER не найден"
  echo ""
  echo "Проверьте:"
  echo "  ssh -i $SSH_KEY ${REMOTE_USER}@${REMOTE_HOST}"
  echo "  docker ps | grep $KAFKA_CONTAINER"
  exit 1
fi
echo "✅ Доступен"
echo ""

# Создание топиков партиями по 5 для прогресс-бара
echo "[1/4] Создание planners.* (7 топиков)..."
for t in "${TOPICS[@]:0:7}"; do create_topic "$t"; done

echo ""
echo "[2/4] Создание tasks.* + events.* + alerts.* (6 топиков)..."
for t in "${TOPICS[@]:7:6}"; do create_topic "$t"; done

echo ""
echo "[3/4] Создание skills.* + knowledge.* + metrics.* + external.* (8 топиков)..."
for t in "${TOPICS[@]:13:8}"; do create_topic "$t"; done

echo ""
echo "[4/4] Создание новых топиков (orders.*, secrets.*, oversight.*, security.*, future_scans.*)..."

for t in "${TOPICS[@]:21}"; do create_topic "$t"; done

echo ""
echo "═══════════════════════════════════════════════════════════════════"
echo "✅ Все топики созданы на $REMOTE_HOST (${#TOPICS[@]} шт)"
echo "═══════════════════════════════════════════════════════════════════"
echo ""
echo "Проверить:"
echo "  ssh -i $SSH_KEY ${REMOTE_USER}@${REMOTE_HOST} 'docker exec $KAFKA_CONTAINER rpk topic list --brokers $BOOTSTRAP'"
