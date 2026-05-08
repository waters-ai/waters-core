# OpenCode Tools & MCP Setup — WATERS Platform

## Архитектура

```
┌──────────────────────────────────────────────────────────────┐
│                  OpenCode Agent (tmux)                        │
│                                                              │
│  ┌───────────────┐  ┌──────────────┐  ┌───────────────────┐  │
│  │  mcpServers    │  │  mcpServers  │  │  mcpServers       │  │
│  │  github        │  │  filesystem  │  │  memory           │  │
│  │  (GitHub API)  │  │  (локальные  │  │  (16 инструментов)│  │
│  │                │  │   файлы)     │  │                   │  │
│  └───────┬───────┘  └──────┬───────┘  └───────┬───────────┘  │
└──────────┼─────────────────┼──────────────────┼──────────────┘
           │                 │                  │
    ┌──────┴──────┐   ┌──────┴──────┐   ┌──────┴──────────────────┐
    │  GitHub     │   │  /waters-   │   │  ChromaDB  (векторная)  │
    │  (внешний)  │   │  core/      │   │  Redis     (кэш+сост.)  │
    │             │   │  (файлы)    │   │  LightRAG  (графовая)   │
    │             │   │             │   │  Kafka     (события)     │
    └─────────────┘   └─────────────┘   └─────────────────────────┘


                    scripts/mcp_monitor.sh ── cron (каждые 5 мин)
                           │
                    alerts.security.v1 (при падении)
```

## Полный инвентарь ресурсов

### 🟢 Запущено и доступно

| Ресурс | Адрес | Статус | Через какой MCP |
|--------|-------|--------|-----------------|
| **ChromaDB** (векторная БД) | localhost:8000 | ✅ healthy | memory |
| **Redis waters-memory** (состояния) | localhost:6379 | ✅ healthy | memory |
| **Redis waters-redis** (кэш сессий) | internal Docker | ✅ healthy | memory |
| **Apache Kafka** (брокер) | localhost:9092 | ✅ healthy (KRaft) | memory |
| **LightRAG** (графовая БД) | /data/lightrag | ✅ готова | memory |
| **Ollama** (LLM-модели) | localhost:11434 | ✅ 4 модели | provider в opencode.json |
| **Файлы проекта** | waters-core/ | ✅ | filesystem |
| **GitHub** | github.com/waters-ai | ✅ | github |

### 🟡 Написано, но не запущено

| Скрипт/Сервис | Назначение | Причина |
|---------------|------------|---------|
| `mcp_proxy.py` (Docker) | JSON-RPC 2.0 фасад | Заменён на прямой memory bridge |
| `mcp_brave_search.py` | Brave Search | Нет BRAVE_API_KEY |
| `nginx` | Обратный прокси DMZ | Не требуется в Базовом сценарии |
| `redis-dmz`, `chromadb-dmz` | Изолированные DMZ | Не требуются |

### 🟠 Данные для первичной загрузки

| Путь | Статус | Действие |
|------|--------|----------|
| `knowledge/external/` | ✅ создана | Интегратор заполняет |
| `integrations/sources_map.json` | ❌ не создан | Создать карту источников |
| `integrations/*_adapter.json` | ❌ не созданы | MCP-адаптеры |
| `/data/lightrag/` | ✅ создана | Загрузить доктрины через `memory_graph_insert` |

---

## MCP-серверы

### 1. GitHub MCP

```json
"github": {
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-github"],
  "env": { "GITHUB_TOKEN": "{env:GITHUB_TOKEN}" }
}
```

**Инструменты:** репозитории, issues, PRs, поиск кода, создание файлов.

### 2. Filesystem MCP

```json
"filesystem": {
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-filesystem",
           "/home/ubuntu/WATERS/repos/waters-core"]
}
```

**Инструменты:** чтение/запись/поиск файлов, список директорий.

### 3. Memory MCP (кастомный)

```json
"memory": {
  "command": "python3",
  "args": ["scripts/mcp_memory_bridge.py"]
}
```

**16 инструментов:**

#### Векторная память (ChromaDB)
| Инструмент | Описание |
|-----------|----------|
| `memory_vector_search` | Поиск по эмбеддингам |
| `memory_vector_save` | Сохранение документа |

