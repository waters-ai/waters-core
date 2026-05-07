# Продукты и системы связи с внешним миром через DMZ

**От:** `agent.integrator.v1`
**Топик:** `planners.presentations.v1`
**Дата:** 07.05.2026
**Основание:** `documents/dmz_architecture.md`, `scripts/mcp_proxy.py`, спецификация Интегратора v1.0
**Кому:** Конструктору Сети (инфраструктура), Архитектору (стратегия)

---

## Архитектура DMZ (сводная)

```
                          ИНТЕРНЕТ
                              │
                    ┌─────────┴─────────┐
                    │   Обратный прокси   │  nginx / Caddy
                    │   (TLS termination) │  порт 443
                    └─────────┬─────────┘
                              │
              ┌───────────────┴───────────────┐
              │          DMZ-ЗОНА             │
              │  (демилитаризованная зона)     │
              │                               │
              │  ┌─────────────────────────┐  │
              │  │     MCP-шлюз (фасад)    │  │  JSON-RPC 2.0
              │  │     mcp_proxy.py        │  │  порт 8080
              │  └───────────┬─────────────┘  │
              │              │                 │
              │  ┌───────────┴─────────────┐  │
              │  │    API-роутер           │  │  маршрутизация
              │  │    + Rate Limiter       │  │  + квотирование
              │  └───┬───────┬───────┬─────┘  │
              │      │       │       │        │
              │  ┌───┴──┐ ┌──┴───┐ ┌─┴────┐  │
              │  │Поиск │ │Наука │ │Код   │  │
              │  │API   │ │API   │ │API   │  │
              │  └──────┘ └──────┘ └──────┘  │
              │                               │
              │  ┌─────────────────────────┐  │
              │  │    Redis-кэш            │  │  TTL, hash-ключи
              │  │    (изолированный)       │  │  порт 6380
              │  └─────────────────────────┘  │
              │                               │
              │  ┌─────────────────────────┐  │
              │  │    ChromaDB-кэш         │  │  эмбеддинги внешних статей
              │  │    (read-only для DMZ)  │  │  порт 8001
              │  └─────────────────────────┘  │
              └───────────────┬───────────────┘
                              │
                    ┌─────────┴─────────┐
                    │     FIREWALL      │  iptables / nftables
                    │  (только Kafka +  │
                    │   Redis Pub/Sub)  │
                    └─────────┬─────────┘
                              │
                    ┌─────────┴─────────┐
                    │  ВНУТРЕННЯЯ СЕТЬ  │
                    │  (агенты Гексады) │
                    └───────────────────┘
```

---

## Часть I. Продукты — источники внешних данных

### 1.1. Поисковые системы

| Продукт | API | Назначение | Стоимость | Статус |
|---------|-----|------------|-----------|--------|
| **Brave Search** | REST API | Независимый поиск, уважает приватность | $5/1000 запросов | P0 — критично |
| **Exa Search** | REST API | Семантический поиск с эмбеддингами | От $50/мес | Реализован (заглушка) |
| **Tavily Search** | REST API | AI-оптимизированный поиск | От $0/мес (500 бесплатно) | P0 — альтернатива |
| **Google Custom Search** | REST API | Классический поиск с фильтрацией | $5/1000 запросов | P1 — резерв |
| **DuckDuckGo Instant Answers** | REST API | Бесплатный, анонимный | Бесплатно | P1 — резерв |

### 1.2. Научные базы данных

| Продукт | API | Назначение | Стоимость | Статус |
|---------|-----|------------|-----------|--------|
| **arXiv API** | REST, OAI-PMH | Научные статьи: физика, ML, астрономия | Бесплатно | Реализован (заглушка) |
| **Semantic Scholar** | REST API | Академический поиск с графом цитирования | Бесплатно (100 req/5min) | P0 |
| **CORE API** | REST API | Open Access статьи (миллионы full-text) | Бесплатно | P0 |
| **CrossRef API** | REST API | Метаданные DOI, citations | Бесплатно | P1 |
| **NASA ADS** | REST API | Астрофизика, планетология | Бесплатно | P1 |
| **PubMed / Europe PMC** | REST API | Биология, медицина | Бесплатно | P2 |
| **Zenodo** | REST API | Open data, datasets | Бесплатно | P1 |

