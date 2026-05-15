# Организация разработки — Миссия 1

**Статус:** Утверждено Архитектором
**Дата:** 2026-05-15

---

## 1. Роли и участники

### 1.1. Текущая команда

| Роль | Агент | Ответственность |
|------|-------|-----------------|
| **Архитектор** | `agent.architect.v1` | Онтология, спецификации, роадмап, приёмка |
| **Конструктор Сети** | `agent.constructor.v1` | Форк mission-control, инфраструктура, деплой |
| **Интегратор Знаний** | `agent.integrator.v1` | MCP-серверы, NASA API, данные |

### 1.2. Распределение задач по форку

| Модуль | Исполнитель | Приоритет |
|--------|------------|-----------|
| `mission_mode.rs` — автономный режим | Конструктор | P0 |
| `redis_bus.rs` — pub/sub шина | Конструктор | P0 |
| `bridge.rs` — HTTP bridge к 238 | Конструктор | P0 |
| `dtn.rs` — DTN-эмуляция | Конструктор | P1 |
| MCP-серверы (NASA, спектры, траектории) | Интегратор | P1 |
| Конвертеры скиллов (TUI ↔ TOML) | Архитектор + Конструктор | P1 |
| Skill meteorite-search v1 | Архитектор | P2 |
| Сайт виртуальной миссии | Конструктор + Интегратор | P2 |

## 2. Git-репозитории

### 2.1. Структура

```
waters-core/          # Существующий — доктрины, доки, схемы, онтологии
  └── docs/mission-1/ # Документация миссии (архитектура, требования)

mission-control/       # НОВЫЙ — форк DeepSeek-TUI (чистый Rust)
  ├── src/            # Исходники mission-control
  ├── skills/         # Наши скиллы в TOML
  ├── mcp-servers/    # MCP-серверы (Rust или Python)
  └── docker-compose.yml

mission-website/       # НОВЫЙ — сайт виртуальной миссии
  └── (фронтенд + бэкенд)
```

### 2.2. Стратегия ветвления

```
main              # Стабильный релиз
  ├── develop     # Интеграционная ветка
  │   ├── feat/mission-mode    # mission_mode.rs
  │   ├── feat/redis-bus       # redis_bus.rs
  │   ├── feat/bridge          # bridge.rs
  │   ├── feat/dtn             # dtn.rs
  │   └── feat/mcp-*          # MCP-серверы
  └── release/v*   # Релизные ветки
```

### 2.3. Правила

- `main` — только через PR, код-ревью обязателен
- `develop` — ежедневные merge feat/* веток
- Коммиты: `[M1] краткое описание на русском` (M1 = Миссия 1)
- Теги: `v0.1.0`, `v0.2.0`, `v1.0.0`

## 3. CI/CD (GitHub Actions)

### 3.1. mission-control

```yaml
# .github/workflows/ci.yml
on: [push, pull_request]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
      - run: cargo test
      - run: cargo clippy -- -D warnings
  docker:
    needs: build
    steps:
      - run: docker build -t mission-control .
```

### 3.2. Авто-деплой на миссионный сервер

```yaml
# .github/workflows/deploy.yml
on:
  push:
    tags: ['v*']
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release --target x86_64-unknown-linux-gnu
      - run: scp target/release/mission-control user@mission-server:/opt/mission-control/
      - run: ssh user@mission-server 'systemctl restart mission-control'
```

## 4. Процесс разработки

### 4.1. Один цикл = 1 задача

```
1. Архитектор пишет SPEC в waters-core/docs/mission-1/
2. Конструктор читает SPEC, задаёт вопросы
3. Конструктор создаёт feat/* ветку в mission-control
4. Конструктор пишет код
5. Конструктор тестирует (cargo test + интеграционный тест)
6. Конструктор создаёт PR в develop
7. Архитектор + Интегратор ревьюят
8. Merge → develop
9. Автоматические тесты
10. По готовности — merge в main + tag + деплой
```

### 4.2. Формат задач (в issues GitHub)

```markdown
## [M1] module_name — краткое описание

### Спецификация
Ссылка на SPEC в waters-core

### Чек-лист
- [ ] Реализован модуль
- [ ] Написаны unit-тесты
- [ ] Интеграционный тест пройден
- [ ] Документация обновлена
- [ ] PR в develop

### Приёмка
Критерии:
1. Модуль компилируется без ошибок
2. Тесты проходят
3. Интеграция с существующими модулями не сломана
```

## 5. Стандарты кода

### 5.1. Rust

- `cargo clippy` — без предупреждений
- `cargo fmt` — перед каждым коммитом
- Комментарии — на русском (для контекста) или английском (для API)
- `unwrap()` запрещён — только `?` или `expect("контекст")`
- Все публичные функции документированы (`///`)

### 5.2. Конфиги (TOML/JSON)

- TOML — для скиллов (skill.toml)
- JSON — для схем, latency_plan, конфигов миссии
- snake_case для ключей

## 6. Тестирование

### 6.1. Уровни

| Уровень | Инструмент | Запуск |
|---------|-----------|--------|
| Unit | `cargo test` | Каждый PR |
| Интеграционные | `cargo test --test integration` | Каждый PR в main |
| E2E (миссия) | docker-compose + скрипты | Перед релизом |
| Нагрузочное | `locust` или `rust` bench | Раз в спринт |

### 6.2. Интеграционный тест mission-control

```bash
# test/e2e.sh
# 1. Запустить Redis (docker)
# 2. Запустить mission-control с тестовой миссией
# 3. Подождать N секунд
# 4. Проверить что findings появились
# 5. Проверить heartbeat
# 6. Остановить
# → exit 0 если всё ок
```

## 7. Релизный процесс

| Версия | Содержание | Дата |
|--------|-----------|------|
| **v0.1.0** | Скелет: компилируется, 1 Flash работает | 2026-05-18 |
| **v0.2.0** | Redis bus + Mission Mode (L0-L2) | 2026-05-21 |
| **v0.3.0** | MCP-серверы + Bridge к 238 | 2026-05-25 |
| **v0.4.0** | DTN + Skills + Website alpha | 2026-05-28 |
| **v1.0.0** | Релиз: полный цикл миссии | 2026-06-01 |

## 8. Коммуникация

| Канал | Назначение |
|-------|-----------|
| GitHub Issues mission-control | Задачи, баги, PR |
| waters-core/issues | Документация, SPEC, решения |
| Бортовой журнал (каждый агент) | Ход работ, проблемы, решения |

Все решения по архитектуре — через Архитектора.
Все решения по реализации — через Конструктора.
Все решения по данным — через Интегратора.
Спорные вопросы — планёрка втроём через issue.
