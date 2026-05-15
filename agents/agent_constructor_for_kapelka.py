#!/usr/bin/env python3
"""Конструктор для Капельки — готовит инфраструктуру под аудит сайта H2O

Читает план аудита из Kafka -> planners.answers.v1
Ходит в Ollama (localhost:11434) за инфраструктурными решениями
Пишет готовые конфиги в -> planners.answers.v1
"""

import json
import os
import time
import logging
from http.client import HTTPConnection
from typing import Optional

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
log = logging.getLogger("constructor-kapelka")

OLLAMA_HOST = os.getenv("OLLAMA_HOST", "localhost")
OLLAMA_PORT = int(os.getenv("OLLAMA_PORT", "11434"))
OLLAMA_MODEL = os.getenv("OLLAMA_MODEL", "qwen2.5:14b")

KAFKA_BROKER = os.getenv("KAFKA_BROKER", "171.22.180.238:9092")
KAFKA_ANSWERS = "planners.answers.v1"

POLL_INTERVAL = int(os.getenv("POLL_INTERVAL", "30"))

class ConstructorKapelka:
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
                KAFKA_ANSWERS,
                bootstrap_servers=KAFKA_BROKER,
                value_deserializer=lambda v: json.loads(v.decode()),
                group_id="constructor-kapelka",
                auto_offset_reset="earliest",
            )
            log.info("Kafka connected: %s", KAFKA_BROKER)
        except Exception as e:
            log.warning("Kafka unavailable: %s (running in file-mode)", e)

    def call_ollama(self, prompt: str) -> Optional[str]:
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

    def generate_configs(self, audit_plan: dict) -> dict:
        target = audit_plan.get("target", "https://h2o.waters.ai")
        scope = audit_plan.get("scope", "full")

        prompt = f"""Ты — Конструктор аудита H2O. Подготовь инфраструктуру.

План аудита: {audit_plan.get("plan", "")[:500]}
Цель: {target}
Объём: {scope}

Сгенерируй:
1. Docker-команды для запуска инструментов аудита (Lighthouse, npm audit, testssl)
2. Структуру отчёта
3. Переменные окружения

Ответь на русском, структурированно."""
        response = self.call_ollama(prompt)
        return {
            "type": "infrastructure_config",
            "agent": "agent.constructor.kapelka.v1",
            "target": target,
            "config": response,
            "status": "ready",
        }

    def publish_answer(self, answer: dict):
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
        log.info("Constructor Kapelka started. Model: %s", OLLAMA_MODEL)
        while True:
            try:
                if self.consumer:
                    for msg in self.consumer:
                        plan = msg.value
                        if plan.get("type") == "audit_plan":
                            log.info("Received audit plan: %s", plan.get("target"))
                            config = self.generate_configs(plan)
                            self.publish_answer(config)
                else:
                    answers_dir = "/home/ubuntu/waters-core/planners/answers.v1"
                    os.makedirs(answers_dir, exist_ok=True)
                    for fname in os.listdir(answers_dir):
                        if fname.endswith(".done"):
                            continue
                        fpath = os.path.join(answers_dir, fname)
                        with open(fpath) as f:
                            plan = json.load(f)
                        if plan.get("type") == "audit_plan":
                            config = self.generate_configs(plan)
                            self.publish_answer(config)
                            os.rename(fpath, fpath + ".done")
                    time.sleep(POLL_INTERVAL)
            except KeyboardInterrupt:
                log.info("Shutting down")
                break
            except Exception as e:
                log.error("Error: %s", e)
                time.sleep(POLL_INTERVAL)

if __name__ == "__main__":
    agent = ConstructorKapelka()
    agent.run()
