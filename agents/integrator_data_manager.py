#!/usr/bin/env python3
"""
Integrator Data Manager v1.0 — агент-перегонщик на 238.
Подчиняется Основному Integrator-у.

Слушает: planners.answers.v1 (filter: type == "file_ready")
Делает:  SCP с 167 → agents/{agent}/data/
Шлёт:    delivery.ack.v1 + data_received Основному Integrator-у
"""

import json
import logging
import os
import subprocess
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler("/home/ubuntu/WATERS/repos/waters-core/logs/integrator_dm.log")
    ]
)
log = logging.getLogger("integrator-dm")

KAFKA_BROKER = "171.22.180.238:9092"
REMOTE_HOST = "ubuntu@171.22.180.167"
REMOTE_RAW_BASE = "/home/waters-data/raw"
LOCAL_AGENTS_BASE = "/home/ubuntu/WATERS/repos/waters-core/agents"
SSH_KEY = os.path.expanduser("~/.ssh/id_rsa")


def _ensure_dir(path: str):
    Path(path).mkdir(parents=True, exist_ok=True)


class SCPAgent:
    def copy_file(self, remote_path: str, local_dir: str) -> Optional[str]:
        try:
            _ensure_dir(local_dir)
            subprocess.run(
                ["ssh", "-i", SSH_KEY, "-o", "StrictHostKeyChecking=no",
                 REMOTE_HOST, f"ls -la '{remote_path}'"],
                check=True, capture_output=True, timeout=15,
            )
            result = subprocess.run(
                ["scp", "-i", SSH_KEY, "-o", "StrictHostKeyChecking=no",
                 f"{REMOTE_HOST}:'{remote_path}'", local_dir + "/"],
                check=True, capture_output=True, timeout=60,
            )
            local_path = os.path.join(local_dir, os.path.basename(remote_path))
            log.info("Copied %s → %s", remote_path, local_path)
            return local_path
        except subprocess.CalledProcessError as e:
            log.error("SCP failed for %s: %s", remote_path, e.stderr.decode()[:200])
            return None


class KafkaManager:
    def __init__(self):
        self._producer = None
        self._consumer = None

    def _get_producer(self):
        if self._producer is None:
            try:
                from kafka import KafkaProducer
                self._producer = KafkaProducer(
                    bootstrap_servers=KAFKA_BROKER,
                    value_serializer=lambda v: json.dumps(v, ensure_ascii=False).encode("utf-8"),
                    acks="all",
                    retries=3,
                )
            except Exception as e:
                log.error("Failed to create Kafka producer: %s", e)
        return self._producer

    def _get_consumer(self):
        if self._consumer is None:
            try:
                from kafka import KafkaConsumer
                self._consumer = KafkaConsumer(
                    "planners.answers.v1",
                    bootstrap_servers=KAFKA_BROKER,
                    group_id="integrator-dm",
                    value_deserializer=lambda v: json.loads(v.decode("utf-8")),
                    auto_offset_reset="latest",
                    enable_auto_commit=True,
                )
            except Exception as e:
                log.error("Failed to create Kafka consumer: %s", e)
        return self._consumer

    def send_delivery_ack(self, path: str, task_id: str, agent: str):
        producer = self._get_producer()
        if not producer:
            return
        msg = {
            "type": "delivery_ack",
            "path": path,
            "task_id": task_id,
            "target_agent": agent,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        try:
            producer.send("delivery.ack.v1", msg)
            producer.flush()
            log.info("Sent delivery_ack for %s → %s", agent, path)
        except Exception as e:
            log.error("Failed to send delivery_ack: %s", e)

    def send_data_received(self, local_path: str, task_id: str, agent: str,
                            summary: str = "", checksum: str = ""):
        producer = self._get_producer()
        if not producer:
            return
        msg = {
            "type": "data_received",
            "path": local_path,
            "task_id": task_id,
            "target_agent": agent,
            "summary": summary,
            "checksum_sha256": checksum,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        try:
            producer.send("data.received.v1", msg)
            producer.flush()
            log.info("Sent data_received for %s → %s", agent, local_path)
        except Exception as e:
            log.error("Failed to send data_received: %s", e)

    def listen(self, stop_event: threading.Event, callback):
        consumer = self._get_consumer()
        if not consumer:
            log.error("Kafka unavailable — cannot start")
            return
        log.info("Listening on planners.answers.v1 for file_ready...")
        try:
            for msg in consumer:
                if stop_event.is_set():
                    break
                try:
                    data = msg.value
                    if isinstance(data, dict) and data.get("type") == "file_ready":
                        callback(data)
                except Exception as e:
                    log.error("Error processing message: %s", e)
        except Exception as e:
            log.error("Kafka consumer error: %s", e)


class IntegratorDataManager:
    def __init__(self):
        self._scp = SCPAgent()
        self._kafka = KafkaManager()
        self._stop_event = threading.Event()

    def _process_file_ready(self, data: dict):
        remote_path = data.get("path", "")
        task_id = data.get("task_id", "")
        source_agent = data.get("agent", "scout")
        query = data.get("query", "")
        summary = data.get("summary", "")
        checksum = data.get("checksum_sha256", "")

        if not remote_path or not os.path.basename(remote_path):
            log.warning("Invalid file_ready: no path")
            return

        target_agent = data.get("requester", "scout")
        if ":" in target_agent:
            target_agent = target_agent.split(":")[0]

        date_str = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        local_dir = f"{LOCAL_AGENTS_BASE}/{target_agent}/data/{date_str}"

        log.info("Copying file for agent=%s task=%s", target_agent, task_id)
        local_path = self._scp.copy_file(remote_path, local_dir)

        if local_path:
            self._kafka.send_delivery_ack(remote_path, task_id, target_agent)
            self._kafka.send_data_received(local_path, task_id, target_agent, summary, checksum)

    def run(self):
        log.info("=" * 60)
        log.info("Integrator Data Manager v1.0 starting...")
        log.info("167 → 238 SCP pipeline")
        log.info("=" * 60)

        kafka_thread = threading.Thread(
            target=self._kafka.listen,
            args=(self._stop_event, self._process_file_ready),
            daemon=True,
            name="kafka",
        )
        kafka_thread.start()

        try:
            while not self._stop_event.is_set():
                time.sleep(1)
        except KeyboardInterrupt:
            log.info("Shutting down...")
            self._stop_event.set()


if __name__ == "__main__":
    _ensure_dir(f"{LOCAL_AGENTS_BASE}/scout/data")
    agent = IntegratorDataManager()
    agent.run()
