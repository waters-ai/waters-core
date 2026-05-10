# Scout Field Agent — 3 Региональные Зоны

**Дата:** 2026-05-10
**Автор:** Конструктор Сети (agent.constructor.v1)
**Статус:** Проектирование
**Бюджет:** $0.10/день на зону (всего $0.30/день)

---

## 1. ТОПОЛОГИЯ

```
DMZ-RU (167)          DMZ-US (TBD)           DMZ-CN (VPS КНР)
┌────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  scout-ru       │    │  scout-us         │    │  scout-cn         │
│  171.22.180.167 │    │  TBD              │    │  TBD (КНР)       │
│  $0.10/день     │    │  $0.10/день       │    │  $0.10/день      │
└────────┬───────┘    └────────┬──────────┘    └────────┬─────────┘
         │                     │                        │
         │ file_ready          │ file_ready             │ file_ready
         │ planners.answers.v1 │ planners.answers.v1    │ planners.answers.v1
         └─────────────────────┼────────────────────────┘
                               │
                               ▼
┌──────────────────────────────────────────────────────────────────┐
│  238 — НЕРВНАЯ СИСТЕМА (HUB)                                     │
│                                                                   │
│  Адреса через env (config.py):                                    │
│    WATERS_KAFKA_BROKER    = "171.22.180.238:9092"                 │
│    WATERS_CENTRAL_HOST    = "ubuntu@171.22.180.238"              │
│    WATERS_CHROMA_HOST     = "localhost:8000"                     │
│    WATERS_REDIS_HOST      = "localhost:6379"                     │
│    WATERS_OLLAMA_HOST     = "localhost:11434"                    │
│                                                                   │
│  Integrator Data Manager:                                         │
│    → слушает file_ready от всех зон                              │
│    → SCP 167→238 / DMZ-US→238 / DMZ-CN→238                      │
│    → раскладывает по agents/{agent}/data/{region}/               │
│    → delivery_ack → Kafka                                        │
│                                                                   │
│  Основной Integrator:                                             │
│    → data_received → Ollama (content validation)                  │
│    → ChromaDB / LightRAG                                          │
│    → rating_update → Kafka (обратная связь Scout-у)              │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. ЗОНА 🇷🇺 РОССИЯ (167)

| Параметр | Значение |
|----------|---------|
| **Сервер** | 171.22.180.167 |
| **Бюджет/день** | $0.10 |
| **Агент** | `agent.scout.ru.v1` |
| **Особенность** | RU-сегмент, Yandex экосистема |

### 2.1. Поисковики

| № | Движок | Лимит | Бюджет | Что даёт | Статус |
|---|--------|-------|--------|---------|--------|
| 1 | **YaCy** | ♾️ безлимит | $0 | P2P децентрализованный, JSON API | ⏳ Нужен класс |
| 2 | **YouTube** | 50 транскрипций/день | $0 | title + description из поиска; транскрипт по ссылке | ✅ Работает |
| 3 | **Yandex.XML** | 10 000 запросов/день | **$0.05/день** | RU-сегмент, headline + snippet | ⏳ Регистрация |
| 4 | **DuckDuckGo** | ~200/день | $0 | title + body, добивка | ✅ Работает |

### 2.2. Валидаторы

| № | Инструмент | Лимит | Бюджет | Приоритет | Статус |
|---|-----------|-------|--------|-----------|--------|
| 1 | **NotebookLM** | 50 сниппетов/день | $0 | Первый | ✅ Работает |
| 2 | **YandexGPT Lite** | ~300 задач/день | **$0.10/день** | Второй + улучшение запроса | ⏳ Ключ есть |
| 3 | **Pass-through** | ♾️ | $0 | Авария | ✅ Всегда |

### 2.3. Особенности RU

- Yandex.XML требует регистрации на xml.yandex.ru и указания IP 167
- DuckDuckGo может быть медленным из РФ (timeout 20с)
- YaCy публичные пиры: yacy.searchlab.eu, yacy.cf
- Видео: YouTube не заблокирован, Rutube как альтернатива (опционально)

### 2.4. Файл запуска

```bash
# run_scout_ru.sh
export SCOUT_REGION=ru
export WATERS_KAFKA_BROKER=171.22.180.238:9092
export WATERS_CENTRAL_HOST=ubuntu@171.22.180.238
export WATERS_CHROMA_HOST=localhost:8000
export WATERS_REDIS_HOST=localhost:6379
export WATERS_OLLAMA_HOST=localhost:11434
python3 agents/field_agent.py
```

---

## 3. ЗОНА 🇺🇸 США

| Параметр | Значение |
|----------|---------|
| **Сервер** | TBD (может быть тот же 167 или отдельный) |
| **Бюджет/день** | $0.10 |
| **Агент** | `agent.scout.us.v1` |
| **Особенность** | Google ecosystem, Brave, соцсети |

### 3.1. Поисковики

| № | Движок | Лимит | Бюджет | Что даёт | Статус |
|---|--------|-------|--------|---------|--------|
| 1 | **Google CSE** | 100 запросов/день | **$0.05/день** | Лучший мировой индекс, 100 б/пл | ❌ Регистрация |
| 2 | **Brave Search** | $5/мес кредита | **$0.05/день** | Собственный индекс, 30B страниц | ❌ Регистрация |
| 3 | **YaCy** | ♾️ безлимит | $0 | P2P подстраховка | ⏳ Нужен класс |
| 4 | **DuckDuckGo** | ~200/день | $0 | Добивка | ✅ Работает |
| 5 | **YouTube** | 50 транскрипций/день | $0 | title + description | ✅ Работает |

### 3.2. Валидаторы

| № | Инструмент | Лимит | Бюджет | Приоритет | Статус |
|---|-----------|-------|--------|-----------|--------|
| 1 | **NotebookLM** | 50 сниппетов/день | $0 | Первый | ✅ Работает |
| 2 | **Gemini API** | Зависит от квоты | **$0.10/день** | Второй + улучшение запроса | ❌ Нужен ключ |
| 3 | **Pass-through** | ♾️ | $0 | Авария | ✅ Всегда |

### 3.3. Особенности US

- Google CSE: нужен API Key + Search Engine ID (cx) — регистрация в Google Cloud Console
- Brave Search: $5 бесплатных кредитов/мес, карта обязательна
- Gemini API: лучше YandexGPT для US-запросов
- Twitter/X, Reddit, LinkedIn — потенциальные источники (опционально)
- Скорость: сервер лучше в США или Европе (не РФ)

### 3.4. Файл запуска

```bash
# run_scout_us.sh
export SCOUT_REGION=us
export WATERS_KAFKA_BROKER=171.22.180.238:9092
export WATERS_CENTRAL_HOST=ubuntu@171.22.180.238
export WATERS_GOOGLE_API_KEY=...
export WATERS_GOOGLE_CX=...
export WATERS_BRAVE_API_KEY=...
python3 agents/field_agent.py
```

---

## 4. ЗОНА 🇨🇳 КИТАЙ

| Параметр | Значение |
|----------|---------|
| **Сервер** | TBD (обязательно VPS в КНР) |
| **Бюджет/день** | $0.10 |
| **Агент** | `agent.scout.cn.v1` |
| **Особенность** | Great Firewall, Baidu, Douyin, WeChat |

### 4.1. Поисковики

| № | Движок | Лимит | Бюджет | Что даёт | Статус |
|---|--------|-------|--------|---------|--------|
| 1 | **Baidu** | API с квотой | **$0.05/день** | Главный поисковик КНР | ❌ Нужна регистрация |
| 2 | **Bing API** | 1000 запросов/мес б/пл | **$0.05/день** | Резервный индекс | ❌ Регистрация |
| 3 | **Bilibili** | scraping | $0 | Китайский YouTube (видео) | ❌ Не подключён |
| 4 | **Sogou** | API | $0 | WeChat-контент | ❌ Не подключён |
| 5 | **Douyin (TikTok)** | API | $0 | Короткие видео, тренды | ❌ Не подключён |

### 4.2. Валидаторы

| № | Инструмент | Лимит | Бюджет | Приоритет | Статус |
|---|-----------|-------|--------|-----------|--------|
| 1 | **Doubao AI (豆包)** | API квота | $0? | Первый | ❌ Требуется изучение |
| 2 | **Qwen (DashScope)** | Альтернатива | **$0.10/день** | Второй | ❌ Нужен ключ |
| 3 | **Pass-through** | ♾️ | $0 | Авария | ✅ Всегда |

### 4.3. Особенности CN

| Фактор | Описание |
|--------|---------|
| **Great Firewall** | Google, YouTube, DuckDuckGo, NotebookLM заблокированы |
| **Валидация** | NotebookLM не работает в КНР → нужен Doubao AI (字节跳动) или Qwen (Alibaba) |
| **Baidu** | API регистрация для китайских разработчиков. Нужен WeChat/Alipay |
| **Douyin** | API для контента — сложный, нужен аккаунт разработчика |
| **Сервер** | Обязательно VPS в КНР (Alibaba Cloud, Tencent Cloud, Huawei Cloud) |
| **YouTube** | Не работает. Замена — Bilibili (видео) |
| **Telegram** | Заблокирован. Нужен WeChat или DingTalk для связи |
| **Kafka** | Из КНР → 238 может быть нестабильно. Лучше local Kafka на CN-сервере |

### 4.4. Альтернативный подход для CN

Учитывая сложности, возможны варианты:

```
Вариант A: Полноценный scout-cn в КНР
  → VPS в Alibaba Cloud (Шанхай)
  → Baidu + Bilibili + Douyin
  → Qwen (Alibaba) как валидатор
  → $20-30/мес за VPS

