#!/usr/bin/env bash
# ask_integrator_for_h2o_audit.sh — Запрос Интегратору через Kafka
#
# Конструктор Сети v1.0 запрашивает у Интегратора Знаний:
#   - скиллы и информацию для аудита H2O-сайта
#   - React + Vite + Anchor + Solana + Nginx + CI/CD
#
# Топик: planners.questions.v1
# Формат: question.schema.json (message_id, from, to, payload)
#
# Зависимости: docker (Redpanda контейнер с rpk)
# Использование:
#   ./scripts/ask_integrator_for_h2o_audit.sh
#

set -euo pipefail

KAFKA_CONTAINER="waters-kafka"
BOOTSTRAP="localhost:9092"
TOPIC="planners.questions.v1"
MESSAGE_ID="$(uuidgen 2>/dev/null || date +%s | md5sum | head -c 32)"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
TRACE_ID="trace-constructor-h2o-audit-$(date +%s)"

echo "=== Конструктор Сети → Интегратору: запрос скиллов для аудита H2O ==="
echo ""

# 1. Создаём топик (если нет — игнорируем ошибку)
echo "[1/2] Создание топика $TOPIC..."
docker exec "$REDPANDA_CONTAINER" rpk topic create "$TOPIC" \
  --brokers "$BOOTSTRAP" 2>/dev/null || true

# 2. Формируем и отправляем сообщение
PAYLOAD=$(cat <<JSON
{
  "message_id": "${MESSAGE_ID}",
  "from": "agent.constructor.v1",
  "to": "agent.integrator.v1",
  "topic": "planners.questions.v1",
  "payload": {
    "question_id": "${MESSAGE_ID}",
    "subject": "H2O site audit — request skills and intelligence",
    "content": "Конструктор Сети запрашивает у Интегратора Знаний подбор скиллов и информации для аудита сайта H2O. Требуемые области: React 18+, Vite, Anchor Framework, Solana/web3, Nginx, CI/CD (GitHub Actions). Необходимо: 1) лучшие практики безопасности для каждой технологии; 2) типовые уязвимости и методы их обнаружения; 3) инструменты аудита (npm audit, lighthouse, anchor test, SSLyze, actionlint). Результат ожидается в skills.proposed.v1 и как ответ в planners.answers.v1.",
    "priority": "high",
    "deadline": "2026-05-08T12:00:00Z",
    "context": {
      "mission": "audit_react_solana_nginx",
      "technologies": ["React 18+", "Vite", "Anchor Framework", "Solana/web3", "Nginx", "CI/CD (GitHub Actions)"],
      "audit_areas": ["security", "performance", "dependencies", "deployment_pipeline"]
    }
  },
  "timestamp": "${TIMESTAMP}",
  "trace_id": "${TRACE_ID}"
}
JSON
)

echo "[2/2] Отправка сообщения в $TOPIC..."
echo "$PAYLOAD" | docker exec -i "$REDPANDA_CONTAINER" rpk topic produce "$TOPIC" \
  --brokers "$BOOTSTRAP" \
  --key "h2o-audit-request"

echo ""
echo "✅ Запрос отправлен Интегратору."
echo "   message_id: ${MESSAGE_ID}"
echo "   trace_id:   ${TRACE_ID}"
echo "   topic:      ${TOPIC}"
echo ""
echo "Ожидать ответ в: planners.answers.v1"
echo "Ожидать скиллы в: skills.proposed.v1"
