# OpenCode 24/7 — Постоянная работа агентов WATERS

## Проблема

Агенты WATERS работают через OpenCode с Ollama (qwen2.5:7b на CPU, сервер 237).
CPU-модели генерируют ответ **дольше 5 минут** — стандартный таймаут OpenCode.
В результате:

- OpenCode обрывает запрос (`AI_APICallError`)
- Сессия агента теряет контекст
- После закрытия SSH процесс opencode умирает полностью
- CEO не может подключиться, ответить агенту и отключиться

## Решение A: Таймауты в opencode.json

OpenCode поддерживает два параметра таймаута на уровне провайдера:

| Параметр | По умолч. | Назначение |
|----------|-----------|------------|
| `timeout` | 300000ms (5 мин) | Полный таймаут запроса |
| `chunkTimeout` | 30000ms (30 сек) | Таймаут между чанками стрима |

Оба можно отключить (`false`) или увеличить.

### Изменение opencode.json

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "deepseek": {
      "name": "DeepSeek (WATERS Primary)",
      "apiKey": "sk-e9bb0e4fa0194955a8a259ec5cf8fd2d",
      "models": {
        "deepseek-v4-flash": {
          "name": "DeepSeek V4 Flash"
        }
      }
    },
    "ollama": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Ollama (237 via SSH Tunnel)",
      "options": {
        "baseURL": "http://localhost:11434/v1",
        "timeout": false,
        "chunkTimeout": 120000
      },
      "models": {
        "qwen2.5:14b": {
          "name": "Qwen 2.5 14B"
        },
        "qwen2.5:7b": {
          "name": "Qwen 2.5 7B"
        },
        "deepseek-r1:7b": {
          "name": "DeepSeek R1 7B"
        }
      }
    }
  },
  "model": "Qwen 2.5 14B"
}
```

**Что изменилось:**
- `"timeout": false` — отключает лимит на полное время ответа
- `"chunkTimeout": 120000` — ждём до 2 минут между токенами (CPU-модели могут «задуматься»)

> **Вариант с числом**: если не хотите отключать timeout полностью, поставьте `600000` (10 минут) — запас для самой медленной генерации.

### Верификация

```bash
# Проверить, что конфиг читается
opencode debug config

# Тест: запустить и дать долгую задачу
opencode run "Напиши подробный анализ архитектуры WATERS"
```

---

## Решение B: tmux — постоянная сессия с detach/attach

tmux держит процесс opencode живым независимо от SSH-соединения.
CEO может **подключиться**, ответить агенту и **отключиться** — агент продолжает работу.

### Скрипт: `scripts/opencode_tmux.sh`

```bash
#!/usr/bin/env bash
# opencode_tmux.sh — запуск агента WATERS в tmux-сессии
#
# Сессия живёт 24/7 независимо от SSH.
# CEO подключается:  tmux attach -t waters:<agent>
# CEO отключается:   Ctrl+B, D (агент продолжает работу)
#
# Использование:
#   ./scripts/opencode_tmux.sh <agent>     # запустить агента
#   ./scripts/opencode_tmux.sh <agent> attach  # подключиться к запущенному
#
# Пример:
#   ./scripts/opencode_tmux.sh constructor
#   ./scripts/opencode_tmux.sh constructor attach

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="${LOG_DIR:-$REPO_DIR/logs}"
SESSION_NAME="${SESSION_NAME:-waters}"

mkdir -p "$LOG_DIR"

agent="${1:-}"
action="${2:-start}"

if [ -z "$agent" ]; then
    echo "Использование: $0 <agent> [start|attach]"
    echo "Агенты: architect, constructor, integrator, director, lawkeeper, keeper"
    exit 1
fi

agent_file="$REPO_DIR/agents/${agent}_AGENTS.md"
window_name="$agent"

if [ ! -f "$agent_file" ]; then
    echo "Ошибка: файл $agent_file не найден"
    exit 1
fi

