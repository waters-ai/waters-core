# 🧠 Нервная система WATERS: Планёрки, События, Оповещения

**Статус:** Доктрина. Фаза 0.
**Дата:** 05.05.2026
**Связанные документы:** [Манифест](manifesto.md), [HiveMind](hivemind_spec.md), [Гексада](hexad.md), [Бионическая архитектура](../documents/bionic_cells_architecture.md)
**Диаграмма:** [nervous_system.svg](../documents/diagrams/nervous_system.svg)
**JSON-схемы:** [schemas/](../schemas/)

---

## Концепция

Нервная система WATERS — это Event-Driven Architecture, построенная на Apache Kafka и Redis. Она обеспечивает: планёрки (обсуждения Гексады с повесткой, вопросами, ответами и решениями), события (всё, что происходит в платформе: рождение агента, завершение миссии, сбой), оповещения (мгновенные уведомления через Redis Pub/Sub) и конвейер решений (путь от обсуждения до задачи и контроля выполнения).

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
| `external.responses.v1` | Ответы внешним системам | 30 дней |
| `knowledge.articles.v1` | Новые статьи базы знаний (ChromaDB + LightRAG). Схема: [knowledge_article.schema.json](../schemas/knowledge_article.schema.json) | 90 дней |
| `knowledge.graph.v1` | Обновления графа знаний (ноды, рёбра, связи). Схема: [knowledge_graph_update.schema.json](../schemas/knowledge_graph_update.schema.json) | 90 дней |

### Новые топики (v1.0.1 — Конструктор Сети)

| Топик | Назначение | Retention |
|:---|:---|:---|
| `orders.constructor.v1` | Приказы Конструктора подчинённым (аудиторам, исполнителям). Формат: `{ from, to, order, deadline, priority }` | 30 дней |
| `secrets.requests.v1` | Запрос на расшифровку/зашифровку секретов (через Хранителя). Формат: `{ agent_id, action: encrypt/decrypt, payload }` | 1 день |
| `secrets.responses.v1` | Ответ Хранителя с расшифрованными/зашифрованными данными | 1 день |
| `skills.proposed.v1` | Предложенные скиллы (Integrator → Constructor) | 30 дней |
| `skills.approved.v1` | Одобренные скиллы (Constructor → Integrator) | Вечно |
| `skills.rejected.v1` | Отклонённые скиллы | 30 дней |
| `skills.incident.v1` | Инциденты со скиллами | Вечно |
| `metrics.raw.v1` | Сырые метрики (от Компонентов → Конструктору) | 90 дней |
| `metrics.kpi.v1` | Агрегированные KPI | Вечно |
| `security.audit.v1` | События аудита безопасности | Вечно |
| `oversight.compliance.v1` | Отчёты о соблюдении Ясы (Законодатель) | Вечно |
| `oversight.reports.v1` | Общие отчёты надзора | 90 дней |
| `future_scans.v1` | Результаты эхолокации будущего (Законодатель) | Вечно |
| `future_scans.proposals.v1` | Предложения по новым специалистам | 90 дней |
| `events.external_ai.v1` | События от внешних AI-систем | 90 дней |

---

## Конвейер решений

Путь от обсуждения до задачи: Планёрка → Обсуждение (вопросы, ответы, +/−) → Интерпретация запросов (Директор по Смыслу) → Презентация вариантов → Решение CEO/Совета → Задание (исполнитель, срок, награда FFF) → Контроль (выполнено / просрочено).

---

## Оповещения (Redis Pub/Sub)

Redis используется для мгновенных уведомлений. Каналы: `notifications.architect`, `notifications.constructor`, `notifications.integrator`, `notifications.director`, `notifications.keeper`, `notifications.lawkeeper`. При новом сообщении в любом топике планёрок Redis рассылает уведомление всем подписанным специалистам. При просрочке (`cortisol`) — уведомление всем. При выполнении (`dopamine`) — уведомление исполнителю.

---

## Команды для создания топиков

```bash
# Локальный сервер
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic planners.meeting.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic planners.questions.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic planners.answers.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic planners.proscons.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic planners.presentations.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic planners.decisions.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic planners.interpretations.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic tasks.assigned.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic tasks.completed.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic tasks.overdue.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic events.system.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic events.agent.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic alerts.security.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic alerts.mission.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic external.requests.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic external.responses.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic knowledge.articles.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic knowledge.graph.v1

# Новые топики (v1.0.1)
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic orders.constructor.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic secrets.requests.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic secrets.responses.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic skills.proposed.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic skills.approved.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic skills.rejected.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic skills.incident.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic metrics.raw.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic metrics.kpi.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic security.audit.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic oversight.compliance.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic oversight.reports.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic future_scans.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic future_scans.proposals.v1
docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --create --topic events.external_ai.v1

Нервная система — это то, что превращает набор специалистов в единый организм.
-