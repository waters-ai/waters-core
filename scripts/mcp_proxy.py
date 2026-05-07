#!/usr/bin/env python3
"""
mcp_proxy.py — MCP-фасад в DMZ-зоне WATERS

Принимает запросы от агентов, проверяет Redis-кэш,
выполняет внешние запросы (поиск, AI), сохраняет в ChromaDB.

Транспорт: HTTP JSON-RPC 2.0
Порт: 8080

Usage:
    python scripts/mcp_proxy.py [--host HOST] [--port PORT] [--chroma-host HOST]
"""

import argparse
import hashlib
import json
import logging
import os
import re
import sys
import time
import uuid
from datetime import datetime, timezone
from http.server import HTTPServer, BaseHTTPRequestHandler
from pathlib import Path
from urllib.request import urlopen, Request
from urllib.error import URLError

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
)
logger = logging.getLogger("mcp-proxy")


# ────────────────────────────────────────────
# Конфигурация
# ────────────────────────────────────────────

REDIS_HOST = os.environ.get("REDIS_HOST", "localhost")
REDIS_PORT = int(os.environ.get("REDIS_PORT", "6379"))
CHROMA_HOST = os.environ.get("CHROMA_HOST", "localhost")
CHROMA_PORT = int(os.environ.get("CHROMA_PORT", "8000"))
KAFKA_BOOTSTRAP = os.environ.get("KAFKA_BOOTSTRAP", "localhost:9092")
RATE_LIMIT = 100  # req/min
TIMEOUT = 30  # seconds

ALLOWED_APIS = ["exa_search", "web_fetch", "arxiv", "deepseek_chat"]
ALLOWED_EXTERNAL_AI = ["claude", "gpt", "ollama"]

# TTL для разных типов кэша
TTL = {
    "ai": 3600,
    "search": 21600,
    "config": 86400,
    "schema": 604800,
    "default": 86400,
}


# ────────────────────────────────────────────
# Redis клиент (минимальная реализация через TCP)
# ────────────────────────────────────────────

class RedisClient:
    def __init__(self, host, port):
        self.host = host
        self.port = port

    def _cmd(self, *args):
        import socket
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(5)
        s.connect((self.host, self.port))
        cmd = f"*{len(args)}\r\n"
        for a in args:
            cmd += f"${len(str(a))}\r\n{a}\r\n"
        s.send(cmd.encode())
        resp = s.recv(4096).decode()
        s.close()
        return resp

    def get(self, key):
        resp = self._cmd("GET", key)
        if resp.startswith("$"):
            length = int(resp[1:].split("\r\n")[0])
            if length < 0:
                return None
            return resp.split("\r\n", 1)[1][:length]
        return None

    def setex(self, key, ttl, value):
        self._cmd("SETEX", key, str(ttl), str(value))

    def incr(self, key):
        self._cmd("INCR", key)

    def keys(self, pattern):
        resp = self._cmd("KEYS", pattern)
        if resp.startswith("*"):
            count = int(resp[1:].split("\r\n")[0])
            parts = resp.split("\r\n")
            result = []
            i = 1
            for _ in range(count):
                i += 1
                if i < len(parts):
                    result.append(parts[i])
                    i += 1
            return result
        return []


# ────────────────────────────────────────────
# ChromaDB клиент
# ────────────────────────────────────────────

_chroma_client = None
_collection = None


def get_chroma_collection(name="external_articles"):
    global _chroma_client, _collection
    if _collection is None:
        try:
            import chromadb
            _chroma_client = chromadb.HttpClient(
                host=CHROMA_HOST,
                port=CHROMA_PORT,
            )
            _collection = _chroma_client.get_or_create_collection(
                name=name,
                metadata={"hnsw:space": "cosine"},
            )
        except ImportError:
            logger.warning("ChromaDB не установлен, сохранение в векторную БД пропущено")
            return None
        except Exception as e:
            logger.error(f"Не удалось подключиться к ChromaDB: {e}")
            return None
    return _collection


def generate_embedding(text):
    try:
        from sentence_transformers import SentenceTransformer
        model = SentenceTransformer("all-MiniLM-L6-v2")
        return model.encode([text])[0].tolist()
    except Exception as e:
        logger.warning(f"Не удалось сгенерировать эмбеддинг: {e}")
        return None


def save_to_chromadb(article):
    collection = get_chroma_collection()
    if collection is None:
        return False

    doc_id = article.get("article_id", str(uuid.uuid4())[:16])
    content = article.get("content", "")
    embedding = generate_embedding(content)

    collection.upsert(
        ids=[doc_id],
        documents=[content],
        metadatas=[{
            "title": article.get("title", ""),
            "source": article.get("source", "external"),
            "author": article.get("author", ""),
            "tags": ",".join(article.get("tags", [])),
        }],
        embeddings=[embedding] if embedding else None,
    )
    logger.info(f"Статья сохранена в ChromaDB: {doc_id}")
    return True


# ────────────────────────────────────────────
# Cache helper
# ────────────────────────────────────────────

_redis = None