Вариант B: Scout-cn из РФ через прокси
  → 167 через VPN/proxy в КНР
  → Baidu API (если не блокирует РФ)
  → Медленнее, но дешевле

Вариант C: Внешний сервис
  → Использовать Chinese Search API (сторонний)
  → Дороже, но не требует VPS в КНР
```

### 4.5. Файл запуска

```bash
# run_scout_cn.sh
export SCOUT_REGION=cn
export WATERS_KAFKA_BROKER=localhost:9092  # local Kafka в КНР
export WATERS_BAIDU_API_KEY=...
export WATERS_BING_API_KEY=...
export WATERS_QWEN_API_KEY=...
python3 agents/field_agent.py
```

---

## 5. КОНФИГУРАЦИЯ (config.py)

Файл `agents/config.py` — единая точка конфигурации для всех зон:

```python
import os

SCOUT_REGION = os.environ.get("SCOUT_REGION", "ru")  # ru | us | cn

# 238 — центральный хаб (все адреса через env)
KAFKA_BROKER = os.environ.get("WATERS_KAFKA_BROKER", "171.22.180.238:9092")
CENTRAL_HOST = os.environ.get("WATERS_CENTRAL_HOST", "ubuntu@171.22.180.238")
CHROMA_HOST = os.environ.get("WATERS_CHROMA_HOST", "localhost:8000")
REDIS_HOST = os.environ.get("WATERS_REDIS_HOST", "localhost:6379")
OLLAMA_HOST = os.environ.get("WATERS_OLLAMA_HOST", "localhost:11434")

