"""config.py — единая конфигурация Scout для всех зон."""

import os

SCOUT_REGION = os.environ.get("SCOUT_REGION", "ru")

CENTRAL_HOST = os.environ.get("WATERS_CENTRAL_HOST", "ubuntu@171.22.180.238")
KAFKA_BROKER = os.environ.get("WATERS_KAFKA_BROKER", "171.22.180.238:9092")
CHROMA_HOST = os.environ.get("WATERS_CHROMA_HOST", "localhost:8000")
REDIS_HOST = os.environ.get("WATERS_REDIS_HOST", "localhost:6379")
OLLAMA_HOST = os.environ.get("WATERS_OLLAMA_HOST", "localhost:11434")

AGENT_ID = "agent.scout.v1"
RAW_BASE = "/home/waters-data/raw"
DB_PATH = "/home/waters-data/scout_state.db"
LOG_PATH = "/home/ubuntu/WATERS/repos/waters-core/logs/scout.log"

HEARTBEAT_INTERVAL = 300
CLEANUP_INTERVAL = 3600
TASK_TIMEOUT = 600
CLEANUP_MAX_DAYS = 7

SEARCH_ENGINES = {
    "ru": ["yacy", "youtube", "yandex_xml", "duckduckgo"],
    "us": ["google_cse", "brave", "yacy", "duckduckgo", "youtube"],
    "cn": ["baidu", "bing", "bilibili", "sogou"],
}

VALIDATION_PRIORITY = ["notebooklm", "yandexgpt", "pass"]

BUDGETS = {
    "ru": {"yandexgpt": 0.10, "yandex_xml": 0.05},
    "us": {"google_cse": 0.05, "brave": 0.05, "gemini": 0.10},
    "cn": {"baidu": 0.05, "bing": 0.05, "qwen": 0.10},
}

DAILY_BUDGET = BUDGETS.get(SCOUT_REGION, BUDGETS["ru"])

FREE_LIMITS = {
    "notebooklm": 50,
    "ddg": 200,
    "youtube": 50,
    "yandex_xml": 10000,
}

RATE_LIMITS = {
    "yacy": {"rate": 2.0, "burst": 5},
    "duckduckgo": {"rate": 0.5, "burst": 2},
    "youtube": {"rate": 0.2, "burst": 1},
    "page_fetch": {"rate": 0.33, "burst": 3},
    "notebooklm": {"rate": 0.1, "burst": 1},
    "yandexgpt": {"rate": 5.0, "burst": 3},
    "yandex_xml": {"rate": 5.0, "burst": 10},
}

PRIORITY_MAP = {
    "telegram": 10,
    "kafka:integrator": 8,
    "kafka:architect": 7,
    "kafka:director": 6,
    "kafka:keeper": 5,
    "kafka:lawkeeper": 4,
    "kafka:constructor": 3,
    "default": 5,
}

SECRET_DIR = os.path.expanduser("~/.secrets")

YANDEXGPT_API_KEY_FILE = os.path.join(SECRET_DIR, ".secret_yandexgpt_api_key")
YANDEXGPT_BUDGET_FILE = os.path.join(SECRET_DIR, ".secret_yandexgpt_budget")
YANDEX_XML_KEY_FILE = os.path.join(SECRET_DIR, ".secret_yandex_xml_api_key")
YANDEX_XML_BUDGET_FILE = os.path.join(SECRET_DIR, ".secret_yandexxml_budget")
TELEGRAM_TOKEN_FILE = os.path.join(SECRET_DIR, ".secret_telegram_token")
TELEGRAM_USERS_FILE = os.path.join(SECRET_DIR, ".secret_telegram_users")
