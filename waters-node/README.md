# 🌊 waters-node

**Один бинарник — сеть интеллекта.**

`waters-node` — распределённый рантайм для ИИ-агентов. Как TUI, но для команды.  
Одна команда — и твоя нода готова работать, общаться с другими нодами, создавать группы и решать задачи.

```bash
wget https://kapelka.h2o-mining.space/download/waters-node.tar.gz
tar xzf waters-node.tar.gz
./waters-node
```

Готово. Ты — часть сети.

---

## Зачем это

TUI крут для одного разработчика. Но агенты должны работать вместе.  
`waters-node` даёт то же удобство TUI + сеть, каналы, группы, голосовое управление.

| TUI | waters-node |
|-----|-------------|
| Один разработчик | Команда нод |
| Терминал | Чат и голос |
| In-process агенты | P2P сеть |
| Нет автономии | L0-L4 + DTN |

---

## Быстрый старт

```bash
# Скачать
wget waters-node.tar.gz
tar xzf waters-node.tar.gz

# Запустить (автономно, как TUI)
./waters-node

# Или сразу в сеть
./waters-node --connect 87.242.102.177:42069
```

Первый запуск:
1. Создаётся кошелёк (ed25519 ключ)
2. Нода получает NodeID
3. Открывается порт 42069
4. discovery.v1 канал активен
5. Ты в сети WATERS

Чат управления:
```
> найди все ноды в сети
→ Найдено 142 ноды. 12 в твоей группе.

> создай группу kapelka для аудита
→ Группа kapelka создана. Каналы: commands, data, alerts.

> добавь ноду FxAt...3kE в kapelka
→ Нода добавлена. ACL обновлён.
```

---

## Возможности

### Ядро (как TUI, но лучше)
- Agent Loop — полноценный рантайм агентов
- Sub-агенты — 7 ролей (explore, plan, review, implement, verify, general, custom)
- Инструменты — read_file, write_file, grep_files, exec_shell, list_dir
- MCP Client — подключение любых MCP-серверов
- Сессии — save/resume контекста
- LLM — DeepSeek API или Ollama local
- Автономия L0-L4 — работа без связи днями
- DTN — эмуляция космических задержек

### Сеть (чего нет у TUI)
- NodeID — уникальный идентификатор (UUID / ed25519)
- Каналы — топики с ACL (как радиочастоты)
- Группы — команды, проекты, роли
- Discovery — mDNS + TCP gossip
- P2P синхронизация — без единого центра
- Защита каналов — открытые, закрытые, приватные
- Режимы — P2P (гражданский) / централизованный (военный)

### Управление
- CLI — `waters-node`, `waters-node --connect <ip>`
- Чат — естественный язык через LLM
- Голос — Whisper STT (скоро)
- Web UI — localhost:42069 (скоро)
- TUI — ratatui (скоро)

---

## Архитектура

```
┌─────────────────────────────────────────────────┐
│  waters-node (6MB, один бинарник)                │
│                                                   │
│  Core                      Network                │
│  ├── Agent Loop            ├── Channels (WAL)     │
│  ├── SubAgents (mpsc)      ├── Groups + ACL       │
│  ├── Tools (7 шт)          ├── Gossip (mDNS+TCP)  │
│  ├── Sessions              ├── Discovery          │
│  ├── LLM (DS/Ollama)       ├── NodeID             │
│  ├── MCP Client            └── Port 42069 API     │
│  ├── Autonomy L0-L4                               │
│  └── DTN                                          │
└─────────────────────────────────────────────────┘

Режимы:
  waters-node                → Standalone (как TUI)
  waters-node --connect <ip> → Network (P2P)
```

---

## Требования

- Linux x86_64 (macOS ARM64 скоро)
- Rust не нужен (готовый бинарник)
- Python не нужен
- Docker не нужен
- Kafka не нужен
- Redis не нужен

**Один файл. Никаких зависимостей.**

---

## Сборка

```bash
git clone https://github.com/waters-ai/waters-node.git
cd waters-node
cargo build --release
./target/release/waters-node
```

---

## Лицензия

MIT — открыто для всех.

---

## Дорожная карта

```
v0.2 — standalone как TUI ✅
v0.3 — каналы + группы + P2P 🔴 (текущий спринт)
v0.4 — Web UI + Telegram bridge 🟡
v0.5 — Голосовое управление 🟡
v0.6 — Крипто + FFF 🟡
v1.0 — Стабильная сеть WATERS 🟡
```
