# Сайт виртуальной миссии

---

## 1. Назначение

Публичный веб-сайт, визуализирующий Миссию 1 в реальном времени. Данные читает из Kafka (topic `mission.1.*.v1`).

## 2. Страницы

| Страница | Описание |
|----------|----------|
| **Dashboard** | 3D Globe (Three.js) с треками метеоритов. Статистика: findings, агенты, uptime |
| **Findings** | Лента находок с фильтрами по типу, дате, агенту, рангу |
| **Detail** | Детали одной находки: спектр, траектория, классификация |
| **Agents** | Список sub-agent'ов с рангами, статусами, KPI |
| **Ranks** | Leaderboard: личные и командные ранги |

## 3. Технологии

| Компонент | Технология |
|-----------|-----------|
| Frontend | React 19 + TypeScript + Vite |
| 3D | Three.js + Globe.GL |
| Realtime | Kafka consumer → WebSocket (bridge) |
| Backend | FastAPI (Python) |
| DB | PostgreSQL (findings, agents, ranks) |

## 4. Kafka → WebSocket bridge

```
mission.1.findings.v1 ─┐
mission.1.agents.v1 ───┤→ Kafka Consumer (Python) → WebSocket → React
mission.1.ranks.v1 ────┘
mission.1.heartbeat.v1 ─┘
```
