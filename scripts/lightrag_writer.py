#!/usr/bin/env python3
"""
lightrag_writer.py — Фоновый writer для LightRAG
Читает очередь из Redis (lightrag:queue) и постепенно пишет в LightRAG.
Запускается как демон, не блокирует MCP-мост.

Запуск:
  python3 scripts/lightrag_writer.py                    # однократный проход
  python3 scripts/lightrag_writer.py --daemon            # бесконечный цикл
  python3 scripts/lightrag_writer.py --daemon --interval 30  # проверка каждые 30с
  python3 scripts/lightrag_writer.py --once              # один заход и выход

Переменные окружения:
  REDIS_HOST       — localhost
  REDIS_PORT       — 6379
  LIGHTRAG_DIR     — /data/lightrag
  LR_WRITER_INTERVAL — секунд между проверками (по умолч. 15)
  LR_WRITER_BATCH   — записей за один раз (по умолч. 3)
"""

import json
import logging
import os
import sys
import time
from datetime import datetime, timezone

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [lightrag-writer] %(message)s",
)
logger = logging.getLogger("lightrag-writer")

REDIS_HOST = os.environ.get("REDIS_HOST", "localhost")
REDIS_PORT = int(os.environ.get("REDIS_PORT", "6379"))
LIGHTRAG_DIR = os.environ.get("LIGHTRAG_DIR", "/data/lightrag")
QUEUE_KEY = "lightrag:queue"
POLL_INTERVAL = int(os.environ.get("LR_WRITER_INTERVAL", "15"))
BATCH_SIZE = int(os.environ.get("LR_WRITER_BATCH", "3"))


def get_redis():
    try:
        import redis as r
        conn = r.Redis(host=REDIS_HOST, port=REDIS_PORT, db=0, socket_connect_timeout=5)
        conn.ping()
        return conn
    except Exception as e:
        logger.warning(f"Redis unavailable: {e}")
        return None


def get_lightrag():
    try:
        from lightrag import LightRAG as LR
        from lightrag.llm.ollama import ollama_embed, ollama_model_complete

        rag = LR(
            working_dir=LIGHTRAG_DIR,
            llm_model_func=ollama_model_complete,
            llm_model_name="qwen2.5:7b",
            llm_model_kwargs={},
            embedding_func=ollama_embed,
        )
        import asyncio
        asyncio.run(rag.initialize_storages())
        logger.info(f"LightRAG initialized: {LIGHTRAG_DIR}")
        return rag
    except Exception as e:
        logger.error(f"LightRAG init failed: {e}")
        return None


def process_queue(rag, r):
    """Process one batch from the queue. Returns number of items processed."""
    batch = []
    for _ in range(BATCH_SIZE):
        raw = r.lpop(QUEUE_KEY)
        if raw is None:
            break
        try:
            entry = json.loads(raw) if isinstance(raw, bytes) else json.loads(raw)
            batch.append(entry)
        except (json.JSONDecodeError, TypeError):
            logger.warning(f"Invalid queue entry, skipping: {raw[:100]}")

    if not batch:
        return 0

    for entry in batch:
        text = entry.get("text", "")
        source = entry.get("source", "agent")
        if not text:
            continue
        try:
            rag.insert(text)
            logger.info(f"Inserted into LightRAG ({len(text)} chars from {source})")
        except Exception as e:
            logger.error(f"LightRAG insert failed ({len(text)} chars from {source}): {e}")
            # Push back to queue for retry
            r.rpush(QUEUE_KEY, json.dumps(entry, ensure_ascii=False))
            logger.info(f"Pushed back to queue for retry")
            time.sleep(1)

    qlen = r.llen(QUEUE_KEY) if r else 0
    logger.info(f"Batch done: {len(batch)} processed, {qlen} remaining in queue")
    return len(batch)


def run_once():
    """Single pass: process what's in the queue and exit."""
    r = get_redis()
    if not r:
        logger.error("Redis not available, exiting")
        return 1

    qlen = r.llen(QUEUE_KEY)
    if qlen == 0:
        logger.info("Queue is empty, nothing to do")
        return 0

    logger.info(f"Queue has {qlen} items")
    rag = get_lightrag()
    if not rag:
        logger.error("LightRAG not available, exiting")
        return 1

    processed = process_queue(rag, r)
    logger.info(f"Done: {processed} processed")
    return 0


def run_daemon():
    """Infinite loop: check queue every POLL_INTERVAL seconds."""
    logger.info(f"Starting LightRAG writer daemon (interval={POLL_INTERVAL}s, batch={BATCH_SIZE})")

    r = get_redis()
    if not r:
        logger.error("Redis not available, cannot start daemon")
        return 1

    # Lazy init LightRAG — only when first item appears
    rag = None

    while True:
        try:
            qlen = r.llen(QUEUE_KEY)
            if qlen > 0:
                if rag is None:
                    rag = get_lightrag()
                if rag:
                    process_queue(rag, r)
                else:
                    logger.warning("LightRAG not available, will retry")
            else:
                if qlen == 0:
                    pass  # nothing to do

            time.sleep(POLL_INTERVAL)
        except KeyboardInterrupt:
            logger.info("Shutting down")
            break
        except Exception as e:
            logger.error(f"Daemon error: {e}")
            time.sleep(POLL_INTERVAL)

    return 0


def main():
    import argparse
    global POLL_INTERVAL, BATCH_SIZE
    parser = argparse.ArgumentParser(description="LightRAG background writer")
    parser.add_argument("--daemon", action="store_true", help="Run as daemon (infinite loop)")
    parser.add_argument("--once", action="store_true", help="Process one batch and exit")
    parser.add_argument("--interval", type=int, default=POLL_INTERVAL, help=f"Poll interval in seconds (default: {POLL_INTERVAL})")
    parser.add_argument("--batch", type=int, default=BATCH_SIZE, help=f"Batch size (default: {BATCH_SIZE})")
    args = parser.parse_args()

    if args.interval:
        POLL_INTERVAL = args.interval
    if args.batch:
        BATCH_SIZE = args.batch

    if args.daemon:
        sys.exit(run_daemon())
    elif args.once:
        sys.exit(run_once())
    else:
        # Default: one pass
        sys.exit(run_once())


if __name__ == "__main__":
    main()
