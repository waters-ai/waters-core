# OpenCode Tools & MCP Setup — WATERS Platform

## Архитектура

```
┌──────────────────────────────────────────────────────────┐
│               OpenCode Agent (tmux)                       │
│                                                          │
│  ┌───────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │  mcpServers    │  │  mcpServers  │  │  mcpServers   │  │
│  │  github        │  │  filesystem  │  │  memory       │  │
│  │  (GitHub API)  │  │  (локальные  │  │  (базы памяти)│  │
│  │                │  │   файлы)     │  │               │  │
│  └───────┬───────┘  └──────┬───────┘  └───────┬───────┘  │
└──────────┼─────────────────┼──────────────────┼──────────┘
           │                 │                  │
    ┌──────┴──────┐   ┌──────┴──────┐   ┌──────┴──────────────┐
    │  GitHub     │   │  /waters-   │   │  ChromaDB (векторы) │
    │  (внешний)  │   │  core/      │   │  Redis (кэш/сост.)  │
    │             │   │  (файлы)    │   │  LightRAG (графы)   │
    │             │   │             │   │  Kafka (события)    │
    └─────────────┘   └─────────────┘   └─────────────────────┘
```

## Полный инвентарь ресурсов

### 🟢 Запущено и доступно

| Ресурс | Адрес | Через какой MCP |
|--------|-------|-----------------|
| **ChromaDB** (векторная БД) | localhost:8000 | memory (memory_vector_search/save) |
| **Redis waters-memory** (кэш состояний) | localhost:6379 | memory (memory_cache_get/set, memory_state_load/save) |
| **Redis waters-redis** (кэш сессий) | internal (Docker) | memory |
| **Apache Kafka** (брокер событий) | localhost:9092 | memory (memory_kafka_send/list) |
| **LightRAG** (графовая БД) | /data/lightrag | memory (memory_graph_query/insert) |
| **ZooKeeper** (координация Kafka) | localhost:2181 | косвенно через Kafka |
| **Ollama** (LLM-модели) | localhost:11434 | provider в opencode.json |
| **Файлы проекта** | /home/ubuntu/WATERS/repos/waters-core | filesystem |
| **GitHub** | github.com/waters-ai/waters-core | github |

### 🟡 Написано, но не запущено

| Скрипт/Сервис | Назначение | Почему не запущен |
|---------------|------------|-------------------|
| `mcp_proxy.py` (Docker) | JSON-RPC 2.0 фасад для внешних API | Заменён на прямой mcp_memory_bridge |
| `mcp_brave_search.py` (Docker) | Brave Search API адаптер | Нет BRAVE_API_KEY |
| `nginx` (Docker) | Обратный прокси для DMZ | Не требуется в Базовом сценарии |
| `redis-dmz`, `chromadb-dmz` | Изолированные DMZ-сервисы | Не требуются в Базовом сценарии |

### 🟠 Данные, требующие первичной загрузки

| Путь | Статус | Что делать |
|------|--------|------------|
| `knowledge/external/` | ✅ Создана, пуста | Интегратор заполняет внешними данными |
| `integrations/sources_map.json` | ❌ Не создан | Создать карту внешних источников |
| `integrations/*_adapter.json` | ❌ Не созданы | Создать MCP-адаптеры (Wikipedia, arXiv, GitHub) |
| `/data/lightrag/` | ✅ Создана, пуста | Загрузить доктрины через memory_graph_insert |

## MCP-серверы

### 1. GitHub MCP

```bash
# Доступ к GitHub API: репозитории, issues, PRs, поиск
# Устанавливается автоматически через npx при первом запуске
# Требует GITHUB_TOKEN в окружении
```

**Конфиг в opencode.json:**
```json
"github": {
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-github"],
  "env": {
    "GITHUB_TOKEN": "{env:GITHUB_TOKEN}"
  }
}
```

**Инструменты:** просмотр репозитория, создание файлов, поиск кода, issues, PRs.

---

### 2. Filesystem MCP

```bash
# Доступ к локальной файловой системе проекта
# Устанавливается автоматически через npx
```

**Конфиг в opencode.json:**
```json
"filesystem": {
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-filesystem",
           "/home/ubuntu/WATERS/repos/waters-core"]
}
```

**Инструменты:** чтение/запись файлов, поиск, список директорий.

---

### 3. Memory MCP (кастомный)

```python
# scripts/mcp_memory_bridge.py
# Python-сервер, работающий через stdio-протокол MCP
# Подключается ко всем базам памяти WATERS
```

**Конфиг в opencode.json:**
```json
"memory": {
  "command": "python3",
  "args": ["scripts/mcp_memory_bridge.py"]
}
```

**Инструменты (12):**

| Инструмент | Описание | Бэкенд |
|-----------|----------|--------|
| `memory_vector_search` | Поиск по эмбеддингам | ChromaDB |
| `memory_vector_save` | Сохранение в векторную БД | ChromaDB |
| `memory_graph_query` | Графовый запрос (local/global/hybrid) | LightRAG |
| `memory_graph_insert` | Вставка текста в граф знаний | LightRAG |
| `memory_cache_get` | Чтение из Redis-кэша | Redis |
| `memory_cache_set` | Запись в Redis-кэш с TTL | Redis |
| `memory_kafka_send` | Отправка сообщения в Kafka-топик | Apache Kafka |
| `memory_kafka_list` | Список топиков Kafka | Apache Kafka |
| `memory_state_load` | Загрузка состояния агента | Redis |
| `memory_state_save` | Сохранение состояния агента | Redis |
| `memory_stats` | Статистика использования всех БД | все |

