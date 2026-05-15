---
name: integrator-tinyfish
description: Поиск в вебе и извлечение контента страниц через TinyFish REST API. Ранжированные результаты, JavaScript-рендеринг, geo-таргетинг, батч-загрузка до 10 URL. Адаптировано для WATERS.
---

# TinyFish — веб-поиск и извлечение контента

Поиск в вебе и извлечение чистого контента страниц через TinyFish REST API.
Не требует MCP-сервера — работает через bash-клиент.

## Когда использовать

- Поиск по вебу с ранжированными результатами (заголовки, сниппеты, URL)
- Извлечение контента с одной или нескольких страниц (включая JavaScript)
- Конвейер поиск → извлечение: найди URL, затем получи полный контент
- Geo-таргетинговый поиск по стране и языку

## Установка

API-ключ получается на https://agent.tinyfish.ai/api-keys.
Хранится в `.secret_tinyfish_api_key` (YASA-DUTY-5).

## Протокол

### Шаг 1: Поиск

```bash
scripts/tinyfish.sh search "<запрос>" [страна] [язык] [страница]
```

**Параметры:**
- `query` (обязательно) — поисковый запрос. Поддерживает `site:` и `-site:`
- `location` (опционально) — код страны для geo-таргетинга (US, GB, FR, DE, RU)
- `language` (опционально) — код языка (en, fr, de, ru)
- `page` (опционально) — номер страницы (0-based, макс 10)

**Примеры:**
```bash
scripts/tinyfish.sh search "агенты ИИ колонизация" RU ru
scripts/tinyfish.sh search "site:github.com waters platform"
scripts/tinyfish.sh search "space habitat 3d printing" US en
```

### Шаг 2: Извлечение контента

```bash
scripts/tinyfish.sh fetch "<url1>" ["<url2>"] [--format markdown|html|json] [--links]
```

**Параметры:**
- `urls` (обязательно) — один или несколько URL (макс 10)
- `--format` (опционально) — формат вывода: `markdown` (по умолчанию), `html`, `json`
- `--links` (опционально) — включить все `href` ссылки в вывод

**Примеры:**
```bash
scripts/tinyfish.sh fetch "https://example.com"
scripts/tinyfish.sh fetch "https://arxiv.org/abs/2401.12345" --format markdown
```

### Шаг 3: Валидация ключей

```bash
scripts/tinyfish.sh validate
```

Проверяет все настроенные API-ключи.

## Интеграция с WATERS

- Полевой агент (`agents/field_agent.py`) использует TinyFish как один из поисковых движков
- Результаты сохраняются в `/home/waters-data/raw/scout/{date}/`
- После валидации NotebookLM копируются на 238

## Безопасность (YASA)

- API-ключ в `.secret_tinyfish_api_key` (не в git)
- Поддержка ротации нескольких ключей (round-robin)
- Автоматический failover при 429

## Связанные файлы

- `scripts/tinyfish.sh` — bash-клиент TinyFish
- `agents/field_agent.py` — полевой агент
- `scripts/deploy_field_agent.sh` — скрипт развёртывания