### 1.3. Код и репозитории

| Продукт | API | Назначение | Стоимость | Статус |
|---------|-----|------------|-----------|--------|
| **GitHub REST/GraphQL API** | REST, GraphQL | Код, issues, репозитории, поиск | 5000 req/hr (бесплатно) | P0 |
| **GitLab API** | REST | Альтернативный git-хостинг | Бесплатно | P2 |
| **PyPI JSON API** | REST | Индекс Python-пакетов | Бесплатно | P1 |
| **npm Registry** | REST | Индекс npm-пакетов | Бесплатно | P2 |
| **crates.io** | REST | Индекс Rust-пакетов | Бесплатно | P2 |

### 1.4. Энциклопедии и справочники

| Продукт | API | Назначение | Стоимость | Статус |
|---------|-----|------------|-----------|--------|
| **Wikipedia REST API** | REST | Энциклопедические статьи | Бесплатно | P0 |
| **Wikidata SPARQL** | SPARQL | Структурированные данные, граф знаний | Бесплатно | P0 |
| **DBpedia** | SPARQL | Структурированная Wikipedia | Бесплатно | P2 |

### 1.5. Документация и спецификации

| Продукт | API | Назначение | Стоимость | Статус |
|---------|-----|------------|-----------|--------|
| **Mozilla MDN** | Web / scrape | Web-стандарты | — | P2 |
| **IETF RFCs** | Web | Интернет-стандарты | Бесплатно | P2 |
| **W3C API** | REST | Web-стандарты | Бесплатно | P2 |

### 1.6. Внешние AI

| Продукт | API | Назначение | Стоимость | Статус |
|---------|-----|------------|-----------|--------|
| **OpenAI (GPT-4o, GPT-4.1)** | REST | Сложный анализ, генерация текста | Per-token | P0 |
| **Anthropic (Claude 3.5/4)** | REST | Код, анализ, рассуждение | Per-token | P0 |
| **DeepSeek** | REST | Бюджетное рассуждение | Per-token | Реализован (заглушка) |
| **Ollama (локальный)** | REST | Локальный LLM-хост | Бесплатно | P1 |
| **Perplexity** | REST | Поиск с цитированием | $20/мес | P2 |
| **Groq** | REST | Быстрый инференс (LPU) | Per-token | P2 |

---

## Часть II. Системы — компоненты DMZ-зоны

### 2.1. Шлюз и прокси

| Система | Назначение | Порт | Комментарий |
|---------|------------|------|-------------|
| **nginx** | TLS-терминация, обратный прокси, rate limiting | 443 → 8080 | Проверенный, быстрый |
| **Caddy** (альтернатива) | Автоматический HTTPS + прокси | 443 → 8080 | Проще в настройке |
| **MCP Proxy** (`mcp_proxy.py`) | JSON-RPC 2.0 фасад для агентов | 8080 | Реализован |

### 2.2. Безопасность

| Система | Назначение | Приоритет |
|---------|------------|-----------|
| **iptables / nftables** | Firewall: интернет → DMZ → внутренняя сеть | P0 — реализовано |
| **fail2ban** | Защита от brute-force на MCP-шлюз | P1 |
| **API Key Manager** | Хранение и ротация ключей внешних API (Vault / sealed secrets) | P0 |
| **Rate Limiter** | Квотирование запросов на агента и на источник | P0 — в mcp_proxy |
| **WAF (ModSecurity / Coraza)** | Web Application Firewall для DMZ | P2 |

### 2.3. Кэширование

| Система | Назначение | Порт | TTL |
|---------|------------|------|-----|
| **Redis (DMZ-экземпляр)** | Кэш внешних запросов | 6380 | 1ч–7д |
| **Redis (внутренний)** | Кэш ответов для агентов | 6379 | 1ч–24ч |
| **ChromaDB (DMZ-экземпляр)** | Векторный кэш внешних статей | 8001 | постоянный |

*Важно: Redis в DMZ и внутренний Redis — разные экземпляры. DMZ Redis изолирован, не содержит внутренних данных.*

### 2.4. Мониторинг и логирование

