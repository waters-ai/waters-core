# Спецификация mission-control v2.0 (исправленная)

**Статус:** Утверждено Архитектором
**Дата:** 2026-05-15
**Основание:** Анализ реального DeepSeek TUI v0.8.37 (github.com/Hmbown/DeepSeek-TUI, 29.6k ⭐)

---

## 1. Ключевое решение: НЕ форкаем TUI

После изучения реальной архитектуры TUI выяснилось:
- **sub-agent система** — 7 ролей (general, explore, plan, review, implementer, verifier, custom). Не "RLM Pro/Flash"
- **RUNTIME_API** — полноценный HTTP/SSE API (sessions, turns, tasks, automations). Не нужен свой Mission API
- **Skills** — встроенная система SKILL.md. Не нужны конвертеры
- **MCP** — встроенный клиент. Не нужны доработки
- **Memory** — простой Markdown-файл (max 100KB). Не Redis на миссионном сервере

**Форк отменяется.** Вместо этого:

```
┌──────────────────────────────────────────────────┐
│              mission-control                      │
│   ┌────────────────────────────────────────────┐ │
│   │  TUI (git dependency, crate tui)           │ │
│   │  └── sub-agents (наши SKILL.md)            │ │
│   │  └── RUNTIME_API (HTTP serve)              │ │
│   │  └── MCP клиент (наши серверы)             │ │
│   └────────────────────────────────────────────┘ │
│   ┌────────────────────────────────────────────┐ │
│   │  bridge-238 (отдельный процесс)            │ │
│   │  └── читает RUNTIME_API (findings)         │ │
│   │  └── шлёт HTTP POST на 238                 │ │
│   │  └── принимает orders с 238                │ │
│   └────────────────────────────────────────────┘ │
│   ┌────────────────────────────────────────────┐ │
│   │  dtn.sh (tc-netem wrapper)                 │ │
│   └────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
```

## 2. Архитектура

### 2.1. Ядро — TUI (без изменений)

```bash
# Запуск headless-режима TUI = ядро миссии
deepseek serve --http --port 7878 --workers 4
```

Это даёт:
- Sub-agents (7 ролей) через `agent_open`/`agent_eval`/`agent_close`
- RUNTIME_API: session management, turn execution, SSE events
- Skills system: SKILL.md загружаются при старте
- MCP-client: подключает наши MCP-серверы
- Memory: Markdown-файл (можно симлинкнуть на наш)

### 2.2. Bridge к 238 (новый процесс)

```python
# bridge.py — отдельный Python-процесс
#
# Читает RUNTIME_API TUI:
#   GET /sessions — активные сессии
#   GET /session/{id}/turns — результаты агентов
#   POST /session/{id}/turn — отправить задачу агенту
#
# Пишет в 238:
#   POST https://238.hub.waters/mission/1/findings  (batch findings)
#   GET  https://238.hub.waters/mission/1/orders      (orders)
```

### 2.3. MCP-серверы (отдельные процессы)

```
mcp-nasa-fireball  — Python, stdio MCP, NASA Fireball API
mcp-spectra        — Python, stdio MCP, спектральные БД
mcp-trajectory     — Rust/Python, расчёт траекторий
mcp-bridge-238     — Python, stdio MCP, компас для отправки on-demand
```

### 2.4. DTN (shell-скрипт)

```bash
# dtn.sh — tc-netem wrapper
./dtn.sh apply lunar    # задержка 1.3s
./dtn.sh remove         # сброс
```

## 3. Sub-agents для миссии

### 3.1. SKILL.md: meteorite-search

```markdown
---
name: meteorite-collector
role: general
model: deepseek-v4-flash
instructions: |
  Ты — DataCollectorAgent. Твоя задача — собирать данные из NASA API.
  Используй MCP-сервер mcp-nasa-fireball.
  Найденные данные возвращай в формате JSON с полями:
  - type: "fireball"
  - coordinates: {lat, lng}
  - date: ISO8601
  - energy: kT
  - confidence: 0.0-1.0
---

```

### 3.2. Какие SKILL.md нужны

| SKILL | Роль sub-agent | MCP-серверы |
|-------|---------------|-------------|
| `meteorite-collector` | general | mcp-nasa-fireball |
| `meteorite-analyzer` | plan+review | mcp-spectra, mcp-trajectory |
| `meteorite-matcher` | explore+general | (локальный анализ) |
| `mission-coordinator` | plan | mcp-bridge-238 |

### 3.3. Запуск sub-agent'а

```
agent_open(
  role="general",
  task="Собери данные NASA fireball за последние 24 часа",
  skill="meteorite-collector",
  fork_context=true
)
```

## 4. Bridge к 238 (детали)

### 4.1. Цикл bridge

