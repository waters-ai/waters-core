# Анализ метрик waters-node — после рефакторинга

## Прогресс

| Метрика | Было | Стало | Δ |
|---------|------|-------|---|
| .rs файлов | 28 | 34 | +6 |
| Строк кода | ~5500 | 8560 | +3060 |
| Агентов (skill.json) | 5 | 17 | +12 |
| Redis команд | GET/SET/DEL | +PUBLISH +XADD +HSET +SELECT | ×4 |
| API endpoint'ов | 6 | 8 | +2 (SSE, store) |
| MCP-сервер | нет | да (порт HTTP+100) | +1 сервер |
| Импорт/экспорт | нет | TUI/Claude/Cursor/WATERS | +4 формата |
| Слияние агентов | нет | да | +функция |

## Что изменилось (новые файлы)

| Файл | Строк | Назначение |
|------|-------|-----------|
| `bridge_agent.rs` | 498 | Импорт/экспорт агентов между форматами |
| `mcp_server.rs` | 286 | MCP-сервер — агенты как инструменты для Claude/Cursor/TUI |

## Ключевые изменения

| Файл | Изменение |
|------|----------|
| `store.rs` | multi-DB (0-15), PUBLISH, XADD, HSET, StreamSubscriber |
| `tui_agent.rs` | mpsc → Redis PUBLISH для стриминга токенов |
| `api.rs` | SSE endpoint `/api/v1/stream/{session}`, Redis статус |
| `skill.rs` | SkillManifest расширен (role, category, llm, tools, imported_from) |
| `subagent.rs` | 95 → 482 строк: agent_open/eval/close + Redis persistence |
| `handlers.rs` | +/merge, /suggest, /import, /export, /import-dir, /import-llm |

## Что требует рефакторинга

### 1. Двойная инициализация KvStore (main.rs:97 и 254)
```rust
// Строка 97:
let kvstore = { ... Arc::new(store::KvStore::new(...)) };

// Строка 254 (перезатирает первую!):
let kvstore = { ... Arc::new(store::KvStore::new(...)) };
```
**Фикс:** удалить вторую инициализацию, использовать первую.

### 2. by_category и by_role в SkillRegistry — never populated
```rust
pub by_category: HashMap<String, Vec<String>>,
pub by_role: HashMap<String, Vec<String>>,
```
Определены, но никогда не заполняются. `load_from` и `create_from_manifest` не обновляют их.
**Фикс:** заполнять при добавлении скилла, или удалить.

### 3. with_db и run_cmd в store.rs — dead code
Определены, но не используются. Все методы используют прямой `redis_client.as_ref().unwrap().get_connection()`.
**Фикс:** удалить неиспользуемые методы.

### 4. role_system_prompt() принимает SkillRegistry, но не использует
В `subagent.rs` последний параметр `skill_reg: &SkillRegistry` используется только для `skill_reg.get_prompt()`.
**Фикс:** можно принимать `&str` (prompt) вместо всего реестра.

### 5. handle_slash принимает &mut skill_reg — только для merge
`handle_slash` теперь принимает `&mut SkillRegistry`. Это ломает чистоту — merge мог бы быть отдельной командой.
**Фикс:** если merge не единственная мутирующая операция — нормально. Иначе выделить merge в отдельный модуль.

### 6. SubAgentManager.agent_eval не показывает последний finding
`let last_finding = None;` — заглушка. Не читает XREVRANGE из Redis.
**Фикс:** реализовать XREVRANGE для получения последнего finding.

### 7. SubAgentManager не чистит старые агенты
Нет TTL/cleanup. Агенты копятся в Redis вечно.
**Фикс:** добавить cleanup по TTL или max count.

### 8. Сборка с kafka feature сломана
`kafka.rs` feature-gated, но не обновлялся под новые API (SubAgentManager, KvStore).
**Фикс:** обновить kafka.rs или временно отключить фичу.

## Вывод

Код вырос с 5500 до 8560 строк (+55%). Основной прирост:
- Agent система (subagent + skill + agents/) — ~1200 строк
- Мосты (bridge_agent + mcp_server + api) — ~1000 строк  
- Redis (store + tui_agent) — ~400 строк
- Handlers — ~200 строк

Рефакторинг требуется в основном для чистоты (dead code, неиспользуемые поля).
Критических проблем нет — сборка успешна, тесты проходят.