#### Графовая память (LightRAG)
| Инструмент | Описание |
|-----------|----------|
| `memory_graph_query` | Графовый запрос (local/global/hybrid) |
| `memory_graph_insert` | Вставка текста в граф знаний |

#### Кэш и состояния (Redis)
| Инструмент | Описание |
|-----------|----------|
| `memory_cache_get` | Чтение Redis-ключа |
| `memory_cache_set` | Запись Redis-ключа с TTL |
| `memory_state_load` | Загрузка состояния агента |
| `memory_state_save` | Сохранение состояния агента |

#### Снэпшоты сессий (Redis)
| Инструмент | Описание |
|-----------|----------|
| `memory_session_save` | Сохранение снэпшота сессии (`session:<agent_id>`) |
| `memory_session_load` | Восстановление снэпшота |
| `memory_session_list` | Список сохранённых снэпшотов |

#### События (Apache Kafka)
| Инструмент | Описание |
|-----------|----------|
| `memory_kafka_send` | Отправка сообщения в топик |
| `memory_kafka_list` | Список топиков |
| `memory_kafka_consume` | Чтение последних N сообщений (саморефлексия) |

#### Системные
| Инструмент | Описание |
|-----------|----------|
| `memory_health` | Полный healthcheck всех сервисов |
| `memory_stats` | Статистика использования |

---

## Форматтеры (кастомные инструменты)

Установлены в `.opencode/tools/`:

| Инструмент | Команда | Формат |
|-----------|---------|--------|
| `format_python` | `black ${filepath}` | .py |
| `format_markdown` | `prettier --parser markdown --write ${filepath}` | .md |
| `format_json` | `prettier --parser json --write ${filepath}` | .json |

---

## Startup-последовательность агентов

Каждый агент при запуске в tmux выполняет (прописано в `agents/*_AGENTS.md`):

```
 1. connect → filesystem, github, memory      ← MCP-серверы
 2. memory_state_load("agent_id")             ← состояние из Redis
 3. memory_session_load("agent_id")           ← снэпшот прошлой сессии
 4. memory_kafka_consume("answers", 5)        ← саморефлексия: свои прошлые ответы
 5. memory_vector_search(query, 5)            ← контекст из ChromaDB
 6. memory_graph_query(query)                 ← контекст из LightRAG
 7. memory_cache_get("tasks:agent:pending")   ← ожидающие задачи
 8. memory_health                             ← healthcheck всех сервисов
```

---

## Мониторинг

### `scripts/mcp_monitor.sh`

Проверяет все компоненты каждые 5 минут (cron):

```bash
# Полный отчёт
bash scripts/mcp_monitor.sh

# Только exit code для cron/CI
bash scripts/mcp_monitor.sh status
# exit 0 = всё ok, exit 1 = есть проблемы

# С отправкой алерта в Kafka alerts.security.v1
bash scripts/mcp_monitor.sh alert
```

**Рекомендуемый cron** (`crontab -e`):
```cron
*/5 * * * * /home/ubuntu/WATERS/repos/waters-core/scripts/mcp_monitor.sh alert
```

---

## Healthcheck'и Docker-контейнеров

| Контейнер | Метод healthcheck | Статус |
|-----------|-------------------|--------|
| `waters-kafka` | `kafka-topics.sh --list` | ✅ healthy |
| `waters-redis` | `redis-cli ping` | ✅ healthy |
| `waters-chroma` | bash `/dev/tcp` + `GET /api/v2/heartbeat` | ✅ healthy |

ChromaDB использует bash TCP-соединение (вместо curl, которого нет в образе):
```
CMD ["bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/8000 && \
  echo -e 'GET /api/v2/heartbeat HTTP/1.0\r\n\r\n' >&3 && \
  timeout 3 cat <&3 | grep -qi nanosecond"]
```

---

## Безопасность

| Мера | Описание |
|------|----------|
| `.env` | Все секреты в `.env`, файл в `.gitignore` |
| Git remote | PAT удалён из URL — используется `https://github.com/waters-ai/waters-core.git` |
| GITHUB_TOKEN | Только в `.env`, подгружается через `{env:GITHUB_TOKEN}` в opencode.json |
| `chmod 600` | `.env` защищён от чтения другими пользователями |

---

## Установка и настройка

### Предусловия
```bash
node --version     # ≥ 18 (v22.22.2)
npm --version      # ≥ 9 (10.9.7)
python3 --version  # ≥ 3.10 (3.12)
```

