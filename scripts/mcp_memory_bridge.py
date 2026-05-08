#!/usr/bin/env python3
"""
mcp_memory_bridge.py — MCP-сервер доступа к базам памяти WATERS

Протокол: MCP stdio (JSON-RPC 2.0 через stdin/stdout)
Инструменты:
  - memory_vector_search    — поиск по ChromaDB
  - memory_vector_save      — сохранение в ChromaDB
  - memory_graph_query      — графовый запрос LightRAG
  - memory_graph_insert     — вставка текста в LightRAG
  - memory_cache_get        — чтение Redis-ключа
  - memory_cache_set        — запись Redis-ключа
  - memory_kafka_send       — отправка сообщения в Kafka
  - memory_kafka_list       — список топиков Kafka
  - memory_kafka_consume    — чтение прошлых сообщений из Kafka
  - memory_state_load       — загрузка состояния агента
  - memory_state_save       — сохранение состояния агента
  - memory_session_save     — сохранение снэпшота сессии
  - memory_session_load     — загрузка снэпшота сессии
  - memory_session_list     — список снэпшотов
  - memory_health           — полный healthcheck всех сервисов
  - memory_stats            — статистика использования

Запуск: OpenCode запускает как subprocess через mcpServers
"""

import asyncio
import json
import logging
import os
import sys
import uuid
from datetime import datetime, timezone

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [memory-bridge] %(message)s",
)
logger = logging.getLogger("memory-bridge")

# Конфигурация из окружения
REDIS_HOST = os.environ.get("REDIS_HOST", "localhost")
REDIS_PORT = int(os.environ.get("REDIS_PORT", "6379"))
CHROMA_HOST = os.environ.get("CHROMA_HOST", "localhost")
CHROMA_PORT = int(os.environ.get("CHROMA_PORT", "8000"))
KAFKA_BOOTSTRAP = os.environ.get("KAFKA_BOOTSTRAP", "localhost:9092")
LIGHTRAG_DIR = os.environ.get("LIGHTRAG_DIR", "/data/lightrag")

# LightRAG queue (Redis)
LIGHTRAG_QUEUE_KEY = "lightrag:queue"

# ── Клиенты (lazy init) ────────────────────────

_chroma = None
_redis = None
_kafka_producer = None
_lightrag = None


def get_redis():
    global _redis
    if _redis is None:
        try:
            import redis as r
            _redis = r.Redis(host=REDIS_HOST, port=REDIS_PORT, db=0,
                             socket_connect_timeout=3)
            _redis.ping()
            logger.info(f"Redis connected: {REDIS_HOST}:{REDIS_PORT}")
        except Exception as e:
            logger.warning(f"Redis unavailable: {e}")
            _redis = None
    return _redis


def get_chroma():
    global _chroma
    if _chroma is None:
        try:
            import chromadb
            _chroma = chromadb.HttpClient(host=CHROMA_HOST, port=CHROMA_PORT)
            _chroma.heartbeat()
            logger.info(f"ChromaDB connected: {CHROMA_HOST}:{CHROMA_PORT}")
        except Exception as e:
            logger.warning(f"ChromaDB unavailable: {e}")
            _chroma = None
    return _chroma


def get_lightrag():
    global _lightrag
    if _lightrag is None:
        try:
            from lightrag import LightRAG as LR
            from lightrag.base import QueryParam
            from lightrag.llm.ollama import ollama_embed, ollama_model_complete

            _lightrag = LR(
                working_dir=LIGHTRAG_DIR,
                llm_model_func=ollama_model_complete,
                llm_model_name="qwen2.5:7b",
                llm_model_kwargs={},
                embedding_func=ollama_embed,
            )
            logger.info(f"LightRAG instance created: {LIGHTRAG_DIR}")
        except Exception as e:
            logger.warning(f"LightRAG unavailable: {e}")
            _lightrag = None
    return _lightrag


def get_kafka_producer():
    global _kafka_producer
    if _kafka_producer is None:
        try:
            from kafka import KafkaProducer as KP
            _kafka_producer = KP(
                bootstrap_servers=KAFKA_BOOTSTRAP,
                value_serializer=lambda v: json.dumps(v, ensure_ascii=False).encode(),
                acks="all",
                retries=3,
            )
            logger.info(f"Kafka producer connected: {KAFKA_BOOTSTRAP}")
        except Exception as e:
            logger.warning(f"Kafka unavailable: {e}")
            _kafka_producer = None
    return _kafka_producer


