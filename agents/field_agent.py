#!/usr/bin/env python3
"""
Field Agent (Scout) v1.0 — DMZ-based information gathering subagent.
Подчиняется agent.integrator.v1. Работает на 167, без MCP-доступа.

Каналы ввода:
  - Kafka (tasks.assigned.v1) — задания от Интегратора
  - Telegram bot — прямые запросы CEO

Поисковые движки:
  - DuckDuckGo (duckduckgo-search)
  - TinyFish REST API
  - YouTube Transcript (youtube-transcript-api)
  - Qwen Deep Research (dashscope)

Валидация: NotebookLM (notebooklm-py) → quality_score < 0.7 = удаление
Доставка: scp на 238 → /home/ubuntu/WATERS/repos/waters-core/agents/scout/data/
Уведомление: Kafka → data.ready.v1

YASA compliance:
  - Секреты в .secret_* файлах (YASA-DUTY-5)
  - Комментарии на русском, нейминг на английском (YASA-FMT-5)
  - Никаких ключей в коде (YASA-PROH-5)
"""

import asyncio
import hashlib
import json
import logging
import os
import subprocess
import sys
import threading
import time
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

import requests

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler("/home/waters-data/logs/field_agent.log")
    ]
)
log = logging.getLogger("field_agent")

AGENT_ID = "agent.scout.v1"
RAW_BASE = "/home/waters-data/raw"
TARGET_HOST = "ubuntu@171.22.180.238"
TARGET_BASE = "/home/ubuntu/WATERS/repos/waters-core/agents/scout/data"
KAFKA_BROKER = "171.22.180.238:9092"
SECRET_DIR = Path(os.path.expanduser("~")) / ".secrets"


def _load_secret(name: str) -> Optional[str]:
    path = SECRET_DIR / f".secret_{name}"
    if path.exists():
        return path.read_text().strip()
    env_name = f"WATERS_{name.upper()}"
    return os.environ.get(env_name)


def _ensure_dir(path: str):
    Path(path).mkdir(parents=True, exist_ok=True)


@dataclass
class Task:
    task_id: str
    query: str
    requester: str
    source_engines: list[str] = field(default_factory=lambda: ["ddg", "tinyfish", "youtube", "qwen"])
    max_results: int = 10
    language: str = "ru"
    created_at: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())


@dataclass
class SearchResult:
    title: str
    url: str
    snippet: str
    source: str
    content: str = ""
    language: str = ""


class DuckDuckGoSearch:
    def __init__(self):
        try:
            from duckduckgo_search import DDGS
            self._ddgs = DDGS
        except ImportError:
            log.error("duckduckgo-search not installed")
            self._ddgs = None

    def search(self, query: str, max_results: int = 10) -> list[SearchResult]:
        if not self._ddgs:
            return []
        results = []
        try:
            with self._ddgs() as ddgs:
                for r in ddgs.text(query, max_results=max_results):
                    results.append(SearchResult(
                        title=r.get("title", ""),
                        url=r.get("href", ""),
                        snippet=r.get("body", ""),
                        source="duckduckgo"
                    ))
        except Exception as e:
            log.warning("DuckDuckGo search failed: %s", e)
        return results


class TinyFishSearch:
    API_SEARCH = "https://api.search.tinyfish.ai"
    API_FETCH = "https://api.fetch.tinyfish.ai"

    def __init__(self):
        self._api_key = _load_secret("tinyfish_api_key")
        if not self._api_key:
            log.warning("TINYFISH_API_KEY not found")

    def search(self, query: str, max_results: int = 10, location: str = "", language: str = "ru") -> list[SearchResult]:
        if not self._api_key:
            return []
        results = []
        try:
            params = {"query": query, "limit": max_results}
            if location:
                params["location"] = location
            if language:
                params["language"] = language
            resp = requests.get(
                self.API_SEARCH,
                params=params,
                headers={"X-API-Key": self._api_key},
                timeout=30
            )
            if resp.status_code == 200:
                data = resp.json()
                for r in data.get("results", []):
                    results.append(SearchResult(
                        title=r.get("title", ""),
                        url=r.get("url", ""),
                        snippet=r.get("snippet", ""),
                        source="tinyfish"
                    ))
        except Exception as e:
            log.warning("TinyFish search failed: %s", e)
        return results


