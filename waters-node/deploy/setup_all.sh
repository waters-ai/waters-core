#!/bin/bash
# WATERS — установка на 3 ноды

NODE1_IP="${1:-87.242.102.177}"  # 177 сервер
NODE2_IP="${2}"                   # дача (Windows)
NODE3_IP="${3}"                   # дом (Ubuntu)

echo "═══ WATERS v0.4 — установка ═══"
echo ""

# 177 — сервер
echo "→ [177] Установка на $NODE1_IP..."
scp -i ~/new_server_key waters-node deploy/177/config.toml deploy/177/bridges.json constructor@$NODE1_IP:~/waters-node/
rsync -avz -e "ssh -i ~/new_server_key" agents/ skills/ constructor@$NODE1_IP:~/waters-node/ 2>/dev/null | tail -1

# Windows — через SCP (если есть SSH) или вручную
echo ""
echo "→ [WIN] Для установки на Windows:"
echo "  1. Скачать waters-node.exe (Rust → Windows кросс-компиляция)"
echo "  2. Создать папку C:\\waters-node\\"
echo "  3. Скопировать waters-node.exe, bridges.json, config.toml"
echo "  4. Распаковать agents/ и skills/"
echo "  5. Запустить: waters-node.exe --port 42070"
echo ""

# Ubuntu
echo "→ [UBUNTU] Установка на $NODE3_IP..."
scp waters-node deploy/ubuntu/config.toml deploy/ubuntu/bridges.json user@$NODE3_IP:~/waters-node/
rsync -avz agents/ skills/ user@$NODE3_IP:~/waters-node/ 2>/dev/null | tail -1

echo ""
echo "═══ Связываем ноды ═══"
echo ""
echo "На каждой ноде запустить:"
echo "  export DEEPSEEK_API_KEY=sk-xxxxxxxxxxxx"
echo "  export REDIS_URL=redis://127.0.0.1:6379"
echo "  ./waters-node --port 42069"
echo ""
echo "Соединить:"
echo "  [177] /connect <win_ip>:42070"
echo "  [177] /connect <ubuntu_ip>:42071"
echo ""
echo "Проверить:"
echo "  [177] status"
echo "  Должно быть: Peers: 2"
echo ""
echo "Сценарий:"
echo "  /agent create waters-scout"
echo "  /assign agent.1 собери данные о погоде на Марсе через DeepSeek"
echo "  /say 1 привет с 177 сервера"
