# Scout RU — Инструкция по использованию

**Агент:** `agent.scout.ru.v1`
**Сервер:** 171.22.180.167 (DMZ)
**Бюджет:** $0.10/день (YandexGPT) + $0.05/день (Yandex.XML)
**Дата:** 2026-05-10

---

## 1. ЗАПУСК

### Установка зависимостей

```bash
cd /home/ubuntu/WATERS/repos/waters-core
pip install -r requirements.txt
```

### Запуск

```bash
# Режим разработки (лог в stdout)
python3 agents/field_agent.py

# Режим systemd
sudo systemctl start field-agent
sudo systemctl status field-agent
sudo journalctl -u field-agent -f
```

---

## 2. КАНАЛЫ СВЯЗИ

### 2.1. Telegram (CEO)

Найти бота в Telegram → `/start` → писать запросы.

Если настроен белый список — бот отвечает только указанным user_id.

Узнать свой user_id: написать боту `/start`, посмотреть лог:
```bash
grep -i "telegram" logs/scout.log
```

### 2.2. Kafka (агенты)

Любой агент Гексады может отправить задачу в `tasks.assigned.v1`.

Формат:

```json
{
  "task_id": "uuid-123",
  "query": "новости ИИ 2026",
  "requester": "integrator",
  "source_engines": ["yacy", "youtube", "duckduckgo"],
  "max_results": 10
}
```

Scout отвечает в `planners.answers.v1` двумя типами сообщений:

**file_ready** — каждый прошедший валидацию файл:

```json
{
  "type": "file_ready",
  "region": "ru",
  "agent": "scout",
  "task_id": "uuid-123",
  "source": "yacy",
  "url": "https://habr.com/...",
  "title": "...",
  "snippet": "...",
  "source_rating": 0.8,
  "verdict": "годно",
  "summary": "Краткая выжимка"
}
```

**task_summary** — сводка по всей задаче:

```json
{
  "type": "task_summary",
  "region": "ru",
  "agent": "scout",
  "task_id": "uuid-123",
  "total_files": 45,
  "valid_files": 12
}
```

### 2.3. Heartbeat (мониторинг)

Каждые 5 минут Scout шлёт heartbeat в `events.system.v1`:

```json
{
  "type": "heartbeat",
  "region": "ru",
  "agent": "agent.scout.v1",
  "status": "alive",
  "stats": {
    "total_files": 145,
    "delivered": 98,
    "valid": 82,
    "pending_tasks": 2
  }
}
```

---

## 3. ФОРМАТЫ ЗАПРОСОВ

### 3.1. `search: текст 3-5 строк» — с разбором

Scout отправляет текст в YandexGPT, который генерирует 3 поисковых запроса.

```
search: найди новости про искусственный интеллект в образовании 2026
→ YandexGPT: ["ИИ образование 2026", "нейросети школы Россия", "AI in education Russia"]
→ Поиск по всем 3 запросам через 4 движка
```

**Стоимость:** $0.00033/задача (из бюджета $0.10/день ≈ 300 задач)

### 3.2. `search: !ключевые слова» — без разбора

Знак `!` сразу после `search:` — YandexGPT НЕ вызывается. Экономит бюджет.

```
search: !новости ИИ 2026
→ Без разбора, ключевые слова сразу в поиск
→ $0
```

Использовать когда запрос и так хорошо сформулирован.

### 3.3. `video: https://...» — транскрипция YouTube

Прямая ссылка на YouTube. Scout скачивает субтитры и обрабатывает как документ.

```
video: https://www.youtube.com/watch?v=abc123def45
→ Загрузка транскрипции (ru/en)
→ Полная валидация через NotebookLM / YandexGPT
→ file_ready с типом "video_transcript"
```

**Внимание:** Видео из поиска Scout НЕ транскрибирует (только ссылка). Транскрипция — только по прямой ссылке.

---

## 4. ДИАГНОСТИКА

### 4.1. Логи

```bash
# Основной лог
tail -f /home/ubuntu/WATERS/repos/waters-core/logs/scout.log

# Ротация: 10MB × 5 файлов
ls -la /home/ubuntu/WATERS/repos/waters-core/logs/
```

### 4.2. SQLite