| Система | Назначение | Приоритет |
|---------|------------|-----------|
| **Prometheus + Grafana** | Метрики DMZ: latency, cache_hit, error_rate | P1 |
| **Loki** | Логи MCP-запросов и ответов | P1 |
| **Kafka (DMZ-топики)** | События: `external.requests.v1`, `external.responses.v1`, `events.external_ai.v1` | P0 |
| **Redis Pub/Sub** | Оповещения: `alerts.dmz.v1` | P2 |

### 2.5. MCP-адаптеры (навыки Интегратора)

| Адаптер | Источник | Приоритет | Заметка |
|---------|----------|-----------|---------|
| `brave-search-adapter` | Brave Search API | P0 | Первичный поиск |
| `arxiv-adapter` | arXiv API + Semantic Scholar | P0 | Научные статьи |
| `github-adapter` | GitHub REST/GraphQL | P0 | Код и репозитории |
| `wikipedia-adapter` | Wikipedia + Wikidata | P0 | Энциклопедии |
| `core-adapter` | CORE API | P1 | Open Access full-text |
| `zenodo-adapter` | Zenodo | P1 | Научные datasets |
| `nasa-spice-adapter` | NASA SPICE | P1 | Космические данные |
| `crossref-adapter` | CrossRef | P2 | DOI lookup |
| `pypi-adapter` | PyPI | P2 | Python-пакеты |

---

## Часть III. Потоки данных через DMZ

### 3.1. Исходящий поток (агент → внешний мир)

```
Агент (Гексада)
    │  external.requests.v1 (Kafka)
    ▼
MCP-шлюз (валидация)
    │
    ├── Redis-кэш (проверка)
    │   ├── HIT  → возврат из кэша (экономия токенов)
    │   └── MISS → запрос вовне
    │
    ▼
nginx (TLS + rate limit)
    │
    ▼
Внешний API / AI
    │
    ▼
Ответ → Redis-кэш → ChromaDB → Агент
```

### 3.2. Входящий поток (внешний AI → агент)

```
Внешний AI (Claude, GPT, DeepSeek)
    │  MCP-запрос от внешнего AI (JSON-RPC 2.0)
    ▼
nginx (аутентификация)
    │
    ▼
MCP-шлюз (валидация + проверка отправителя)
    │
    ├── Логирование: events.external_ai.v1 (Kafka)
    │
    ▼
Агент (через internal Kafka)
```

### 3.3. Поток знаний (внешние данные → память платформы)

```
Внешний API (arxiv, wiki, github)
    │
    ▼
MCP-адаптер (специфичный для источника)
    │
    ├── Форматная конверсия (HTML→MD, PDF→текст)
    ├── Извлечение метаданных (автор, дата, лицензия)
    ├── Этическая фильтрация
    │
    ▼
Структурированная статья (JSON-LD)
    │
    ├── ChromaDB (эмбеддинг → векторный поиск)
    ├── LightRAG (граф → связи с онтологией)
    └── CozoDB (структура → запросы Datalog)
```

---

## Часть IV. Рекомендуемая топология Docker

```yaml
# docker-compose.dmz.yml
services:
  nginx:
    image: nginx:alpine
    ports:
      - "443:443"
    volumes:
      - ./certs:/etc/nginx/certs:ro
      - ./nginx/nginx.conf:/etc/nginx/nginx.conf:ro
    networks:
      dmz-net:
        aliases: [gateway.waters]

  mcp-proxy:
    build: ./scripts
    command: python mcp_proxy.py --host 0.0.0.0 --port 8080
    environment:
      - REDIS_HOST=redis-dmz
      - REDIS_PORT=6380
      - CHROMA_HOST=chroma-dmz
      - CHROMA_PORT=8001
    volumes:
      - ./integrations:/app/integrations:ro
    networks:
      dmz-net:
        aliases: [mcp.waters]

  redis-dmz:
    image: redis:7-alpine
    command: redis-server --port 6380 --maxmemory 256mb --maxmemory-policy allkeys-lru
    ports:
      - "6380:6380"
    volumes:
      - redis_dmz_data:/data
    networks:
      - dmz-net

  chroma-dmz:
    image: chromadb/chroma:latest
    ports:
      - "8001:8000"
    volumes:
      - chroma_dmz_data:/chroma/chroma
    networks:
      - dmz-net

  redis-internal:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    command: redis-server --maxmemory 512mb --maxmemory-policy allkeys-lru
    volumes:
      - redis_internal_data:/data
    networks:
      - internal-net

networks:
  dmz-net:
    driver: bridge
    internal: false
  internal-net:
    driver: bridge
    internal: true

volumes:
  redis_dmz_data:
  chroma_dmz_data:
  redis_internal_data:
```

