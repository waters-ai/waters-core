# waters-node v0.3 — Формат скиллов и конвертация из TUI

> **Версия:** 2.0
> **Дата:** 2026-05-15

---

## 1. Наш формат: Skills 2.0

### Структура директории

```
skills/<skill-name>/
├── SKILL.md       # инструкция агенту (markdown)
├── skill.json     # манифест (метаданные, JSON)
└── bookmarks/     # тесты (опционально)
    └── test-1.json
```

### SKILL.md (человекочитаемая инструкция)

```markdown
---
name: scout-us
description: US/global search. English sources, DuckDuckGo.
---

# Scout US

Ты — Scout-US агент. Ищешь информацию в US-сегменте интернета.

## Инструменты
- web_search: DuckDuckGo (free, без ключей)
- fetch_url: чтение страниц

## Формат ответа
```json
{
  "direction": "us",
  "findings": [...],
  "confidence": 0.0-1.0
}
```
```

### skill.json (машинный манифест)

```json
{
  "name": "scout-us",
  "version": "0.1.0",
  "description": "US/global search. English sources, DuckDuckGo.",
  "tags": ["search", "us", "en"],
  "role": "explorer",
  "model_hint": "ollama",
  "dependencies": ["general"],
  "bridges": {
    "required": ["duckduckgo"],
    "optional": ["notebooklm", "chromadb"]
  },
  "group_resources": {
    "llm": {"min_priority": 10},
    "memory": {"chromadb": true, "lightrag": false}
  },
  "bookmarks": [
    {
      "name": "test-search",
      "prompt": "search for latest meteor news",
      "expected": "at least 3 results",
      "min_confidence": 0.5
    }
  ]
}
```

---

## 2. Формат TUI (что конвертируем)

TUI использует **только** SKILL.md с YAML frontmatter:

```markdown
---
name: my-skill
description: Use this when DeepSeek should follow my custom workflow.
---

# My Skill
Instructions for the agent go here.
```

**Чего НЕТ в TUI-формате:**
- `tags` — нет тегов для фильтрации
- `dependencies` — нет зависимостей
- `bridges` — нет привязки к внешним сервисам
- `group_resources` — нет групповых ресурсов
- `bookmarks` — нет тестов
- `role` — нет привязки к роли агента
- `model_hint` — нет подсказки модели

---

## 3. Конвертация TUI → наш формат

### Автоматическая (уже в skill.rs)

```rust
pub fn convert_tui_to_our(tui_path: &Path) -> Result<SkillManifest> {
    // 1. Читаем SKILL.md
    // 2. Парсим YAML frontmatter
    // 3. Создаём skill.json с полями по умолчанию:
    //    - tags: ["converted", "tui"]
    //    - bridges: {"required": [], "optional": []}
    //    - group_resources: default
    // 4. Копируем SKILL.md без изменений
    // 5. Пишем skill.json рядом
}
```

### Ручная (рекомендуется)

```bash
# Скопировать скилл из TUI
cp -r ~/.deepseek/skills/my-skill skills/my-skill

# Конвертировать
waters-node skill convert skills/my-skill
# → skills/my-skill/skill.json создан

# Дополнить поля вручную
# - tags: добавить теги
# - bridges: указать нужные бриджи
# - group_resources: указать ресурсы группы
```

---

## 4. Таблица конвертации TUI → waters-node

| Поле TUI | Поле waters-node | Конвертация |
|----------|-----------------|-------------|
| `name` | `name` | Прямое копирование |
| `description` | `description` | Прямое копирование |
| — | `version` | `"0.1.0"` (default) |
| — | `tags` | `["converted", "tui"]` (default) |
| — | `role` | `"general"` (default) |
| — | `model_hint` | `"auto"` (default) |
| — | `dependencies` | `[]` (default) |
| — | `bridges.required` | `[]` (default) |
| — | `bridges.optional` | `[]` (default) |
| — | `group_resources` | `{}` (default) |
| — | `bookmarks` | `[]` (default) |

---

## 5. Где лежат скиллы (discovery order)

```
Приоритет поиска:
  1. <workspace>/.agents/skills/<name>/    (workspace-specific)
  2. <workspace>/skills/<name>/            (project-level)
  3. ~/.agents/skills/<name>/              (user-level)
  4. <binary>/skills/<name>/               (built-in, v0.2.0)

Каждый уровень может переопределить предыдущий.
Группа может шарить скиллы через shared_skills в group.rs.
```

---

## 6. Привязка скилла к агенту и задаче

```rust
// Пример: создание агента со скиллом
pub fn create_agent_with_skill(
    name: &str,
    skill_name: &str,
    node_id: &str,
) -> Agent {
    let skill = skill_registry.get(skill_name).unwrap();
    Agent {
        name: name.to_string(),
        role: skill.manifest.role.clone(),
        agent_type: "personal",
        owner_node: node_id.to_string(),
        status: "idle",
        active_skill: Some(skill_name.to_string()),
        // Ресурсы из skill.manifest.bridges
        required_bridges: skill.manifest.bridges.required.clone(),
    }
}
```
