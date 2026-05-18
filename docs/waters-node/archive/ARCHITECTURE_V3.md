# waters-node v0.3 — Архитектура ноды

> **Версия:** 3.0
> **Дата:** 2026-05-15
> **Статус:** Проектирование
> **Основание:** waters-node v0.2.0 (20 модулей, ~3400 строк)

---

## 1. Что такое waters-node

**Один бинарник (~6.7MB) = целая нода в P2P-сети.**

Каждая нода — это:
- Свой LLM-провайдер (DeepSeek / Ollama / любой OpenAI-compatible)
- Свой набор бриджей (DuckDuckGo, Yandex, Baidu, NotebookLM, Obsidian...)
- Свои агенты (личные + общие в группе)
- Свой WAL-канал для durable messaging
- Никаких внешних зависимостей: не нужен Kafka, Redis, Docker

---

## 2. Шестигранная топология (Hexagonal Mesh)

```
                  ┌──────────┐
                  │  НОДА A  │
                  │ LLM: DS  │
                  └────┬─────┘
                  /     |     \
          ┌──────┐ ┌────┴────┐ ┌──────┐
          │НOДА B│ │ НОДА C  │ │НOДА D│
          │Ollama│ │ DS Flash│ │Ollama│
          └──────┘ └─────────┘ └──────┘
                  \     |     /
                  ┌────┴─────┐
                  │  НОДА E  │
                  │ DS Flash │
                  └──────────┘
```

- Каждая нода: макс 6 соединений (MAX_PEERS=6)
- Каждая группа: общий токен (UUID v4)
- Каждая группа: свой набор общих ресурсов
- Треугольники: A→B→C→A для связности

---

## 3. Слои ноды

```
┌─────────────────────────────────────────────────────────────┐
│  СЛОЙ UX: Чат (chat.rs + convo.rs)                         │
│  — LLM как интерфейс (chat.rs + система промптов)           │
│  — Convo (встроенный диалог, когда LLM нет)                 │
│  — Голосовой bridge (voice.rs, Whisper STT)                 │
│  — Никаких меню, кнопок, панелей                            │
├─────────────────────────────────────────────────────────────┤
│  СЛОЙ ЯДРО: Engine (engine.rs + handlers.rs)               │
│  — Получение сообщения (чат / gossip / API / голос)         │
│  — Роутинг: команда → task / agent / group / bridge         │
│  — Исполнение: task → subagent → tools → LLM                │
│  — Журналирование: каждый шаг → journal.rs                  │
├─────────────────────────────────────────────────────────────┤
│  СЛОЙ РЕСУРСЫ: Bridges + MCP + Skills                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────┐  │
│  │ Bridge Registry  │  │ MCP Client Pool │  │Skill Reg. │  │
│  │ duckduckgo      │  │ mcp-nasa        │  │ general   │  │
│  │ yandex.search   │  │ mcp-spectra     │  │ explorer  │  │
│  │ baidu.search    │  │ mcp-db (SQLite) │  │ scout-ru  │  │
│  │ notebooklm      │  │ mcp-db (PG)     │  │ scout-us  │  │
│  │ obsidian        │  │ mcp-my-private  │  │ scout-cn  │  │
│  └─────────────────┘  └─────────────────┘  └────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  СЛОЙ СЕТЬ: P2P + Gossip + WAL                             │
│  ┌──────────────────┐  ┌────────────────┐  ┌────────────┐  │
│  │ Gossip (gossip)  │  │ Channels (chan)│  │ Groups     │  │
│  │ mDNS discovery   │  │ WAL per channel│  │ token ACL  │  │
│  │ TCP sync         │  │ message replay │  │ shared res │  │
│  │ periodic sync    │  │ import/export  │  │ roles      │  │
│  └──────────────────┘  └────────────────┘  └────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  СЛОЙ АВТОНОМИЯ: L0-L4 + DTN + Военный режим               │
│  ┌────────────────┐  ┌────────────┐  ┌───────────────────┐  │
│  │ Autonomy Engine│  │ DTN        │  │ Kafka (военный)   │  │
│  │ L0: полная     │  │ tc-netem   │  │ feature-gated     │  │
│  │ L1: эконом.    │  │ lunar 1.3s│  │ объединение всех  │  │
│  │ L2: офлайн     │  │ martian    │  │ нод в единый рой │  │
│  │ L3: кризис     │  │ field      │  │                   │  │
│  │ L4: safe       │  └────────────┘  └───────────────────┘  │
│  └────────────────┘                                          │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Поток задачи от начала до конца

```
1. Чат: "найти метеориты за 24ч, назначить Explorer-A и Scout-B"
         ↓