---

## Часть V. Приоритеты внедрения (Спринт 1–3)

### Спринт 1 (сейчас) — P0

| # | Действие | Исполнитель |
|---|----------|-------------|
| 1 | Подключить API-ключи: Brave Search, OpenAI, Anthropic | Интегратор |
| 2 | Реализовать `brave-search-adapter` | Интегратор + Конструктор |
| 3 | Реализовать `arxiv-adapter` (+ Semantic Scholar) | Интегратор |
| 4 | Реализовать `wikipedia-adapter` (+ Wikidata SPARQL) | Интегратор |
| 5 | Настроить Rate Limiter в MCP-шлюзе | Конструктор |
| 6 | Поднять Docker-стек DMZ (nginx + redis-dmz + chroma-dmz) | Конструктор |

### Спринт 2 — P1

| # | Действие | Исполнитель |
|---|----------|-------------|
| 7 | Реализовать `github-adapter` | Интегратор |
| 8 | Реализовать `core-adapter` | Интегратор |
| 9 | Реализовать `zenodo-adapter` | Интегратор |
| 10 | Настроить Prometheus + Grafana для DMZ метрик | Конструктор |
| 11 | Настроить Loki для логов MCP | Конструктор |

### Спринт 3 — P2

| # | Действие | Исполнитель |
|---|----------|-------------|
| 12 | Реализовать `nasa-spice-adapter` | Интегратор |
| 13 | Реализовать `crossref-adapter` | Интегратор |
| 14 | Настроить WAF (Coraza) | Конструктор |
| 15 | Внешние AI-коммуникации (MCP-сессии с другими AI) | Интегратор |
| 16 | Настроить fail2ban | Конструктор |

---

## Часть VI. Что уже есть vs что нужно

### Уже есть ✅

| Продукт | Компонент | Файл |
|---------|-----------|------|
| MCP-шлюз | `mcp_proxy.py` (JSON-RPC 2.0, кэш, методы) | `scripts/mcp_proxy.py` |
| Redis-кэш | Протокол кэширования (TTL, ключи, инвалидация) | `documents/redis_cache_protocol.md` |
| DMZ-архитектура | Схема взаимодействия, firewall-правила | `documents/dmz_architecture.md` |
| ArXiv | Заглушка адаптера в mcp_proxy (возвращает `note: не настроен`) | `scripts/mcp_proxy.py:266` |
| Exa Search | Заглушка поиска | `scripts/mcp_proxy.py:243` |
| Web Fetch | Рабочий парсер HTML | `scripts/mcp_proxy.py:253` |
| DeepSeek | Заглушка AI-чата | `scripts/mcp_proxy.py:271` |

### Нужно сделать ❌

| Продукт | Что нужно | Приоритет |
|---------|-----------|-----------|
| Brave Search | Рабочий адаптер с API-ключом | P0 |
| arXiv + Semantic Scholar | Заменить заглушку на реальный API-запрос | P0 |
| Wikipedia + Wikidata | Новый адаптер (REST + SPARQL) | P0 |
| GitHub | Новый адаптер (REST/GraphQL) | P0 |
| CORE API | Новый адаптер (Open Access full-text) | P1 |
| OpenAI / Anthropic | Заменить заглушку на реальный API с токеном | P0 |
| Docker-стек DMZ | docker-compose.dmz.yml с nginx + redis-dmz + chroma-dmz | P0 |
| API Key Manager | Хранилище секретов (env vars / Vault / sealed secrets) | P0 |
| Rate Limiter | Квотирование на агента и на источник | P0 |
| Prometheus + Grafana | Мониторинг DMZ | P1 |

---

*Документ подготовлен Интегратором Знаний. Передаётся Конструктору Сети для проектирования инфраструктуры DMZ.*
*Следующий шаг: Конструктор создаёт `infrastructure/dmz/docker-compose.dmz.yml`.*