# ── Обработчики инструментов ───────────────────

def tool_vector_search(args):
    query = args.get("query", "")
    top_k = args.get("top_k", 10)
    collection_name = args.get("collection", "doctrine")
    filter_criteria = args.get("filter", None)

    client = get_chroma()
    if not client:
        return {"error": "ChromaDB not available", "results": []}

    try:
        collection = client.get_or_create_collection(
            name=collection_name,
            metadata={"hnsw:space": "cosine"},
        )
        results = collection.query(
            query_texts=[query],
            n_results=min(top_k, 100),
            where=filter_criteria,
        )
        return {
            "count": len(results.get("ids", [[]])[0]),
            "results": [
                {
                    "id": results["ids"][0][i] if results["ids"] else None,
                    "document": results["documents"][0][i][:2000] if results.get("documents") else "",
                    "metadata": results["metadatas"][0][i] if results.get("metadatas") else {},
                    "distance": results["distances"][0][i] if results.get("distances") else None,
                }
                for i in range(len(results.get("ids", [[]])[0]))
            ],
        }
    except Exception as e:
        logger.error(f"ChromaDB search error: {e}")
        return {"error": str(e), "results": []}


def tool_vector_save(args):
    article = args.get("article", {})
    collection_name = args.get("collection", "doctrine")

    client = get_chroma()
    if not client:
        return {"error": "ChromaDB not available"}

    if not article.get("id"):
        article["id"] = str(uuid.uuid4())[:16]
    if not article.get("created_at"):
        article["created_at"] = datetime.now(timezone.utc).isoformat()

    try:
        collection = client.get_or_create_collection(
            name=collection_name,
            metadata={"hnsw:space": "cosine"},
        )
        collection.upsert(
            ids=[article["id"]],
            documents=[article.get("content", "")],
            metadatas=[{
                "title": article.get("title", ""),
                "source": article.get("source", "agent"),
                "author": article.get("author", "agent.unknown"),
                "created_at": article["created_at"],
                "tags": ",".join(article.get("tags", [])),
            }],
        )
        logger.info(f"Saved to ChromaDB: {article['id']}")
        return {"status": "saved", "id": article["id"]}
    except Exception as e:
        logger.error(f"ChromaDB save error: {e}")
        return {"error": str(e)}


def tool_graph_query(args):
    text = args.get("text", "")
    mode = args.get("mode", "hybrid")

    rag = get_lightrag()
    if not rag:
        return {"error": "LightRAG not available"}

    try:
        from lightrag.base import QueryParam
        param = QueryParam(mode=mode)
        response = rag.query(text, param=param)
        return {"response": response}
    except Exception as e:
        logger.error(f"LightRAG query error: {e}")
        return {"error": str(e)}


def tool_graph_insert(args):
    text = args.get("text", "")
    source = args.get("source", "agent")

    r = get_redis()
    if not r:
        return {"error": "Redis not available — cannot queue LightRAG insert"}

    try:
        entry = {
            "text": text,
            "source": source,
            "ts": datetime.now(timezone.utc).isoformat(),
        }
        r.rpush(LIGHTRAG_QUEUE_KEY, json.dumps(entry, ensure_ascii=False))
        qlen = r.llen(LIGHTRAG_QUEUE_KEY)
        logger.info(f"Queued for LightRAG ({len(text)} chars from {source}), queue={qlen}")
        return {"status": "queued", "chars": len(text), "queue_size": qlen}
    except Exception as e:
        logger.error(f"LightRAG queue error: {e}")
        return {"error": str(e)}


def tool_graph_queue_status(args):
    r = get_redis()
    if not r:
        return {"error": "Redis not available", "queue_size": 0}

    try:
        qlen = r.llen(LIGHTRAG_QUEUE_KEY)
        return {"queue_size": qlen, "queue_key": LIGHTRAG_QUEUE_KEY}
    except Exception as e:
        return {"error": str(e), "queue_size": 0}


