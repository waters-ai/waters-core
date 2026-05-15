#!/usr/bin/env python3
"""
constructor_startup.py — Startup Sequence для Конструктора Сети v1.0

Запускается ДО opencode CLI. Выполняет 9 команд Startup Sequence:
  0. memory_read_journal — чтение бортового журнала (контекст без сервисов)
  1. memory_state_load("constructor")
  2. memory_session_load("constructor")
  3. memory_kafka_consume("planners.answers.v1", count=5)
  4. memory_vector_search("constructor last session", top_k=5)
  5. memory_kafka_list
  6. memory_cache_get("tasks:constructor:pending")
  7. memory_health
  8. Сохранение снэпшота сессии

Результат: JSON в /tmp/opencode/constructor_startup.json
          + markdown-секция на stdout (для препенда в AGENTS.md)
"""

import json
import logging
import os
import re
import sys
import uuid
from datetime import datetime, timezone

logging.basicConfig(level=logging.WARNING, format="%(message)s")
logger = logging.getLogger("constructor-startup")

# ── Конфигурация (копия mcp_memory_bridge) ───────────────────────────
REDIS_HOST = os.environ.get("REDIS_HOST", "localhost")
REDIS_PORT = int(os.environ.get("REDIS_PORT", "6379"))
CHROMA_HOST = os.environ.get("CHROMA_HOST", "localhost")
CHROMA_PORT = int(os.environ.get("CHROMA_PORT", "8000"))
KAFKA_BOOTSTRAP = os.environ.get("KAFKA_BOOTSTRAP", "localhost:9092")
LIGHTRAG_DIR = os.environ.get("LIGHTRAG_DIR", "/data/lightrag")
AGENT_ID = "constructor"
OUTPUT_FILE = "/tmp/opencode/constructor_startup.json"
WORK_DIR = os.environ.get("REPO_DIR", os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
JOURNAL_PATH = os.environ.get(
    "CONSTRUCTOR_JOURNAL",
    os.path.join(WORK_DIR, "infrastructure", "logs", "constructor_journal.log"),
)

# ── Lazy клиенты ──────────────────────────────────────────────────────

def _redis():
    try:
        import redis as r
        c = r.Redis(host=REDIS_HOST, port=REDIS_PORT, db=0, socket_connect_timeout=3)
        c.ping()
        return c
    except Exception as e:
        logger.warning(f"Redis unavailable: {e}")
        return None

def _chroma():
    try:
        import chromadb
        c = chromadb.HttpClient(host=CHROMA_HOST, port=CHROMA_PORT)
        c.heartbeat()
        return c
    except Exception as e:
        logger.warning(f"ChromaDB unavailable: {e}")
        return None

def _kafka_admin():
    try:
        from kafka import KafkaAdminClient
        return KafkaAdminClient(bootstrap_servers=KAFKA_BOOTSTRAP)
    except Exception as e:
        logger.warning(f"Kafka unavailable: {e}")
        return None

def _kafka_consumer(topic, timeout_ms=3000):
    try:
        from kafka import KafkaConsumer
        return KafkaConsumer(
            topic,
            bootstrap_servers=KAFKA_BOOTSTRAP,
            auto_offset_reset="latest",
            enable_auto_commit=False,
            consumer_timeout_ms=timeout_ms,
            value_deserializer=lambda v: json.loads(v.decode()) if v else None,
        )
    except Exception as e:
        logger.warning(f"Kafka consumer error: {e}")
        return None

# ── Startup команды ───────────────────────────────────────────────────

def cmd_state_load():
    """memory_state_load('constructor')"""
    r = _redis()
    if not r:
        return {"status": "unavailable", "reason": "Redis not available"}

    try:
        state_key = f"agent:{AGENT_ID}"
        raw = r.hgetall(state_key)
        state = {}
        if raw:
            state = {k.decode() if isinstance(k, bytes) else k:
                     v.decode() if isinstance(v, bytes) else v
                     for k, v in raw.items()}

        skills_key = f"agent:{AGENT_ID}:skills"
        skills = r.smembers(skills_key)
        if skills:
            state["skills"] = [s.decode() if isinstance(s, bytes) else s for s in skills]

        return {"status": "loaded", "agent_id": AGENT_ID, "keys_count": len(state), "state": state}
    except Exception as e:
        return {"status": "error", "error": str(e)}


def cmd_session_load():
    """memory_session_load('constructor')"""
    r = _redis()
    if not r:
        return {"status": "unavailable", "reason": "Redis not available"}

    try:
        key = f"session:{AGENT_ID}"
        raw = r.get(key)
        if raw is None:
            ts_raw = r.get(f"{key}:ts")
            return {"status": "not_found", "agent_id": AGENT_ID,
                    "last_seen": ts_raw.decode() if ts_raw else None}

        if isinstance(raw, bytes):
            raw = raw.decode()
        session = json.loads(raw)
        return {"status": "found", "agent_id": AGENT_ID, "session": session}
    except Exception as e:
        return {"status": "error", "error": str(e)}


def cmd_kafka_consume(topic="planners.answers.v1", count=5):
    """memory_kafka_consume('planners.answers.v1', count=5)"""
    consumer = _kafka_consumer(topic)
    if not consumer:
        return {"status": "unavailable", "reason": "Kafka consumer not available"}

    try:
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
        return {"status": "ok", "topic": topic, "count": len(messages), "messages": messages}
    except Exception as e:
        return {"status": "error", "error": str(e)}


def cmd_vector_search(query="constructor last session", top_k=5, collection="doctrine"):
    """memory_vector_search('constructor last session', top_k=5)"""
    client = _chroma()
    if not client:
        return {"status": "unavailable", "reason": "ChromaDB not available"}

    try:
        col = client.get_or_create_collection(name=collection, metadata={"hnsw:space": "cosine"})
        results = col.query(query_texts=[query], n_results=min(top_k, 100))
        items = []
        for i in range(len(results.get("ids", [[]])[0])):
            items.append({
                "id": results["ids"][0][i] if results["ids"] else None,
                "document": results["documents"][0][i][:2000] if results.get("documents") else "",
                "metadata": results["metadatas"][0][i] if results.get("metadatas") else {},
                "distance": results["distances"][0][i] if results.get("distances") else None,
            })
        return {"status": "ok", "collection": collection, "count": len(items), "results": items}
    except Exception as e:
        return {"status": "error", "error": str(e)}


def cmd_kafka_list():
    """memory_kafka_list"""
    admin = _kafka_admin()
    if not admin:
        return {"status": "unavailable", "reason": "Kafka admin not available"}

    try:
        topics = admin.list_topics()
        admin.close()
        return {"status": "ok", "topics_count": len(topics), "topics": sorted(topics)}
    except Exception as e:
        return {"status": "error", "error": str(e)}


def cmd_cache_get(key="tasks:constructor:pending"):
    """memory_cache_get('tasks:constructor:pending')"""
    r = _redis()
    if not r:
        return {"status": "unavailable", "reason": "Redis not available"}

    try:
        val = r.get(key)
        if val is None:
            return {"status": "not_found", "key": key, "value": None}
        if isinstance(val, bytes):
            val = val.decode()
        try:
            val = json.loads(val)
        except (json.JSONDecodeError, TypeError):
            pass
        return {"status": "found", "key": key, "value": val}
    except Exception as e:
        return {"status": "error", "error": str(e)}


def cmd_health():
    """memory_health"""
    results = {}
    all_ok = True

    r = _redis()
    if r:
        try:
            results["redis"] = {"status": "ok", "keys": r.dbsize()}
        except Exception as e:
            results["redis"] = {"status": "error", "error": str(e)}
            all_ok = False
    else:
        results["redis"] = {"status": "unavailable"}
        all_ok = False

    client = _chroma()
    if client:
        try:
            cols = client.list_collections()
            results["chromadb"] = {"status": "ok", "collections": [c.name for c in cols]}
        except Exception as e:
            results["chromadb"] = {"status": "error", "error": str(e)}
            all_ok = False
    else:
        results["chromadb"] = {"status": "unavailable"}
        all_ok = False

    try:
        from lightrag import LightRAG as LR
        from lightrag.base import QueryParam
        from lightrag.llm.ollama import ollama_embed, ollama_model_complete
        rag = LR(
            working_dir=LIGHTRAG_DIR,
            llm_model_func=ollama_model_complete,
            llm_model_name="qwen2.5:7b",
            llm_model_kwargs={},
            embedding_func=ollama_embed,
        )
        results["lightrag"] = {"status": "ok", "working_dir": LIGHTRAG_DIR}
    except Exception as e:
        results["lightrag"] = {"status": "unavailable", "error": str(e)}
        all_ok = False

    admin = _kafka_admin()
    if admin:
        try:
            topics = admin.list_topics()
            admin.close()
            results["kafka"] = {"status": "ok", "topics_count": len(topics)}
        except Exception as e:
            results["kafka"] = {"status": "error", "error": str(e)}
            all_ok = False
    else:
        results["kafka"] = {"status": "unavailable"}
        all_ok = False

    results["all_ok"] = all_ok
    results["timestamp"] = datetime.now(timezone.utc).isoformat()
    return results


def cmd_session_save(context=None):
    """Сохранить снэпшот сессии (шаг 8)"""
    r = _redis()
    if not r:
        return {"status": "unavailable", "reason": "Redis not available"}

    try:
        key = f"session:{AGENT_ID}"
        ts = datetime.now(timezone.utc).isoformat()
        snapshot = {
            "agent_id": AGENT_ID,
            "timestamp": ts,
            "context": context or json.dumps({"startup": ts, "source": "run_constructor.sh"}),
        }
        r.setex(key, 604800, json.dumps(snapshot, ensure_ascii=False))
        r.setex(f"{key}:ts", 604800, ts)
        return {"status": "saved", "agent_id": AGENT_ID, "timestamp": ts}
    except Exception as e:
        return {"status": "error", "error": str(e)}


def cmd_read_journal(tail_lines=50):
    """memory_read_journal — чтение последних записей бортового журнала
    Исполняется первой командой при startup для восстановления контекста."""
    path = JOURNAL_PATH
    tail = int(os.environ.get("CONSTRUCTOR_JOURNAL_TAIL", str(tail_lines)))

    if not os.path.exists(path):
        return {"status": "not_found", "path": path, "reason": "Journal file not found"}

    try:
        with open(path, "r") as f:
            lines = f.readlines()

        total = len(lines)
        tail_content = lines[-tail:] if total > tail else lines

        last_state = None
        problems = []
        key_addresses = {}
        last_ts = None

        for line in reversed(tail_content):
            stripped = line.strip()
            ts_match = re.search(r"\[([\d\-T:+Z]+)\]", stripped)
            if ts_match and not last_ts:
                last_ts = ts_match.group(1)

            if any(tag in stripped for tag in ["[STATE]", "[DONE]", "[DEPLOY]", "[TEST]"]):
                if not last_state:
                    last_state = stripped[:300]

            addr_match = re.findall(r'[1-9A-HJ-NP-Za-km-z]{32,44}', stripped)
            for addr in addr_match:
                key_addresses[addr[:10]] = addr

            if any(w in stripped for w in ["Проблема", "❌", "error", "fail", "OOM"]):
                if stripped not in problems:
                    problems.append(stripped[:200])

        return {
            "status": "ok",
            "path": path,
            "total_lines": total,
            "tail_lines": len(tail_content),
            "last_timestamp": last_ts,
            "last_state_block": last_state,
            "problems_count": len(problems),
            "problems": problems[:5],
            "key_addresses_count": len(key_addresses),
            "key_addresses": dict(list(key_addresses.items())[:10]),
        }
    except Exception as e:
        return {"status": "error", "error": str(e)}


# ── Форматирование markdown для AGENTS.md ─────────────────────────────

def fmt_header(data):
    lines = []
    lines.append("\n\n## Startup Context (авто, {})".format(
        datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")))
    lines.append("")

    # Health summary
    health = data.get("health", {})
    h_status = "✅ Все сервисы доступны" if health.get("all_ok") else "⚠️ Есть проблемы"
    lines.append(f"**Health**: {h_status}")
    for svc, info in health.items():
        if svc in ("all_ok", "timestamp"):
            continue
        if isinstance(info, dict):
            s = info.get("status", "unknown")
            icon = "✅" if s == "ok" else "⚠️" if s == "unavailable" else "❌"
            lines.append(f"  {icon} {svc}: {s}")
    lines.append("")

    # Journal (бортовой журнал)
    journal = data.get("journal", {})
    if journal.get("status") == "ok":
        j_ts = journal.get("last_timestamp", "?")
        j_probs = journal.get("problems_count", 0)
        j_state = journal.get("last_state_block", "")
        icon = "⚠️" if j_probs > 0 else "✅"
        lines.append(f"**Onboard Log**: {icon} последняя запись {j_ts} ({journal.get('tail_lines', 0)} строк хвоста)")
        if j_state:
            lines.append(f"  • Состояние: {j_state[:200]}")
        if j_probs > 0:
            for p in journal.get("problems", []):
                lines.append(f"  ⚠ {p[:150]}")
        addrs = journal.get("key_addresses", {})
        if addrs:
            lines.append(f"  • Адреса: {', '.join(f'{k}..' for k in addrs)}")
    else:
        lines.append("**Onboard Log**: ❌ не найден")
    lines.append("")

    # Session
    session = data.get("session", {})
    if session.get("status") == "found":
        s = session.get("session", {})
        lines.append(f"**Session**: найдена — {s.get('timestamp', '?')}")
    else:
        lines.append("**Session**: новая (предыдущая не найдена)")
    lines.append("")

    # State
    state = data.get("state", {})
    if state.get("keys_count", 0) > 0:
        lines.append(f"**Agent State**: {state.get('keys_count')} ключей в Redis")
        st = state.get("state", {})
        for k, v in list(st.items())[:10]:
            lines.append(f"  • {k}: {json.dumps(v, ensure_ascii=False)[:120]}")
    else:
        lines.append("**Agent State**: пусто (первый запуск)")
    lines.append("")

    # Pending tasks
    pending = data.get("pending_tasks", {})
    if pending.get("status") == "found":
        val = pending.get("value", [])
        if val:
            lines.append(f"**Pending Tasks** ({len(val)}):")
            for t in val:
                if isinstance(t, dict):
                    lines.append(f"  • {t.get('task', '?')} [{t.get('priority', '?')}]")
                else:
                    lines.append(f"  • {t}")
        else:
            lines.append("**Pending Tasks**: пусто")
    else:
        lines.append("**Pending Tasks**: пусто")
    lines.append("")

    # Kafka answers (last 5)
    answers = data.get("kafka_answers", {})
    if answers.get("count", 0) > 0:
        lines.append(f"**Kafka Answers** (последние {answers['count']} из {answers.get('topic', '?')}):")
        for msg in answers.get("messages", []):
            val = msg.get("value", {})
            if isinstance(val, dict):
                summary = json.dumps(val, ensure_ascii=False)[:150]
            else:
                summary = str(val)[:150]
            lines.append(f"  • [{msg.get('partition', '?')}:{msg.get('offset', '?')}] {summary}")
    else:
        lines.append("**Kafka Answers**: нет сообщений")
    lines.append("")

    # Vector context
    vec = data.get("vector_context", {})
    if vec.get("count", 0) > 0:
        lines.append(f"**Vector Context** ({vec['count']} результатов из {vec.get('collection', 'doctrine')}):")
        for r in vec.get("results", []):
            doc = r.get("document", "")[:200]
            meta = r.get("metadata", {})
            title = meta.get("title", "?")
            lines.append(f"  • {r.get('id', '?')} — {title}: {doc}")
    else:
        lines.append("**Vector Context**: пусто")
    lines.append("")

    # Kafka topics
    topics = data.get("kafka_topics", {})
    if topics.get("topics_count", 0) > 0:
        lines.append(f"**Kafka Topics**: {topics['topics_count']} топиков доступно")
    lines.append("")

    return "\n".join(lines)


# ── Main ──────────────────────────────────────────────────────────────

def main():
    print("=== Constructor Startup Sequence ===", file=sys.stderr)
    print(f"Agent: {AGENT_ID}", file=sys.stderr)
    print(f"Time: {datetime.now(timezone.utc).isoformat()}", file=sys.stderr)
    print(file=sys.stderr)

    results = {}

    # 0. Journal (бортовой журнал) — восстановление контекста
    print("[0/9] memory_read_journal...", file=sys.stderr)
    results["journal"] = cmd_read_journal()
    j_status = results["journal"].get("status")
    j_lines = results["journal"].get("tail_lines", 0)
    j_ts = results["journal"].get("last_timestamp", "?")
    print(f"  → {j_status}, {j_lines} строк, последняя запись: {j_ts}", file=sys.stderr)
    if results["journal"].get("problems_count", 0) > 0:
        print(f"  ⚠ {results['journal']['problems_count']} проблем(ы) в журнале", file=sys.stderr)

    # 1. State
    print("[1/9] memory_state_load...", file=sys.stderr)
    results["state"] = cmd_state_load()
    print(f"  → {results['state'].get('status')}, {results['state'].get('keys_count', 0)} keys", file=sys.stderr)

    # 2. Session
    print("[2/9] memory_session_load...", file=sys.stderr)
    results["session"] = cmd_session_load()
    print(f"  → {results['session'].get('status')}", file=sys.stderr)

    # 3. Kafka consume
    print("[3/9] memory_kafka_consume...", file=sys.stderr)
    results["kafka_answers"] = cmd_kafka_consume()
    print(f"  → {results['kafka_answers'].get('count', 0)} messages", file=sys.stderr)

    # 4. Vector search
    print("[4/9] memory_vector_search...", file=sys.stderr)
    results["vector_context"] = cmd_vector_search()
    print(f"  → {results['vector_context'].get('count', 0)} results", file=sys.stderr)

    # 5. Kafka topics
    print("[5/9] memory_kafka_list...", file=sys.stderr)
    results["kafka_topics"] = cmd_kafka_list()
    print(f"  → {results['kafka_topics'].get('topics_count', 0)} topics", file=sys.stderr)

    # 6. Pending tasks
    print("[6/9] memory_cache_get...", file=sys.stderr)
    results["pending_tasks"] = cmd_cache_get()
    print(f"  → {results['pending_tasks'].get('status')}", file=sys.stderr)

    # 7. Healthcheck
    print("[7/9] memory_health...", file=sys.stderr)
    results["health"] = cmd_health()
    print(f"  → all_ok={results['health'].get('all_ok')}", file=sys.stderr)

    # 8. Save session snapshot
    print("[8/9] memory_session_save...", file=sys.stderr)
    context_json = json.dumps({
        "startup_ts": datetime.now(timezone.utc).isoformat(),
        "source": "constructor_startup.py",
        "health_all_ok": results["health"].get("all_ok"),
        "pending_count": len(results["pending_tasks"].get("value", [])) if isinstance(results["pending_tasks"].get("value"), list) else 0,
        "kafka_topics_count": results["kafka_topics"].get("topics_count", 0),
    }, ensure_ascii=False)
    results["session_save"] = cmd_session_save(context=context_json)
    print(f"  → {results['session_save'].get('status')}", file=sys.stderr)

    results["timestamp"] = datetime.now(timezone.utc).isoformat()
    results["agent_id"] = AGENT_ID

    # Write JSON
    os.makedirs(os.path.dirname(OUTPUT_FILE), exist_ok=True)
    with open(OUTPUT_FILE, "w") as f:
        json.dump(results, f, ensure_ascii=False, indent=2, default=str)
    print(file=sys.stderr)
    print(f"Saved to {OUTPUT_FILE}", file=sys.stderr)

    # Output markdown for prepend to AGENTS.md → stdout
    print(fmt_header(results))


if __name__ == "__main__":
    main()