case "$action" in
    start)
        echo "Запуск агента $agent в tmux-сессии $SESSION_NAME..."

        # Копируем AGENTS.md агента
        cp "$agent_file" "$REPO_DIR/AGENTS.md"

        # Создаём сессию, если её нет, иначе — новое окно
        if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
            tmux new-session -d -s "$SESSION_NAME" -n "$window_name" \
                "cd '$REPO_DIR' && opencode 2>&1 | tee '$LOG_DIR/${agent}.log'; exec bash"
        else
            # Проверяем, есть ли уже окно для этого агента
            if tmux list-windows -t "$SESSION_NAME" -F '#{window_name}' | grep -q "^$window_name$"; then
                echo "Окно $window_name уже существует. Подключитесь: tmux attach -t $SESSION_NAME:$window_name"
                exit 0
            fi
            tmux new-window -t "$SESSION_NAME" -n "$window_name" \
                "cd '$REPO_DIR' && opencode 2>&1 | tee '$LOG_DIR/${agent}.log'; exec bash"
        fi

        echo "Агент $agent запущен в tmux-сессии $SESSION_NAME:$window_name"
        echo ""
        echo "Подключиться:  tmux attach -t $SESSION_NAME:$window_name"
        echo "Отключиться:   Ctrl+B, D  (агент продолжит работу)"
        echo "Список окон:   tmux list-windows -t $SESSION_NAME"
        echo "Лог:           $LOG_DIR/${agent}.log"
        ;;
    attach)
        if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
            echo "Сессия $SESSION_NAME не найдена. Запустите агента: $0 $agent start"
            exit 1
        fi
        tmux attach-session -t "$SESSION_NAME:$window_name"
        ;;
    stop)
        if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
            if tmux list-windows -t "$SESSION_NAME" -F '#{window_name}' | grep -q "^$window_name$"; then
                tmux kill-window -t "$SESSION_NAME:$window_name"
                echo "Окно $window_name остановлено"
            else
                echo "Окно $window_name не найдено"
            fi
        else
            echo "Сессия $SESSION_NAME не найдена"
        fi
        ;;
    *)
        echo "Действие: start | attach | stop"
        exit 1
        ;;
esac
```

### Шпаргалка для CEO

```bash
# === Подключиться к агенту ===
tmux attach -t waters:constructor

# === Отключиться (агент продолжает работу) ===
# Ctrl+B, затем D

# === Список всех запущенных агентов ===
tmux list-windows -t waters

# === Подключиться к конкретному агенту ===
tmux attach -t waters:architect

# === Смотреть лог, не подключаясь к tmux ===
tail -f logs/constructor.log

# === Остановить одного агента ===
tmux kill-window -t waters:constructor

# === Остановить всех ===
tmux kill-session -t waters
```

### Интеграция с run_constructor.sh

Для обратной совместимости `run_constructor.sh` теперь принимает флаг `--tmux`:

```bash
./run_constructor.sh                    # обычный запуск (как сейчас)
./run_constructor.sh --tmux             # запуск в tmux-сессии
```

---

## Решение C (Фаза 2): headless serve + REST API

Для полной автоматизации и удалённого управления без TUI.

### Скрипт запуска: `scripts/opencode_serve.sh`

Запускает `opencode serve` в tmux-сессии с автогенерацией пароля.

```bash
# Запуск сервера
./scripts/opencode_serve.sh start

# Статус
./scripts/opencode_serve.sh status

# Остановка
./scripts/opencode_serve.sh stop

# Получить пароль
./scripts/opencode_serve.sh password

# Получить URL с паролем
./scripts/opencode_serve.sh url
```

**Параметры** (через переменные окружения):

| Переменная | По умолч. | Назначение |
|------------|-----------|------------|
| `OPENCODE_SERVE_PORT` | `4096` | Порт сервера |
| `OPENCODE_SERVE_HOST` | `0.0.0.0` | Хост |
| `OPENCODE_SERVER_PASSWORD` | автогенерация | Пароль basic auth |
| `LOG_DIR` | `logs/` | Директория логов |

При первом запуске пароль генерируется через `openssl rand` и сохраняется в `.serve_password` (chmod 600).

### REST API

OpenCode serve открывает HTTP API (OpenAPI 3.1 spec: `http://host:4096/doc`).

| Метод | Путь | Назначение |
|-------|------|------------|
| `GET` | `/session` | Список сессий |
| `POST` | `/session` | Создать сессию |
| `POST` | `/session/:id/init` | Инициализировать (AGENTS.md) |
| `POST` | `/session/:id/message` | Отправить сообщение (синхронно) |
| `POST` | `/session/:id/prompt_async` | Отправить сообщение (асинхронно) |
| `GET` | `/session/:id` | Статус сессии |
| `GET` | `/session/:id/message` | Сообщения сессии |
| `POST` | `/session/:id/abort` | Прервать сессию |

