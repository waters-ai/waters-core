---
name: llm-fallback
description: Fallback между LLM-провайдерами при недоступности основного — DeepSeek ↔ Ollama
---

# SKILL: llm-fallback v1.0

## Назначение

Стратегия автоматического переключения между LLM-провайдерами при отказах, таймаутах или деградации качества. Обеспечивает бесперебойную работу агентов Гексады через ротацию: DeepSeek → OpenAI → Anthropic → локальная Ollama.

## Входные данные

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|----------|
| prompt | string | да | Текст запроса к LLM |
| preferred_provider | string | нет | Предпочитаемый провайдер (default: "deepseek") |
| max_retries | integer | нет | Макс. попыток переключения (default: 3) |
| timeout_seconds | integer | нет | Таймаут на один запрос (default: 30) |
| preserve_context | boolean | нет | Сохранять ли контекст при переключении (default: true) |
| emergency_fallback | boolean | нет | Аварийный fallback на локальную Ollama (default: true) |

## Выходные данные

| Поле | Тип | Описание |
|------|-----|----------|
| status | string | "success", "fallback_used", "all_providers_failed" |
| provider | string | Какой провайдер ответил |
| output | string | Ответ LLM |
| attempts | array | История попыток (провайдер, код ошибки, latency_ms) |
| total_latency_ms | integer | Общее время с учётом fallback-ов |

## Провайдеры (порядок fallback)

| # | Провайдер | API | Таймаут | Cost rank | Когда использовать |
|---|-----------|-----|---------|-----------|-------------------|
| 1 | DeepSeek | deepseek/deepseek-v4-flash | 30s | 1 (дёшево) | Основной, всегда первый |
| 2 | OpenAI | gpt-4o / gpt-4-turbo | 30s | 3 (дорого) | Если DeepSeek недоступен |
| 3 | Anthropic | claude-opus-4 / claude-sonnet-4 | 45s | 4 (очень дорого) | Если OpenAI тоже отказал |
| 4 | Ollama (local) | localhost:11434 | 120s | 0 (бесплатно) | Аварийный, когда все API отказали |

## Алгоритм работы

```
1. ПРОВЕРИТЬ: health-чек preferred_provider
   ├── OK → отправить запрос
   │   ├── Успех → вернуть результат (status: success)
   │   └── Ошибка → перейти к следующему провайдеру
   └── FAIL → перейти к следующему провайдеру (status: fallback_used)

2. ДЛЯ КАЖДОГО fallback-провайдера:
   ├── Проверить health
   ├── Восстановить контекст (если preserve_context)
   ├── Отправить запрос с контекстом неудачи предыдущего
   └── Успех или конец списка

3. ЕСЛИ ВСЕ ПРОВАЙДЕРЫ НЕДОСТУПНЫ:
   ├── Опубликовать алерт в alerts.mission.v1
   ├── Сохранить запрос в Redis (queue:pending_llm_requests)
   └── Вернуть статус: all_providers_failed
```

## Health-check

```python
health_endpoints = {
    "deepseek": "https://api.deepseek.com/health",
    "openai": "https://api.openai.com/v1/models",
    "anthropic": "https://api.anthropic.com/v1/messages",
    "ollama": "http://localhost:11434/api/tags"
}
# Проверка: GET endpoint, таймаут 5s, ожидаем 200
```

## Сохранение контекста при fallback

- Сериализовать историю диалога в JSON
- Сохранить в Redis: `llm:context:{session_id}` (TTL: 1 час)
- При переключении: восстановить из Redis + добавить ноту о смене провайдера
- Ключи Redis: `llm:context:*`, `llm:metrics:*`

## Метрики

| Метрика | Куда | Описание |
|---------|------|----------|
| llm_request_total | metrics.raw.v1 | Всего запросов к LLM |
| llm_fallback_count | metrics.raw.v1 | Сколько раз сработал fallback |
| llm_provider_{name}_latency | metrics.raw.v1 | Задержка по каждому провайдеру |
| llm_provider_{name}_errors | metrics.raw.v1 | Ошибки по каждому провайдеру |
| llm_all_down_events | metrics.kpi.v1 | Сколько раз все провайдеры отказали |

## Зависимости

- API-ключи провайдеров (только через `secrets.*` и Хранителя)
- Redis для кэша контекста и очереди
- Ollama (локальный, для аварийного режима)
- Kafka (alerts.mission.v1, metrics.raw.v1)

## Ограничения

- Не поддерживает стриминг при fallback (только полный ответ)
- При смене провайдера может измениться стиль ответа
- Локальная Ollama — только для простых задач (без сложного reasoning)
- Все API-ключи — только через Хранителя (`secrets.requests.v1` / `secrets.responses.v1`)
- В аварийном режиме (все провайдеры отказали) — запросы становятся в очередь Redis
