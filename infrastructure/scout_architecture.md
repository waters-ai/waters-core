# Scout Field Agent v2.0 — Архитектура

**Дата:** 2026-05-10
**Автор:** Конструктор Сети (agent.constructor.v1)
**Статус:** Проектирование
**Бюджет:** макс $0.25/день на все платные API

---

## 1. ЦЕЛЬ

Scout — автономный разведчик информации на сервере 167 (DMZ). 
Принимает задачу от CEO (Telegram) или Integrator (Kafka), собирает данные из интернета, фильтрует по источникам, валидирует сниппеты, отдаёт результаты на 238.
Работает 24/7 с минимальными затратами.

---

## 2. ПОЛНЫЙ ПОТОК

```
                   167 ─── DMZ (Scout)
                         │
┌────────────────────────┼──────────────────────────┐
│ Telegram (CEO)         │ Kafka (Integrator)       │
│ "search: 3-5 строк"   │ tasks.assigned.v1        │
└────────┬───────────────┴──────────┬───────────────┘
         │                          │
         ▼                          ▼
┌──────────────────────────────────────────────────────────────┐
│ 0. YandexGPT Lite — ПОНЯТЬ ЗАПРОС ($0.10/день)               │
│    input:  "найди новости про ИИ в образовании 2026"         │
│    output: ["ИИ образование 2026", "нейросети школы Россия", │
│             "AI in education Russia"]                        │
│    cost: ~200 токенов = $0.00033/задача                       │
│    $0.10 = ~300 задач/день                                   │
├──────────────────────────────────────────────────────────────┤
│ 1. ПОИСК — ТАЩИТЬ СНИППЕТЫ (♾️, $0)                          │
│                                                              │
│    Приоритет: YaCy → YouTube → Yandex.XML → Mojeek/Brave →  │
│               DuckDuckGo                                     │
│                                                              │
│    Каждый поисковик отдаёт: {title, url, snippet (3-5 строк)} │
│                                                              │
│    Осбые случаи:                                              │
│    ┌─────────────────────────────────────────────────────┐   │
│    │ Видео из поиска: {title, url, channel}              │   │
│    │   → НЕ транскрибируем (только ссылка)              │   │
│    │                                                     │   │
│    │ Видео по прямой ссылке CEO:                         │   │
│    │   Telegram "video: https://..."                     │   │
│    │   → загрузить ТРАНСКРИПЦИЮ целиком                 │   │
│    │   → сохранить как обычный документ                 │   │
│    └─────────────────────────────────────────────────────┘   │
├──────────────────────────────────────────────────────────────┤
│ 2. ФИЛЬТР ИСТОЧНИКОВ (♾️, $0)                                │
│                                                              │
│    Domain Authority: .edu/.gov/arxiv → high                  │
│                      habr/github/medium → medium             │
│                      остальное → low                         │
│                                                              │
│    Source Rating:  > 0.80 → доверенный, сразу file_ready    │
│                    0.30-0.80 → на проверку                   │
│                    < 0.30 → мусор, удалить                   │
│                                                              │
│    DeDuplicate: checksum(title+url+snippet)                  │
├──────────────────────────────────────────────────────────────┤
│ 3. ВАЛИДАЦИЯ СНИППЕТОВ                                       │
│                                                              │
│    ▸ NotebookLM (50/день, $0) [приоритет 1]                  │
│      "По этому сниппету: это годно/мусор?"                   │
│                                                              │
│    ▸ YandexGPT Lite (остаток $0.10, $0.00016/снипп)         │
│      [приоритет 2 — только если NotebookLM кончился]         │
│      ~600 сниппетов/день за $0.10                            │
│                                                              │
│    ▸ Pass-through [приоритет 3 — если оба исчерпаны]        │
│      "годно" без проверки                                    │
│                                                              │
│    Результат: годно → file_ready  |  мусор → удалить        │
├──────────────────────────────────────────────────────────────┤
│ 4. ОТДАЧА                                                    │
│                                                              │
│    file_ready → planners.answers.v1 (Kafka)                  │
│    task_summary → planners.answers.v1 (Kafka)                │
│    report_bad → planners.answers.v1 (для 238)                │
│    файлы: /home/waters-data/raw/scout/{date}/                │
└──────────────────────────────────────────────────────────────┘
         │ file_ready
         ▼
┌──────────────────────────────────────────────────────────────┐
│                   238 ─── НЕРВНАЯ СИСТЕМА                     │
│                                                              │
│ Integrator Data Manager:                                     │
│   SCP 167→238 → agents/{agent}/data/{date}/                  │
│   → delivery_ack → Kafka                                     │
│                                                              │
│ Ollama qwen2.5:7b (♾️, $0, наше железо):                     │
│   - Deep content validation                                  │
│   - Entity extraction                                        │
│   - Cross-source verification                                │
│   - Task-level synthesis                                     │
│   - Rating update → Kafka → 167                              │
│                                                              │
│ ChromaDB → векторизация                                      │
│ LightRAG → граф знаний                                       │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. ПОИСКОВЫЕ ДВИЖКИ — ПОЛНАЯ ТАБЛИЦА

| № | Движок | Лимит | Бюджет | Что даёт | Статус |
|---|--------|-------|--------|---------|--------|
| 1 | **YaCy** | ♾️ безлимит | $0 | P2P-децентрализ. индекс, JSON API, свой или публичный пир | ⏳ Нужен класс |
| 2 | **YouTube** | 50 транскрипций/день | $0 | title + description из поиска; полный текст по ссылке | ✅ Работает |
| 3 | **Yandex.XML** | 10 000 запросов/день | $0.05/день | title + headline + snippet из RU-сегмента | ⏳ Нужен ключ |
| 4 | **DuckDuckGo** | ~200 запросов/день | $0 | title + body, добивка ссылками | ✅ Работает |
| 5 | **Mojeek** | 200 запросов/день | $0.05/день | Собственный индекс, независимый | ❌ Не подключён |
| 6 | **Brave Search** | $5/мес кредита | $0.05/день | Собственный индекс, 30B+ страниц | ❌ Не подключён |

### Альтернативные варианты в рамках $0.25/день

| Комбинация | Стоимость | Покрытие | Риски |
|-----------|-----------|---------|-------|
| **YaCy + DDG + YouTube** | $0 | 60-70% интернета | DDG может банить, YaCy медленный |
| **+ Yandex.XML** | $0.05/день | +RU-сегмент | Нужен API-ключ |
| **+ YandexGPT** | $0.10/день | +AI-улучшение запроса | Лимит ~300 задач/день |
| **+ Brave Search** | $0.05/день | +30B страниц, качественный индекс | Нужна регистрация |
| **+ Mojeek** | $0.05/день | +независимый индекс | Нужна регистрация |
| **ИТОГО макс** | **$0.25/день** | **4-5 независимых индексов** | — |

---

## 4. ВИДЕО-ТРАНСКРИПТЫ — ДЕТАЛЬНО

### 4.1. Откуда берутся видео

```
Источник 1: Поиск (любой движок нашёл YouTube-ссылку)
  → { title, url, channel, description }
  → НЕ транскрибируем
  → file_ready { type: "video_link", ... }
  → Integrator на 238 решает: нужно?

