# MCP в Mission Control

**Источник:** DeepSeek TUI MCP.md — адаптировано под WATERS Mission 1

---

## 1. MCP-серверы миссии

Mission Control использует MCP (Model Context Protocol) как **единственный протокол** для всех внешних интеграций. Никаких прямых HTTP-вызовов API.

### 1.1. Встроенные серверы

| Сервер | Транспорт | Данные | Запуск |
|--------|-----------|--------|--------|
| `mcp-nasa-fireball` | stdio | NASA Fireball API | `python3 mcp-servers/mcp-nasa-fireball/server.py` |
| `mcp-spectra` | stdio | Спектральные базы данных | `python3 mcp-servers/mcp-spectra/server.py` |
| `mcp-trajectory` | stdio | Расчёт траекторий болидов | `python3 mcp-servers/mcp-trajectory/server.py` |

### 1.2. Серверы хаба 238 (доступны через Kafka)

| Сервер | Транспорт | Данные | Доступ |
|--------|-----------|--------|--------|
| `mcp-knowledge` | SSE | ChromaDB + LightRAG | Только с 238 (не с миссионного сервера) |

---

## 2. Конфигурация MCP

```json
{
  "mcpServers": {
    "mcp-nasa-fireball": {
      "command": "python3",
      "args": ["mcp-servers/mcp-nasa-fireball/server.py"],
      "env": {
        "NASA_API_KEY": "{env:NASA_API_KEY}"
      },
      "timeout": 30000
    },
    "mcp-spectra": {
      "command": "python3",
      "args": ["mcp-servers/mcp-spectra/server.py"],
      "timeout": 15000
    },
    "mcp-trajectory": {
      "command": "python3",
      "args": ["mcp-servers/mcp-trajectory/server.py"],
      "timeout": 60000
    }
  }
}
```

---

## 3. Инструменты MCP-серверов

### 3.1. mcp-nasa-fireball

| Инструмент | Описание | Вход | Выход |
|-----------|----------|------|-------|
| `search_fireballs` | Поиск болидов по дате/региону | `date_from`, `date_to`, `lat/lon` bounds | Массив `FireballEvent` |
| `get_fireball_detail` | Детали одного события | `event_id` | `FireballDetail` |
| `get_nasa_neo` | Близкие пролёты астероидов | `date_from`, `date_to` | Массив `NEOEvent` |

### 3.2. mcp-spectra

| Инструмент | Описание | Вход | Выход |
|-----------|----------|------|-------|
| `classify_meteorite` | Классификация по спектру | `spectrum_data` (JSON) | `MeteoriteClass` |
| `search_spectra_db` | Поиск по спектральной БД | `mineral_type`, `wavelength_range` | Массив `SpectrumMatch` |

### 3.3. mcp-trajectory

| Инструмент | Описание | Вход | Выход |
|-----------|----------|------|-------|
| `calculate_trajectory` | Расчёт траектории входа | `velocity`, `angle`, `start_alt`, `mass` | `TrajectoryResult` |
| `estimate_landing_zone` | Зона падения | `trajectory_data` | `LandingZone` |

---

## 4. Безопасность

MCP-серверы работают через stdio — никакие порты не открываются наружу. Миссионный сервер не имеет открытых портов, кроме SSH от 238.

Все API-ключи (NASA, спектры) — в переменных окружения, не в коде.
