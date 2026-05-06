# AGENTS.md — WATERS Platform (дашборд Гексады)

## Статус Гексады

| Агент | Код | Версия | Статус | Файл AGENTS |
|-------|-----|--------|--------|-------------|
| Архитектор | `agent.architect.v1` | 1.0 | **active** | `agents/architect_AGENTS.md` |
| Конструктор Сети | `agent.constructor.v1` | 1.0 | **active** | `agents/constructor_AGENTS.md` |
| Интегратор Знаний | `agent.integrator.v1` | — | pending | — |
| Директор по Смыслу | `agent.director.v1` | — | pending | — |
| Хранитель Протокола | `agent.keeper.v1` | — | pending | — |
| Законодатель | `agent.lawkeeper.v1` | — | pending | — |

## Запуск агентов

```bash
# Terminal 1: Архитектор
cp agents/architect_AGENTS.md AGENTS.md && opencode

# Terminal 2: Конструктор Сети
cp agents/constructor_AGENTS.md AGENTS.md && opencode
```

Или используй скрипты:
```bash
./run_architect.sh   # Terminal 1
./run_constructor.sh # Terminal 2

# После завершения — восстановить дашборд:
git checkout AGENTS.md
```

## Общий контекст

- **Платформа**: WATERS — автономные ИИ-агенты для колонизации
- **Фаза**: 0 (Зарождение)
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
3. Изменения в общей файловой системе видны обоим сразу
4. При конфликте — Архитектор имеет приоритет в стратегических вопросах
5. При конфликте — Конструктор имеет приоритет в инфраструктурных вопросах
6. Координация через `planners.questions.v1` / `planners.answers.v1` (файловый протокол до Kafka)

## Файловый протокол (до запуска Kafka)

```
agents/architect_AGENTS.md      ← Архитектор пишет решения
agents/constructor_AGENTS.md    ← Конструктор пишет артефакты
schemas/                        ← Конструктор публикует схемы
product/roadmap.json            ← Архитектор публикует роадмап
```
