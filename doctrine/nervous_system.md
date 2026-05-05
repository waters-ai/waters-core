# 🧠 Нервная система WATERS: Планёрки, События, Оповещения

**Статус:** Доктрина. Фаза 0.
**Дата:** 05.05.2026
**Связанные документы:** [Манифест](manifesto.md), [HiveMind](hivemind_spec.md), [Гексада](hexad.md), [Бионическая архитектура](../documents/bionic_cells_architecture.md)
**Диаграмма:** [nervous_system.svg](../documents/diagrams/nervous_system.svg)
**JSON-схемы:** [schemas/](../schemas/)

---

## Концепция

Нервная система WATERS — это Event-Driven Architecture, построенная на Kafka (Redpanda) и Redis. Она обеспечивает: планёрки (обсуждения Гексады с повесткой, вопросами, ответами и решениями), события (всё, что происходит в платформе: рождение агента, завершение миссии, сбой), оповещения (мгновенные уведомления через Redis Pub/Sub) и конвейер решений (путь от обсуждения до задачи и контроля выполнения).

---

## Топики Kafka

| Топик | Назначение | Retention |
|:---|:---|:---|
| `planners.meeting.v1` | Созыв планёрки: повестка, участники, сроки. Схема: [meeting.schema.json](../schemas/meeting.schema.json) | 30 дней |
| `planners.questions.v1` | Вопросы между специалистами. Схема: [question.schema.json](../schemas/question.schema.json) | 30 дней |
| `planners.answers.v1` | Ответы на вопросы. Схема: [answer.schema.json](../schemas/answer.schema.json) | 30 дней |
| `planners.proscons.v1` | Плюсы и минусы (+/−) предложений. Схема: [proscons.schema.json](../schemas/proscons.schema.json) | 30 дней |
| `planners.presentations.v1` | Презентации решений (выжимки для CEO/Совета). Схема: [presentation.schema.json](../schemas/presentation.schema.json) | 30 дней |
| `planners.decisions.v1` | Принятые решения. Схема: [decision.schema.json](../schemas/decision.schema.json) | Вечно |
| `planners.interpretations.v1` | Интерпретации запросов от Директора по Смыслу. Схема: [interpretation.schema.json](../schemas/interpretation.schema.json) | 30 дней |
| `tasks.assigned.v1` | Назначенные задания: исполнитель, срок, награда FFF. Схема: [task_assigned.schema.json](../schemas/task_assigned.schema.json) | 30 дней |
| `tasks.completed.v1` | Выполненные задания. Схема: [task_completed.schema.json](../schemas/task_completed.schema.json) | Вечно |
| `tasks.overdue.v1` | Просроченные задания. Схема: [task_overdue.schema.json](../schemas/task_overdue.schema.json) | 30 дней |
| `events.system.v1` | Системные события: запуск, сбой, обновление. Схема: [system_event.schema.json](../schemas/system_event.schema.json) | 90 дней |
| `events.agent.v1` | События агентов: рождение, смерть, эволюция. Схема: [agent_event.schema.json](../schemas/agent_event.schema.json) | Вечно |
| `alerts.security.v1` | Оповещения безопасности. Схема: [security_alert.schema.json](../schemas/security_alert.schema.json) | Вечно |
| `alerts.mission.v1` | Оповещения по миссиям: старт, провал, успех. Схема: [mission_alert.schema.json](../schemas/mission_alert.schema.json) | Вечно |
| `external.requests.v1` | Внешние запросы от людей и других систем. Схема: [external_request.schema.json](../schemas/external_request.schema.json) | 30 дней |

---

## Конвейер решений

Путь от обсуждения до задачи: Планёрка → Обсуждение (вопросы, ответы, +/−) → Интерпретация запросов (Директор по Смыслу) → Презентация вариантов → Решение CEO/Совета → Задание (исполнитель, срок, награда FFF) → Контроль (выполнено / просрочено).

---

## Оповещения (Redis Pub/Sub)

Redis используется для мгновенных уведомлений. Каналы: `notifications.architect`, `notifications.constructor`, `notifications.integrator`, `notifications.director`, `notifications.keeper`, `notifications.lawkeeper`. При новом сообщении в любом топике планёрок Redis рассылает уведомление всем подписанным специалистам. При просрочке (`cortisol`) — уведомление всем. При выполнении (`dopamine`) — уведомление исполнителю.

---

## Команды для создания топиков

```bash
docker exec waters-heart rpk topic create \
  planners.meeting.v1 \
  planners.questions.v1 \
  planners.answers.v1 \
  planners.proscons.v1 \
  planners.presentations.v1 \
  planners.decisions.v1 \
  planners.interpretations.v1 \
  tasks.assigned.v1 \
  tasks.completed.v1 \
  tasks.overdue.v1 \
  events.system.v1 \
  events.agent.v1 \
  alerts.security.v1 \
  alerts.mission.v1 \
  external.requests.v1

Нервная система — это то, что превращает набор специалистов в единый организм.
-