### CEO CLI: `scripts/ceo.sh`

Удобная обёртка над REST API для ежедневного использования:

```bash
# Быстрый старт
./scripts/opencode_serve.sh start
export OPENCODE_SERVER=http://opencode:$(./scripts/opencode_serve.sh password)@localhost:4096

# Список сессий
./scripts/ceo.sh sessions

# Создать сессию для агента
./scripts/ceo.sh create constructor

# Инициализировать (создать AGENTS.md в контексте проекта)
./scripts/ceo.sh init <session-id>

# Отправить запрос агенту (синхронно — ждать ответ)
./scripts/ceo.sh msg <session-id> "Проанализируй текущую топологию Docker"

# Отправить запрос агенту (асинхронно — не ждать)
./scripts/ceo.sh tell <session-id> "Продолжай без меня, напиши отчёт"

# Статус сессии
./scripts/ceo.sh status <session-id>

# Последние сообщения
./scripts/ceo.sh log <session-id>

# Прервать выполнение
./scripts/ceo.sh abort <session-id>
```

**Подключение с удалённой машины:**

```bash
# На сервере: узнать URL
URL=$(ssh root@server './scripts/opencode_serve.sh url')

# На своей машине: установить переменную
export OPENCODE_SERVER="$URL"

# Использовать те же команды
./scripts/ceo.sh sessions
./scripts/ceo.sh msg <id> "Продолжай"
```

### Без ceo.sh: чистый curl

```bash
# 1. Получить список сессий
SESSIONS=$(curl -s -u "opencode:$(cat .serve_password)" http://localhost:4096/session)
SESSION_ID=$(echo "$SESSIONS" | jq -r '.[0].id')

# 2. Отправить ответ агенту (синхронно)
curl -s -u "opencode:$(cat .serve_password)" \
  -X POST "http://localhost:4096/session/$SESSION_ID/message" \
  -H "Content-Type: application/json" \
  -d '{
    "parts": [
      { "type": "text", "text": "Используй qwen2.5:14b для этой задачи" }
    ]
  }'

# 3. Асинхронный режим — не ждать ответа
curl -s -u "opencode:$(cat .serve_password)" \
  -X POST "http://localhost:4096/session/$SESSION_ID/prompt_async" \
  -H "Content-Type: application/json" \
  -d '{ "parts": [{ "type": "text", "text": "Продолжай без меня" }] }'

# 4. Позже проверить статус
curl -s -u "opencode:$(cat .serve_password)" \
  "http://localhost:4096/session/$SESSION_ID" | jq '.status'
```

---

## Файлы, изменённые в этом спринте

| Файл | Изменение |
|------|-----------|
| `opencode.json` | Добавлены `timeout` и `chunkTimeout` для Ollama |
| `scripts/opencode_tmux.sh` | **Новый** — универсальный запуск агентов в tmux (Фаза 1) |
| `scripts/opencode_serve.sh` | **Новый** — headless serve-сервер в tmux (Фаза 2) |
| `scripts/ceo.sh` | **Новый** — CEO CLI для REST API (Фаза 2) |
| `.env.carousel` | Добавлены `OPENCODE_SERVE_*` переменные |
| `run_constructor.sh` | Добавлен флаг `--tmux` |
| `documents/opencode_24_7_setup.md` | **Новый** — данный документ |

---

## Чек-лист верификации

- [ ] `opencode.json` — таймауты сняты, сессия не рвётся на долгих ответах
- [ ] `scripts/opencode_tmux.sh start` — агент запускается в tmux
- [ ] `tmux attach` — CEO видит интерфейс агента
- [ ] `Ctrl+B, D` — CEO отключается, агент продолжает работу
- [ ] Повторный `tmux attach` — CEO снова видит актуальное состояние
- [ ] `scripts/opencode_serve.sh start` — сервер запускается в tmux
- [ ] `scripts/opencode_serve.sh status` — показывает RUNNING
- [ ] `scripts/ceo.sh sessions` — возвращает список сессий
- [ ] `scripts/ceo.sh msg <id> "тест"` — синхронный ответ получен
- [ ] `scripts/ceo.sh tell <id> "тест"` — асинхронное сообщение отправлено
- [ ] `curl -u "opencode:pass" http://host:4096/session` — базовый curl работает
- [ ] `OPENCODE_SERVER` с удалённой машины — CEO работает извне