```python
# Псевдокод bridge.py
while True:
    # 1. Читаем findings из RUNTIME_API TUI
    findings = get_tui_findings()
    
    # 2. Шлём на 238
    if findings:
        response = http_post("https://238.hub.waters/mission/1/findings", findings)
    
    # 3. Читаем orders с 238
    orders = http_get("https://238.hub.waters/mission/1/orders")
    
    # 4. Отправляем orders в TUI как новую задачу
    for order in orders:
        post_tui_turn(order)
    
    # 5. Heartbeat
    http_post("https://238.hub.waters/mission/1/heartbeat", {uptime, agents, findings_count})
    
    sleep(60)
```

### 4.2. Автономия (L0-L4)

Реализуется bridge.py, не TUI:
- L0: связь есть, батч каждые 60s
- L1: батч реже (300s), экономим API
- L2: findings в локальный файл, sync при восстановлении
- L3: только critical findings
- L4: только heartbeat

## 5. Реализация (пересмотренная)

### Фаза 0: Bridge + SKILL.md (2 дня)

| Задача | Оценка |
|--------|--------|
| Установка TUI (cargo install / npm) | 1ч |
| Запуск `deepseek serve --http --port 7878` | 1ч |
| bridge.py: читает RUNTIME_API | 4ч |
| bridge.py: POST на 238 | 2ч |
| SKILL.md: meteorite-collector | 2ч |
| Тест: sub-agent → finding → 238 | 2ч |

**Итого:** ~12 часов

### Фаза 1: MCP-серверы (2-3 дня)

| Задача | Оценка |
|--------|--------|
| MCP-сервер mcp-nasa-fireball (Python) | 4ч |
| MCP-сервер mcp-spectra (Python) | 4ч |
| MCP-сервер mcp-trajectory (Rust/Python) | 4ч |
| Bridge: обработка orders с 238 | 3ч |
| SKILL.md: analyzer + matcher + coordinator | 3ч |
| Интеграционный тест | 2ч |

**Итого:** ~20 часов

### Фаза 2: DTN + Автономия (1-2 дня)

| Задача | Оценка |
|--------|--------|
| dtn.sh: tc-netem wrapper | 2ч |
| latency_plan.json | 1ч |
| bridge.py: L0-L4 автономия | 4ч |
| Тест: потеря связи → L2 → восстановление | 2ч |

**Итого:** ~9 часов

### Фаза 3: Сайт + релиз (2 дня)

| Задача | Оценка |
|--------|--------|
| Dockerfile + docker-compose | 2ч |
| CI/CD (GitHub Actions) | 2ч |
| Документация | 2ч |
| Сайт виртуальной миссии | см. WEBSITE_SPEC.md |

**Итого:** ~6 часов + сайт

### Общая оценка: ~47 часов (~6 рабочих дней)

## 6. Отличия от v1 (важно!)

| Параметр | v1 (неверно) | v2 (реальность) |
|----------|-------------|-----------------|
| **Форк TUI** | Да, полный клон Rust-кода | НЕ форкаем. TUI как зависимость |
| **Архитектура** | RLM Pro + Flash агенты | Sub-agents (7 ролей) + SKILL.md |
| **Модули** | 5 новых модулей (mission_mode.rs, dtn.rs, ...) | bridge.py + dtn.sh + SKILL.md |
| **Оценка** | ~76 часов, 10 дней | ~47 часов, 6 дней |
| **Redis** | На миссионном сервере | Не нужен на миссионном. Только на 238 |
| **RocksDB** | Durable queue | Не используется в TUI. JSON файл |
| **Memory** | Redis+ChromaDB+LightRAG | Markdown-файл (memory.md) |
| **Конвертеры** | TUI↔TOML, OpenCode↔TOML | Не нужны. TUI читает SKILL.md напрямую |
| **Rust** | 4 Rust-агента | Python bridge + shell скрипты |

## 7. Репозиторий

```yaml
mission-control/
├── bridge/              # Python-процесс для связи с 238
│   ├── bridge.py
│   ├── requirements.txt
│   └── Dockerfile
├── skills/              # SKILL.md для sub-agent'ов
│   ├── meteorite-collector/
│   │   └── SKILL.md
│   ├── meteorite-analyzer/
│   │   └── SKILL.md
│   ├── meteorite-matcher/
│   │   └── SKILL.md
│   └── mission-coordinator/
│       └── SKILL.md
├── mcp-servers/         # MCP-серверы (отдельные процессы)
│   ├── mcp-nasa-fireball/
│   ├── mcp-spectra/
│   └── mcp-trajectory/
├── dtn/                 # DTN-эмуляция
│   ├── dtn.sh
│   └── latency_plan.json
├── config/              # Конфиги TUI для миссии
│   ├── mission.toml
│   └── mcp.json
├── docker-compose.yml   # Redis для bridge + сервисы
└── README.md
```