Источник 2: CEO дал прямую ссылку
  Telegram: "video: https://youtube.com/watch?v=XXX"
  → загружаем ТРАНСКРИПЦИЮ (youtube-transcript-api)
  → сохраняем как /raw/scout/{date}/{task_id}_video.json
  → валидируем через NotebookLM / YandexGPT
  → file_ready { type: "video_transcript", content: "..." }

Источник 3: YouTube-поиск
  → Scout сам ищет на YouTube по ключевым словам
  → получает список video_id
  → для каждого: { title, url, channel }
  → НЕ транскрибируем
  → file_ready { type: "video_search_result", results: [...] }
```

### 4.2. Формат file_ready для видео

```json
{
  "type": "file_ready",
  "subtype": "video_link",
  "agent": "scout",
  "task_id": "uuid-xxx",
  "query": "AI education",
  "source": "youtube_search",
  "title": "AI in Education: 2026 Trends",
  "url": "https://youtube.com/watch?v=abc123",
  "channel": "Tech Education",
  "transcript_loaded": false
}
```

```json
{
  "type": "file_ready",
  "subtype": "video_transcript",
  "agent": "scout",
  "task_id": "uuid-xxx",
  "source": "telegram_direct_link",
  "title": "...",
  "url": "https://youtube.com/watch?v=abc123",
  "content": "полный текст транскрипции...",
  "transcript_loaded": true,
  "duration_seconds": 720
}
```

### 4.3. Почему не транскрибируем всё

| Фактор | Значение |
|--------|---------|
| Среднее время транскрипции | 3-5 секунд на 1 минуту видео |
| 10 видео из поиска | ~30-50 секунд ожидания |
| Лимит YouTube | ~50 запросов/день (можно заблокировать) |
| **Решение** | Транскрибируем **только** по прямой ссылке CEO. Поиск → ссылки |

---

## 5. ХРАНИЛИЩЕ

### 5.1. Файловая система (167)

```
/home/waters-data/
├── raw/scout/{date}/
│   ├── {task_id}_000_ddg.json
│   ├── {task_id}_001_yacy.json
│   ├── {task_id}_002_youtube.json
│   ├── {task_id}_003_yandex.json
│   └── {task_id}_video.json
├── logs/
│   └── field_agent.log
└── scout_state.db
```

### 5.2. SQLite (scout_state.db)

```sql
-- Таблица: задачи
CREATE TABLE tasks (
    task_id TEXT PRIMARY KEY,
    query TEXT NOT NULL,
    requester TEXT,
    source_engines TEXT,
    status TEXT DEFAULT 'pending',  -- pending | processing | done | failed
    created_at TIMESTAMP,
    updated_at TIMESTAMP,
    completed_at TIMESTAMP,
    error TEXT
);