---

## Форматтеры (кастомные инструменты)

Установлены в `.opencode/tools/`:

| Инструмент | Команда | Формат |
|-----------|---------|--------|
| `format_python` | `black` | .py |
| `format_markdown` | `prettier --parser markdown` | .md |
| `format_json` | `prettier --parser json` | .json |

## Startup-последовательность агентов

Каждый агент при запуске выполняет:

```
1. connect mcpServers → filesystem, github, memory
2. memory_state_load(agent_id)           ← восстановление состояния из Redis
3. memory_vector_search(query)           ← контекст прошлых сессий из ChromaDB
4. memory_graph_query(query)             ← контекст из графа LightRAG
5. memory_cache_get("tasks:agent:pending") ← ожидающие задачи из Redis
6. memory_kafka_list                     ← проверка доступных топиков
```

Эта последовательность прописана в `agents/*_AGENTS.md` каждого агента.

## Установка и настройка

### Предусловия

```bash
node --version  # ≥ 18 (v22.22.2)
npm --version    # ≥ 9 (10.9.7)
python3 --version  # ≥ 3.10 (3.12)
pip --version
```

### 1. MCP-серверы (устанавливаются npx-ом автоматически)

Ничего устанавливать не нужно — npx загружает пакеты при первом запуске OpenCode.

### 2. Форматтеры

```bash
sudo npm install -g prettier     # для Markdown/JSON
pip install --break-system-packages black  # для Python
```

### 3. Директории

```bash
sudo mkdir -p /data/lightrag
sudo chown -R ubuntu:ubuntu /data
mkdir -p knowledge/external integrations
```

### 4. Docker-контейнеры

```bash
cd /home/ubuntu/WATERS/repos/waters-core
sudo docker compose up -d kafka chromadb redis
```

Kafka работает в режиме KRaft (без ZooKeeper) — это современная архитектура Apache Kafka 4.x.

### 5. Переменные окружения

Создать `.env` в корне проекта (не коммитится):

```bash
# .env — секреты WATERS
DEEPSEEK_API_KEY=sk-...
GITHUB_TOKEN=ghp_...
OPENCODE_SERVER_PASSWORD=...
```

Файл `.env` уже добавлен в `.gitignore`.

## Чек-лист верификации

- [ ] `opencode debug config` — показывает mcpServers (github, filesystem, memory)
- [ ] `curl localhost:8000/api/v2/heartbeat` — ChromaDB отвечает
- [ ] `redis-cli -h localhost -p 6379 ping` → PONG
- [ ] `docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --list` — топики видны
- [ ] `python3 scripts/mcp_memory_bridge.py < /dev/null` — сервер запускается (проверка синтаксиса)
- [ ] При запуске агента: `memory_stats` показывает все БД подключёнными
- [ ] `prettier --version` → 3.x
- [ ] `black --version` → 26.x

## Файлы, изменённые в этом спринте

| Файл | Изменение |
|------|-----------|
| `opencode.json` | Добавлены mcpServers (github, filesystem, memory) |
| `.opencode/tools/format_python.json` | **Новый** — инструмент Black |
| `.opencode/tools/format_markdown.json` | **Новый** — инструмент Prettier (markdown) |
| `.opencode/tools/format_json.json` | **Новый** — инструмент Prettier (json) |
| `scripts/mcp_memory_bridge.py` | **Новый** — MCP-сервер доступа к БД памяти (11 инструментов) |
| `docker-compose.yml` | Redpanda → Apache Kafka (KRaft, без ZooKeeper) |
| `infrastructure/docker/topology.json` | Redpanda → Apache Kafka (KRaft) + Zookeeper удалён |
| `infrastructure/docker/topology_troitsa.json` | Redpanda → Apache Kafka (только документация) |
| `infrastructure/docker/topology_forward.json` | Redpanda → Apache Kafka (только документация) |
| `agents/AGENTS.md` (Constructor) | Добавлены Startup Sequence + MCP-интерфейсы |
| `agents/constructor_AGENTS.md` | Redpanda → Apache Kafka + Startup Sequence + MCP |
| `agents/architect_AGENTS.md` | Добавлены Startup Sequence + MCP-интерфейсы |
| `agents/integrator_AGENTS.md` | Добавлены Startup Sequence + MCP-интерфейсы |
| `agents/director_AGENTS.md` | Добавлены Startup Sequence + MCP-интерфейсы |
| `agents/keeper_AGENTS.md` | Добавлены Startup Sequence + MCP-интерфейсы |
| `agents/lawkeeper_AGENTS.md` | Добавлены Startup Sequence + MCP-интерфейсы |
| `doctrine/nervous_system.md` | Redpanda → Apache Kafka; rpk → kafka-topics.sh |
| `run_carousel.sh` | waters-redpanda → waters-kafka |
| `scripts/create_remote_topics.sh` | waters-redpanda → waters-kafka |
| `scripts/ask_integrator_for_h2o_audit.sh` | redpanda → kafka |
| `skills/external-search.skill.md` | waters-redpanda:9092 → waters-kafka:9092 |
| `documents/multi_agent_workflow.md` | redpandadata/redpanda → apache/kafka |
| `.env` | **Новый** — секреты окружения (в .gitignore) |
| `documents/opencode_tools_setup.md` | **Новый** — данный документ |
