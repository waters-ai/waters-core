---
name: external-search
description: Поиск внешних знаний через DMZ-шлюз (Brave Search API) с сохранением в ChromaDB и Kafka
---

# SKILL: external-search v1.0

## Назначение

Поиск внешних знаний через DMZ-шлюз (Brave Search API) с сохранением в ChromaDB и публикацией в Kafka. Обеспечивает легальное получение информации из внешнего мира.

## Входные данные

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|----------|
| query | string | да | Поисковый запрос |
| count | integer | нет | Количество результатов (max 10 для бесплатного тарифа) |
| save_to_kafka | boolean | нет | Отправлять ли в `knowledge.articles.v1` (default: true) |
| save_to_chromadb | boolean | нет | Сохранять ли в ChromaDB (default: true) |
| language | string | нет | Язык поиска (default: "ru") |

## Выходные данные

| Поле | Тип | Описание |
|------|-----|----------|
| status | string | Статус операции: "success", "cache_hit", "error" |
| articles_found | integer | Количество найденных статей |
| articles_saved | integer | Количество сохраненных статей |
| chromadb_collection | string | Коллекция ChromaDB ("external_articles") |
| cache_key | string | Ключ кэша (для отладки) |
| articles | array | Список статей (если запрошено) |

## Формат запроса к DMZ

```json
{
  "method": "brave_search",
  "params": {
    "query": "HiveMind protocol WATERS",
    "count": 5,
    "save_to_kafka": true,
    "save_to_chromadb": true
  }
}
```

## Формат ответа

```json
{
  "status": "success",
  "articles_found": 5,
  "articles_saved": 5,
  "chromadb_collection": "external_articles",
  "cache_key": "cache:brave:abc123def456",
  "articles": [
    {
      "article_id": "brave_123456",
      "title": "HiveMind Protocol",
      "content": "Description...",
      "url": "https://example.com",
      "source": "external",
      "author": "agent.integrator.v1"
    }
  ]
}
```

## Алгоритм работы

1. **Валидация запроса** (проверка параметров)
2. **Проверка кэша** (Redis: `cache:brave:<hash>`)
   - Если HIT → возврат из кэша (status: "cache_hit")
3. **Запрос к DMZ** (external-search:8081)
   - Если MISS → запрос к Brave Search API
4. **Нормализация данных** (в формат `knowledge_article.schema.json`)
5. **Сохранение в ChromaDB** (если `save_to_chromadb=true`)
6. **Публикация в Kafka** (если `save_to_kafka=true`)
   - Топик `knowledge.articles.v1` — статьи
   - Топик `external.responses.v1` — ответы
7. **Сохранение в кэш** (TTL 1 час)
8. **Возврат результата**

## Пример использования

### Python
```python
from external_search import ExternalSearchSkill

skill = ExternalSearchSkill()
result = skill.execute(
    query="Autonomous agents for space colonization",
    count=5,
    save_to_kafka=True,
    save_to_chromadb=True
)

print(f"Found {result['articles_found']} articles")
print(f"Saved {result['articles_saved']} to ChromaDB")
```

### JSON-RPC (через MCP-прокси)
```json
{
  "jsonrpc": "2.0",
  "method": "brave_search",
  "params": {
    "query": "WATERS platform",
    "count": 3
  },
  "id": 1
}
```

## Зависимости

| Зависимость | Назначение | Где находится |
|-------------|------------|----------------|
| Brave Search API | Источник данных (2000 req/mes бесплатно) | DMZ: external-search:8081 |
| Redis (DMZ) | Кэширование запросов | DMZ: redis-dmz:6380 |
| ChromaDB | Векторный поиск | waters-chroma:8000 |
| Kafka | Публикация событий | waters-kafka:9092 |

## Интеграция с Нервной системой

### Читает топики
- `external.requests.v1` — входящие запросы агентов

### Пишет в топики
- `external.responses.v1` — ответы на запросы
- `knowledge.articles.v1` — новые статьи для базы знаний
- `events.external_ai.v1` — логи внешних запросов

## Безопасность

- ✅ Только через DMZ (прямой доступ запрещен)
- ✅ API key хранится в .env (не в коде)
- ✅ Rate limiting: 100 req/min (настроено в nginx)
- ✅ TTL кэша: 1 час (экономия квот)

## Ограничения

- Бесплатный тариф: 2000 запросов/месяц
- Максимум 10 результатов за запрос
- Только публичные данные (без аутентификации)
- Без сохранения внутренних документов доктрины

## Связанные файлы

- `scripts/mcp_brave_search.py` — реализация адаптера
- `scripts/Dockerfile.brave-search` — Docker-образ
- `docker-compose.yml` — сервис brave-search
- `documents/dmz_architecture.md` — архитектура DMZ
- `integrations/dmz_products_and_systems.md` — описание продуктов DMZ

## KPI для Интегратора

| Метрика | Цель |
|----------|------|
| Успешность запросов | > 95% |
| Cache hit rate | > 60% |
| Статей в ChromaDB | Рост на 100+ в день |
| Экономия токенов | За счёт кэша |
