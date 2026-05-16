# Установка WATERS v0.4

---

## 📦 Ubuntu / Linux

```bash
# 1. Скачать дистрибутив
wget https://github.com/waters-ai/waters-core/releases/download/v0.4/waters-node-v0.4.tar.gz
tar xzf waters-node-v0.4.tar.gz
cd waters-node-v0.4

# 2. Установить Redis (если нет)
sudo apt update && sudo apt install -y redis-server
sudo systemctl start redis

# 3. Настроить API-ключ DeepSeek
export DEEPSEEK_API_KEY=sk-xxxxxxxxxxxxxxxx

# 4. Запустить
./bin/waters-node --port 42069
# Или с авто-подключением к мастеру 177:
# ./connect-177.sh

# 5. Открыть в браузере
# http://localhost:42069
```

---

## 🪟 Windows

```powershell
# 1. Установить Redis (через WSL или Memurai)
# Вариант A: WSL (рекомендуется)
wsl --install -d Ubuntu
wsl sudo apt update && sudo apt install -y redis-server
wsl sudo redis-server --daemonize yes

# Вариант B: Memurai (нативный Redis для Windows)
# https://www.memurai.com/ — скачать и установить

# 2. Скачать дистрибутив
# https://github.com/waters-ai/waters-core/releases/download/v0.4/waters-node-v0.4.zip
# Распаковать в C:\waters-node\

# 3. Открыть PowerShell (Администратор)
cd C:\waters-node\waters-node-v0.4

# 4. Настроить API-ключ и Redis
$env:DEEPSEEK_API_KEY = "sk-xxxxxxxxxxxxxxxx"
$env:REDIS_URL = "redis://127.0.0.1:6379"

# 5. Запустить
.\bin\waters-node.exe --port 42070

# 6. Подключиться к мастеру 177
# В консоли ноды написать:
# /connect 87.242.102.177:42069

# 7. Открыть в браузере
# http://localhost:42070
```

---

## ☁️ Сервер 177 (Ubuntu)

```bash
# Уже установлено. Запуск:
ssh constructor@87.242.102.177
cd ~/waters-node
export DEEPSEEK_API_KEY=sk-xxxxxxxxxxxx
./start.sh
```

---

## 🧪 Быстрый старт после запуска

После запуска ноды введи в консоли:

```
/connect <ip_другой_ноды>:<port>     # соединить ноды
/agent create waters-scout            # создать агента
/assign agent.1 найти метеориты       # дать задачу
/say 1 привет всем                    # написать в чат группы
/agent list                           # список агентов
/screen waters-scout                  # досмотр безопасности
/top                                  # топ по рейтингу
/rating waters-scout                  # рейтинг агента
```

---

## 🌐 Веб-дашборд

Открыть в браузере: `http://localhost:<port>`

Возможности:
- 💬 Чат с ассистентом (текст)
- 🎤 Голосовой ввод (Chrome/Edge) — кнопка 🎤
- 🔊 Озвучивание ответов — кнопка 🔇/🔊
- 📡 Статус: peers, redis, uptime
- 🔄 SSE-стриминг токенов в реальном времени

---

## 📋 Требования

| Компонент | Минимум | Рекомендуется |
|-----------|---------|---------------|
| CPU | 1 ядро | 2 ядра |
| RAM | 256 MB | 512 MB |
| Redis | 6.x | 7.x |
| LLM | DeepSeek API | DeepSeek + Ollama |
| Браузер | Chrome 90+ | Chrome 120+ |
