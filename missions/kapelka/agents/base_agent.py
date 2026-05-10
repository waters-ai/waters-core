#!/usr/bin/env python3
"""base_agent.py — общая база для агентов Kapelka

Предоставляет:
- Параметризацию через переменные окружения (не хардкод)
- Два режима: Kafka и file-протокол
- Ollama-клиент для вызова LLM
- Структурированное логирование
- Heartbeat для supervisord

Использование в наследниках:
    class MyAgent(BaseAgent):
        def process(self, data: dict) -> dict:
            # твоя логика
            return result
"""

import json
import os
import time
import logging
import threading
import uuid
from typing import Optional, Callable
from http.client import HTTPConnection
from abc import ABC, abstractmethod

# ===== Настройки через окружение =====
KAPELKA_MODE = os.getenv("KAPELKA_MODE", "file")  # "kafka" | "file"
KAPELKA_WORKSPACE = os.getenv("KAPELKA_WORKSPACE", os.path.join(os.path.dirname(os.path.dirname(__file__)), ""))
KAFKA_BROKER = os.getenv("KAFKA_BROKER", "localhost:9092")
KAFKA_ORDERS = os.getenv("KAFKA_ORDERS", "orders.constructor.v1")
KAFKA_ANSWERS = os.getenv("KAFKA_ANSWERS", "planners.answers.v1")
KAFKA_ALERTS = os.getenv("KAFKA_ALERTS", "alerts.security.v1")
KAFKA_GROUP_ID = os.getenv("KAFKA_GROUP_ID", "kapelka")
OLLAMA_HOST = os.getenv("OLLAMA_HOST", "171.22.180.237")
OLLAMA_PORT = int(os.getenv("OLLAMA_PORT", "11434"))
OLLAMA_MODEL = os.getenv("OLLAMA_MODEL", "qwen2.5:14b")
POLL_INTERVAL = int(os.getenv("POLL_INTERVAL", "30"))
HEARTBEAT_INTERVAL = int(os.getenv("HEARTBEAT_INTERVAL", "60"))
LOG_LEVEL = os.getenv("LOG_LEVEL", "INFO")


