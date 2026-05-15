# Анализ TUI: что брать, что адаптировать, что делать своё

> **Версия:** 1.0
> **Дата:** 2026-05-15
> **Контекст:** waters-node — это НЕ 6 копий TUI. Это P2P-сеть для кросс-доменной коллеборации.
> **Метафора:** TUI = сварочный аппарат для одного сварщика. waters-node = мастерская, где
> астроном, писатель, химик и программист работают над разными проектами, делясь инструментами.

---

## 0. Ключевое отличие: TUI vs waters-node

```
TUI                          waters-node
──────────────────────────   ──────────────────────────
Один пользователь            Много пользователей
Один LLM провайдер           Каждая нода = свой LLM (до 6)
Только софт-разработка       Любая деятельность
Сам себе выбирает инструменты  Группа делит ресурсы по качеству
Файлы на локальной FS        Каждый приносит свои данные
Нет понятия "участник"       Роли, группы, cross-domain collaboration
Нет P2P                      P2P-рой — основа архитектуры
Чат с LLM = интерфейс        Чат = интерфейс, но распределённый
```

**water-node — это не "TUI для шестерых".**  
Это платформа, где:
- **Астроном А** сидит на ноде 1 (LLM: DeepSeek, ресурс: телескоп, база: NASA)
- **Писатель Б** сидит на ноде 2 (LLM: Ollama, ресурс: Obsidian, база: книги)
- **Исследователь В** сидит на ноде 3 (LLM: Claude API, ресурс: DuckDB, база: метеориты)
- **Разработчик Г** сидит на ноде 4 (LLM: DeepSeek Pro, ресурс: GitHub, база: код)

Они объединяются в **6 разных групп** под разные проекты.  
Один человек может участвовать в нескольких группах, отдавая в каждую свои ресурсы.

---

## 1. File Operations (работа с файлами)

### TUI

| Инструмент | Назначение |
|-----------|-----------|
| `read_file` | Чтение UTF-8, PDF авто-экстракт |
| `write_file` | Создание/перезапись |
| `edit_file` | Search-and-replace |
| `apply_patch` | Diff-патчи |
| `list_dir` | gitignore-aware листинг |
| `retrieve_tool_result` | Чтение старых больших выводов |
| `handle_read` | Проекции из var_handle контекстов |

### waters-node: НУЖНО, НО ИНАЧЕ

| Инструмент | Решение | Почему |
|-----------|---------|--------|
| `read_file` | **Берём из TUI** | Универсально, нужно всем |
| `write_file` | **Берём из TUI** | Базово |
| `edit_file` | **Не нужно** | waters-node — не код-редактор. У нас есть `write_file` |
| `apply_patch` | **Не нужно** | Кто пишет код — использует свой git, не наш редактор |
| `list_dir` | **Берём из TUI** | Нужен для навигации |
| `retrieve_tool_result` | **Своё** | У нас WAL + gossip, не файловый spillover |
| `handle_read` | **Своё** | У нас агентские журналы и каналы, не var_handle |

### Вывод

```
Берём: read_file, write_file, list_dir (скопировать код из TUI)
Не нужно: edit_file, apply_patch (это для софт-разработки)
Своё: retrieve_tool_result → WAL-replay, handle_read → journal-read
```

---

## 2. Search (поиск)

### TUI

| Инструмент | Назначение |
|-----------|-----------|
| `grep_files` | Regex внутри workspace |
| `file_search` | Fuzzy имя файла |
| `web_search` | Bing/DDG/Tavily/Bocha |
| `fetch_url` | HTTP GET страницы |

### waters-node: НУЖНО ПО-СВОЕМУ

| Инструмент | Решение | Почему |
|-----------|---------|--------|
| `grep_files` | **Своё** | У нас поиск идёт по всей сети, не только локально. Через gossip + bridges |
| `file_search` | **Не нужно** | В P2P-сети нет одного workspace |
| `web_search` | **УЖЕ ЕСТЬ** (bridge.rs) | DuckDuckGo/Yandex/Baidu — сделано |
| `fetch_url` | **Берём из TUI** | Универсально |

### Новое: распределённый поиск

```
В TUI: grep_files ищет в локальной папке.
У нас: 
  — поиск по задачам группы (gossip)
  — поиск по находкам (findings)
  — поиск по бриджам всех нод (распределённый web_search)
  — поиск по личным базам (NotebookLM, Obsidian, ChromaDB)
```

---

## 3. Shell (команды)

### TUI

| Инструмент | Назначение |
|-----------|-----------|
| `exec_shell` | Запуск команды (foreground/cancellable) |
| `exec_shell_wait` | Poll background |
| `exec_shell_interact` | Stdin в фоновую |
| `exec_shell_cancel` | Отмена |
| `task_shell_start/wait` | durable background |

