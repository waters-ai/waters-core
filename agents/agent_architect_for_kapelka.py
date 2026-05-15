#!/usr/bin/env python3
"""Архитектор для Капельки — проектирует план аудита сайта H2O

Читает приказы из Kafka -> orders.constructor.v1
Ходит в Ollama (localhost:11434) за архитектурным решением
Пишет план аудита в -> planners.answers.v1
"""

import json
import os
import time
import logging
from typing import Optional
from http.client import HTTPSConnection, HTTPConnection

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
log = logging.getLogger("architect-kapelka")

OLLAMA_HOST = os.getenv("OLLAMA_HOST", "localhost")
OLLAMA_PORT = int(os.getenv("OLLAMA_PORT", "11434"))
OLLAMA_MODEL = os.getenv("OLLAMA_MODEL", "qwen2.5:14b")

KAFKA_BROKER = os.getenv("KAFKA_BROKER", "171.22.180.238:9092")
KAFKA_ORDERS = "orders.constructor.v1"
KAFKA_ANSWERS = "planners.answers.v1"

POLL_INTERVAL = int(os.getenv("POLL_INTERVAL", "30"))

class ArchitectKapelka:
    def __init__(self):
        self.producer = None
        self.consumer = None
        self._init_kafka()

    def _init_kafka(self):
        try:
            from kafka import KafkaProducer, KafkaConsumer
            self.producer = KafkaProducer(
                bootstrap_servers=KAFKA_BROKER,
                value_serializer=lambda v: json.dumps(v, ensure_ascii=False).encode(),
            )
            self.consumer = KafkaConsumer(
                KAFKA_ORDERS,
                bootstrap_servers=KAFKA_BROKER,
                value_deserializer=lambda v: json.loads(v.decode()),
                group_id="architect-kapelka",
                auto_offset_reset="earliest",
            )
            log.info("Kafka connected: %s", KAFKA_BROKER)
        except Exception as e:
            log.warning("Kafka unavailable: %s (running in file-mode)", e)

    def call_ollama(self, prompt: str) -> Optional[str]:
        """Call Ollama LLM on the remote server."""
        import http.client
        conn = HTTPConnection(OLLAMA_HOST, OLLAMA_PORT, timeout=120)
        payload = json.dumps({
            "model": OLLAMA_MODEL,
            "prompt": prompt,
            "stream": False,
            "options": {"num_ctx": 32768},
        })
        try:
            conn.request("POST", "/api/generate", payload, {"Content-Type": "application/json"})
            resp = conn.getresponse()
            data = json.loads(resp.read())
            return data.get("response", "")
        except Exception as e:
            log.error("Ollama call failed: %s", e)
            return None
        finally:
            conn.close()

    def design_audit_plan(self, order: dict) -> dict:
        """Design an audit plan for H2O based on the order."""
        target = order.get("target_url", "https://h2o.waters.ai")
        scope = order.get("scope", "full")
        depth = order.get("depth", "quick")

        prompt = f"""Ты — Архитектор аудита сайта H2O. Спроектируй план аудита.

Цель: {target}
Объём: {scope}
Глубина: {depth}

Опиши:
1. Какие компоненты проверить (frontend, backend, infra, blockchain)
2. Какие инструменты использовать
3. Ожидаемые артефакты
4. Критерии оценки (A-F)
5. Риски

Ответь на русском языко, структурированно."""
        response = self.call_ollama(prompt)
        return {
            "type": "audit_plan",
            "agent": "agent.architect.kapelka.v1",
            "target": target,
            "plan": response,
            "scope": scope,
            "depth": depth,
            "status": "ready",
        }

    def publish_answer(self, answer: dict):
        """Publish audit plan to Kafka or file."""
        if self.producer:
            self.producer.send(KAFKA_ANSWERS, answer)
            log.info("Published to %s", KAFKA_ANSWERS)
        else:
            path = f"/home/ubuntu/waters-core/planners/answers.v1/{int(time.time())}.json"
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w") as f:
                json.dump(answer, f, ensure_ascii=False, indent=2)
            log.info("Written to %s", path)

    def run(self):
        log.info("Architect Kapelka started. Model: %s, Poll: %ds", OLLAMA_MODEL, POLL_INTERVAL)
        while True:
            try:
                if self.consumer:
                    for msg in self.consumer:
                        order = msg.value
                        log.info("Received order: %s", order.get("message_id", "unknown"))
                        plan = self.design_audit_plan(order)
                        self.publish_answer(plan)
                else:
                    # File-mode: watch for new orders
                    orders_dir = "/home/ubuntu/waters-core/orders.constructor.v1"
                    os.makedirs(orders_dir, exist_ok=True)
                    for fname in os.listdir(orders_dir):
                        fpath = os.path.join(orders_dir, fname)
                        with open(fpath) as f:
                            order = json.load(f)
                        plan = self.design_audit_plan(order)
                        self.publish_answer(plan)
                        os.rename(fpath, fpath + ".done")
                    time.sleep(POLL_INTERVAL)
            except KeyboardInterrupt:
                log.info("Shutting down")
                break
            except Exception as e:
                log.error("Error: %s", e)
                time.sleep(POLL_INTERVAL)

if __name__ == "__main__":
    agent = ArchitectKapelka()
    agent.run()
