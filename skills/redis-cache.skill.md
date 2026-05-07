# SKILL: redis-cache v1.0

## Назначение

Скилл для управления кэшированием через Redis. Обеспечивает проверку кэша перед запросом к внешним AI, сохранение ответов и инвалидацию при обновлении данных.

## Входные данные

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|----------|
| operation | string | да | Тип операции: `get`, `set`, `delete`, `invalidate` |
| source | string | да | Источник данных: `ai`, `chromadb`, `search`, `schema` |
| query | object | да (для get/set) | Запрос или данные для кэширования |
| ttl | integer | нет | Время жизни в секундах (по умолчанию 3600) |
| hash | string | нет | Предварительно вычисленный hash (опционально) |

## Выходные данные

| Поле | Тип | Описание |
|------|-----|----------|
| status | string | Статус операции: `hit`, `miss`, `stored`, `deleted` |
| data | object | Данные из кэша (для `get`) |
| cache_key | string | Использованный ключ кэша |
| ttl_remaining | integer | Оставшееся время жизни (секунды) |

## Формат ключей

```
cache:<source>:<hash>
```

### Примеры ключей

| Ключ | Описание |
|------|----------|
| `cache:ai:abc123def456` | Ответ от внешнего AI |
| `cache:chromadb:xyz789` | Результат запроса к ChromaDB |
| `cache:search:exa:qwerty` | Результат поиска через Exa |
| `cache:schema:hivemind_military` | Схема HiveMind (военная) |

## Алгоритм работы

### 1. Проверка кэша (operation: get)

```python
def get_from_cache(source, query):
    # Генерируем ключ
    cache_key = generate_cache_key(source, query)

    # Проверяем наличие
    cached_data = redis.get(cache_key)

    if cached_data:
        ttl = redis.ttl(cache_key)
        return {
            "status": "hit",
            "data": json.loads(cached_data),
            "cache_key": cache_key,
            "ttl_remaining": ttl
        }

    return {
        "status": "miss",
        "cache_key": cache_key,
        "ttl_remaining": 0
    }
```

### 2. Сохранение в кэш (operation: set)

```python
def save_to_cache(source, query, response, ttl=None):
    cache_key = generate_cache_key(source, query)

    # Определяем TTL по типу данных
    if ttl is None:
        ttl = get_default_ttl(source)

    # Сохраняем с TTL
    redis.setex(cache_key, ttl, json.dumps(response))

    return {
        "status": "stored",
        "cache_key": cache_key,
        "ttl_remaining": ttl
    }
```

### 3. Инвалидация кэша (operation: invalidate)

```python
def invalidate_cache(pattern):
    # Удаляем по паттерну
    keys = redis.keys(pattern)
    if keys:
        redis.delete(*keys)
        return {"status": "deleted", "count": len(keys)}
    return {"status": "miss", "count": 0}
```

## TTL (время жизни)

| Тип данных | TTL | Секунды | Обоснование |
|------------|-----|---------|-------------|
| Частые запросы к AI | 1 час | 3600 | Высокая вероятность повтора |
| Результаты поиска | 6 часов | 21600 | Средняя стабильность |
| Конфигурации | 24 часа | 86400 | Редко меняются |
| Схемы JSON | 7 дней | 604800 | Стабильные данные |
| Редкие запросы | 24 часа | 86400 | Защита от устаревания |

## Генерация hash

```python
import hashlib
import json

def generate_cache_key(source, query):
    # Нормализуем запрос (сортируем ключи)
    normalized = json.dumps(query, sort_keys=True)
    # Генерируем hash (берём первые 16 символов)
    hash_value = hashlib.sha256(normalized.encode()).hexdigest()[:16]
    return f"cache:{source}:{hash_value}"
```

## Примеры использования

### Пример 1: Проверка кэша перед запросом к AI

```python
from redis_cache import RedisCache

cache = RedisCache(host="localhost", port=6379)

# Проверяем кэш
result = cache.get(source="ai", query={"prompt": "Объясни HiveMind"})

if result["status"] == "hit":
    print("Данные из кэша:", result["data"])
else:
    print("Кэш пуст, делаем запрос к AI...")
    # Запрашиваем AI
    response = call_ai("Объясни HiveMind")
    # Сохраняем в кэш
    cache.set(source="ai", query={"prompt": "Объясни HiveMind"}, response=response, ttl=3600)
```

### Пример 2: Инвалидация при обновлении схемы

```python
# После обновления схемы HiveMind
cache.invalidate("cache:schema:hivemind_*")
print("Кэш схем очищен")
```

## Зависимости

- Redis (кэш)
- Python: `redis`, `hashlib`, `json`

## Интеграция с Нервной системой

- Метрики кэша отправляются в `metrics.raw.v1` (cache_hit, cache_miss)
- События инвалидации в `events.system.v1`
- Экономия токенов учитывается в KPI Конструктора

## Мониторинг

| Метрика | Описание | Цель |
|---------|----------|------|
| cache_hit_rate | Доля попаданий | > 60% |
| tokens_saved | Сэкономленные токены | Максимизация |
| cache_size | Размер кэша (MB) | < 100 MB |
| eviction_count | Количество вытеснений | Минимизация |