class YouTubeTranscriptSearch:
    def search(self, query: str, max_results: int = 5) -> list[SearchResult]:
        results = []
        try:
            from youtube_transcript_api import YouTubeTranscriptApi
            from youtube_transcript_api.formatters import TextFormatter
            import urllib.parse

            search_url = f"https://www.youtube.com/results?search_query={urllib.parse.quote(query)}"
            resp = requests.get(search_url, timeout=10)
            import re
            video_ids = re.findall(r'watch\?v=([a-zA-Z0-9_-]{11})', resp.text)
            video_ids = list(dict.fromkeys(video_ids))[:max_results]

            formatter = TextFormatter()
            ytt_api = YouTubeTranscriptApi()

            for vid in video_ids:
                try:
                    transcript = ytt_api.fetch(vid, languages=["ru", "en"])
                    text = formatter.format_transcript(transcript)
                    results.append(SearchResult(
                        title=f"YouTube: {vid}",
                        url=f"https://youtube.com/watch?v={vid}",
                        snippet=text[:500] if text else "",
                        content=text,
                        source="youtube"
                    ))
                except Exception:
                    continue
        except ImportError:
            log.error("youtube-transcript-api not installed")
        except Exception as e:
            log.warning("YouTube search failed: %s", e)
        return results


class QwenDeepResearch:
    def __init__(self):
        self._api_key = _load_secret("dashscope_api_key")
        if not self._api_key:
            log.warning("DASHSCOPE_API_KEY not found")

    def search(self, query: str) -> list[SearchResult]:
        if not self._api_key:
            return []
        results = []
        try:
            import dashscope
            dashscope.api_key = self._api_key
            resp = dashscope.Generation.call(
                model="qwen-plus",
                prompt=f"Проведи глубокое исследование по теме: {query}. "
                       f"Найди ключевые факты, источники, даты, авторов. "
                       f"Верни структурированный ответ с ссылками.",
                result_format="message"
            )
            if resp and resp.status_code == 200:
                text = resp.output.text if hasattr(resp.output, "text") else str(resp.output)
                results.append(SearchResult(
                    title=f"Qwen Deep Research: {query[:50]}",
                    url="",
                    snippet=text[:1000],
                    content=text,
                    source="qwen"
                ))
        except ImportError:
            log.error("dashscope not installed")
        except Exception as e:
            log.warning("Qwen Deep Research failed: %s", e)
        return results


class SearchEngine:
    def __init__(self):
        self._ddg = DuckDuckGoSearch()
        self._tinyfish = TinyFishSearch()
        self._youtube = YouTubeTranscriptSearch()
        self._qwen = QwenDeepResearch()

        self._engines = {
            "ddg": self._ddg.search,
            "tinyfish": self._tinyfish.search,
            "youtube": self._youtube.search,
            "qwen": self._qwen.search,
        }

    def search(self, task: Task) -> list[SearchResult]:
        all_results = []
        for engine_name in task.source_engines:
            engine = self._engines.get(engine_name)
            if not engine:
                log.warning("Unknown engine: %s", engine_name)
                continue
            try:
                log.info("Searching with %s: %s", engine_name, task.query)
                if engine_name == "youtube":
                    results = engine(task.query, min(task.max_results, 5))
                elif engine_name == "qwen":
                    results = engine(task.query)
                else:
                    results = engine(task.query, task.max_results)
                all_results.extend(results)
                log.info("Found %d results from %s", len(results), engine_name)
            except Exception as e:
                log.error("Engine %s failed: %s", engine_name, e)
        return all_results


