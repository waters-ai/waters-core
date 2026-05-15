---
name: redis-cache
description: Кэширование состояния агентов в Redis с TTL и фоновой записью в LightRAG
---

# SKILL.md — redis-cache

## Навык: кэширование через Redis для экономии токенов и ускорения запросов

### Идентификация

| Поле | Значение |
|------|----------|
| **skill_id** | `redis-cache` |
| **версия** | 1.0.0 |
| **владелец** | `agent.integrator.v1` |
| **тип** | `support` |
| **слой** | I — Сеть |
| **статус** | `active` |

### Назначение

`redis-cache` — навык для управления кэшированием через Redis. Проверяет кэш перед запросом к внешним AI, сохраняет результаты, инвалидирует при обновлениях. Основная цель: экономия токенов и ускорение ответов.

### Структура ключей

Формат: `cache:<source>:<hash>`

| Паттерн | Описание |
|---------|----------|
| `cache:ai:<hash>` | Ответ от внешнего AI (Claude, GPT) |
| `cache:chromadb:<hash>` | Результат запроса к ChromaDB |
| `cache:search:exa:<hash>` | Результат поиска через Exa |
| `cache:search:brave:<hash>` | Результат поиска через Brave |
| `cache:schema:<name>` | Схема JSON (hivemind_military, etc.) |
| `cache:config:<component>` | Конфигурация компонента |
| `cache:article:<hash>` | Полная статья из базы знаний |

### TTL (время жизни)

| Тип данных | TTL | Обоснование |
|------------|-----|-------------|
| Частые запросы к AI | 3600s (1 час) | Высокая вероятность повтора |
| Результаты поиска | 21600s (6 часов) | Средняя стабильность |
| Конфигурации | 86400s (24 часа) | Редко меняются |
| Схемы JSON | 604800s (7 дней) | Стабильные данные |
| Редкие запросы | 86400s (24 часа) | Защита от устаревания |

### Инструкции

#### 1. Проверка кэша перед запросом

```python
import hashlib
import json
import redis

def check_cache(source, query):
    normalized = json.dumps(query, sort_keys=True)
    hash_value = hashlib.sha256(normalized.encode()).hexdigest()[:16]
    cache_key = f"cache:{source}:{hash_value}"
    
    r = redis.Redis(host="waters-redis", port=6379, db=0)
    cached = r.get(cache_key)
    
    if cached:
        r.incr("metrics:cache_hit")
        return json.loads(cached)
    
    r.incr("metrics:cache_miss")
    return None
```

#### 2. Сохранение в кэш

```python
def save_to_cache(source, query, response, ttl):
    normalized = json.dumps(query, sort_keys=True)
    hash_value = hashlib.sha256(normalized.encode()).hexdigest()[:16]
    cache_key = f"cache:{source}:{hash_value}"
    
    r = redis.Redis(host="waters-redis", port=6379, db=0)
    r.setex(cache_key, ttl, json.dumps(response))
```

#### 3. Инвалидация кэша

| Сценарий | Действие |
|----------|----------|
| Обновление схемы | `DEL cache:schema:*` |
| Обновление конфигурации | `DEL cache:config:*` |
| Принудительная очистка (dev) | `FLUSHDB` |
| Инвалидация по паттерну | `SCAN` + `DEL` |

```python
def invalidate_by_pattern(pattern):
    r = redis.Redis(host="waters-redis", port=6379, db=0)
    cursor = 0
    while True:
        cursor, keys = r.scan(cursor, match=pattern, count=100)
        if keys:
            r.delete(*keys)
        if cursor == 0:
            break
```

#### 4. Полный алгоритм AI-запроса с кэшем

```
1. Нормализовать запрос
2. Сгенерировать cache:ai:<hash>
3. GET cache key из Redis
4. Если HIT:
     — Вернуть из кэша
     — Записать metrics:cache_hit
     — Опубликовать событие в events.system.v1
5. Если MISS:
     — Выполнить запрос к внешнему AI
     — Сохранить в кэш с TTL=3600s
     — Записать metrics:cache_miss
     — Опубликовать событие в events.system.v1
     — Вернуть ответ
```

### Метрики

| Метрика | Описание | Цель |
|---------|----------|------|
| `metrics:cache_hit` | Счётчик попаданий | > 60% hit rate |
| `metrics:cache_miss` | Счётчик промахов | < 40% miss rate |
| `metrics:tokens_saved` | Сэкономленные токены | Максимизация |
| `metrics:eviction_count` | Количество вытеснений | Минимизация |

### Интеграция с Нервной системой

- Кэш-метрики отправляются в `metrics.raw.v1`
- События инвалидации в `events.system.v1`
- Конфигурация правил кэша в CozoDB (таблица `cache_rules`)

### Ограничения

1. Не кэшировать запросы с чувствительными данными
2. Не кэшировать запросы с временным контекстом (дата/время)
3. Максимальный размер значения: 512 KB
4. Eviction policy: `allkeys-lru`
5. Кэш не заменяет ChromaDB — только ускорение повторяющихся запросов

### Чек-лист кэширования

- [ ] Ключ соответствует формату `cache:<source>:<hash>`
- [ ] Hash генерируется из нормализованного запроса (sort_keys=True)
- [ ] TTL установлен согласно типу данных
- [ ] Метрика cache_hit/cache_miss обновлена
- [ ] Чувствительные данные не кэшируются
- [ ] Инвалидация опубликована в events.system.v1

---

*SKILL.md создан: Архитектор v1.0*
*Владелец: agent.integrator.v1*