def tool_cache_get(args):
    key = args.get("key", "")
    r = get_redis()
    if not r:
        return {"error": "Redis not available"}

    try:
        val = r.get(key)
        if val is None:
            return {"key": key, "value": None, "found": False}
        if isinstance(val, bytes):
            val = val.decode()
        try:
            val = json.loads(val)
        except (json.JSONDecodeError, TypeError):
            pass
        return {"key": key, "value": val, "found": True}
    except Exception as e:
        return {"error": str(e)}


def tool_cache_set(args):
    key = args.get("key", "")
    value = args.get("value", "")
    ttl = args.get("ttl", 86400)

    r = get_redis()
    if not r:
        return {"error": "Redis not available"}

    try:
        if isinstance(value, (dict, list)):
            value = json.dumps(value, ensure_ascii=False)
        r.setex(key, ttl, str(value))
        logger.info(f"Redis SET: {key} (TTL={ttl}s)")
        return {"status": "set", "key": key, "ttl": ttl}
    except Exception as e:
        return {"error": str(e)}


def tool_kafka_send(args):
    topic = args.get("topic", "")
    message = args.get("message", {})

    producer = get_kafka_producer()
    if not producer:
        return {"error": "Kafka not available"}

    if isinstance(message, str):
        try:
            message = json.loads(message)
        except json.JSONDecodeError:
            message = {"text": message}

    message.setdefault("msg_id", str(uuid.uuid4()))
    message.setdefault("ts", datetime.now(timezone.utc).isoformat())

    try:
        producer.send(topic, message)
        producer.flush()
        logger.info(f"Kafka sent to {topic}: {message.get('msg_id')}")
        return {"status": "sent", "topic": topic, "msg_id": message["msg_id"]}
    except Exception as e:
        return {"error": str(e)}


def tool_kafka_list(args):
    try:
        from kafka import KafkaAdminClient
        admin = KafkaAdminClient(bootstrap_servers=KAFKA_BOOTSTRAP)
        topics = admin.list_topics()
        admin.close()
        return {"topics": list(topics)}
    except Exception as e:
        return {"error": str(e), "topics": []}


def tool_state_load(args):
    agent_id = args.get("agent_id", "")

    r = get_redis()
    if not r:
        return {"error": "Redis not available", "state": {}}

    state = {}
    try:
        state_key = f"agent:{agent_id}"
        raw = r.hgetall(state_key)
        if raw:
            state = {k.decode() if isinstance(k, bytes) else k:
                     v.decode() if isinstance(v, bytes) else v
                     for k, v in raw.items()}

        skills_key = f"agent:{agent_id}:skills"
        skills = r.smembers(skills_key)
        if skills:
            state["skills"] = [s.decode() if isinstance(s, bytes) else s for s in skills]

        logger.info(f"State loaded for {agent_id}: {len(state)} keys")
        return {"agent_id": agent_id, "state": state}
    except Exception as e:
        return {"error": str(e), "state": {}}


def tool_state_save(args):
    agent_id = args.get("agent_id", "")
    data = args.get("data", {})

    r = get_redis()
    if not r:
        return {"error": "Redis not available"}

    try:
        state_key = f"agent:{agent_id}"
        skills = data.pop("skills", [])
        if data:
            r.hset(state_key, mapping=data)

        if skills:
            skills_key = f"agent:{agent_id}:skills"
            r.delete(skills_key)
            for s in skills:
                r.sadd(skills_key, s)

        r.expire(state_key, 604800)
        logger.info(f"State saved for {agent_id}")
        return {"status": "saved", "agent_id": agent_id}
    except Exception as e:
        return {"error": str(e)}


def tool_session_save(args):
    agent_id = args.get("agent_id", "")
    context = args.get("context", "")
    ttl = args.get("ttl", 604800)

    r = get_redis()
    if not r:
        return {"error": "Redis not available"}

    try:
        key = f"session:{agent_id}"
        ts = datetime.now(timezone.utc).isoformat()
        snapshot = {
            "agent_id": agent_id,
            "timestamp": ts,
            "context": context,
        }
        r.setex(key, ttl, json.dumps(snapshot, ensure_ascii=False))
        r.setex(f"{key}:ts", ttl, ts)
        logger.info(f"Session saved: {key} (TTL={ttl}s)")
        return {"status": "saved", "agent_id": agent_id, "timestamp": ts}
    except Exception as e:
        return {"error": str(e)}


