#!/usr/bin/env python3
"""Аудитор для Капельки — выполняет аудит сайта H2O

Читает конфиги из Kafka -> planners.answers.v1
Ходит в Ollama (localhost:11434) за аудиторскими отчётами
Пишет результаты аудита в -> planners.answers.v1 и -> alerts.security.v1
"""

import json
import os
import time
import logging
import subprocess
from http.client import HTTPConnection
from typing import Optional

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
log = logging.getLogger("auditor-kapelka")

OLLAMA_HOST = os.getenv("OLLAMA_HOST", "localhost")
OLLAMA_PORT = int(os.getenv("OLLAMA_PORT", "11434"))
OLLAMA_MODEL = os.getenv("OLLAMA_MODEL", "qwen2.5:14b")

KAFKA_BROKER = os.getenv("KAFKA_BROKER", "171.22.180.238:9092")
KAFKA_ANSWERS = "planners.answers.v1"
KAFKA_ALERTS = "alerts.security.v1"

POLL_INTERVAL = int(os.getenv("POLL_INTERVAL", "30"))

class AuditorKapelka:
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
                group_id="auditor-kapelka",
                auto_offset_reset="earliest",
            )
            log.info("Kafka connected: %s", KAFKA_BROKER)
        except Exception as e:
            log.warning("Kafka unavailable: %s", e)

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

    def run_external_tools(self, target: str) -> dict:
        """Run external audit tools if available."""
        results = {"ssl": None, "lighthouse": None}

        # SSL check via openssl
        hostname = target.replace("https://", "").split("/")[0]
        try:
            result = subprocess.run(
                ["openssl", "s_client", "-connect", f"{hostname}:443", "-servername", hostname],
                capture_output=True, text=True, timeout=15,
            )
            results["ssl"] = result.stdout[:1000]
        except Exception as e:
            log.warning("SSL check failed: %s", e)

        return results

    def perform_audit(self, config: dict) -> dict:
        target = config.get("target", "https://h2o.waters.ai")
        tool_results = self.run_external_tools(target)

        prompt = f"""Ты — Аудитор сайта H2O. Выполни аудит.

Цель: {target}
Конфигурация: {json.dumps(config.get("config", ""), ensure_ascii=False)[:500]}
Результаты инструментов: {json.dumps(tool_results, ensure_ascii=False)[:500]}

Проверь:
1. SSL/TLS — протоколы, сертификаты, HSTS
2. Security headers — CSP, X-Frame-Options, CORS
3. Рекомендации по исправлению

Ответь на русском, структурированно, с оценкой (A-F)."""
        response = self.call_ollama(prompt)
        is_critical = "A" not in (response or "")[:50]

        report = {
            "type": "audit_report",
            "agent": "agent.auditor.kapelka.v1",
            "target": target,
            "report": response,
            "tools": tool_results,
            "status": "completed",
        }

        if is_critical:
            report["severity"] = "critical"
            self.publish_alert(report)

        return report

    def publish_answer(self, answer: dict):
        if self.producer:
            self.producer.send(KAFKA_ANSWERS, answer)
            log.info("Published audit to %s", KAFKA_ANSWERS)
        else:
            path = f"/home/ubuntu/waters-core/planners/answers.v1/audit_{int(time.time())}.json"
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w") as f:
                json.dump(answer, f, ensure_ascii=False, indent=2)
            log.info("Written to %s", path)

    def publish_alert(self, alert: dict):
        if self.producer:
            self.producer.send(KAFKA_ALERTS, alert)
            log.warning("Critical alert published to %s", KAFKA_ALERTS)

    def run(self):
        log.info("Auditor Kapelka started. Model: %s", OLLAMA_MODEL)
        while True:
            try:
                if self.consumer:
                    for msg in self.consumer:
                        config = msg.value
                        if config.get("type") == "infrastructure_config":
                            log.info("Received config for: %s", config.get("target"))
                            report = self.perform_audit(config)
                            self.publish_answer(report)
                else:
                    answers_dir = "/home/ubuntu/waters-core/planners/answers.v1"
                    os.makedirs(answers_dir, exist_ok=True)
                    for fname in os.listdir(answers_dir):
                        if fname.endswith(".done"):
                            continue
                        fpath = os.path.join(answers_dir, fname)
                        with open(fpath) as f:
                            config = json.load(f)
                        if config.get("type") == "infrastructure_config":
                            report = self.perform_audit(config)
                            self.publish_answer(report)
                            os.rename(fpath, fpath + ".done")
                    time.sleep(POLL_INTERVAL)
            except KeyboardInterrupt:
                log.info("Shutting down")
                break
            except Exception as e:
                log.error("Error: %s", e)
                time.sleep(POLL_INTERVAL)

if __name__ == "__main__":
    agent = AuditorKapelka()
    agent.run()