### waters-node: НУЖНО, НО УПРОЩЁННО

| Инструмент | Решение | Почему |
|-----------|---------|--------|
| `exec_shell` | **Берём из TUI** | Базовый запуск команд |
| `exec_shell_wait` | **Не нужно** | В чат-интерфейсе нет polling |
| `exec_shell_interact` | **Не нужно** | Интерактивные команды — через чат |
| `exec_shell_cancel` | **Берём** | Нужно уметь отменять |
| `task_shell_*` | **Своё** | У нас task.rs — своя durable система |

### Вывод

```
Берём: exec_shell (код из TUI, упростить), exec_shell_cancel
Не нужно: exec_shell_wait, exec_shell_interact (chat-native UX)
Своё: durable задачи через task.rs + journal.rs
```

---

## 4. Sub-Agent система (главное отличие)

### TUI (7 ролей для софт-разработки)

| Роль | Для чего |
|------|----------|
| `general` | "сделай всё" |
| `explore` | "найди код" |
| `plan` | "спроектируй" |
| `review` | "проверь код" |
| `implementer` | "напиши код" |
| `verifier` | "запусти тесты" |
| `custom` | "только эти инструменты" |

### waters-node: ДРУГИЕ РОЛИ

**В TUI все роли — для производства софта.**  
**У нас роли = профессии участников сети:**

```
Роль              Профессия     Что даёт группе
───────────────── ───────────   ──────────────────────────
collector         Исследователь Сбор данных (NASA API, телескоп, сенсоры)
analyst           Аналитик      Обработка, визуализация, выводы
scout-ru          Лингвист      Поиск в русскоязычных источниках
scout-us          Журналист     Поиск в англоязычных источниках
scout-cn          Синолог       Поиск в китайских источниках
synthesizer       Редактор      Синтез findings в отчёты
archivist         Библиотекарь  Сохранение, индексация, каталогизация
coordinator       Проджект-менеджер | Координация, назначения, контроль
developer         Программист   Код, git, деплой (если нужно)
stargazer         Астроном      Телескоп, спектры, координаты
```

**Один участник сети может быть в нескольких ролях.**  
Астроном = stargazer + collector. Писатель = synthesizer + archivist.

### Жизненный цикл sub-agent

```
TUI: agent_open → agent_eval → agent_close
(всё внутри одного процесса, in-process mpsc)

У нас: agent_open → agent_work → agent_finding → agent_journal
(агент живёт на СВОЕЙ ноде, в группу отдаёт только finding)
```

### Вывод

```
Не берём: роли TUI (они для софта)
Берём: концепцию lifecycle (open → work → close)
Своё: роли под профессии, distributed execution, per-node agents
```

---

## 5. MCP (Model Context Protocol)

### TUI

| Возможность | Описание |
|------------|----------|
| MCP client (stdio) | Запуск MCP-серверов как subprocess |
| MCP config (`~/.deepseek/mcp.json`) | JSON-файл конфигурации |
| Auto-discovery | MCP-серверы находятся при старте |
| MCP tool naming | `mcp_<server>_<tool>` |
| `/mcp` manager | add, enable, disable, remove, validate |
| MCP approval | read-only auto, side-effect approve |
| MCP resources | list/read ресурсы сервера |
| Timeout config | connect/execute/read timeout |

### waters-node: БЕРЁМ ПОЧТИ ВСЁ

MCP — универсальный протокол, не привязан к софт-разработке.
Через MCP можно подключить что угодно: телескоп, базу данных, API.

| Возможность | Решение |
|------------|---------|
| MCP client (stdio) | **✅ Есть** (mcp.rs) |
| MCP config file | **🔴 Доделать** (bridges.json, P0) |
| Auto-discovery | **Своё** (gossip находит MCP-серверы других нод) |
| Tool naming | **Своё** (gossip-пространство имён) |
| MCP manager | **Своё** (через чат, без меню) |
| Approval | **Не нужно** (P2P = свои процессы) |
| Timeout config | **Берём из TUI** |

### Новое: MCP как универсальный шлюз к личным ресурсам

```
TUI: MCP-серверы = внешние инструменты для кодинга
У нас: MCP-серверы = шлюз к личным ресурсам участника сети

Примеры:
  mcp-db-duckdb     — база данных астронома
  mcp-obsidian      — заметки писателя
  mcp-telescope     — управление телескопом
  mcp-nasa          — NASA API исследователя
  mcp-github        — код разработчика
  mcp-notebooklm    — AI-ноутбук аналитика
```

---

## 6. Runtime API (HTTP/SSE)

### TUI