def tool_session_load(args):
    agent_id = args.get("agent_id", "")

    r = get_redis()
    if not r:
        return {"error": "Redis not available", "session": None}

    try:
        key = f"session:{agent_id}"
        raw = r.get(key)
        if raw is None:
            ts_raw = r.get(f"{key}:ts")
            return {"status": "not_found", "agent_id": agent_id,
                    "last_seen": ts_raw.decode() if ts_raw else None}
        if isinstance(raw, bytes):
            raw = raw.decode()
        session = json.loads(raw)
        return {"status": "found", "agent_id": agent_id, "session": session}
    except Exception as e:
        return {"error": str(e), "agent_id": agent_id}


def tool_session_list(args):
    r = get_redis()
    if not r:
        return {"error": "Redis not available", "sessions": []}

    try:
        keys = r.keys("session:*")
        sessions = []
        for k in keys:
            name = k.decode() if isinstance(k, bytes) else k
            if name.endswith(":ts"):
                continue
            ttl = r.ttl(name)
            sessions.append({"key": name, "ttl": ttl})
        return {"count": len(sessions), "sessions": sessions}
    except Exception as e:
        return {"error": str(e), "sessions": []}


def tool_kafka_consume(args):
    topic = args.get("topic", "")
    count = args.get("count", 10)
    timeout_ms = args.get("timeout_ms", 3000)

    if not topic:
        return {"error": "topic is required"}

    try:
        from kafka import KafkaConsumer
        consumer = KafkaConsumer(
            topic,
            bootstrap_servers=KAFKA_BOOTSTRAP,
            auto_offset_reset="latest",
            enable_auto_commit=False,
            consumer_timeout_ms=timeout_ms,
            value_deserializer=lambda v: json.loads(v.decode()) if v else None,
        )
        messages = []
        partitions = consumer.assignment()
        if not partitions:
            consumer.poll(timeout_ms=1000)
            partitions = consumer.assignment()

        for part in partitions:
            consumer.seek_to_end(part)
            end_offset = consumer.position(part)
            start_offset = max(0, end_offset - count)
            consumer.seek(part, start_offset)
            for msg in consumer:
                messages.append({
                    "offset": msg.offset,
                    "partition": msg.partition,
                    "timestamp": datetime.fromtimestamp(msg.timestamp / 1000, tz=timezone.utc).isoformat(),
                    "value": msg.value,
                })
                if len(messages) >= count:
                    break

        consumer.close()
        logger.info(f"Kafka consumed {len(messages)} from {topic}")
        return {"topic": topic, "count": len(messages), "messages": messages}
    except Exception as e:
        logger.warning(f"Kafka consume error: {e}")
        return {"error": str(e), "topic": topic, "messages": []}


def tool_health(args):
    results = {}
    all_ok = True

    r = get_redis()
    if r:
        try:
            r.ping()
            results["redis"] = {"status": "ok", "keys": r.dbsize()}
        except Exception as e:
            results["redis"] = {"status": "error", "error": str(e)}
            all_ok = False
    else:
        results["redis"] = {"status": "unavailable"}
        all_ok = False

    client = get_chroma()
    if client:
        try:
            client.heartbeat()
            cols = client.list_collections()
            results["chromadb"] = {"status": "ok", "collections": [c.name for c in cols]}
        except Exception as e:
            results["chromadb"] = {"status": "error", "error": str(e)}
            all_ok = False
    else:
        results["chromadb"] = {"status": "unavailable"}
        all_ok = False

    rag = get_lightrag()
    if rag:
        results["lightrag"] = {"status": "ok", "working_dir": LIGHTRAG_DIR}
    else:
        results["lightrag"] = {"status": "unavailable"}
        all_ok = False

    try:
        from kafka import KafkaAdminClient
        admin = KafkaAdminClient(bootstrap_servers=KAFKA_BOOTSTRAP)
        topics = admin.list_topics()
        admin.close()
        results["kafka"] = {"status": "ok", "topics_count": len(topics)}
    except Exception as e:
        results["kafka"] = {"status": "error", "error": str(e)}
        all_ok = False

    results["all_ok"] = all_ok
    results["timestamp"] = datetime.now(timezone.utc).isoformat()
    results["bridge_version"] = "1.0.0"
    return results