-- Таблица: файлы (результаты поиска + сниппеты)
CREATE TABLE files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT REFERENCES tasks(task_id),
    path TEXT NOT NULL,
    checksum_sha256 TEXT UNIQUE,
    source TEXT,
    url TEXT,
    title TEXT,
    domain_authority TEXT DEFAULT 'unknown',
    notebooklm_verdict TEXT,
    notebooklm_summary TEXT,
    delivered INTEGER DEFAULT 0,
    delivery_ack_at TIMESTAMP,
    created_at TIMESTAMP
);

-- Таблица: рейтинг источников (обучается от 238)
CREATE TABLE source_ratings (
    domain TEXT PRIMARY KEY,
    authority TEXT DEFAULT 'unknown',
    rating REAL DEFAULT 0.5,     -- 0.0 - 1.0
    total_files INTEGER DEFAULT 0,
    passed_files INTEGER DEFAULT 0,
    last_checked TIMESTAMP
);

-- Таблица: дневное потребление бюджета
CREATE TABLE daily_usage (
    usage_date TEXT,
    service TEXT,
    metric TEXT,
    value REAL DEFAULT 0,
    PRIMARY KEY (usage_date, service, metric)
);
```

---

## 6. ЛИМИТЫ И БЮДЖЕТЫ

### 6.1. Бесплатные (♾️)

| Сервис | Лимит | Механизм |
|--------|-------|----------|
| **YaCy** | ♾️ безлимит | P2P сеть, RateLimiter 2 req/s |
| **DuckDuckGo** | ~200/день | TokenBucket 0.5 req/s |
| **YouTube** | 50 транскрипций/день | TokenBucket 0.2 req/s |
| **NotebookLM** | 50 запросов/день | SQLite счётчик |
| **Domain Authority** | ♾️ | Регулярка, $0 |
| **Dedup + Rating** | ♾️ | SQLite, $0 |

### 6.2. Платные

| Сервис | Дневной бюджет | $0.25 pool | Цена за единицу |
|--------|---------------|------------|-----------------|
| **YandexGPT Lite** | $0.10 | 40% | $0.00033/запрос ~300 задач/день |
| **Yandex.XML** | $0.05 | 20% | $4/1000 запросов ~12 запросов/день |
| **Brave Search** | $0.05 | 20% | $5/1000 запросов ~10 запросов/день |
| **Mojeek** | $0.05 | 20% | $???/1000 |

### 6.3. Когда всё кончилось

```
Если все бюджеты исчерпаны:
  → Scout работает только на YaCy + DDG + YouTube (бесплатно)
  → валидация: pass-through (годно без проверки)
  → file_ready с пометкой:
    "validation: skipped — budgets exhausted"

Если даже бесплатные не дали результатов:
  → task_deferred:
    { "type": "task_deferred",
      "reason": "all search engines returned empty or all budgets exhausted",
      "retry": "next_day" }
```

---

## 7. КОНТРОЛЬ И САМОСОХРАНЕНИЕ

### 7.1. Heartbeat (каждые 5 мин)

```json
{
  "type": "heartbeat",
  "agent": "agent.scout.v1",
  "status": "alive",
  "timestamp": "2026-05-10T12:00:00Z",
  "stats": {
    "total_files_today": 45,
    "delivered_today": 32,
    "notebooklm_used": 28,
    "yandexgpt_cost": 0.0023,
    "pending_tasks": 2,
    "disk_free_gb": 12.5,
    "uptime_hours": 48
  }
}
```

### 7.2. Healthcheck (при старте)

| Проверка | Что делаем | Критичность |
|----------|-----------|-------------|
| **YaCy** | ping публичного пира | warn — работает без него |
| **DDG** | test search "test" | warn — работает без него |
| **YouTube** | ping youtube.com | warn — работает без него |
| **Kafka producer** | send test message | **critical** — не работает без Kafka |
| **Kafka consumer** | subscribe to topics | **critical** |
| **Disk** | free space > 500MB | **critical** |
| **SQLite** | read/write test | **critical** |
| **NotebookLM** | init client | warn |

### 7.3. Error Reporting

```python
def report_error(self, severity: str, message: str, details: dict = None):
    # Если 3+ поисковика отказали → critical alert
    # Если 1-2 отказали → warning
    # Пишет в events.system.v1, дублирует в лог