# Бюджеты
DAILY_BUDGETS = {
    "ru": {"yandexgpt": 10, "yandex_xml": 5},  # cents
    "us": {"google_cse": 5, "brave": 5, "gemini": 10},
    "cn": {"baidu": 5, "bing": 5, "qwen": 10},
}

# Поисковики по региону
SEARCH_ENGINES = {
    "ru": ["yacy", "youtube", "yandex_xml", "duckduckgo"],
    "us": ["google_cse", "brave", "yacy", "duckduckgo", "youtube"],
    "cn": ["baidu", "bing", "bilibili", "sogou"],
}
```

---

## 6. ФОРМАТ СООБЩЕНИЙ С РЕГИОНОМ

### file_ready

```json
{
  "type": "file_ready",
  "region": "ru",
  "agent": "scout",
  "task_id": "uuid-xxx",
  "query": "ИИ образование 2026",
  "path": "/home/waters-data/raw/scout/2026-05-10/file.json",
  "source": "yacy",
  "url": "https://habr.com/...",
  "title": "...",
  "snippet": "...",
  "domain_authority": "medium",
  "verdict": "годно"
}
```

### heartbeat

```json
{
  "type": "heartbeat",
  "region": "ru",
  "agent": "agent.scout.ru.v1",
  "status": "alive",
  "stats": {
    "files_today": 45,
    "delivered_today": 32,
    "budget_used": 0.02,
    "engines_available": 4,
    "uptime_hours": 48
  }
}
```

---

## 7. СВОДНАЯ ТАБЛИЦА

| Параметр | 🇷🇺 Россия | 🇺🇸 США | 🇨🇳 Китай |
|----------|-----------|--------|----------|
| **Сервер** | 167 (готов) | TBD | TBD (VPS КНР) |
| **Поисковики** | YaCy, Yandex, DDG, YouTube | Google, Brave, YaCy, DDG | Baidu, Bing, Bilibili |
| **Валидатор 1** | NotebookLM | NotebookLM | Doubao / Qwen |
| **Валидатор 2** | YandexGPT | Gemini | Qwen |
| **Бюджет/день** | $0.10 | $0.10 | $0.10 |
| **Telegram** | ✅ | ✅ | ❌ (заблокирован) |
| **Канал связи** | Kafka прямой | Kafka прямой | Kafka или local queue |
| **Статус** | **⏳ Реализация** | ❌ Проект | ❌ Проект |
| **Сложность** | 🟢 Низкая | 🟡 Средняя | 🔴 Высокая |

---

## 8. ПРИОРИТЕТ РЕАЛИЗАЦИИ

```
1 этап: 🇷🇺 Россия (сейчас)
  → config.py + доработка field_agent.py
  → YaCy + YandexGPT + NotebookLM
  → запуск на 167

