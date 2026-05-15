---
name: integrator-notebooklm
description: Полный программный доступ к Google NotebookLM — создание ноутбуков, добавление источников, генерация артефактов (подкасты, видео, слайды, квизы, flashcards, mind-map), чат с источниками. Адаптировано для платформы WATERS.
---

# NotebookLM Automation

Полный программный доступ к Google NotebookLM — включая функции, недоступные в веб-UI.
Создавай ноутбуки, добавляй источники (URL, YouTube, PDF, аудио, видео, изображения),
общайся с контентом, генерируй все типы артефактов и скачивай результаты в различных форматах.

## Установка

```bash
pip install notebooklm-py
```

## Аутентификация

**Важно:** Перед использованием любой команды необходимо аутентифицироваться:

```bash
notebooklm login          # Открывает браузер для Google OAuth
notebooklm list           # Проверить, что аутентификация работает
```

**CI/CD и параллельные агенты:**

| Переменная | Назначение |
|------------|------------|
| `NOTEBOOKLM_HOME` | Кастомная директория конфига |
| `NOTEBOOKLM_PROFILE` | Имя активного профиля |
| `NOTEBOOKLM_AUTH_JSON` | Inline auth JSON — без записи файлов |

## Быстрый старт

### CLI

```bash
# Создать ноутбук и добавить источники
notebooklm create "Моё исследование"
notebooklm source add "https://example.com"
notebooklm source add "./paper.pdf"

# Чат с источниками
notebooklm ask "Каковы ключевые темы?"

# Генерация контента
notebooklm generate audio "сделай увлекательно" --wait
notebooklm generate quiz --difficulty hard
notebooklm generate flashcards --quantity more
notebooklm generate video --style whiteboard --wait
notebooklm generate slide-deck
notebooklm generate infographic --orientation portrait
notebooklm generate mind-map
notebooklm generate data-table "сравни ключевые концепции"

# Скачивание артефактов
notebooklm download audio ./podcast.mp3
notebooklm download quiz --format json ./quiz.json
notebooklm download mind-map ./mindmap.json
```

### Python API

```python
import asyncio
from notebooklm import NotebookLMClient

async def main():
    async with await NotebookLMClient.from_storage() as client:
        nb = await client.notebooks.create("Исследование")
        await client.sources.add_url(nb.id, "https://example.com", wait=True)
        result = await client.chat.ask(nb.id, "Резюмируй это")
        print(result.answer)

asyncio.run(main())
```

## Артефакты (типы генерации)

| Тип | Команда | Формат скачивания |
|-----|---------|-------------------|
| Audio Overview (подкаст) | `generate audio` | MP3/MP4 |
| Video Overview | `generate video` | MP4 |
| Slide Deck | `generate slide-deck` | PDF, PPTX |
| Infographic | `generate infographic` | PNG |
| Quiz | `generate quiz` | JSON, Markdown, HTML |
| Flashcards | `generate flashcards` | JSON, Markdown, HTML |
| Report | `generate report` | Markdown |
| Data Table | `generate data-table` | CSV |
| Mind Map | `generate mind-map` | JSON |

## Интеграция с WATERS

- **Чат с источниками** — получай ответы с цитатами для проверки фактов
- **Валидация качества** — используй `notebooklm ask` для оценки источника (научный/любительский/фейк)
- **Quality Score** — запроси у NotebookLM оценку достоверности по шкале 0.0-1.0
- **Подготовка данных** — загружай сырые файлы, получай структурированные отчёты

## Безопасность (YASA)

- Токен Telegram: в `.secret_telegram_token` (YASA-DUTY-5)
- Сессия NotebookLM: в `~/.notebooklm/storage_state.json` (не в git)
- API-ключи: переменные окружения, не хардкодить

## Связанные файлы

- `agents/field_agent.py` — полевой агент-разведчик
- `schemas/data_ready.schema.json` — схема Kafka-уведомления
- `scripts/deploy_field_agent.sh` — скрипт развёртывания

## Ограничения

- Неофициальное API Google — может сломаться без предупреждения
- Rate limits: ~5 аудио/час, ~10 квизов/час
- Требует browser-based аутентификации при первом запуске
