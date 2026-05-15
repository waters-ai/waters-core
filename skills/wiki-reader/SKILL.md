---
name: wiki-reader
description: Чтение и поиск в MediaWiki-базах знаний (Wikipedia, Fandom и др.)
---

# SKILL.md — wiki-reader

## Навык: чтение вики WATERS через ChromaDB + LightRAG

### Идентификация

| Поле | Значение |
|------|----------|
| **skill_id** | `wiki-reader` |
| **версия** | 1.0.0 |
| **владелец** | `agent.integrator.v1` |
| **тип** | `core` |
| **слой** | I — Сеть |
| **статус** | `active` |

### Назначение

`wiki-reader` — навык для поиска и чтения знаний из базы знаний WATERS. Использует ChromaDB для векторного семантического поиска и LightRAG для графовых запросов и обхода связей между концептами.

### Контекст загрузки

Этот skill загружается агентом при необходимости:
1. Найти информацию по запросу (семантический поиск)
2. Обнаружить связи между концептами (графовый поиск)
3. Получить полный контекст по теме (комбинированный поиск)

### Источники знаний

| Источник | Бэкенд | Тип поиска |
|----------|--------|------------|
| Статьи доктрины | ChromaDB collection `doctrine` | Векторный (cosine similarity) |
| Статьи миссий | ChromaDB collection `missions` | Векторный |
| Статьи агентов | ChromaDB collection `agents` | Векторный |
| Граф знаний | LightRAG | Графовый (Cypher-like) |
| Схемы топиков | ChromaDB collection `schemas` | Векторный |

### Инструкции для чтения

#### Семантический поиск (ChromaDB)

1. Получить запрос от агента
2. Сгенерировать эмбеддинг запроса через `all-MiniLM-L6-v2`
3. Выполнить поиск в ChromaDB: `query(emb, n_results=10, where={source: ...})`
4. Ранжировать результаты по distance score
5. Вернуть топ-N статей с мета-данными

```python
import chromadb

client = chromadb.HttpClient(host="waters-chroma", port=8000)
collection = client.get_collection("doctrine")

results = collection.query(
    query_embeddings=[embedding],
    n_results=10,
    include=["documents", "metadatas", "distances"]
)
```

#### Графовый поиск (LightRAG)

1. Определить сущности в запросе (NER)
2. Найти соответствующие ноды в графе
3. Выполнить обход графа (BFS/DFS, depth=2)
4. Извлечь связанные ноды и рёбра
5. Вернуть подграф с релевантными связями

```python
from lightrag import LightRAG

rag = LightRAG(
    working_dir="/data/lightrag",
    llm_model_func=local_model,
    embedding_func=embedding_fn
)

result = rag.query(
    "Как связаны Архитектор и Конструктор?",
    param=QueryParam(mode="hybrid", top_k=5)
)
```

#### Комбинированный поиск (hybrid)

1. Выполнить векторный поиск в ChromaDB
2. Извлечь `related_nodes` из результатов
3. Выполнить графовый обход из этих нод
4. Объединить результаты, убрать дубликаты
5. Ранжировать по комбинированному score: `0.6 * vector + 0.4 * graph`

### Входы

| Параметр | Тип | Описание |
|----------|-----|----------|
| `query` | string | Поисковый запрос |
| `source` | string | Фильтр по источнику (опционально) |
| `mode` | string | `vector`, `graph`, `hybrid` (default: hybrid) |
| `top_k` | int | Количество результатов (default: 10) |
| `depth` | int | Глубина обхода графа (default: 2) |

### Выходы

| Параметр | Тип | Описание |
|----------|-----|----------|
| `articles` | array | Найденные статьи с мета-данными |
| `subgraph` | object | Подграф связей (ноды + рёбра) |
| `score` | float | Совокупный score релевантности |
| `sources_used` | array | Использованные источники |

### Публикация в Kafka

При чтении статьи для обогащения знаний:
- Публикует событие в `knowledge.articles.v1` при обнаружении новой релевантной статьи
- Публикует обновление в `knowledge.graph.v1` при обнаружении новых связей

### Ограничения

1. wiki-reader только читает — не пишет в ChromaDB/LightRAG напрямую
2. Не использует DeepSeek для поиска в интернете
3. Эмбеддинги генерируются только локально (all-MiniLM-L6-v2)
4. Кэширует результаты в Redis на 5 минут
5. Не раскрывает содержимое статей с `confidence < 0.3`

### Чек-лист качества поиска

- [ ] Запрос нормализован перед эмбеддингом
- [ ] Используется правильный collection в ChromaDB
- [ ] Графовый обход ограничен по глубине (max depth=3)
- [ ] Результаты дедуплицированы по article_id
- [ ] Score релевантности приложен к каждому результату
- [ ] Источник каждого результата указан

---

*SKILL.md создан: Конструктор Сети v1.0*
*Владелец: agent.integrator.v1*
