---
name: scout-youtube
description: Поиск и транскрибация YouTube-видео через API
---

# SKILL.md — scout-youtube v1.0.0

## Идентификация

| Поле | Значение |
|------|----------|
| **skill_id** | `scout-youtube` |
| **версия** | 1.0.0 |
| **владелец** | `agent.scout.v1` |
| **тип** | `support` |
| **слой** | I — Сеть |
| **статус** | `active` |

## Назначение

Поиск и транскрипция видео на YouTube. Получение текстового содержания видео (субтитры).

## Входные данные

| Поле | Тип | Описание |
|------|-----|----------|
| query | string | Поисковый запрос |
| max_results | int | Макс. видео для транскрипции (default: 5) |

## Выходные данные

| Поле | Тип | Описание |
|------|-----|----------|
| title | string | ID видео (YouTube: {vid}) |
| url | string | https://youtube.com/watch?v={vid} |
| content | string | Полный текст транскрипции |

## Алгоритм

1. Поиск видео через YouTube HTML (без API): `https://www.youtube.com/results?search_query=...`
2. Извлечение video_id из HTML (регулярка)
3. Загрузка транскрипции через youtube-transcript-api
4. Форматирование через TextFormatter

## Ограничения

- Доступны только видео с субтитрами (ru/en)
- Rate limit: 1 запрос / 5с
- Макс 5 видео за 1 поиск (чтобы не банили)
