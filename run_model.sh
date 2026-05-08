#!/usr/bin/env bash
# run_model.sh — Запуск OpenCode с выбором модели и набора агентов
#
# Поддерживает 3 модели (Ollama 7B/14B через SSH-туннель, DeepSeek V4 через API),
# 4 предустановленных набора агентов (Троицы) и одиночных агентов.
#
# Синтаксис:
#   ./run_model.sh -m <3|7|14|4> -s <arch|build|meaning|safe>   # набор агентов
#   ./run_model.sh -m <3|7|14|4> -a <agent_name>                 # одиночный агент
#
# Модели (-m):
#   3  — Ollama Qwen 2.5 3B (через SSH-туннель на 237:11434)
#   7  — Ollama Qwen 2.5 7B (через SSH-туннель на 237:11434)
#   14 — Ollama Qwen 2.5 14B (через SSH-туннель на 237:11435)
#   4  — DeepSeek V4 Flash (через API, ключ из .env)
#
# Наборы агентов (-s):
#   arch    — Троица Структуры: architect(14b) + constructor(14b) + integrator(14b)
#   build   — Сборка: constructor(14b) + architect(7b) + integrator(7b)
#   meaning — Троица Смысла: lawkeeper(14b) + director(7b) + keeper(7b)
#   safe    — Надзор (микс): keeper(14b) + lawkeeper(7b) + architect(7b)
#
# Одиночные агенты (-a):
#   architect | constructor | integrator | director | lawkeeper | keeper
#
# Примеры:
#   ./run_model.sh -m 14 -s arch            # Троица Структуры на 14B
#   ./run_model.sh -m 7 -a constructor      # Конструктор на 7B
#   ./run_model.sh -m 4 -s arch             # Троица Структуры на DeepSeek V4
#   ./run_model.sh -m 4 -a architect        # Архитектор на DeepSeek V4
#   ./run_model.sh -m 7 -a integrator       # Интегратор на 7B
#
# Как это работает:
#   1. Создаёт профиль в /tmp/opencode/{set_name|agent_name}/
#   2. Копирует нужный opencode-*.json, подменяет модель
#   3. Для DeepSeek — добавляет провайдера deepseek в конфиг
#   4. Создаёт симлинк agents/ → репозиторий (для резолва {file:./agents/...})
#   5. Для одиночного агента — подкладывает его AGENTS.md
#   6. Проверяет/запускает SSH-туннели для Ollama
#   7. Открывает OpenCode TUI в папке профиля
#
# Важно: sub-agent'ы в наборах имеют свои закреплённые модели в конфиге.
#   Флаг -m меняет модель ТОЛЬКО для основной сессии OpenCode.
#   Sub-agent'ы используют модель, прописанную в их секции agent.{name}.model.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
WORK_BASE="/tmp/opencode"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'

usage() {
    echo -e "${CYAN}run_model.sh${NC} — запуск OpenCode с выбором модели и набора агентов"
    echo ""
    echo "Синтаксис:"
    echo "  $0 -m ${YELLOW}<7|14|4>${NC} -s ${YELLOW}<arch|build|meaning|safe>${NC}   # набор агентов"
    echo "  $0 -m ${YELLOW}<7|14|4>${NC} -a ${YELLOW}<agent>${NC}                       # одиночный агент"
    echo ""
echo -e "${GREEN}Модели${NC} (-m):"
echo "  3  — Ollama Qwen 2.5 3B   (:11434, туннель → 237)"
echo "  7  — Ollama Qwen 2.5 7B   (:11434, туннель → 237)"
echo "  14 — Ollama Qwen 2.5 14B  (:11435, туннель → 237)"
echo "  4  — DeepSeek V4 Flash    (API, ключ из .env)"
    echo ""
    echo -e "${GREEN}Наборы агентов${NC} (-s):"
    echo "  arch    — architect(14b) + constructor(14b) + integrator(14b)"
    echo "  build   — constructor(14b) + architect(7b) + integrator(7b)"
    echo "  meaning — lawkeeper(14b) + director(7b) + keeper(7b)"
    echo "  safe    — keeper(14b) + lawkeeper(7b) + architect(7b)"
    echo ""
    echo -e "${GREEN}Одиночные агенты${NC} (-a):"
    echo "  architect | constructor | integrator | director | lawkeeper | keeper"
    echo ""
    echo -e "${YELLOW}Примеры${NC}:"
    echo "  $0 -m 14 -s arch            # Троица Структуры на 14B"
    echo "  $0 -m 7 -a constructor      # Конструктор на 7B"
    echo "  $0 -m 4 -s arch             # Троица Структуры на DeepSeek"
    echo "  $0 -m 4 -a architect        # Архитектор на DeepSeek"
    exit 1
}

MODE=""; SET=""; AGENT=""
while getopts "m:s:a:h" opt; do
    case $opt in
        m) MODE="$OPTARG" ;;
        s) SET="$OPTARG" ;;
        a) AGENT="$OPTARG" ;;
        h|*) usage ;;
    esac
