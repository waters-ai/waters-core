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

```bash
# Terminal 1: Архитектор
./run_architect.sh

# Terminal 2: Конструктор Сети
./run_constructor.sh

# Terminal 3: Интегратор Знаний
./run_integrator.sh

# Terminal 4: Директор по Смыслу
cp agents/director_AGENTS.md AGENTS.md && opencode

# Terminal 5: Хранитель Протокола
cp agents/keeper_AGENTS.md AGENTS.md && opencode

# Terminal 6: Законодатель
cp agents/lawkeeper_AGENTS.md AGENTS.md && opencode

# После завершения — восстановить дашборд:
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
