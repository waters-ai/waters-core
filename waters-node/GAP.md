# GAP Analysis — waters-node vs TUI + Roadmap

**Дата:** 2026-05-16
**Версия:** 0.3.0 → 0.4.0
**Размер:** 36 .rs файлов, 9460 строк, 20 агентов

## Условные обозначения

- ✅ = сделано
- ⬜ = в работе / запланировано
- ❌ = не нужно (сознательно)
- 🔄 = требует доработки

---

## 1. TUI-совместимость (7 ролей 1:1)

| Агент | Статус | Примечание |
|-------|--------|-----------|
| tui-general | ✅ | Универсальный |
| tui-explorer | ✅ | Поиск (только чтение) |
| tui-planner | ✅ | Проектирование |
| tui-reviewer | ✅ | Ревью |
| tui-implementer | ✅ | Реализация |
| tui-verifier | ✅ | Верификация |
| tui-custom | ✅ | Кастом |

## 2. WATERS профессии (базовые)

| Агент | Статус | Примечание |
|-------|--------|-----------|
| waters-collector | ✅ | Сбор данных |
| waters-scout | ✅ | Разведка |
| waters-analyst | ✅ | Анализ |
| waters-synthesizer | ✅ | Синтез |
| waters-coordinator | ✅ | Оркестрация |
| waters-archivist | ✅ | Память |
| waters-specialist | ✅ | Кастом |

## 3. WATERS профессии (новые)

| Агент | Статус | Примечание |
|-------|--------|-----------|
| waters-camera-operator | ✅ | Камеры, фото, видео, NDI, OBS |
| waters-video-editor | ✅ | Монтаж, цветокоррекция |
| waters-lab-operator | ✅ | Спектрометры, микроскопы |

## 4. SubAgent Runtime

| Фича | TUI | WATERS | Статус |
|------|-----|--------|--------|
| agent_open | ✅ | ✅ | Через Redis Hash + Stream |
| agent_eval | ✅ | ✅ | Чтение из Redis |
| agent_close | ✅ | ✅ | Закрытие, journal |
| agent_assign | ✅ | ⬜ | Смена задачи mid-flight |
| agent_send_input | ✅ | ⬜ | Отправка сообщения агенту |
| agent_merge | ❌ | ✅ | Наша уникальная фича |
| learn_from | ❌ | ⬜ | Наследование навыков |
| SubAgentResult | ✅ | ✅ | Finding JSON |
| SubAgent mailbox | ✅ | ⬜ | События между агентами |
| Concurrency cap (max 10) | ✅ | ⬜ | Защита от перегрузки |
| Background runtime | ✅ | ⬜ | Агент живёт после отмены |
| Cancellation cascade | ✅ | ⬜ | Родитель→дети |
| Session boundaries | ✅ | 🔄 | session_boot_id не реализован |
| Fork context | ✅ | ❌ | DeepSeek-specific |
| RLM (1M контекст) | ✅ | ❌ | Тяжело, не для профессий |
| Side-git snapshots | ✅ | ❌ | Не IDE |

## 5. Система памяти

| Компонент | Статус | Примечание |
|-----------|--------|-----------|
| Redis multi-DB (0-15) | ✅ | PUBLISH, XADD, HSET, Streams |
| Redis DB 0 — system | ✅ | node state, rating, security |
| Redis DB 1-6 — группы | ✅ | findings, journal, stream |
| Redis DB 15 — LLM cache | ✅ | Кэш ответов LLM |
| ChromaDB | 🔄 | Через memory bridge (есть) |
| LightRAG | 🔄 | Через memory bridge (есть) |
| ✦ Tamagotchi memory | ⬜ | user personality, name, pets, children, desires |

## 6. Интерфейсы

| Интерфейс | Статус | Примечание |
|-----------|--------|-----------|
| Web dashboard (SSE) | ✅ | Redis PUBLISH → браузер |
| HTTP API | ✅ | status, peers, chat, store |
| MCP Server (порт HTTP+100) | ✅ | agents as tools |
| Telegram bot | ✅✅ | ChatBridge есть, команды ⬜ |
| Terminal TUI (ratatui) | ⬜ | ~500 строк, cross-platform |
| Видеомикшер (NDI/OBS) | ✅ | MediaBridge |
| ✦ Голос (STT/TTS) | 🔄 | VoiceBridge код есть, не подключён |
| ✦ Файлообмен в группе | ⬜ | HTTP file server |

## 7. Безопасность (YASA)

| Компонент | Статус | Примечание |
|-----------|--------|-----------|
| Security screening (досмотр) | ✅ | bridges, tools, prompt scan |
| Rating system | ✅ | score, votes, rank |
| Top agents | ✅ | Сортировка по рейтингу |
| Agent import control | ✅ | Проверка источника |

## 8. Медиа-производство

| Компонент | Статус | Примечание |
|-----------|--------|-----------|
| NDI output | ✅ | Redis → NDI |
| OBS WebSocket | ✅ | switch scene, stream control |
| RTMP streaming | ✅ | YouTube, Twitch |
| HDMI display | ✅ | Redis → SDL2/framebuffer |
| ✦ Studio feedback (кадры → LLM) | ⬜ | Захват, анализ, ответ |
| ✦ Audio playback | ⬜ | SDL2/ALSA |

## 9. Tamagotchi (Капелька)

| Компонент | Статус | Примечание |
|-----------|--------|-----------|
| Personality prompt | ✅ | "живой собеседник, друг" |
| 3 языка (ru/en/zh) | ✅ | Через AssistantLang |
| Имя пользователя в Redis | ⬜ | HSET node:personality |
| Характер, питомцы, дети | ⬜ | Redis Hash |
| Проактивные триггеры | ⬜ | Утро, погода, reminders |
| Анимированная капелька | ⬜ | Web + Telegram |
| Эмоции (радость/грусть) | ⬜ | По событиям группы |

## 10. Рефакторинг (выполнено)

| Задача | Статус |
|--------|--------|
| Удалён дубль KvStore в main.rs | ✅ |
| Удалён dead code (by_category, by_role) | ✅ |
| Добавлен Default для AgentRating | ✅ |
| Исправлена сборка с токио-паникой | ✅ |
| Исправлены импорты bridge.rs | ✅ |

## 11. Рефакторинг (TODO)

| Задача | Приоритет |
|--------|-----------|
| Реализовать SubAgent агents_send_input + agent_assign | P0 |
| Добавить concurrency cap (max 10) | P0 |
| Подключить VoiceBridge (STT/TTS) | P1 |
| Tamagotchi memory (имя, характер) | P1 |
| Telegram команды + уведомления | P1 |
| SubAgent mailbox (события) | P1 |
| Background runtime | P1 |
| XREVRANGE для last_finding | P2 |
| TTL-чистка старых агентов | P2 |
| ratatui TUI (~500 строк) | P2 |
| Файлообмен в группе | P2 |

## Итого

| Категория | ✅ | ⬜ | ❌ |
|-----------|---|---|---|
| Агенты | 20 | 0 | 0 |
| SubAgent Runtime | 4 | 5 | 3 |
| Память | 5 | 1 | 0 |
| Интерфейсы | 5 | 3 | 0 |
| Безопасность | 4 | 0 | 0 |
| Медиа | 4 | 2 | 0 |
| Tamagotchi | 2 | 4 | 0 |
| Рефакторинг | 5 | 8 | 0 |
| **Всего** | **49** | **23** | **3** |