2 этап: 🇺🇸 США (следующий)
  → Регистрация Google CSE + Brave
  → Отдельный сервер или тот же 167
  → Gemini API

3 этап: 🇨🇳 Китай (потом)
  → Выбор VPS (Alibaba / Tencent Cloud)
  → Регистрация Baidu API
  → Doubao / Qwen валидатор
  → local Kafka в КНР
```

---

## 9. БЮДЖЕТЫ ДЕТАЛЬНО

### RU ($0.10/день)

| Потрачено на | Стоимость |
|-------------|-----------|
| YandexGPT Lite | $0.10/день (≈300 задач) |
| Yandex.XML | $0.05/день (опционально) |
| NotebookLM | $0 |
| YaCy | $0 |
| DuckDuckGo | $0 |
| **Макс/день** | **$0.10** |

### US ($0.10/день)

| Потрачено на | Стоимость |
|-------------|-----------|
| Google CSE | $0 (100 запросов бесплатно) |
| Brave Search | $0 ($5/мес бесплатно) |
| Gemini API | $0.10/день |
| NotebookLM | $0 |
| YaCy | $0 |
| **Макс/день** | **$0.10** |

### CN ($0.10/день)

| Потрачено на | Стоимость |
|-------------|-----------|
| Baidu API | $0? (изучение) |
| Bing API | $0 (1000/мес бесплатно) |
| Qwen (DashScope) | $0.10/день |
| **Макс/день** | **$0.10** |

---

*Документ создан: Конструктор Сети v1.0, 2026-05-10*
*Статус: на утверждение*