2. Engine: парсит → создаёт Task {id, title, executors, resources}
         ↓
3. Task Manager: назначает Agent-A (Explorer, нода 1, DeepSeek Pro)
                 назначает Agent-B (Scout-US, нода 2, Ollama)
         ↓
4. SubAgent: spawn(Agent-A), spawn(Agent-B)
         ↓
5. Tools → Bridges → LLM:
   Agent-A: MCP-nasa → NASA Fireball API → finding
   Agent-B: DuckDuckGo → web_search → finding
         ↓
6. Journal: Agent-A записывает шаги в task-0001/explorer.log
            Agent-B записывает шаги в task-0001/scout-us.log
         ↓
7. Группа синтезирует: finding от Agent-A (conf=0.9) + от Agent-B (conf=0.7)
         → группа решает: "US-направление даёт качество, усиливаем его"
         ↓
8. Результат: публикуется в групповой канал
              + сохраняется в хранилище задачи (git / Obsidian / DuckDB)
```

---

## 5. Как нода выбирает LLM

```
Приоритет (определяется в llm.rs):
  1. DeepSeek Pro   (env DEEPSEEK_API_KEY + model=deepseek-v4-pro)
  2. DeepSeek Flash (env DEEPSEEK_API_KEY, default)
  3. Ollama local   (http://127.0.0.1:11434, qwen2.5:14b)
  4. Любой OpenAI-compatible (config.toml)
  5. None → Convo mode (чат без LLM)

Каждая нода может иметь СВОЙ провайдер.
В группе 6 нод = до 6 разных LLM.
Агент привязан к ноде: "Agent-A работает на ноде 1 (DeepSeek Pro)".
```

---

## 6. Как нода выбирает бриджи

```
НЕЛЬЗЯ: "У DuckDuckGo кончились ключи, переключаем на бесплатный RU"
МОЖНО:  "Группа видит: US-направление даёт conf=0.9 vs RU conf=0.6
         → выделяем US-скауту DeepSeek Pro + ChromaDB + NotebookLM"

Логика выбора:
  1. Все бриджи региона работают (нода решила: "у меня есть ключи")
  2. Результаты → группа оценивает качество (confidence)
  3. Группа перераспределяет: "усиливаем направление X"
  4. Механизм: AgentHandle::boost() или ResourceAllocation в group.rs
```

---

## 7. Персистентность (без Kafka)

```
WAL-канал (channel.rs):
  ┌──────────┐     ┌──────────┐     ┌──────────┐
  │ Agent A  │────►│ WAL file │◄────│ Agent B  │
  │ пишет    │     │ .waters/ │     │ читает   │
  │ finding  │     │ channels/│     │ finding  │
  └──────────┘     └──────────┘     └──────────┘

Чекипоинты (session.rs):
  ┌─────────────────────────────────────────────┐
  │ Перед каждым шагом: save_checkpoint()        │
  │   → .waters/checkpoints/latest.json          │
  │ После успеха: clear_checkpoint()             │
  │ При падении: resume_from_checkpoint()        │
  └─────────────────────────────────────────────┘

Офлайн-очередь (autonomy.rs):
  ┌─────────────────────────────────────────────┐
  │ L1-L2: findings пишутся в .waters/offline/   │
  │ При L0: batch-отправка в группу через gossip │
  └─────────────────────────────────────────────┘
```

---

## 8. Что НЕ изменится в v0.3

| Модуль | Статус | Комментарий |
|--------|--------|-------------|
| `node.rs` | ✅ | NodeID, state — работает |
| `channel.rs` | ✅ | WAL — правильная архитектура |
| `group.rs` | ✅ | Основа есть, добавить shared resources |
| `skill.rs` | ✅ | Skills 2.0 — работает |
| `tools.rs` | ✅ | 9 инструментов — достаточно |
| `mode.rs` | ✅ | 5 режимов — ок |
| `autonomy.rs` | ✅ | L0-L4 — ок |
| `dtn.rs` | ✅ | DTN — ок |
| `store.rs` | ✅ | Redis feature-gate — ок |
| `demo.rs` | ✅ | Демо — ок |