| Эндпоинт | Назначение |
|----------|-----------|
| `GET /health` | Healthcheck |
| Sessions CRUD | `GET/DELETE /v1/sessions` |
| Threads CRUD | `GET/POST/PATCH /v1/threads` |
| Turns | `POST /v1/threads/{id}/turns` |
| Steer/Interrupt | Управление turn |
| SSE Events | `GET /v1/threads/{id}/events` |
| Tasks | `GET/POST /v1/tasks` |
| Automations | Scheduled tasks |
| Usage | Token/cost tracking |
| Skills | MCP introspection |

### waters-node: ДРУГОЙ API

**TUI REST API — для управления одним процессом.**  
**У нас API — для управления распределённой сетью.**

```
waters-node API (порт 42069):
────────────────────────────────
  /health              — статус ноды
  /status              — LLM, бриджи, агенты, группы
  /chat                — отправка сообщения в чат
  /task                — CRUD задач
  /agent               — CRUD агентов
  /group               — CRUD групп
  /bridge              — CRUD бриджей
  /journal             — чтение журналов
  /gossip              — статус P2P-сети
  /events              — SSE-лента событий группы (P1)
```

### Вывод

```
Не берём: REST API TUI (он про управление одним TUI-процессом)
Своё: API распределённой сети (нода как участник роя, не как сервер)
```

---

## 7. Session Management

### TUI

| Возможность | Описание |
|------------|----------|
| Session save/resume | JSON-файлы сессий |
| Thread/Turn/Item | Durable lifecycle |
| Thread fork | Ветвление сессии |
| Crash checkpoint | Pre-turn snapshot |
| Offline queue | Очередь при деградации |
| Context compaction | `/compact` |
| Schema versioning | Forward migration |
| Side-git snapshots | `/restore N` |

### waters-node: НАДО ПО-СВОЕМУ

```
TUI: сессия = история работы с LLM (линейная)
У нас: сессия = группа задач + агенты + находки (сетевая)

Что нужно:
  — WAL-чекипоинты (уже есть в channel.rs)
  — Восстановление после падения (P0)
  — Офлайн-очередь находок (P0)
  — История группы через gossip (P1)
  — schema versioning для данных (P2)

Что не нужно:
  — Thread/Turn/Item model (TUI для одного пользователя)
  — Thread fork (ветвление в P2P = создание новой группы)
  — Context compaction (мы не работаем с 1M контекстом)
  — Side-git snapshots (у нас каждый сам хранит свои данные)
```

---

## 8. Skills (скиллы)

### TUI

| Возможность | Описание |
|------------|----------|
| SKILL.md (YAML frontmatter + markdown) | Формат |
| Discovery: `~/.deepseek/skills/` и др. | Где лежат |
| `/skills` listing | Просмотр |
| Skill install from GitHub | `skill install github:<owner>/<repo>` |
| Bundled skills | `skill-creator`, `delegate`, etc. |

### waters-node: УЖЕ ЛУЧШЕ

**У нас Skills 2.0** — расширенный формат с:
- `skill.json` (манифест: теги, зависимости, бриджи, ресурсы группы)
- `bridges: [...]` — явная привязка к внешним сервисам
- `group_resources: {...}` — запрос ресурсов группы
- `bookmarks: [...]` — тесты

### Вывод

```
Берём из TUI: SKILL.md frontmatter + markdown (совместимость)
Уже лучше TUI: skill.json, bridges, group_resources
Не нужно: установка с GitHub (у нас gossip peers делятся скиллами)
Своё: конвертер TUI → наш формат (уже есть в skill.rs)
```

---

## 9. Security / Sandbox

### TUI

| Возможность | Описание |
|------------|----------|
| macOS Seatbelt | Песочница для shell |
| Linux Landlock | Linux-песочница |
| Windows Job Objects | Windows-песочница |
| Approval flow | Plan/Agent/YOLO |
| Keyring integration | API-ключи в OS keyring |
| Managed config | `/etc/deepseek/managed_config.toml` |

### waters-node: ДРУГАЯ МОДЕЛЬ БЕЗОПАСНОСТИ

```
TUI: защита одного пользователя от самого себя (sandbox + approval)
У нас: защита участников сети друг от друга (токены + ACL)

Что нужно:
  — Токены групп (уже есть в gossip.rs)
  — Чат-аппрув подключений (P0)
  — ACL на ресурсы (чей bridge можно использовать)
  — ed25519 для identity ноды (запланировано в crypto.rs)

Что не нужно:
  — Sandbox (каждый сам отвечает за свою ноду)
  — Approval flow (чат = естественный approval)
  — Keyring (конфиг-файл ноды, не OS keyring)
  — Managed config (нода сама себе хозяин)
```

---

## 10. Что реально тянем из TUI (список для Конструктора)

### Код, который можно скопировать/адаптировать