def get_redis():
    global _redis
    if _redis is None:
        try:
            import redis as redis_lib
            _redis = redis_lib.Redis(host=REDIS_HOST, port=REDIS_PORT, db=0)
        except ImportError:
            _redis = RedisClient(REDIS_HOST, REDIS_PORT)
        except Exception:
            _redis = RedisClient(REDIS_HOST, REDIS_PORT)
    return _redis


def generate_cache_key(source, query):
    normalized = json.dumps(query, sort_keys=True)
    hash_value = hashlib.sha256(normalized.encode()).hexdigest()[:16]
    return f"cache:{source}:{hash_value}"


def check_cache(source, query):
    r = get_redis()
    cache_key = generate_cache_key(source, query)
    try:
        cached = r.get(cache_key)
        if cached:
            if hasattr(r, "incr"):
                r.incr("metrics:cache_hit")
            logger.info(f"Cache HIT: {cache_key}")
            if isinstance(cached, bytes):
                cached = cached.decode()
            return json.loads(cached)
    except Exception as e:
        logger.warning(f"Ошибка чтения кэша: {e}")
    try:
        if hasattr(r, "incr"):
            r.incr("metrics:cache_miss")
    except Exception:
        pass
    return None


def save_cache(source, query, response, ttl=None):
    if ttl is None:
        ttl = TTL.get(source, TTL["default"])
    r = get_redis()
    cache_key = generate_cache_key(source, query)
    try:
        r.setex(cache_key, ttl, json.dumps(response))
        logger.info(f"Cache SET: {cache_key} (TTL={ttl}s)")
    except Exception as e:
        logger.warning(f"Ошибка записи в кэш: {e}")


# ────────────────────────────────────────────
# Внешние запросы (заглушки)
# ────────────────────────────────────────────

def call_exa_search(query):
    logger.info(f"Вызов Exa Search: {query}")
    return {
        "results": [],
        "source": "exa",
        "query": query,
        "note": "Exa API key не настроен",
    }


def call_web_fetch(url):
    logger.info(f"Web Fetch: {url}")
    try:
        req = Request(url, headers={"User-Agent": "WATERS-MCP/1.0"})
        resp = urlopen(req, timeout=TIMEOUT)
        content = resp.read().decode("utf-8", errors="replace")
        content = re.sub(r"<[^>]+>", " ", content)
        content = re.sub(r"\s+", " ", content).strip()
        return {"url": url, "content": content[:10000], "source": "web_fetch"}
    except Exception as e:
        return {"url": url, "error": str(e), "source": "web_fetch"}


def call_arxiv(query):
    logger.info(f"ArXiv Search: {query}")
    return {"results": [], "source": "arxiv", "query": query, "note": "ArXiv API не настроен"}


def call_deepseek_chat(query):
    logger.info(f"DeepSeek Chat: {query}")
    return {"response": "", "source": "deepseek", "note": "DeepSeek API key не настроен"}


def call_external_ai(ai_provider, query):
    logger.info(f"External AI ({ai_provider}): {query}")
    return {"response": "", "provider": ai_provider, "note": f"{ai_provider} API key не настроен"}


# ────────────────────────────────────────────
# Обработчик запросов
# ────────────────────────────────────────────

API_HANDLERS = {
    "exa_search": call_exa_search,
    "web_fetch": call_web_fetch,
    "arxiv": call_arxiv,
    "deepseek_chat": call_deepseek_chat,
}


