# Анализ DeepSeek TUI v0.8.37 — отличия от наших предположений

**Дата:** 2026-05-15 (ред. 2.0)
**Репозиторий:** github.com/Hmbown/DeepSeek-TUI (29.6k ⭐, MIT)

---

## 1. TUI как reference, не как форк

TUI — coding-агент для одного разработчика. WATERS — распределённая система для роя агентов.
**Мы не форкаем TUI.** Используем его идеи и архитектуру как reference для нашей Kafka-native системы.

| Что было в TUI | Как используем в WATERS |
|---------------|------------------------|
| **Sub-agent система** (7 ролей: explore, plan, review, implementer, verifier, general, custom) | Берём концепцию ролей. Наш SubAgent Manager делает то же, но через Kafka вместо in-process mpsc |
| **SharedSubAgentManager (Arc<RwLock>)** | Наш аналог — Kafka topics для меж-агентной коммуникации |
| **Sub-agents persist в JSON** | Наш аналог — Kafka log (immutable, replayable) |
| **MCP клиент** | Берём как есть — индустриальный стандарт |
| **Memory.md (Markdown-файл)** | Наш аналог — ChromaDB/LightRAG на 238 |
| **RUNTIME_API (HTTP/SSE)** | Наш аналог — Kafka-native протокол (6 топиков) |
| **Skills система (SKILL.md)** | Берём как есть — SKILL.md наш формат |
| **Plan/Agent/YOLO режимы** | Наш аналог — L0-L4 автономия |

### Что мы добавили (чего нет в TUI)

- **Kafka-native архитектура** — распределённая шина вместо in-process mpsc
- **Автономия L0-L4** — работа без связи днями/неделями
- **Система рангов** — KPI агентов (адаптирована из h2o-mining.space)
- **Сеансовый протокол** — Contact Window Protocol для связи с задержками
- **Rust core + Python edge** — оптимальный баланс производительности и скорости разработки
- **Voice/Telegram/Web управление** — все через Kafka

## 2. Как на самом деле работает Sub-agent система

### 2.1. Роли (из SUBAGENTS.md)

| Роль | Описание | 
|------|----------|
| `general` | Flexible, multi-step tasks (default) |
| `explore` | Read-only, fast mapping (grep/glob/read file) |
| `plan` | Analyze and produce strategy |
| `review` | Read-and-grade with severity scores |
| `implementer` | Land specific change (narrow tools) |
| `verifier` | Run tests/validation |
| `custom` | Explicit narrow tool allowlist |

### 2.2. Жизненный цикл

```
Pending → Running → (Completed | Failed | Cancelled | Interrupted)
```

### 2.3. Инструменты

```
agent_open  — открыть sub-agent с ролью и задачей
agent_eval  — получить результат sub-agent'а
agent_close — закрыть sub-agent
```

### 2.4. Output contract

Sub-agent возвращает 5 секций: SUMMARY, CHANGES, EVIDENCE, RISKS, BLOCKERS

## 3. Как на самом деле работает RLM

В `crates/tui/src/rlm/`:
- `rlm.rs` — инструменты `rlm_open`, `rlm_eval`, `rlm_configure`, `rlm_close`
- RLM = контекстное окно в 1M токенов + иерархическое управление
- Не отдельный оркестратор, а **инструмент** для LLM

## 4. Что это значит для mission-control

### 4.1. Стратегия форка меняется

Было: "Форкаем, добавляем Mission Mode + Redis bus"
Стало: **Ничего не форкаем. Используем TUI как dependency.**

```
Наш подход:
  ┌─────────────────────────────────────────┐
  │  mission-control (наше приложение)       │
  │  ┌───────────────────────────────────┐  │
  │  │  TUI (библиотека, git dependency)  │  │
  │  │  └── sub-agent система            │  │
  │  │  └── RUNTIME_API (HTTP)           │  │
  │  │  └── MCP                          │  │
  │  │  └── skills                       │  │
  │  └───────────────────────────────────┘  │
  │  ┌───────────────────────────────────┐  │
  │  │  Наши модули:                     │  │
  │  │  └── bridge.rs → 238             │  │
  │  │  └── mission.yml конфиг          │  │
  │  │  └── наши SKILL.md               │  │
  │  └───────────────────────────────────┘  │
  └─────────────────────────────────────────┘
```

### 4.2. Что реально нужно сделать

| Компонент | Действие |
|-----------|----------|
| **Sub-agents** | Создать SKILL.md с ролью `general` + custom для миссии |
| **MCP-серверы** | Написать на Python/Rust как отдельные процессы |
| **Bridge к 238** | Отдельный процесс (Python или Rust), читает RUNTIME_API |
| **Headless режим** | `deepseek serve --http` — уже есть |
| **Memory** | Использовать стандартную `~/.deepseek/memory.md` + ChromaDB на 238 |
| **DTN** | tc-netem, отдельный скрипт |

### 4.3. Пример запуска миссии

```bash
# 1. Запускаем TUI в headless режиме (ядро миссии)
deepseek serve --http --port 7878 --workers 4

# 2. Bridge читает findings из RUNTIME_API и шлёт на 238
python3 bridge.py --tui-api http://localhost:7878 --hub-url https://238.hub.waters

# 3. SKILL.md миссии говорит sub-agent'ам что делать
# (загружается при старте TUI)
```

## 5. Вывод

Весь план форка в docs/mission-1/FORK_SPECIFICATION.md нужно переписать.
Форк не нужен — достаточно конфигурации + bridge + SKILL.md.
Mission Mode как отдельный модуль не нужен — это комбинация `serve --http` + Agent mode.
