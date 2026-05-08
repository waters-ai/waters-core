# Миграция на штатные механизмы OpenCode

**Дата:** 08.05.2026
**Автор:** Конструктор Сети v1.0

## Что изменилось

| Было (кастом) | Стало (штатное) |
|---|---|
| `.opencode/tools/format_*.json` (3 файла) | `formatter` секция в `opencode.json` (prettier + ruff) |
| Ручная JSON-правка MCP-серверов | MCP через `opencode.json` (валидная схема) |
| Redis-снэпшоты сессий (`session_snapshot.sh`) | `opencode -c` / `opencode -s` — штатные сессии |
| Кастомные launch-скрипты (tmux) | `opencode serve` + `opencode attach` |
| MCP memory bridge protocol v0.1.0 | protocol v2024-11-05 |

## Новая архитектура

### Внутри одного сервера
- Один экземпляр OpenCode
- WATERS агенты как subagents: @architect, @constructor, @integrator, @director, @keeper, @lawkeeper
- Коммуникация через @mention и Task tool

### Между серверами / ЦОДами
- Kafka — нервная система (сохранена)
- Redis — кэш и состояния (сохранён)
- ChromaDB — векторная память (сохранена)
- LightRAG — графовая память (сохранён)
- Dashboard — командный центр (сохранён)

## Конфигурация

```json
{
  "formatter": {
    "prettier": { "disabled": false },
    "ruff": { "disabled": false }
  },
  "server": {
    "port": 4096,
    "hostname": "0.0.0.0",
    "mdns": true
  },
  "mcp": {
    "filesystem": { "type": "local", "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "..."] },
    "github": { "type": "local", "command": ["npx", "-y", "@modelcontextprotocol/server-github"], "environment": { "GITHUB_TOKEN": "{env:GITHUB_TOKEN}" } },
    "memory": { "type": "local", "command": ["python3", "scripts/mcp_memory_bridge.py"] }
  },
  "agent": {
    "architect": { "mode": "subagent", "prompt": "{file:./prompts/architect_system.md}" },
    "constructor": { "mode": "subagent", "prompt": "{file:./prompts/constructor_system.md}" },
    "integrator": { "mode": "subagent", "prompt": "{file:./prompts/integrator_system.md}" },
    "director": { "mode": "subagent", "prompt": "{file:./prompts/meaning_director.md}" },
    "keeper": { "mode": "subagent", "prompt": "{file:./prompts/protocol_keeper.md}" },
    "lawkeeper": { "mode": "subagent", "prompt": "{file:./prompts/lawkeeper.md}" }
  }
}
```

## Как продолжить сессию

```bash
opencode -c                    # продолжить последнюю
opencode -s <session-id>       # продолжить конкретную
opencode session list          # список сессий
opencode export <id>           # экспорт сессии
opencode import <file>         # импорт сессии
```

## Как запустить serve-режим

```bash
opencode serve --port 4096 --hostname 0.0.0.0
opencode attach http://localhost:4096
```