### 1. MCP-серверы
Ничего устанавливать не нужно — npx загружает пакеты при первом запуске OpenCode.

### 2. Форматтеры
```bash
sudo npm install -g prettier
pip install --break-system-packages black
```

### 3. Директории
```bash
sudo mkdir -p /data/lightrag && sudo chown -R ubuntu:ubuntu /data
mkdir -p knowledge/external integrations
```

### 4. Docker-контейнеры
```bash
cd /home/ubuntu/WATERS/repos/waters-core
sudo docker compose up -d kafka chromadb redis
```

Kafka работает в режиме KRaft (без ZooKeeper) — Apache Kafka 4.x.

### 5. Переменные окружения
```bash
# .env — секреты WATERS (не коммитится, в .gitignore)
DEEPSEEK_API_KEY=sk-...
GITHUB_TOKEN=ghp_...
OPENCODE_SERVER_PASSWORD=...
```

### 6. Мониторинг
```bash
crontab -e
# */5 * * * * /home/ubuntu/WATERS/repos/waters-core/scripts/mcp_monitor.sh alert
```

---

## Чек-лист верификации

- [ ] `opencode debug config` — mcpServers: github, filesystem, memory
- [ ] `curl -sf http://localhost:8000/api/v2/heartbeat` — ChromaDB отвечает
- [ ] `sudo docker exec waters-redis redis-cli ping` → PONG
- [ ] `sudo docker exec waters-kafka kafka-topics.sh --bootstrap-server localhost:9092 --list` — 33 топика
- [ ] `python3 -c "import ast; ast.parse(open('scripts/mcp_memory_bridge.py').read())"` — синтаксис OK
- [ ] `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | timeout 3 python3 scripts/mcp_memory_bridge.py | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(len(d['result']['tools']))"` — 16 инструментов
- [ ] `bash scripts/mcp_monitor.sh` — все 7 проверок OK
- [ ] `sudo docker ps --format "table {{.Names}}\t{{.Status}}"` — все healthy
- [ ] `prettier --version` → 3.x
- [ ] `black --version` → 26.x

---

## Файлы, изменённые в этом спринте

| Файл | Изменение |
|------|-----------|
| `opencode.json` | Добавлены mcpServers: github, filesystem, memory |
| `.opencode/tools/format_python.json` | **Новый** — Black |
| `.opencode/tools/format_markdown.json` | **Новый** — Prettier (.md) |
| `.opencode/tools/format_json.json` | **Новый** — Prettier (.json) |
| `scripts/mcp_memory_bridge.py` | **Новый** — 16 MCP-инструментов для баз памяти |
| `scripts/mcp_monitor.sh` | **Новый** — мониторинг всех сервисов |
| `docker-compose.yml` | Redpanda → Apache Kafka (KRaft); healthcheck ChromaDB исправлен |
| `infrastructure/docker/topology.json` | Redpanda → Kafka KRaft + Zookeeper удалён |
| `infrastructure/docker/topology_troitsa.json` | Redpanda → Kafka |
| `infrastructure/docker/topology_forward.json` | Redpanda → Kafka |
| `doctrine/nervous_system.md` | Redpanda → Apache Kafka; rpk → kafka-topics.sh |
| `AGENTS.md` (Constructor) | Startup Sequence + MCP-интерфейсы |
| `agents/constructor_AGENTS.md` | Kafka + Startup + MCP |
| `agents/architect_AGENTS.md` | Startup + MCP |
| `agents/integrator_AGENTS.md` | Startup + MCP |
| `agents/director_AGENTS.md` | Startup + MCP |
| `agents/keeper_AGENTS.md` | Startup + MCP |
| `agents/lawkeeper_AGENTS.md` | Startup + MCP |
| `run_carousel.sh` | waters-redpanda → waters-kafka |
| `scripts/create_remote_topics.sh` | waters-redpanda → waters-kafka |
| `scripts/ask_integrator_for_h2o_audit.sh` | redpanda → kafka |
| `skills/external-search.skill.md` | waters-redpanda:9092 → waters-kafka:9092 |
| `documents/multi_agent_workflow.md` | redpandadata/redpanda → apache/kafka |
| `.env` | **Новый** — секреты (в .gitignore) |
| `documents/opencode_tools_setup.md` | **Новый** — данный документ |