| Модуль TUI | Файл | Что брать |
|-----------|------|-----------|
| `read_file` | `tools/file.rs` | Чтение UTF-8 + PDF pdftotext |
| `write_file` | `tools/file.rs` | Запись с резервной копией |
| `list_dir` | `tools/file.rs` | gitignore-aware листинг |
| `exec_shell` | `tools/shell.rs` | Запуск команд с timeout |
| `exec_shell_cancel` | `tools/shell.rs` | Отмена команд |
| `web_search` | `tools/search.rs` | DuckDuckGo/Bing парсер |
| `fetch_url` | `tools/search.rs` | HTTP GET + HTML→text |
| `MCP client` | `mcp.rs` | stdio JSON-RPC (уже есть) |
| `SKILL.md parser` | `skills.rs` | YAML frontmatter (уже есть) |

### Концепции, которые адаптируем

| Концепция TUI | Как адаптируем |
|--------------|----------------|
| Sub-agent lifecycle | Open → Work → Close, но распределённо |
| Tool registry | Брать типы, схемы, но наш набор инструментов |
| Session persistence | WAL + channel.rs вместо JSON-файлов |
| LLM client | Streaming + SSE + fallback |
| Crash recovery | Checkpoint + offline queue |
| MCP config | bridges.json вместо mcp.json |

### Что делаем полностью своё

| Компонент | Почему своё |
|-----------|-------------|
| **Gossip P2P** (mDNS + TCP) | В TUI нет сети |
| **Groups** (токен + ACL + shared resources) | В TUI нет групп |
| **Channels** (WAL + импорт/экспорт) | В TUI in-process mpsc |
| **Task resource binding** | Per-task бриджи, базы, git |
| **Group quality engine** | Распределение ресурсов по качеству |
| **Agent journals** | Per-task, per-agent логи |
| **Voice bridge** | Whisper STT |
| **Convo chat UI** | LLM как интерфейс, без меню |
| **Personal resources** | NotebookLM, Obsidian, свои базы |
| **Bridge registry** | 3-layer: builtin / MCP / personal |
| **Autonomy L0-L4** | Работа без связи |
| **DTN** | Задержки и разрывы |
| **Военный Kafka** | Feature-gate для централизации |

---

## 11. Сводная таблица

| Компонент | TUI | waters-node | Решение |
|-----------|-----|-------------|---------|
| **File tools** | 7 tools | 3 tools | **Берём** read/write/list_dir |
| **Search tools** | 4 tools | 3 tools + distributed | **Берём** web_search/fetch_url, **своё** distributed search |
| **Shell tools** | 6 tools | 2 tools | **Берём** exec_shell + cancel |
| **Sub-agents** | 7 ролей (code) | ∞ ролей (professions) | **Своё** — роли под профессии |
| **MCP** | Full client + manager | Full client + gossip | **Берём** client, **своё** manager |
| **REST API** | 40+ endpoints | 10 endpoints | **Своё** — network-native |
| **Sessions** | Thread/Turn/Item | WAL + gossip | **Своё** — распределённые |
| **Skills** | SKILL.md | Skills 2.0 | **Уже лучше** TUI |
| **Sandbox** | OS-level | ACL + tokens | **Своё** — P2P security |
| **LLM** | DeepSeek only | 6 LLM на 6 нод | **Своё** — per-node providers |
| **Bridges** | MCP only | 3-layer | **Своё** — уникальная фича |
| **P2P** | Нет | mDNS + TCP gossip | **Своё** — основа архитектуры |
| **Groups** | Нет | Токен + ACL + shared | **Своё** — core concept |
| **Chat UI** | TUI (ratatui) | Convo + LLM | **Своё** — chat-native |
| **Quality engine** | Нет | Group resource boost | **Своё** — уникальная фича |

---

## 12. Итог: waters-node — это НЕ "6 TUI в одной коробке"

**TUI** — инструмент для одного разработчика. Один человек, один LLM, один workspace, софт.

**waters-node** — P2P-сеть участников с разными профессиями. Каждый приносит свои ресурсы,
группы формируются под проекты, ресурсы распределяются по качеству результата.

```
TUI:         [разработчик] → код
             ↓
waters-node: [астроном] → телескоп + NASA данные
             [писатель] → Obsidian + книги
             [исследователь] → DuckDB + метеориты
             [разработчик] → GitHub + код
             [журналист] → поиск + синтез
             [аналитик] → визуализация + выводы
             ↓
             6 групп, разные проекты, общие ресурсы
             ↓
             Результат: НЕ софт, а НАХОДКИ (findings)
             Отчёты, данные, статьи, код — всё что угодно
```

**waters-node берёт из TUI только базовые инструменты (файлы, шелл, MCP).**  
**Всё остальное — своё, под P2P-коллеборацию.***