def tool_stats(args):
    stats = {"service": "memory-bridge", "version": "1.0.0"}
    r = get_redis()
    if r:
        try:
            stats["redis_keys"] = r.dbsize()
        except Exception:
            stats["redis_keys"] = -1

    client = get_chroma()
    if client:
        try:
            collections = client.list_collections()
            stats["chroma_collections"] = [c.name for c in collections]
        except Exception:
            stats["chroma_collections"] = []

    rag = get_lightrag()
    stats["lightrag_available"] = rag is not None
    stats["lightrag_dir"] = LIGHTRAG_DIR

    stats["timestamp"] = datetime.now(timezone.utc).isoformat()
    return stats


TOOLS = {
    "memory_vector_search": {
        "fn": tool_vector_search,
        "description": "Поиск по векторной памяти ChromaDB",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Текст запроса"},
                "top_k": {"type": "number", "default": 10},
                "collection": {"type": "string", "default": "doctrine"},
                "filter": {"type": "object", "default": None},
            },
            "required": ["query"],
        },
    },
    "memory_vector_save": {
        "fn": tool_vector_save,
        "description": "Сохранить документ в ChromaDB",
        "inputSchema": {
            "type": "object",
            "properties": {
                "article": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"},
                        "content": {"type": "string"},
                        "source": {"type": "string"},
                        "author": {"type": "string"},
                        "tags": {"type": "array", "items": {"type": "string"}},
                    },
                },
                "collection": {"type": "string", "default": "doctrine"},
            },
            "required": ["article"],
        },
    },
    "memory_graph_query": {
        "fn": tool_graph_query,
        "description": "Графовый запрос к LightRAG (local/global/hybrid)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Вопрос к графу знаний"},
                "mode": {"type": "string", "enum": ["local", "global", "hybrid"], "default": "hybrid"},
            },
            "required": ["text"],
        },
    },
    "memory_graph_insert": {
        "fn": tool_graph_insert,
        "description": "Поставить текст в очередь на запись в LightRAG (фоновый writer обработает)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Текст для индексации"},
                "source": {"type": "string", "default": "agent"},
            },
            "required": ["text"],
        },
    },
    "memory_graph_queue_status": {
        "fn": tool_graph_queue_status,
        "description": "Статус очереди на запись в LightRAG",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    },
    "memory_cache_get": {
        "fn": tool_cache_get,
        "description": "Прочитать ключ из Redis-кэша",
        "inputSchema": {
            "type": "object",
            "properties": {"key": {"type": "string"}},
            "required": ["key"],
        },
    },
    "memory_cache_set": {
        "fn": tool_cache_set,
        "description": "Записать ключ в Redis-кэш с TTL",
        "inputSchema": {
            "type": "object",
            "properties": {
                "key": {"type": "string"},
                "value": {"description": "Значение (строка, число, объект)"},
                "ttl": {"type": "number", "default": 86400},
            },
            "required": ["key", "value"],
        },
    },
    "memory_kafka_send": {
        "fn": tool_kafka_send,
        "description": "Отправить сообщение в топик Kafka",
        "inputSchema": {
            "type": "object",
            "properties": {
                "topic": {"type": "string", "description": "Имя топика"},
                "message": {"description": "Объект сообщения"},
            },
            "required": ["topic", "message"],
        },
    },
    "memory_kafka_list": {
        "fn": tool_kafka_list,
        "description": "Список доступных топиков Kafka",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    },
    "memory_state_load": {
        "fn": tool_state_load,
        "description": "Загрузить состояние агента из Redis",
        "inputSchema": {
            "type": "object",
            "properties": {"agent_id": {"type": "string"}},
            "required": ["agent_id"],
        },
    },
    "memory_state_save": {
        "fn": tool_state_save,
        "description": "Сохранить состояние агента в Redis",
        "inputSchema": {
            "type": "object",
            "properties": {
                "agent_id": {"type": "string"},
                "data": {"type": "object"},
            },
            "required": ["agent_id", "data"],
        },
    },
    "memory_session_save": {
        "fn": tool_session_save,
        "description": "Сохранить снэпшот сессии агента в Redis (session:<agent_id>)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "agent_id": {"type": "string"},
                "context": {"type": "string", "description": "Контекст сессии (JSON-строка)"},
                "ttl": {"type": "number", "default": 604800},
            },
            "required": ["agent_id", "context"],
        },
    },
    "memory_session_load": {
        "fn": tool_session_load,
        "description": "Загрузить снэпшот сессии агента из Redis",
        "inputSchema": {
            "type": "object",
            "properties": {"agent_id": {"type": "string"}},
            "required": ["agent_id"],
        },
    },
    "memory_session_list": {
        "fn": tool_session_list,
        "description": "Список сохранённых снэпшотов сессий",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    },
    "memory_kafka_consume": {
        "fn": tool_kafka_consume,
        "description": "Прочитать последние сообщения из Kafka-топика",
        "inputSchema": {
            "type": "object",
            "properties": {
                "topic": {"type": "string"},
                "count": {"type": "number", "default": 10},
                "timeout_ms": {"type": "number", "default": 3000},
            },
            "required": ["topic"],
        },
    },
    "memory_health": {
        "fn": tool_health,
        "description": "Полный healthcheck всех сервисов памяти",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    },
    "memory_stats": {
        "fn": tool_stats,
        "description": "Статистика использования памяти",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    },
}