```

### 7.4. Retry на перезапуск

```python
def on_startup(self):
    # Все задачи в статусе "processing" → "pending"
    # Scout начинает их заново
    # Не теряем задачи при падении
```

---

## 8. РЕЙТИНГ ИСТОЧНИКОВ (ОБРАТНАЯ СВЯЗЬ 238→167)

### 8.1. Поток

```
238 (Ollama) → content validation → verdict
       ↓
Integrator → rating_update
  Kafka: ratins.update.v1
  {
    "type": "rating_update",
    "domain": "habr.com",
    "delta": +0.05,
    "reason": "content_passed_ollama",
    "confidence": 0.9
  }
       ↓
167 (Scout) → SQLite: source_ratings
  UPDATE SET rating = rating + 0.05, total_files++, passed_files++
       ↓
В следующий раз: habr.com = 0.92 > 0.80 → сразу file_ready
```

### 8.2. Шкала рейтингов

| Рейтинг | Решение Scout |
|---------|--------------|
| **> 0.80** | Доверенный. Сразу `file_ready`, без валидации сниппета |
| **0.50 - 0.80** | Нейтральный. Полная валидация (NotebookLM / YandexGPT) |
| **0.30 - 0.50** | Сомнительный. Валидация + пометка `authority: low` |
| **< 0.30** | Мусорный. Удалить, не отправлять на 238 |

---

## 9. ОГРАНИЧЕНИЯ

### 9.1. Scout НЕ делает (на 167)

- Не понимает сложный контекст запроса (использует YandexGPT)
- Не анализирует полный текст документа (отдаёт на 238)
- Не принимает этических решений
- Не транскрибирует видео из поиска (только по ссылке CEO)
- Не качает PDF/тяжёлые документы (238 решает)

### 9.2. Scout ДЕЛАЕТ (на 167)

- Тащит массу сниппетов из 5+ поисковиков
- Фильтрует источники (domain + рейтинг + dedup)
- Валидирует сниппеты (NotebookLM → YandexGPT → pass)
- Отдаёт структурированный результат на 238
- Учится через обратную связь (rating_update)
- Работает в рамках бюджета $0.25/день
- Чистит файлы > 7 дней
- Шлёт heartbeat, healthcheck, error-reporting

---

## 10. СТРУКТУРА ФАЙЛОВ ПРОЕКТА

```
agents/
  ├── field_agent.py               — главный Scout
  ├── integrator_data_manager.py   — SCP-перегонщик
  ├── scout_state.py               — SQLite слой + бюджеты
  ├── scout_ratelimit.py           — Token Bucket
  └── scout_cleanup.py             — очистка > 7 дней

skills/
  ├── scout-self.skill.md
  ├── scout-telegram.skill.md
  ├── scout-duckduckgo.skill.md
  ├── scout-youtube.skill.md
  ├── scout-notebooklm.skill.md
  ├── scout-yandex.skill.md
  └── scout-yacy.skill.md          (НОВЫЙ)

infrastructure/
  ├── docker/topology.json
  ├── systemd/field-agent.service
  ├── systemd/integrator-dm.service
  └── scout_architecture.md        (этот файл)

~/.secrets/
  .secret_telegram_token
  .secret_telegram_users           (НОВЫЙ — белый список)
  .secret_yandexgpt_api_key

scripts/
  └── setup_data_storage.sh
```

---

## 11. ВОПРОСЫ НА УТВЕРЖДЕНИЕ

- [ ] 1. Kapelka-агенты — Scout отдельный субагент, не часть Kapelka?
- [ ] 2. Порядок поисковиков YaCy → YouTube → Yandex → Mojeek/Brave → DDG?
- [ ] 3. Yandex.XML — ты сам регистрируешься или пропускаем?
- [ ] 4. Brave + Mojeek — добавляем на 1 этапе или позже?
- [ ] 5. $0.25/день — нормально, или уменьшить до $0.15?
- [ ] 6. Telegram `video:` команда — нужна?

---

*Документ создан: Конструктор Сети v1.0, 2026-05-10*
*Статус: на утверждение*
