# AGENTS.md — WATERS Platform (дашборд Гексады)

## Статус Гексады

| Агент | Код | Версия | Статус | Файл AGENTS |
|-------|-----|--------|--------|-------------|
| Архитектор | `agent.architect.v1` | 1.0 | **active** | `agents/architect_AGENTS.md` |
| Конструктор Сети | `agent.constructor.v1` | 1.0 | **active** | `agents/constructor_AGENTS.md` |
| Интегратор Знаний | `agent.integrator.v1` | 1.0 | **active** | `agents/integrator_AGENTS.md` |
| Директор по Смыслу | `agent.director.v1` | 1.0 | **active** | `agents/director_AGENTS.md` |
| Хранитель Протокола | `agent.keeper.v1` | 1.0 | **active** | `agents/keeper_AGENTS.md` |
| Законодатель | `agent.lawkeeper.v1` | 1.0 | **active** | `agents/lawkeeper_AGENTS.md` |

## Запуск агентов

### Быстрый запуск (tmux, 24/7, локальные модели)

```bash
# 1. Конструктор Сети (на сервере 238)
./scripts/opencode_tmux.sh constructor

# 2. Остальные агенты (на сервере 238)
./scripts/opencode_tmux.sh architect
./scripts/opencode_tmux.sh integrator
./scripts/opencode_tmux.sh director
./scripts/opencode_tmux.sh lawkeeper
./scripts/opencode_tmux.sh keeper

# Подключиться к агенту:
tmux attach -t waters:constructor

# Отключиться (агент продолжает работу):
# Ctrl+B, затем D
```

### Модели

Агенты по умолчанию используют `Qwen 2.5 14B` (Ollama на 237).
Доступные модели:

| Модель | ID для `--model` | Характеристика |
|--------|-------------------|----------------|
| Qwen 2.5 14B | `ollama/qwen2.5:14b` | Сбалансированная, по умолчанию |
| Qwen 2.5 7B | `ollama/qwen2.5:7b` | Быстрая (CPU) |
| DeepSeek R1 7B | `ollama/deepseek-r1:7b` | R1-рассуждения, CPU |
| DeepSeek V4 Flash | `deepseek/deepseek-v4-flash` | Внешняя, нужен ключ |

```bash
# Запуск с конкретной моделью:
opencode --model ollama/qwen2.5:7b
opencode --model deepseek/deepseek-v4-flash
```

### Полный синтаксис запуска

```bash
# Вариант A: через tmux (рекомендуется — сессия не рвётся)
./scripts/opencode_tmux.sh <agent> [start|attach|stop]
# Пример:
./scripts/opencode_tmux.sh constructor       # запустить
./scripts/opencode_tmux.sh constructor attach # подключиться
./scripts/opencode_tmux.sh list              # все запущенные

# Вариант B: через run_*.sh (простой запуск)
./run_constructor.sh          # обычный
./run_constructor.sh --tmux   # или в tmux

# Вариант C: прямой opencode (для отладки)
cp agents/constructor_AGENTS.md AGENTS.md && opencode

# Вариант D: headless serve + CEO API (см. scripts/ceo.sh)
./scripts/opencode_serve.sh start
export OPENCODE_SERVER=http://opencode:$(./scripts/opencode_serve.sh password)@localhost:4096
./scripts/ceo.sh create constructor
./scripts/ceo.sh msg <id> "задача"
```

### После завершения

```bash
# Восстановить дашборд:
git checkout AGENTS.md
```

## Троицы

| Троица | Агенты | Назначение |
|--------|--------|------------|
| Структуры | Архитектор, Конструктор, Интегратор | Как построить? |
| Смысла | Директор, Законодатель, Хранитель | Зачем и по каким законам? |
| Надзора | Хранитель, Законодатель, Архитектор | Всё ли верно? |

## Общий контекст

- **Платформа**: WATERS — автономные ИИ-агенты для колонизации
- **Фаза**: 0 (Зарождение) → 1 (Активация Гексады)
- **Спринт**: 1
- **Язык**: русский

## Общие артефакты (read-only для всех)

| Файл | Назначение |
|------|------------|
| `doctrine/manifesto.md` | Манифест платформы |
| `doctrine/hivemind_spec.md` | HiveMind протокол |
| `doctrine/hexad.md` | Состав и структура Гексады |
| `doctrine/nervous_system.md` | Нервная система (Kafka) |

## Правила совместной работы

1. Каждый агент работает в своей сессии OpenCode
2. Каждый агент использует свой `agents/{имя}_AGENTS.md`
3. Изменения в общей файловой системе видны всем сразу
4. При конфликте — Архитектор имеет приоритет в стратегических вопросах
5. При конфликте — Конструктор имеет приоритет в инфраструктурных вопросах
6. Координация через `planners.questions.v1` / `planners.answers.v1` (файловый протокол до Kafka)

## Файловый протокол (до запуска Kafka)

```
agents/architect_AGENTS.md      ← Архитектор пишет решения
agents/constructor_AGENTS.md    ← Конструктор пишет артефакты
agents/integrator_AGENTS.md     ← Интегратор пишет данные из внешнего мира
agents/director_AGENTS.md       ← Директор пишет интерпретации
agents/keeper_AGENTS.md         ← Хранитель пишет аудиты
agents/lawkeeper_AGENTS.md      ← Законодатель пишет законы
schemas/                        ← Конструктор публикует схемы
product/roadmap.json            ← Архитектор публикует роадмап
integrations/                   ← Интегратор публикует адаптеры
```
