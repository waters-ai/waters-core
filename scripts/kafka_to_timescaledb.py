#!/usr/bin/env python3
"""
kafka_to_timescaledb.py — стриминг Kafka-топиков в TimescaleDB

Читает все топики Kafka и пишет в соответствующие hypertables TimescaleDB.
Запуск: python3 scripts/kafka_to_timescaledb.py

Для работы нужен SSH-туннель: ssh -fNL 25432:localhost:5432 ubuntu@171.22.180.237
"""

import json
import logging
import os
import signal
import sys
import time
from datetime import datetime, timezone
from collections import defaultdict

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [kafka2ts] %(message)s",
)
logger = logging.getLogger("kafka2ts")

PG_DSN = os.environ.get("PG_DSN", "host=localhost port=25432 dbname=waters_analytics user=postgres password=waters_telemetry")
KAFKA_BOOTSTRAP = os.environ.get("KAFKA_BOOTSTRAP", "localhost:9092")
BATCH_SIZE = int(os.environ.get("BATCH_SIZE", "100"))
BATCH_INTERVAL = float(os.environ.get("BATCH_INTERVAL", "10.0"))

# Маппинг топиков → hypertables + парсеры
TOPIC_MAP = {
    "planners": ("planners_messages", lambda m: {
        "topic": m.get("topic", ""), "msg_id": m.get("msg_id", ""),
        "from_agent": m.get("from", ""), "to_agent": m.get("to", ""),
        "msg_type": m.get("type", ""), "payload": json.dumps(m.get("payload", {})),
    }),
    "events": ("system_events", lambda m: {
        "event_type": m.get("event_type", m.get("type", "")),
        "source": m.get("source", m.get("from", "")),
        "severity": m.get("severity", "info"),
        "data": json.dumps(m),
    }),
    "tasks": ("task_events", lambda m: {
        "task_id": m.get("task_id", m.get("msg_id", "")),
        "event_type": m.get("type", "assigned"),
        "agent_id": m.get("agent", m.get("to", "")),
        "payload": json.dumps(m),
    }),
    "alerts": ("alerts_log", lambda m: {
        "alert_type": m.get("alert_type", m.get("type", "unknown")),
        "severity": m.get("severity", "high"),
        "source": m.get("source", m.get("from", "")),
        "message": json.dumps(m.get("payload", m)),
        "data": json.dumps(m),
    }),
    "metrics": ("raw_metrics", lambda m: {
        "metric_name": m.get("metric", m.get("name", "unknown")),
        "metric_value": float(m.get("value", m.get("val", 0))),
        "labels": json.dumps({k: v for k, v in m.items() if k not in ("metric", "value", "time")}),
    }),
    "external": ("external_log", lambda m: {
        "direction": "in" if "request" in m.get("type", "") else "out",
        "request_id": m.get("msg_id", ""),
        "source": m.get("source", m.get("from", "")),
        "payload": json.dumps(m),
    }),
    "space": ("space_telemetry", lambda m: {
        "target": m.get("target", ""), "observer": m.get("observer", ""),
        "distance_km": float(m.get("distance_km", 0)),
        "position_km": json.dumps(m.get("position_km", [])),
        "velocity_km_s": json.dumps(m.get("velocity_km_s", [])),
        "lt_seconds": float(m.get("lt_seconds", 0)),
        "source": m.get("source", "space_bridge"),
    }),
}

running = True


def signal_handler(sig, frame):
    global running
    logger.info("Shutting down...")
    running = False


signal.signal(signal.SIGINT, signal_handler)
signal.signal(signal.SIGTERM, signal_handler)


def get_pg_conn():
    import psycopg2
    parts = PG_DSN.split()
    params = {}
    for p in parts:
        if "=" in p:
            k, v = p.split("=", 1)
            params[k] = v
    return psycopg2.connect(**params)


def get_kafka_consumer():
    from kafka import KafkaConsumer
    from kafka.admin import KafkaAdminClient

    admin = KafkaAdminClient(bootstrap_servers=KAFKA_BOOTSTRAP)
    topics = admin.list_topics()
    admin.close()

    return KafkaConsumer(
        *topics,
        bootstrap_servers=KAFKA_BOOTSTRAP,
        auto_offset_reset="latest",
        enable_auto_commit=True,
        value_deserializer=lambda v: json.loads(v.decode()) if v else None,
        consumer_timeout_ms=5000,
    ), topics


def route_message(topic, msg_value):
    if not msg_value:
        return None, None

    topic_prefix = topic.split(".")[0] if "." in topic else topic
    if topic_prefix in TOPIC_MAP:
        table, parser = TOPIC_MAP[topic_prefix]
        return table, parser(msg_value)
    return None, None


def batch_insert(conn, batches):
    with conn.cursor() as cur:
        for table, rows in batches.items():
            if not rows:
                continue
            try:
                columns = list(rows[0].keys())
                placeholders = ", ".join([f"%({c})s" for c in columns])
                cols = ", ".join(columns)
                sql = f"INSERT INTO {table} (time, {cols}) VALUES (NOW(), {placeholders})"
                cur.executemany(sql, rows)
            except Exception as e:
                logger.warning(f"Insert error [{table}]: {e}")
    conn.commit()


def ensure_space_topics(topics):
    """Ensure space.* topics exist in Kafka"""
    if "space.ephemeris.v1" not in topics:
        from kafka.admin import KafkaAdminClient, NewTopic
        admin = KafkaAdminClient(bootstrap_servers=KAFKA_BOOTSTRAP)
        admin.create_topics([NewTopic("space.ephemeris.v1", num_partitions=1, replication_factor=1)])
        admin.close()
        logger.info("Created topic: space.ephemeris.v1")


def main():
    logger.info("=== Kafka → TimescaleDB Consumer ===")
    logger.info(f"Kafka: {KAFKA_BOOTSTRAP}")
    logger.info(f"PostgreSQL: {PG_DSN}")
    logger.info(f"Batch: {BATCH_SIZE}msgs / {BATCH_INTERVAL}s")

    consumer, topics = get_kafka_consumer()
    logger.info(f"Consuming from {len(topics)} topics")
    ensure_space_topics(topics)

    conn = get_pg_conn()
    logger.info("PostgreSQL connected")

    batches = defaultdict(list)
    last_flush = time.time()
    total_inserted = 0

    while running:
        msg_pack = consumer.poll(timeout_ms=1000)
        for tp, messages in msg_pack.items():
            topic = tp.topic
            for msg in messages:
                table, row = route_message(topic, msg.value)
                if table and row:
                    batches[table].append(row)

        should_flush = (
            sum(len(v) for v in batches.values()) >= BATCH_SIZE
            or (time.time() - last_flush) >= BATCH_INTERVAL
        )

        if should_flush and batches:
            total = sum(len(v) for v in batches.values())
            try:
                batch_insert(conn, dict(batches))
                total_inserted += total
                logger.info(f"Inserted {total} rows (total: {total_inserted})")
            except Exception as e:
                logger.error(f"Batch insert failed: {e}")
                conn = get_pg_conn()
            batches.clear()
            last_flush = time.time()

    consumer.close()
    conn.close()
    logger.info(f"Stopped. Total inserted: {total_inserted}")


if __name__ == "__main__":
    main()
