# Режимы автономии Mission Control

**Источник:** DeepSeek TUI MODES.md (Plan/Agent/YOLO) — адаптировано для WATERS Mission 1

---

## 1. Режимы работы

Mission Control работает в **5 уровнях автономии (L0-L4)**, в отличие от TUI где режимы (Plan/Agent/YOLO) определяют поведение LLM.

### 1.1. Уровни автономии

```
L0 ─── L1 ─── L2 ─── L3 ─── L4
 │      │      │      │      │
Полная  Эконо- Авто-  Энерго- Safe
мощность мия    номный сбере-  mode
                (офлайн) жение
```

### 1.2. Детали

| Уровень | Kafka | LLM | Режим | Когда |
|---------|-------|-----|-------|-------|
| **L0** | ✅ Online | DeepSeek API | Полная мощность. Все sub-agent'ы активны, findings отправляются немедленно | Штатная работа |
| **L1** | ✅ Online | Ollama local | Экономия API. Те же возможности, но LLM локально | Лимит API-ключей |
| **L2** | ❌ Offline | Ollama local | **Автономный**. Agенты работают, findings копятся в Redis. При восстановлении — batch-отправка | Потеря связи с 238 < 24h |
| **L3** | ❌ Offline | Ollama mini | **Энергосбережение**. Только explorer + collector. Анализатор и matcher отключены | Потеря связи > 24h |
| **L4** | ❌ Offline | Нет | **Safe mode**. Только heartbeat + логи. Sub-agent'ы остановлены | Критический сбой / нет энергии |

---

## 2. Переключение режимов

### 2.1. Автоматическое

```
Autonomy Engine (проверка каждые 60 секунд):
  1. Kafka heartbeat к 238?
     ✅ → проверяем LLM
       DeepSeek API → L0
       Ollama local → L1
     ❌ → L2 (ждём 24h)
       → через 24h → L3
       → через 48h → L4 (если батарея < 20%)

  2. При восстановлении Kafka:
     L2/L3 → L0
     Находки из Redis → batch → Kafka
```

### 2.2. Ручное

Через Kafka `mission.1.orders.v1` c type=`reconfigure`:

```json
{
  "order_id": "ord_force_l2",
  "type": "reconfigure",
  "payload": {
    "force_level": "L2"
  }
}
```

---

## 3. Что меняется на каждом уровне

| Функция | L0 | L1 | L2 | L3 | L4 |
|---------|----|----|----|----|----|
| explorer | ✅ | ✅ | ✅ | ✅ | ❌ |
| collector | ✅ | ✅ | ✅ | ✅ | ❌ |
| analyzer | ✅ | ✅ | ✅ | ❌ | ❌ |
| matcher | ✅ | ✅ | ✅ | ❌ | ❌ |
| coordinator | ✅ | ✅ | ✅ | ✅ | ✅ (лог) |
| Kafka findings | real-time | real-time | batch | batch | ❌ |
| Kafka heartbeat | 60s | 60s | 300s | 600s | 3600s |
| Redis findings queue | нет | нет | ✅ до 1000 | ✅ до 500 | ❌ |
| DTN (задержка симуляция) | ✅ | ✅ | ✅ | ❌ | ❌ |
