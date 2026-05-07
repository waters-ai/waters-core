# Obsidian Setup Guide — WATERS Knowledge Base

## Настройка Obsidian на ноутбуке CEO

### Шаг 1: Открыть Vault

1. Открой **Obsidian**
2. Нажми **Open folder as Vault**
3. Выбери: `C:\Users\Admin\waters-ai\waters-core`
4. Нажми **Open**

### Шаг 2: Установить плагин Git

1. Открой **Settings** (иконка шестерёнки)
2. Перейди в **Community plugins** → **Browse**
3. Найди **Obsidian Git**
4. Нажми **Install** → **Enable**
5. В настройках плагина:
   - `Auto commit interval`: 30 минут
   - `Auto pull interval`: 30 минут
   - `Commit message`: `docs: auto-sync from Obsidian`
   - `Pull on startup`: включить

### Шаг 3: Деплоить изменения

Через Git:

```bash
git add .
git commit -m "docs: update from Obsidian"
git push
```

### Шаг 4: Рекомендуемые плагины

| Плагин | Зачем |
|--------|-------|
| **Obsidian Git** | Автосинхронизация с репозиторием |
| **Graph View** (встроенный) | Визуализация связей между документами |
| **Outline** (встроенный) | Быстрая навигация по структуре .md |
| **Tag Wrangler** | Управление тегами |

### Структура Vault

```
waters-core/
├── doctrine/        # Доктрины и манифесты
├── documents/       # Отчёты и архитектурные решения
├── missions/        # Миссии колонизации
├── ontology/        # Онтологии (Архитектор)
├── planners/        # Планёрки и вопросы агентов
├── prompts/         # Промпты агентов
├── skills/          # SKILL.md всех агентов
├── security/        # Политики безопасности
├── agents/          # AGENTS.md каждого агента
└── infrastructure/  # Docker и топология
```

### ОпенCode

Не забудь после изменений запустить `opencode` для синхронизации:

```bash
cd C:\Users\Admin\waters-ai\waters-core
opencode
```
