# Миссия 1: Поиск метеоритов

## Обзор

Миссия 1 — первая полевая миссия платформы WATERS. Цель: автономный поиск, классификация и трекинг метеоритов с использованием роя RLM-агентов на базе форка DeepSeek-TUI.

| Поле | Значение |
|------|----------|
| **Код** | `mission-1-meteorite` |
| **Тип** | Наземная (Земля) |
| **Движок** | Форк DeepSeek-TUI (Rust) → `mission-control` |
| **Субагенты** | 16+ RLM Flash |
| **LLM** | DeepSeek API + Ollama fallback |
| **Связь с центром** | HTTP-bridge → Kafka на 238 |
| **Сервер** | Hetzner AX102 |

## Состав миссии

### Агенты (RLM Pro + Flash)

| Тип агента | Кол-во | Назначение |
|------------|--------|------------|
| `DataCollectorAgent` | 4 | Сбор данных из NASA API, публикаций, спектральных БД |
| `AnalyzerAgent` | 6 | Анализ траекторий, классификация метеоритов |
| `PatternMatcherAgent` | 4 | Поиск аномалий, корреляций, паттернов |
| `CoordinatorAgent` | 2 | Оркестрация, отчётность, коммуникация с центром |

### MCP-серверы

| Сервер | Данные | Протокол |
|--------|--------|----------|
| `mcp-nasa-fireball` | NASA Fireball API | MCP stdio/SSE |
| `mcp-spectra` | Спектральные базы данных | MCP stdio/SSE |
| `mcp-trajectory` | Расчёт траекторий болидов | MCP stdio/SSE |
| `mcp-knowledge` | ChromaDB + LightRAG (локально) | MCP stdio |

### Система памяти

| Хранилище | Данные | Доступ |
|-----------|--------|--------|
| Redis | Состояние миссии, pub/sub, кэш | Все агенты |
| ChromaDB | Векторные эмбеддинги аномалий | PatternMatcherAgent |
| LightRAG | Граф знаний метеоритов | Все агенты |
| RocksDB (TUI) | Durable Task Queue | RLM Pro |
