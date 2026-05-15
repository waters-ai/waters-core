---
name: scout-duckduckgo
description: Поиск через DuckDuckGO API — веб-страницы, новости, изображения
---

# SKILL.md — scout-duckduckgo v1.0.0

## Идентификация

| Поле | Значение |
|------|----------|
| **skill_id** | `scout-duckduckgo` |
| **версия** | 1.0.0 |
| **владелец** | `agent.scout.v1` |
| **тип** | `support` |
| **слой** | I — Сеть |
| **статус** | `active` |

## Назначение

Поиск информации через DuckDuckGo + извлечение полного текста страниц. Основной бесплатный поисковый движок Scout.

## Входные данные

| Поле | Тип | Описание |
|------|-----|----------|
| query | string | Поисковый запрос |
| max_results | int | Макс. результатов (default: 10) |
| timeout | int | Таймаут для RU-зоны (default: 20с) |

## Выходные данные

| Поле | Тип | Описание |
|------|-----|----------|
| title | string | Заголовок результата |
| url | string | Ссылка на страницу |
| snippet | string | Сниппет от DDG |
| content | string | Полный текст страницы (через PageFetcher) |
| domain_authority | string | high / medium / low |

## Особенности RU-зоны

- Таймаут увеличен до 20с (DDG может быть медленным из РФ)
- При ошибке → fallback на Yandex.XML (если настроен)
- User-Agent ротация (3 варианта)

## Бесплатно

DuckDuckGo не требует API-ключа. 0 токенов.