def handle_request(method, params):
    """Обработка MCP-запроса."""
    trace_id = str(uuid.uuid4())
    timestamp = datetime.now(timezone.utc).isoformat()

    logger.info(f"[{trace_id}] Запрос: {method}, params={params}")

    # 1. Поиск (через разрешённые API)
    if method == "search":
        api = params.get("api", "exa_search")
        if api not in ALLOWED_APIS:
            return {"error": f"API '{api}' не разрешён", "allowed": ALLOWED_APIS}

        query = params.get("query", "")
        cache_key = {"api": api, "query": query}

        # Проверяем кэш
        cached = check_cache("search", cache_key)
        if cached:
            return cached

        # Выполняем запрос
        handler = API_HANDLERS.get(api)
        if handler:
            result = handler(query)
        else:
            result = {"error": f"Неизвестный API: {api}"}

        # Сохраняем в кэш
        save_cache("search", cache_key, result, ttl=TTL["search"])

        # Сохраняем в ChromaDB если есть контент
        if result.get("content") or result.get("results"):
            article = {
                "article_id": str(uuid.uuid4()),
                "title": f"Search: {query[:100]}",
                "content": result.get("content", json.dumps(result.get("results", ""))),
                "source": "external",
                "author": params.get("author", "agent.unknown"),
                "tags": ["web", "search", api],
            }
            save_to_chromadb(article)

        return result

    # 2. Запрос к внешнему AI
    if method == "ai_query":
        provider = params.get("provider", "")
        if provider not in ALLOWED_EXTERNAL_AI:
            return {"error": f"AI '{provider}' не разрешён", "allowed": ALLOWED_EXTERNAL_AI}

        query = params.get("query", "")
        cache_key = {"provider": provider, "query": query}

        # Проверяем кэш
        cached = check_cache("ai", cache_key)
        if cached:
            return cached

        # Выполняем запрос
        result = call_external_ai(provider, query)

        # Сохраняем в кэш
        save_cache("ai", cache_key, result, ttl=TTL["ai"])

        return result

    # 3. Получение конфигурации
    if method == "get_config":
        component = params.get("component", "")
        cache_key = {"component": component}
        cached = check_cache("config", cache_key)
        if cached:
            return cached
        return {"error": f"Конфигурация '{component}' не найдена"}

    # 4. Получение схемы
    if method == "get_schema":
        name = params.get("name", "")
        cache_key = {"name": name}
        cached = check_cache("schema", cache_key)
        if cached:
            return cached
        return {"error": f"Схема '{name}' не найдена"}

    # 5. Сохранение статьи (от агента)
    if method == "save_article":
        article = params.get("article", {})
        article.setdefault("article_id", str(uuid.uuid4()))
        article.setdefault("created_at", timestamp)
        save_to_chromadb(article)
        return {"status": "saved", "article_id": article["article_id"]}

    # 6. Метрики кэша
    if method == "cache_stats":
        r = get_redis()
        try:
            hits = r.get("metrics:cache_hit") or "0"
            misses = r.get("metrics:cache_miss") or "0"
            hits = int(hits.decode() if isinstance(hits, bytes) else hits)
            misses = int(misses.decode() if isinstance(misses, bytes) else misses)
            total = hits + misses
            return {
                "cache_hit": hits,
                "cache_miss": misses,
                "hit_rate": round(hits / total * 100, 1) if total > 0 else 0,
                "total_requests": total,
            }
        except Exception as e:
            return {"error": str(e)}

    return {"error": f"Неизвестный метод: {method}"}


# ────────────────────────────────────────────
# HTTP JSON-RPC 2.0 сервер
# ────────────────────────────────────────────

class MCPHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length)

        try:
            request = json.loads(body)
        except json.JSONDecodeError:
            self._send_error(-32700, "Parse error", 400)
            return

        method = request.get("method", "")
        params = request.get("params", {})
        rpc_id = request.get("id", None)

        try:
            result = handle_request(method, params)
            response = {"jsonrpc": "2.0", "id": rpc_id, "result": result}
            self._send_json(response, 200)
        except Exception as e:
            logger.exception(f"Ошибка обработки: {e}")
            self._send_error(-32603, str(e), 500)

    def do_GET(self):
        if self.path == "/health":
            self._send_json({"status": "ok", "service": "mcp-proxy", "version": "1.0.0"}, 200)
        else:
            self._send_json({"error": "Use POST with JSON-RPC 2.0"}, 404)

    def _send_json(self, data, status=200):
        body = json.dumps(data, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_error(self, code, message, status=500):
        self._send_json({
            "jsonrpc": "2.0",
            "error": {"code": code, "message": message},
            "id": None,
        }, status)

    def log_message(self, format, *args):
        logger.info(f"{self.client_address[0]} - {format % args}")


def main():
    parser = argparse.ArgumentParser(description="MCP-фасад WATERS (DMZ-зона)")
    parser.add_argument("--host", default="0.0.0.0", help="Listen host")
    parser.add_argument("--port", type=int, default=8080, help="Listen port")
    parser.add_argument("--redis-host", default=None, help="Redis host")
    parser.add_argument("--redis-port", type=int, default=None, help="Redis port")
    parser.add_argument("--chroma-host", default=None, help="ChromaDB host")
    parser.add_argument("--chroma-port", type=int, default=None, help="ChromaDB port")
    args = parser.parse_args()

    if args.redis_host:
        global REDIS_HOST
        REDIS_HOST = args.redis_host
    if args.redis_port:
        global REDIS_PORT
        REDIS_PORT = args.redis_port
    if args.chroma_host:
        global CHROMA_HOST
        CHROMA_HOST = args.chroma_host
    if args.chroma_port:
        global CHROMA_PORT
        CHROMA_PORT = args.chroma_port

    logger.info(f"=== MCP-фасад WATERS ===")
    logger.info(f"Listen: {args.host}:{args.port}")
    logger.info(f"Redis: {REDIS_HOST}:{REDIS_PORT}")
    logger.info(f"ChromaDB: {CHROMA_HOST}:{CHROMA_PORT}")
    logger.info(f"Allowed APIs: {ALLOWED_APIS}")
    logger.info(f"Allowed AI: {ALLOWED_EXTERNAL_AI}")
    logger.info("")
    logger.info("JSON-RPC 2.0 методы:")
    logger.info("  search — поиск через разрешённые API")
    logger.info("  ai_query — запрос к внешнему AI")
    logger.info("  get_config — получение конфигурации")
    logger.info("  get_schema — получение JSON-схемы")
    logger.info("  save_article — сохранение статьи в ChromaDB")
    logger.info("  cache_stats — метрики кэша")
    logger.info("")

    server = HTTPServer((args.host, args.port), MCPHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        logger.info("Остановка...")
        server.shutdown()


if __name__ == "__main__":
    main()
