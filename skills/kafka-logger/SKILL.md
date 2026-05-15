---
name: kafka-logger
description: Логирование событий и метрик в Kafka-топики для централизованного сбора
---

# SKILL: kafka-logger v1.0

## Назначение

Скилл для логирования всех сообщений агентов в Kafka-топики. Обеспечивает полную трассируемость общения между агентами и компонентами платформы.

## Входные данные

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|----------|
| from | string | да | Код агента-отправителя (например, agent.architect.v1) |
| to | string | да | Код агента-получателя или сервиса |
| topic | string | да | Топик Kafka для отправки |
| payload | object | да | Полезная нагрузка сообщения |
| timestamp | string | нет | Время отправки (по умолчанию текущее) |

## Выходные данные

| Поле | Тип | Описание |
|------|-----|----------|
| status | string | Статус операции (success/error) |
| message_id | string | ID отправленного сообщения |
| topic | string | Топик, куда отправлено |

## Формат лога

```json
{
  "message_id": "uuid",
  "from": "agent.architect.v1",
  "to": "agent.constructor.v1",
  "topic": "planners.questions.v1",
  "payload": {
    "question_id": "uuid",
    "content": "Текст вопроса",
    "priority": "high"
  },
  "timestamp": "2026-05-07T10:30:00Z",
  "trace_id": "uuid"
}
```

## Топики для логирования

### Планёрки (planners.*)

| Топик | Схема | Retention |
|-------|-------|-----------|
| `planners.meeting.v1` | meeting.schema.json | 30 дней |
| `planners.questions.v1` | question.schema.json | 30 дней |
| `planners.answers.v1` | answer.schema.json | 30 дней |
| `planners.proscons.v1` | proscons.schema.json | 30 дней |
| `planners.presentations.v1` | presentation.schema.json | 30 дней |
| `planners.decisions.v1` | decision.schema.json | Вечно |
| `planners.interpretations.v1` | interpretation.schema.json | 30 дней |

### Скиллы (skills.*)

| Топик | Назначение |
|-------|------------|
| `skills.proposed.v1` | Предложенные скиллы |
| `skills.approved.v1` | Одобренные скиллы |
| `skills.rejected.v1` | Отклонённые скиллы |
| `skills.incident.v1` | Инциденты со скиллами |

### Задачи (tasks.*)

| Топик | Схема | Retention |
|-------|-------|-----------|
| `tasks.assigned.v1` | task_assigned.schema.json | 30 дней |
| `tasks.completed.v1` | task_completed.schema.json | Вечно |
| `tasks.overdue.v1` | task_overdue.schema.json | 30 дней |

## Алгоритм работы

1. Валидация входных данных по схеме топика
2. Обогащение сообщения (добавление message_id, timestamp, trace_id)
3. Публикация в Kafka-топик
4. Подтверждение публикации
5. Возврат статуса

## Пример использования

```python
from kafka_logger import KafkaLogger

logger = KafkaLogger(broker="localhost:9092")

# Логирование вопроса
result = logger.log(
    from_agent="agent.architect.v1",
    to_agent="agent.constructor.v1",
    topic="planners.questions.v1",
    payload={
        "question_id": "uuid",
        "content": "Как настроить Redpanda?",
        "priority": "high"
    }
)

print(result["status"])  # success
```

## Зависимости

- Kafka/Redpanda (брокер сообщений)
- Схемы JSON из `schemas/`
- Библиотека kafka-python или confluent-kafka

## Ограничения

- Размер сообщения не более 1 MB
- Топик должен существовать (предварительно создан)
- Payload должен соответствовать схеме топика

## Интеграция

Скилл используется всеми агентами Гексады для:
- Соблюдения протокола HiveMind
- Обеспечения прозрачности общения
- Аудита действий агентов
- Отладки и мониторинга