# ── MCP stdio протокол ─────────────────────────

def send_message(msg):
    line = json.dumps(msg, ensure_ascii=False)
    sys.stdout.write(line + "\n")
    sys.stdout.flush()


def handle_request(request):
    method = request.get("method", "")
    params = request.get("params", {})
    req_id = request.get("id")

    if method == "initialize":
        send_message({
            "jsonrpc": "2.0", "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        name: {
                            "name": name,
                            "description": info["description"],
                            "inputSchema": info["inputSchema"],
                        }
                        for name, info in TOOLS.items()
                    }
                },
                "serverInfo": {"name": "waters-memory-bridge", "version": "1.0.0"},
            },
        })
        return

    if method == "initialized":
        logger.info("MCP client initialized")
        return

    if method == "tools/list":
        send_message({
            "jsonrpc": "2.0", "id": req_id,
            "result": {
                "tools": [
                    {"name": name, "description": info["description"],
                     "inputSchema": info["inputSchema"]}
                    for name, info in TOOLS.items()
                ]
            },
        })
        return

    if method == "tools/call":
        tool_name = params.get("name", "")
        arguments = params.get("arguments", {})

        tool = TOOLS.get(tool_name)
        if not tool:
            send_message({
                "jsonrpc": "2.0", "id": req_id,
                "error": {"code": -32601, "message": f"Tool not found: {tool_name}"},
            })
            return

        try:
            result = tool["fn"](arguments)
            send_message({
                "jsonrpc": "2.0", "id": req_id,
                "result": {"content": [{"type": "text", "text": json.dumps(result, ensure_ascii=False)}]},
            })
        except Exception as e:
            logger.exception(f"Tool error: {tool_name}")
            send_message({
                "jsonrpc": "2.0", "id": req_id,
                "error": {"code": -32603, "message": str(e)},
            })
        return

    logger.warning(f"Unknown method: {method}")
    send_message({
        "jsonrpc": "2.0", "id": req_id,
        "error": {"code": -32601, "message": f"Method not found: {method}"},
    })


def main():
    logger.info("=== WATERS Memory Bridge MCP Server ===")
    logger.info(f"Redis: {REDIS_HOST}:{REDIS_PORT}")
    logger.info(f"ChromaDB: {CHROMA_HOST}:{CHROMA_PORT}")
    logger.info(f"Kafka: {KAFKA_BOOTSTRAP}")
    logger.info(f"LightRAG: {LIGHTRAG_DIR}")
    logger.info(f"Tools loaded: {len(TOOLS)} (LightRAG writes queued via Redis, sync by lightrag_writer)")
    logger.info("Ready on stdin/stdout (MCP stdio protocol)")

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            handle_request(request)
        except json.JSONDecodeError as e:
            logger.warning(f"Invalid JSON: {e}")
        except Exception as e:
            logger.exception(f"Fatal error: {e}")


if __name__ == "__main__":
    main()