done

if [ -z "$MODE" ] || { [ -z "$SET" ] && [ -z "$AGENT" ]; }; then usage; fi
if [ -n "$SET" ] && [ -n "$AGENT" ]; then
    echo -e "${RED}Ошибка:${NC} укажите только -s ИЛИ -a"; usage
fi

case "$MODE" in 3|7|14|4) ;; *) echo -e "${RED}Ошибка:${NC} модель '$MODE' не поддерживается"; usage ;; esac

case "$MODE" in
    3)  MODEL_STR="ollama-3b/qwen2.5:3b" ;;
    7)  MODEL_STR="ollama-7b/qwen2.5:7b" ;;
    14) MODEL_STR="ollama-14b/qwen2.5:14b" ;;
    4)  MODEL_STR="deepseek/deepseek-v4-flash" ;;
esac

if [ -n "$SET" ]; then
    case "$SET" in
        arch|build|meaning|safe) ;;
        *) echo -e "${RED}Ошибка:${NC} набор '$SET' не существует"; usage ;;
    esac
    PROFILE="$SET"
    SRC_CONFIG="$REPO_DIR/opencode-${SET}.json"
else
    case "$AGENT" in
        architect|constructor|integrator|director|lawkeeper|keeper) ;;
        *) echo -e "${RED}Ошибка:${NC} агент '$AGENT' не существует"; usage ;;
    esac
    PROFILE="$AGENT"
    SRC_CONFIG="$REPO_DIR/opencode-test-agent.json"
fi

WORK_DIR="$WORK_BASE/$PROFILE"

# ─── Загрузка окружения ──────────────────────────────────────────────────────
if [ -f "$REPO_DIR/.env" ]; then
    set -a; source "$REPO_DIR/.env"; set +a
fi

# ─── Проверка SSH-туннелей ────────────────────────────────────────────────────
if [ "$MODE" = "3" ] || [ "$MODE" = "7" ] || [ "$MODE" = "14" ]; then
    case "$MODE" in
        3|7) PORT="11434" ;;
        14)  PORT="11435" ;;
    esac
    if ! curl -s --max-time 2 "http://localhost:$PORT/api/tags" >/dev/null 2>&1; then
        echo -e "${YELLOW}Туннель на :$PORT не отвечает, запускаю...${NC}"
        ssh -o StrictHostKeyChecking=no -o ExitOnForwardFailure=yes \
            -fNL "$PORT":localhost:"$PORT" ubuntu@171.22.180.237 2>/dev/null || true
        sleep 1
    fi
    echo -e "${GREEN}✅ Ollama :$PORT — доступен${NC}"
fi

# ─── Создание профиля ─────────────────────────────────────────────────────────
mkdir -p "$WORK_DIR"
ln -sfn "$REPO_DIR/agents" "$WORK_DIR/agents"
cp "$SRC_CONFIG" "$WORK_DIR/opencode.json"

# ─── Настройка модели в конфиге ──────────────────────────────────────────────
python3 << EOF
import json, os

config_path = os.path.join("${WORK_DIR}", "opencode.json")
with open(config_path) as f:
    cfg = json.load(f)

cfg['model'] = "${MODEL_STR}"

if "${MODE}" == "4":
    cfg.setdefault('provider', {})['deepseek'] = {
        'name': 'DeepSeek (WATERS Primary)',
        'apiKey': '{env:DEEPSEEK_API_KEY}',
        'models': {
            'deepseek-v4-flash': {'name': 'DeepSeek V4 Flash'}
        }
    }

with open(config_path, 'w') as f:
    json.dump(cfg, f, indent=2, ensure_ascii=False)
EOF

echo -e "${GREEN}✅ Профиль создан: ${WORK_DIR}${NC}"
echo -e "   Конфиг: ${CYAN}opencode.json${NC}"
echo -e "   Модель: ${YELLOW}${MODEL_STR}${NC}"

# ─── Для одиночного агента — подложить AGENTS.md ─────────────────────────────
if [ -n "$AGENT" ]; then
    AGENT_FILE="$REPO_DIR/agents/${AGENT}_AGENTS.md"
    if [ -f "$AGENT_FILE" ]; then
        cp "$AGENT_FILE" "$WORK_DIR/AGENTS.md"
        echo -e "   AGENTS.md: ${CYAN}${AGENT}_AGENTS.md${NC}"
    else
        echo -e "${YELLOW}⚠ AGENTS.md для $AGENT не найден${NC}"
    fi
fi

# ─── Запуск ────────────────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  Запуск OpenCode в ${WORK_DIR}${NC}"
echo -e "${CYAN}  Модель: ${MODEL_STR}${NC}"
if [ -n "$SET" ]; then
    echo -e "${CYAN}  Набор: ${SET}${NC}"
else
    echo -e "${CYAN}  Агент: ${AGENT}${NC}"
fi
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo ""

cd "$WORK_DIR" && exec opencode