class NotebookLMValidator:
    def __init__(self):
        self._client = None

    async def _get_client(self):
        if self._client is None:
            try:
                from notebooklm import NotebookLMClient
                self._client = await NotebookLMClient.from_storage()
            except Exception as e:
                log.error("Failed to init NotebookLM: %s", e)
        return self._client

    async def validate(self, file_path: str, query: str) -> dict:
        result = {
            "quality_score": 0.5,
            "is_valid_source": False,
            "is_relevant": False,
            "issues": [],
            "summary": ""
        }
        client = await self._get_client()
        if not client:
            return result

        try:
            nb = await client.notebooks.create(f"Validation: {Path(file_path).name}")
            await client.sources.add_file(nb.id, file_path, wait=True)

            valid_check = await client.chat.ask(
                nb.id,
                f"Оцени источник по шкале 0-1: это научный источник, "
                f"любительский или фейк? Ответь только числом."
            )
            relevance_check = await client.chat.ask(
                nb.id,
                f"Релевантен ли этот документ запросу: '{query}'? "
                f"Ответь числом от 0 до 1."
            )
            quality_check = await client.chat.ask(
                nb.id,
                f"Оцени качество данных: есть ли ссылки, даты, авторы? "
                f"Ответь числом от 0 до 1."
            )

            def extract_score(text: str) -> float:
                import re
                matches = re.findall(r'0\.\d+|1\.0|1|0', text)
                return float(matches[0]) if matches else 0.5

            validity = extract_score(valid_check.answer)
            relevance = extract_score(relevance_check.answer)
            quality = extract_score(quality_check.answer)

            quality_score = (validity * 0.4 + relevance * 0.35 + quality * 0.25)
            result["quality_score"] = round(quality_score, 2)
            result["is_valid_source"] = validity >= 0.6
            result["is_relevant"] = relevance >= 0.5
            result["summary"] = valid_check.answer[:500]

            if quality_score < 0.7:
                result["issues"].append("quality_score below 0.7 threshold")

            await client.notebooks.delete(nb.id)

        except Exception as e:
            log.error("NotebookLM validation failed for %s: %s", file_path, e)
            result["issues"].append(str(e))

        return result


class QualityFilter:
    MIN_SCORE = 0.7

    def filter(self, file_path: str, validation: dict) -> bool:
        score = validation.get("quality_score", 0)
        if score < self.MIN_SCORE:
            log.info("Deleting %s (score=%.2f < %.2f)", file_path, score, self.MIN_SCORE)
            try:
                os.remove(file_path)
            except OSError as e:
                log.error("Failed to delete %s: %s", file_path, e)
            return False
        log.info("File %s passed filter (score=%.2f)", file_path, score)
        return True


class SCPCopier:
    def __init__(self):
        self._ssh_key = _load_secret("ssh_key_path") or os.path.expanduser("~/.ssh/id_rsa")

    def copy(self, local_path: str, task: Task) -> Optional[str]:
        date_str = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        target_dir = f"{TARGET_BASE}/{task.task_id}/{date_str}"
        try:
            subprocess.run(
                ["ssh", "-i", self._ssh_key, "-o", "StrictHostKeyChecking=no",
                 TARGET_HOST, f"mkdir -p {target_dir}"],
                check=True, capture_output=True, timeout=30
            )
            result = subprocess.run(
                ["scp", "-i", self._ssh_key, "-o", "StrictHostKeyChecking=no",
                 local_path, f"{TARGET_HOST}:{target_dir}/"],
                check=True, capture_output=True, timeout=60
            )
            remote_path = f"{target_dir}/{Path(local_path).name}"
            log.info("Copied %s to %s:%s", local_path, TARGET_HOST, remote_path)
            return remote_path
        except subprocess.CalledProcessError as e:
            log.error("SCP failed: %s", e.stderr.decode())
            return None


class FileSaver:
    def __init__(self):
        self._agent_dir = f"{RAW_BASE}/scout"

    def save(self, task: Task, results: list[SearchResult]) -> list[str]:
        date_str = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        save_dir = f"{self._agent_dir}/{date_str}"
        _ensure_dir(save_dir)
        saved_files = []

        for i, r in enumerate(results):
            filename = f"{task.task_id}_{i}_{r.source}_{int(time.time())}.json"
            filepath = f"{save_dir}/{filename}"
            content = {
                "task_id": task.task_id,
                "query": task.query,
                "source": r.source,
                "title": r.title,
                "url": r.url,
                "snippet": r.snippet,
                "content": r.content,
                "fetched_at": datetime.now(timezone.utc).isoformat(),
                "checksum_sha256": hashlib.sha256(
                    (r.title + r.url + r.content).encode()
                ).hexdigest()
            }
            with open(filepath, "w", encoding="utf-8") as f:
                json.dump(content, f, ensure_ascii=False, indent=2)
            saved_files.append(filepath)

        log.info("Saved %d files to %s", len(saved_files), save_dir)
        return saved_files


