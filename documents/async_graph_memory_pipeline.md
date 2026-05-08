# Async Graph Memory Pipeline

Agents пишут в LightRAG **асинхронно** — через Redis-очередь, чтобы не блокировать MCP-вызовы.

## Схема

```
┌──────────────┐     memory_graph_insert     ┌──────────────┐     lightrag_writer.py     ┌──────────┐
│  Агенты (MCP) │ ──────────────────────────→ │  Redis queue │ ─────── (systemd) ──────→ │ LightRAG │
│  (быстрая     │     (мгновенно, не ждёт)    │ lightrag:q   │     (batch: 3, pause: 15с)  │ (граф)   │
│  запись)      │                              └──────────────┘                           └──────────┘
└──────────────┘
       │
       │ memory_vector_save
       ▼
┌──────────────┐
│   ChromaDB   │  ← быстрая векторная память (прямая запись)
└──────────────┘
```

## Компоненты

### 1. `mcp_memory_bridge.py`
- `memory_graph_insert` — **не блокирует**: пишет JSON в Redis `lightrag:queue`, возвращает `{status: queued}`
- `memory_graph_query` — **read-only** из LightRAG
- `memory_graph_queue_status` — проверка размера очереди
- `get_lightrag()` — без `initialize_storages()`, ленивый

### 2. `scripts/lightrag_writer.py`
- Читает из `lightrag:queue` (lpop)
- Пишет в LightRAG порциями (batch: 3)
- Пауза между батчами (interval: 15с)
- Режимы: `--once` (однократно), `--daemon` (бесконечный цикл)

### 3. `infrastructure/systemd/lightrag-writer.service`
- Supervisor: `systemd` с `Restart=always`
- Логи: `logs/lightrag_writer.log`
- Запуск при загрузке: `systemctl enable lightrag-writer`

## Управление

```bash
# Статус
systemctl status lightrag-writer

# Логи
tail -f logs/lightrag_writer.log

# Однократный проход
python3 scripts/lightrag_writer.py --once

# Перезапуск демона
systemctl restart lightrag-writer
```

## Почему так

- LightRAG.init вызывает Ollama (медленно, 5-60с)
- Прямой вызов блокирует все MCP-инструменты (таймаут)
- Очередь Redis — единственный способ не тормозить агентов
- ChromaDB (vector_save) — быстрая, пишет напрямую
