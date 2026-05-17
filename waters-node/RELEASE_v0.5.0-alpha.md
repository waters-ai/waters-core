# v0.5.0-alpha — Agent ACL, MCP Store, Agent-to-Agent Chat, i18n

**12MB, 44 .rs, ~13000 строк.**

## Новое

| Фича | Описание |
|------|----------|
| Agent-to-Agent Chat | JSON-протокол: @agent <id> <action>, @all <topic>. Request/Response/Broadcast/ToolCall/Coordinate |
| Agent ACL | /acl allow|block|block-all — хозяин контролирует кто кому пишет |
| MCP Store | Реестр скилов, search/install/uninstall из taps |
| Channel Isolation | 4 уровня доступа: Public/Group/PeerList/Private |
| i18n | ru/en/zh для уведомлений и системных сообщений |
| Push Notifications | Через любой активный чат-транспорт (telegram/discord/email) |
| Presence | Статус online/offline в Redis, TTL 5 мин |
| Health Endpoint | GET /health — JSON 200/503 |
| systemd unit | deploy/waters-node.service |
| DND/SOS режимы | Не беспокоить + Аварийный |
| SecurityLearner | Адаптивное доверие, чёрные/белые списки |
| Plugin System | Plugin trait, 6 хуков (agent_open/close, llm_call, tool_call...) |
| Self-improving skills | skill_evolve — при закрытии агента создаётся улучшенная версия |

## Транспорты

telegram / discord / whatsapp / wechat / email / stdin

## Установка

```bash
wget https://github.com/waters-ai/waters-core/releases/download/v0.5.0-alpha/waters-node
chmod +x waters-node
export DEEPSEEK_API_KEY=sk-xxx
export REDIS_URL=redis://127.0.0.1:6379
./waters-node --port 42069
```