class BaseAgent(ABC):
    def __init__(self, agent_name: str, agent_code: str):
        self.agent_name = agent_name
        self.agent_code = agent_code
        self.producer = None
        self.consumer = None

        # Логгер
        self.log = logging.getLogger(agent_name)
        self.log.setLevel(getattr(logging, LOG_LEVEL.upper(), logging.INFO))
        if not self.log.handlers:
            handler = logging.StreamHandler()
            handler.setFormatter(logging.Formatter("%(asctime)s [%(levelname)s] %(name)s %(message)s"))
            self.log.addHandler(handler)

        # Инициализация
        self._ensure_workspace()
        self._init_kafka()
        self._heartbeat_count = 0
        self._heartbeat_thread = None

    def _ensure_workspace(self):
        """Создаёт рабочие директории, если их нет."""
        dirs = [
            os.path.join(KAPELKA_WORKSPACE, "reports"),
            os.path.join(KAPELKA_WORKSPACE, "logs"),
        ]
        for d in dirs:
            os.makedirs(d, exist_ok=True)

    def _init_kafka(self):
        """Инициализация Kafka (если доступна)."""
        if KAPELKA_MODE != "kafka":
            self.log.info("Kafka-режим отключён (KAPELKA_MODE=%s)", KAPELKA_MODE)
            return
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
                group_id=KAFKA_GROUP_ID,
                auto_offset_reset="earliest",
            )
            self.log.info("Kafka подключён: %s (group=%s)", KAFKA_BROKER, KAFKA_GROUP_ID)
        except Exception as e:
            self.log.warning("Kafka недоступен: %s (работаем в file-режиме)", e)

    def call_ollama(self, prompt: str, system: Optional[str] = None, timeout: int = 120) -> Optional[str]:
        """Вызов Ollama LLM на удалённом сервере."""
        conn = HTTPConnection(OLLAMA_HOST, OLLAMA_PORT, timeout=timeout)
        payload = {
            "model": OLLAMA_MODEL,
            "prompt": prompt,
            "stream": False,
            "options": {"num_ctx": 32768},
        }
        if system:
            payload["system"] = system

        try:
            conn.request("POST", "/api/generate", json.dumps(payload), {"Content-Type": "application/json"})
            resp = conn.getresponse()
            data = json.loads(resp.read())
            result = data.get("response", "")
            self.log.info("Ollama OK (len=%d)", len(result))
            return result
        except Exception as e:
            self.log.error("Ollama error: %s", e)
            return None
        finally:
            conn.close()

    def publish(self, data: dict, topic: Optional[str] = None):
        """Публикация в Kafka или file."""
        if not data.get("agent"):
            data["agent"] = self.agent_code
        if not data.get("timestamp"):
            data["timestamp"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        if not data.get("message_id"):
            data["message_id"] = str(uuid.uuid4())

        if self.producer and KAPELKA_MODE == "kafka":
            t = topic or KAFKA_ANSWERS
            self.producer.send(t, data)
            self.log.info("Опубликовано в Kafka/%s", t)
        else:
            t = topic or KAFKA_ANSWERS
            base = KAPELKA_WORKSPACE.replace("/home/ubuntu/waters-core", "").strip("/") or "kapelka"
            out_dir = os.path.join(KAPELKA_WORKSPACE, "reports")
            os.makedirs(out_dir, exist_ok=True)
            fname = f"{int(time.time())}_{uuid.uuid4().hex[:8]}.json"
            fpath = os.path.join(out_dir, fname)
            with open(fpath, "w") as f:
                json.dump(data, f, ensure_ascii=False, indent=2)
            self.log.info("Сохранено в %s", fpath)

    def alert(self, severity: str, title: str, details: dict):
        """Отправка алерта безопасности."""
        alert_msg = {
            "type": "security_alert",
            "severity": severity,
            "title": title,
            "details": details,
            "source": self.agent_code,
        }
        self.publish(alert_msg, topic=KAFKA_ALERTS)
        self.log.warning("🔴 АЛЕРТ [%s]: %s", severity, title)

    def read_orders_file(self) -> list:
        """Чтение приказов из file-режима."""
        orders_dir = os.path.join(KAPELKA_WORKSPACE, "orders")
        os.makedirs(orders_dir, exist_ok=True)
        orders = []
        for fname in sorted(os.listdir(orders_dir)):
            if fname.endswith(".done") or fname.startswith("."):
                continue
            fpath = os.path.join(orders_dir, fname)
            try:
                with open(fpath) as f:
                    data = json.load(f)
                orders.append((fpath, data))
            except (json.JSONDecodeError, OSError) as e:
                self.log.warning("Ошибка чтения %s: %s", fname, e)
        return orders

    def mark_done(self, fpath: str):
        """Помечает файл как обработанный."""
        os.rename(fpath, fpath + ".done")

    @abstractmethod
    def process(self, data: dict) -> Optional[dict]:
        """Обработка одного приказа — переопределить в наследнике."""
        ...

    def _heartbeat_loop(self):
        while True:
            try:
                hb_dir = os.path.join(KAPELKA_WORKSPACE, "logs", "heartbeat")
                os.makedirs(hb_dir, exist_ok=True)
                hb_path = os.path.join(hb_dir, f"{self.agent_name}.hb")
                with open(hb_path, "w") as f:
                    f.write(f"{time.time()}\n")
                self._heartbeat_count += 1
                if self._heartbeat_count % 10 == 0:
                    self.log.debug("Heartbeat OK")
            except Exception:
                pass
            time.sleep(HEARTBEAT_INTERVAL)

    def run(self):
        """Основной цикл."""
        self._heartbeat_thread = threading.Thread(target=self._heartbeat_loop, daemon=True)
        self._heartbeat_thread.start()
        self.log.info("%s (%s) запущен. Режим: %s, Ollama: %s:%s/%s",
                      self.agent_name, self.agent_code, KAPELKA_MODE,
                      OLLAMA_HOST, OLLAMA_PORT, OLLAMA_MODEL)

        if KAPELKA_MODE == "kafka" and self.consumer:
            self._run_kafka()
        else:
            self._run_file()

    def _run_kafka(self):
        """Цикл в Kafka-режиме."""
        try:
            for msg in self.consumer:
                data = msg.value
                self.log.info("Приказ: %s", data.get("message_id", "?"))
                result = self.process(data)
                if result:
                    self.publish(result)
        except KeyboardInterrupt:
            self.log.info("Остановка")
        except Exception as e:
            self.log.error("Ошибка: %s", e)

    def _run_file(self):
        """Цикл в file-режиме."""
        try:
            while True:
                for fpath, data in self.read_orders_file():
                    self.log.info("Приказ: %s", os.path.basename(fpath))
                    result = self.process(data)
                    if result:
                        self.publish(result)
                    self.mark_done(fpath)
                time.sleep(POLL_INTERVAL)
        except KeyboardInterrupt:
            self.log.info("Остановка")
        except Exception as e:
            self.log.error("Ошибка: %s", e)
            time.sleep(POLL_INTERVAL)
