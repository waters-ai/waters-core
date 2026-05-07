# SKILL.md — ollama-local-model

## Навык: подключение локальной LLM через Ollama

### Идентификация

| Поле | Значение |
|------|----------|
| **skill_id** | `ollama-local-model` |
| **версия** | 1.0.0 |
| **владелец** | `agent.constructor.v1` |
| **тип** | `core` |
| **слой** | III — Воля |
| **статус** | `active` |

### Назначение

`ollama-local-model` — навык подключения агентов WATERS к локальной LLM через Ollama.
Используется как основной или fallback-провайдер, когда внешние API (DeepSeek, OpenAI, Anthropic) недоступны или нежелательны.

Рекомендуемая модель: **Qwen 2.5 7B** — 5-6 GB RAM (Q4), ~60-65% качества DeepSeek V4 Flash, хорошее tool calling.

### Установка модели

```bash
# Проверить, какие модели уже скачаны
ollama list

# Скачать Qwen 2.5 7B (~4.7 GB)
ollama pull qwen2.5:7b

# Для серверов с 4 ГБ RAM (fallback)
ollama pull qwen2.5:3b
```

### Проверка

```bash
# Проверить, что Ollama работает и модель отвечает
curl http://localhost:11434/api/tags

# Быстрый тест генерации
curl http://localhost:11434/api/generate -d '{
  "model": "qwen2.5:7b",
  "prompt": "Привет, ответь одним словом.",
  "stream": false
}'
```

### Конфигурация OpenCode

Файл `opencode.json` в корне репозитория:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "ollama": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Ollama (local)",
      "options": {
        "baseURL": "http://localhost:11434/v1"
      },
      "models": {
        "qwen2.5:7b": {
          "name": "Qwen 2.5 7B"
        },
        "qwen2.5:3b": {
          "name": "Qwen 2.5 3B"
        },
        "qwen2.5:0.5b": {
          "name": "Qwen 2.5 0.5B"
        }
      }
    }
  },
  "model": "ollama/qwen2.5:7b"
}
```

После создания конфига OpenCode автоматически подхватит провайдера при следующем запуске.

### Использование в карусели

Карусель (`run_carousel.sh`) запускает `opencode` для каждого агента.
OpenCode читает `opencode.json` из корня репозитория — модель подключается автоматически.

Переменные окружения в `.env.carousel`:
```
LLM_DEFAULT_PROVIDER=deepseek
LLM_FALLBACK_PROVIDER=ollama
```

При отказе DeepSeek агенты автоматически переключаются на локальную Qwen 2.5 7B.

### Советы

- Если tool calling работает плохо — увеличьте `num_ctx` в Ollama: `ollama run qwen2.5:7b` и установите `/set parameter num_ctx 32768`
- На 8 ГБ RAM модель влезает с запасом. На 4 ГБ используйте `qwen2.5:3b`
- Для продакшена с 32+ ГБ рассмотрите `qwen2.5-coder:32b` или `qwen3-coder:a3b`
