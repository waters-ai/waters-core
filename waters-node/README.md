# 🌊 WATERS Node v0.4

**Агентский интернет.** P2P-рой агентов с Redis, голосовым управлением, медиа-мостами.

```bash
# Быстрый старт:
wget https://github.com/waters-ai/waters-core/releases/download/v0.4/waters-node-v0.4.tar.gz
tar xzf waters-node-v0.4.tar.gz
cd waters-node-v0.4
export DEEPSEEK_API_KEY=sk-xxxx
./bin/waters-node --port 42069
# → http://localhost:42069
```

**Возможности:** 25+ агентов, Redis, P2P, голос 🎤, MCP, импорт TUI/Claude/Cursor, слияние агентов, медиа-мосты NDI/OBS/HDMI, рейтинг, YASA-досмотр, групповой чат.

**Установка:** [INSTALL.md](INSTALL.md)

## Быстрый старт

```bash
# Linux
wget https://kapelka.h2o-mining.space/download/waters-node-v0.3.0-linux-x64.tar.gz
tar xzf waters-node-v0.3.0-linux-x64.tar.gz
cd linux
./install.sh

# Windows
# Скачать waters-node-v0.3.0-windows-x64.zip
# Распаковать, запустить PowerShell от администратора:
# .\install.ps1
```

## Архитектура

```
Каждая нода — маршрутизатор с 6 DTN-линками.
На каждом линке своя политика (always-on / periodic).
Агенты ходят в обе стороны: скилы + журнал ± память ± бортовая LLM.
```

### Режимы задачи (как OpenCode)
| Режим | Описание |
|-------|----------|
| **Plan** | Планирование: сбор требований, декомпозиция |
| **Execute** | Исполнение: агенты работают, группа синтезирует |
| **Stop** | Остановка: задача приостановлена |

### Режимы группы
| Режим | Когда | Поведение |
|-------|-------|-----------|
| **Storm** | Срочно, параллельно | Все агенты работают параллельно |
| **Hunt** | Поиск данных | Scout-ы по всем направлениям, группа усиливает лучшее |
| **Synthesis** | Анализ | Coordinator синтезирует findings |
| **Focus** | Один исполнитель | Один агент, остальные наблюдают |
| **Watch** | Мониторинг | Фоновый поиск, триггер → Storm |

## Структура агента

```
Агент = Скилы + Бортовой журнал
      + Onboard LLM (опц., маленькая 1-3B)
      + Снапшот памяти по задаче
      + Состояние (ключ-значение)
```

Режимы передачи: **Full** (всё), **Standard** (скилы+журнал), **Lite** (только манифест).

## Развёртывание сцены: сервер + дом + дача

### Сервер (171.22.180.177) — хаб, всегда онлайн
```bash
waters-node --name server-177
```
Ждёт подключений. Всегда онлайн, fixed_ip=true.

### Дом (локально) — домашняя нода
```bash
waters-node --name home-node --connect 171.22.180.177:42069
```
Подключается к серверу. Может иметь свои агенты и бриджи.

### Дача (локально) — третья нода
```bash
waters-node --name dacha-node --connect 171.22.180.177:42069
```
Подключается к серверу.

### Создание группы
В чате любой ноды:
```
chat создай группу meteorite-hunt
chat пригласи ноду 171.22.180.177
chat создай задачу найти метеориты за 24ч
режим выполнение
```

## DTN-линки (концепт)

```toml
[links.neptune]
mode = "periodic"
rtt_ms = 500_000
[links.neptune.policy]
tasks = "duplex"
agents = "full"
findings = "duplex"
```

## Чат-команды

```
/help              — справка
/skills            — список скиллов
/agents            — список агентов
/bridges           — список бриджей
/status            — состояние ноды
/mode <режим>      — переключить режим (план/сбор/выполнение/стоп/журнал)
/connect <ip>      — подключиться к пиру
/chat <текст>      — отправить LLM
/exit              — завершение
```

Также на русском:
```
режим план|сбор|выполнение|стоп|журнал
chat создай группу <имя>
chat создай задачу <описание>
статус
```

## Сборка из исходников

```bash
git clone https://github.com/waters-ai/waters-core
cd waters-core/waters-node
cargo build --release
./target/release/waters-node
```

Для Windows:
```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64
cargo build --release --target x86_64-pc-windows-gnu
```