```bash
# Подключиться к базе
sqlite3 /home/waters-data/scout_state.db

# Сколько задач сегодня?
SELECT status, COUNT(*) FROM tasks GROUP BY status;

# Статус файлов
SELECT source, notebooklm_verdict, COUNT(*) FROM files GROUP BY source, notebooklm_verdict;

# Бюджет сегодня
SELECT * FROM daily_usage WHERE usage_date = date('now');

# Рейтинг источников
SELECT * FROM source_ratings ORDER BY rating DESC LIMIT 20;
```

### 4.3. Healthcheck

Scout проверяет при старте:

| Проверка | Что делает |
|----------|-----------|
| **YaCy** | Ping публичного пира |
| **SQLite** | Доступность БД |
| **Диск** | > 500MB свободно |
| **Kafka** | Producer test |

Если критичная проверка не прошла — Scout не запускается.

---

## 5. БЮДЖЕТЫ

### Текущие

| Сервис | Дневной лимит | Расход сегодня |
|--------|--------------|----------------|
| **YandexGPT** | $0.10 (или дневной лимит) | `SELECT * FROM daily_usage WHERE service='yandexgpt'` |
| **Yandex.XML** | $0.05 | `SELECT * FROM daily_usage WHERE service='yandex_xml'` |
| **NotebookLM** | 50 запросов | `SELECT * FROM daily_usage WHERE service='notebooklm'` |
| **DuckDuckGo** | ~200 запросов | `SELECT * FROM daily_usage WHERE service='ddg'` |

### Изменение бюджета

```bash
# изменить значение в файле бюджета YandexGPT
systemctl restart field-agent
```

### Когда всё кончилось

Scout продолжает работать на бесплатных движках (YaCy + DDG + YouTube) с pass-through валидацией. Задачи не теряются, но качество падает.

---

## 6. БЕЗОПАСНОСТЬ

| Правило | Описание |
|---------|----------|
| **Секреты** | Через переменные окружения или файлы. Ни одного в коде |
| **Telegram** | Белый список. Неизвестные пользователи получают «Доступ запрещён» |
| **Файлы** | Автоудаление через 7 дней. Диск не переполняется |
| **Rate limit** | В 2 раза ниже официальных. 100% без бана |
| **Падение** | Systemd Restart=always. Задачи не теряются (retry) |
| **Лимиты** | Бюджеты в SQLite. Не превышаются |

---

## 7. АРХИТЕКТУРА (КРАТКО)

```
167 (DMZ-RU)
  ├── Telegram (CEO) — приоритет 10
  ├── Kafka (агенты) — приоритет 8-3
  │
  ├── YandexGPT — понимание запроса + валидация ($0.10/день)
  ├── YaCy — безлимитный P2P-поиск
  ├── Yandex.XML — RU-поиск ($0.05/день)
  ├── DuckDuckGo — добивка ссылками
  ├── YouTube — поиск ссылок / транскрипция по ссылке CEO
  │
  ├── NotebookLM — валидация сниппетов (50/день, $0)
  ├── Source Rating — рейтинг доменов (SQLite)
  │
  └── file_ready → Kafka → 238 (Integrator DM)

238 (HUB)
  ├── SCP 167→238 → agents/{agent}/data/
  ├── Ollama — глубокая аналитика (♾️)
  ├── ChromaDB / LightRAG — хранение
  └── rating_update → Kafka → 167 (обратная связь)
```

---

## 8. ЧАСТЫЕ ПРОБЛЕМЫ

| Проблема | Решение |
|----------|---------|
| **«Scout не отвечает в Telegram»** | Проверь токен Telegram-бота. Проверь лог: `tail -f logs/scout.log` |
| **«Нет результатов поиска»** | Проверь лимиты: `sqlite3 /home/waters-data/scout_state.db "SELECT * FROM daily_usage"` |
| **«Ошибка Kafka»** | Проверь `WATERS_KAFKA_BROKER`. Проверь что 238 доступен: `ping 171.22.180.238` |
| **«YaCy не отвечает»** | Может быть недоступен публичный пир. Scout переключится на DDG |
| **«NotebookLM ошибка»** | Проверь Google-аккаунт. Scout переключится на YandexGPT |
| **«Бюджет исчерпан»** | Подожди до завтра (сброс в 00:00 UTC) или увеличь бюджет |
| **«Scout упал»** | Systemd перезапустит за 10 секунд. Задачи не теряются |

---

*Документ создан: Конструктор Сети v1.0, 2026-05-10*
*Обновляется по мере изменений*