class KafkaManager:
    def __init__(self):
        self._producer = None
        self._consumer = None
        self._running = False

    def _get_producer(self):
        if self._producer is None:
            try:
                from kafka import KafkaProducer
                self._producer = KafkaProducer(
                    bootstrap_servers=KAFKA_BROKER,
                    value_serializer=lambda v: json.dumps(v, ensure_ascii=False).encode("utf-8"),
                    acks="all",
                    retries=3
                )
            except Exception as e:
                log.error("Failed to create Kafka producer: %s", e)
        return self._producer

    def _get_consumer(self, group_id: str = "field-agent"):
        try:
            from kafka import KafkaConsumer
            return KafkaConsumer(
                "tasks.assigned.v1",
                bootstrap_servers=KAFKA_BROKER,
                group_id=group_id,
                value_deserializer=lambda v: json.loads(v.decode("utf-8")),
                auto_offset_reset="latest",
                enable_auto_commit=True
            )
        except Exception as e:
            log.error("Failed to create Kafka consumer: %s", e)
        return None

    def send_data_ready(self, task: Task, path: str, summary: str,
                         quality_score: float, files: list[dict]):
        producer = self._get_producer()
        if not producer:
            return
        message = {
            "type": "data_ready",
            "agent": "scout",
            "task_id": task.task_id,
            "path": path,
            "summary": summary,
            "quality_score": quality_score,
            "files_count": len(files),
            "files": files,
            "timestamp": datetime.now(timezone.utc).isoformat()
        }
        try:
            producer.send("data.ready.v1", message)
            producer.flush()
            log.info("Sent data_ready for task %s", task.task_id)
        except Exception as e:
            log.error("Failed to send data_ready: %s", e)

    def consume_tasks(self, callback, stop_event: threading.Event):
        consumer = self._get_consumer()
        if not consumer:
            log.error("Cannot consume tasks — Kafka unavailable")
            return
        log.info("Listening on tasks.assigned.v1...")
        try:
            for msg in consumer:
                if stop_event.is_set():
                    break
                try:
                    task_data = msg.value
                    if not isinstance(task_data, dict):
                        continue
                    task = Task(
                        task_id=task_data.get("task_id", str(uuid.uuid4())),
                        query=task_data.get("query", ""),
                        requester=task_data.get("requester", "unknown"),
                        source_engines=task_data.get("source_engines", ["ddg", "tinyfish", "youtube", "qwen"]),
                        max_results=task_data.get("max_results", 10)
                    )
                    if task.query:
                        log.info("Received task from Kafka: %s", task.task_id)
                        callback(task)
                except Exception as e:
                    log.error("Error processing Kafka message: %s", e)
        except Exception as e:
            log.error("Kafka consumer error: %s", e)


class TelegramBot:
    def __init__(self, task_callback):
        self._task_callback = task_callback
        self._token = _load_secret("telegram_token")
        self._allowed_users = set()

    def _load_allowed_users(self):
        users_file = SECRET_DIR / ".secret_telegram_users"
        if users_file.exists():
            for line in users_file.read_text().strip().splitlines():
                if line.strip():
                    self._allowed_users.add(line.strip())

    def start(self, stop_event: threading.Event):
        if not self._token:
            log.warning("TELEGRAM_BOT_TOKEN not set — Telegram bot disabled")
            return
        self._load_allowed_users()
        try:
            import telegram
            from telegram.ext import Application, CommandHandler, MessageHandler, filters

            app = Application.builder().token(self._token).build()

            async def start_cmd(update, context):
                uid = str(update.effective_user.id)
                if self._allowed_users and uid not in self._allowed_users:
                    await update.message.reply_text("Доступ запрещён.")
                    return
                await update.message.reply_text(
                    "Scout Agent ready.\n"
                    "Формат: search: <запрос>\n"
                    "Пример: search: последние новости ИИ 2026"
                )

            async def handle_message(update, context):
                uid = str(update.effective_user.id)
                if self._allowed_users and uid not in self._allowed_users:
                    return
                text = update.message.text or ""
                if text.lower().startswith("search:"):
                    query = text[7:].strip()
                    if query:
                        task = Task(
                            task_id=str(uuid.uuid4()),
                            query=query,
                            requester=f"telegram:{uid}"
                        )
                        log.info("Received task from Telegram: %s", task.task_id)
                        self._task_callback(task)
                        await update.message.reply_text(f"Задача принята: {task.task_id}")
                    else:
                        await update.message.reply_text("Пустой запрос.")
                else:
                    await update.message.reply_text(
                        "Используй формат: search: <запрос>"
                    )

            app.add_handler(CommandHandler("start", start_cmd))
            app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, handle_message))

            log.info("Telegram bot started")
            app.run_polling(stop_signals=[], close_loop=False)

        except ImportError:
            log.error("python-telegram-bot not installed")
        except Exception as e:
            log.error("Telegram bot error: %s", e)


class FieldAgent:
    def __init__(self):
        self._engine = SearchEngine()
        self._validator = NotebookLMValidator()
        self._filter = QualityFilter()
        self._copier = SCPCopier()
        self._saver = FileSaver()
        self._kafka = KafkaManager()
        self._stop_event = threading.Event()

    def _process_task(self, task: Task):
        log.info("Processing task %s: %s", task.task_id, task.query)
        try:
            results = self._engine.search(task)
            if not results:
                log.warning("No results for task %s", task.task_id)
                return

            saved_files = self._saver.save(task, results)

            validated_files = []
            total_score = 0.0
            all_summaries = []

            for fpath in saved_files:
                validation = asyncio.run(self._validator.validate(fpath, task.query))
                if self._filter.filter(fpath, validation):
                    remote_path = self._copier.copy(fpath, task)
                    if remote_path:
                        validated_files.append({
                            "filename": Path(fpath).name,
                            "size_bytes": os.path.getsize(fpath),
                            "source_url": remote_path,
                            "checksum_sha256": hashlib.sha256(
                                open(fpath, "rb").read()
                            ).hexdigest()
                        })
                        total_score += validation.get("quality_score", 0)
                        all_summaries.append(validation.get("summary", ""))

            avg_score = total_score / len(validated_files) if validated_files else 0
            combined_summary = " | ".join(all_summaries[:5]) if all_summaries else "Данные собраны"

            self._kafka.send_data_ready(
                task=task,
                path=f"{TARGET_BASE}/{task.task_id}",
                summary=combined_summary[:2000],
                quality_score=round(avg_score, 2),
                files=validated_files
            )

            log.info("Task %s completed: %d/%d files passed QC",
                     task.task_id, len(validated_files), len(saved_files))

        except Exception as e:
            log.error("Task %s failed: %s", task.task_id, e)

    def run(self):
        log.info("=" * 60)
        log.info("Field Agent (Scout) v1.0 starting...")
        log.info("Agent ID: %s", AGENT_ID)
        log.info("Kafka: %s", KAFKA_BROKER)
        log.info("=" * 60)

        telegram = TelegramBot(self._process_task)
        tgram_thread = threading.Thread(
            target=telegram.start, args=(self._stop_event,), daemon=True
        )
        tgram_thread.start()

        kafka_thread = threading.Thread(
            target=self._kafka.consume_tasks,
            args=(self._process_task, self._stop_event),
            daemon=True
        )
        kafka_thread.start()

        try:
            while not self._stop_event.is_set():
                time.sleep(1)
        except KeyboardInterrupt:
            log.info("Shutting down...")
            self._stop_event.set()


if __name__ == "__main__":
    _ensure_dir("/home/waters-data/logs")
    _ensure_dir(RAW_BASE)
    agent = FieldAgent()
    agent.run()
