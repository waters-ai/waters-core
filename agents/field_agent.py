#!/usr/bin/env python3
"""
Scout Field Agent v2.0 — DMZ information gatherer.
Зона: RU. 167 → 238. $0.10/день.
"""

import asyncio
import hashlib
import json
import logging
import os
import re
import threading
import time
import uuid
from concurrent.futures import TimeoutError
from dataclasses import dataclass, field
from datetime import datetime, timezone
from logging.handlers import RotatingFileHandler
from pathlib import Path
from typing import Optional

import requests

from agents.config import (AGENT_ID, RAW_BASE, SCOUT_REGION, HEARTBEAT_INTERVAL,
                            CLEANUP_INTERVAL, TASK_TIMEOUT, CLEANUP_MAX_DAYS,
                            PRIORITY_MAP, SEARCH_ENGINES, KAFKA_BROKER,
                            LOG_PATH, SECRET_DIR)
from agents.scout_state import StateManager
from agents.scout_ratelimit import RateLimiter
from agents.scout_cleanup import CleanupScheduler

log = logging.getLogger("field_agent")
handler = RotatingFileHandler(LOG_PATH, maxBytes=10*1024*1024, backupCount=5)
handler.setFormatter(logging.Formatter("%(asctime)s [%(levelname)s] %(name)s: %(message)s"))
log.addHandler(handler)
log.addHandler(logging.StreamHandler())


def _load_secret(name: str) -> Optional[str]:
    path = os.path.join(SECRET_DIR, f".secret_{name}")
    if os.path.exists(path):
        with open(path) as f:
            return f.read().strip()
    return os.environ.get(f"WATERS_{name.upper()}")


def _ensure_dir(path: str):
    Path(path).mkdir(parents=True, exist_ok=True)


def _checksum(text: str, url: str = "", title: str = "") -> str:
    return hashlib.sha256((title + url + text).encode()).hexdigest()


# ─── Dataclasses ─────────────────────────────────────────────────

@dataclass
class Task:
    task_id: str
    query: str
    requester: str
    priority: int = 5
    source_engines: list[str] = field(default_factory=lambda: SEARCH_ENGINES.get(SCOUT_REGION, ["ddg"]))
    max_results: int = 10
    language: str = "ru"


@dataclass
class SearchResult:
    title: str
    url: str
    snippet: str
    content: str = ""
    source: str = ""
    source_rating: float = 0.5


# ─── YaCy Search ♾️ ──────────────────────────────────────────────

class YaCySearch:
    PEERS = ["https://yacy.searchlab.eu", "https://yacy.cf"]

    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit

    def search(self, query: str, max_results: int = 20) -> list[SearchResult]:
        results = []
        for peer in self.PEERS:
            try:
                self._ratelimit.acquire("yacy")
                resp = requests.get(f"{peer}/yacysearch.json", params={
                    "query": query, "maximumRecords": max_results, "resource": "global",
                }, timeout=15)
                if resp.status_code != 200:
                    continue
                data = resp.json()
                for item in data.get("channels", [{}])[0].get("items", []):
                    results.append(SearchResult(
                        title=item.get("title", ""),
                        url=item.get("link", ""),
                        snippet=item.get("snippet", ""),
                        source="yacy",
                    ))
                if results:
                    break
            except Exception as e:
                log.warning("YaCy peer %s failed: %s", peer, e)
        return results


# ─── DuckDuckGo Search ───────────────────────────────────────────

class DuckDuckGoSearch:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit
        try:
            from duckduckgo_search import DDGS
            self._ddgs = DDGS
        except ImportError:
            self._ddgs = None

    def search(self, query: str, max_results: int = 10) -> list[SearchResult]:
        if not self._ddgs:
            return []
        results = []
        try:
            self._ratelimit.acquire("duckduckgo")
            with self._ddgs(timeout=20) as ddgs:
                for r in ddgs.text(query, max_results=max_results):
                    results.append(SearchResult(
                        title=r.get("title", ""), url=r.get("href", ""),
                        snippet=r.get("body", ""), source="duckduckgo",
                    ))
        except Exception as e:
            log.warning("DDG failed: %s", e)
        return results


# ─── YouTube Search ──────────────────────────────────────────────

class YouTubeTranscriptSearch:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit

    def search(self, query: str, max_results: int = 5) -> list[SearchResult]:
        results = []
        try:
            self._ratelimit.acquire("youtube")
            resp = requests.get(
                f"https://www.youtube.com/results?search_query={requests.utils.quote(query)}",
                timeout=15,
            )
            vids = re.findall(r'watch\?v=([a-zA-Z0-9_-]{11})', resp.text)
            for vid in list(dict.fromkeys(vids))[:max_results]:
                results.append(SearchResult(
                    title=f"YouTube: {vid}", url=f"https://youtube.com/watch?v={vid}",
                    snippet="", source="youtube",
                ))
        except Exception as e:
            log.warning("YouTube failed: %s", e)
        return results

    def fetch_transcript(self, video_id: str) -> Optional[str]:
        try:
            from youtube_transcript_api import YouTubeTranscriptApi
            from youtube_transcript_api.formatters import TextFormatter
            self._ratelimit.acquire("youtube")
            transcript = YouTubeTranscriptApi().fetch(video_id, languages=["ru", "en"])
            return TextFormatter().format_transcript(transcript)
        except Exception as e:
            log.warning("Transcript failed for %s: %s", video_id, e)
            return None


# ─── Yandex.XML Search (optional) ────────────────────────────────

class YandexXMLSearch:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit
        self._key = _load_secret("yandex_xml_api_key")
        self._user = _load_secret("yandex_xml_user")

    def search(self, query: str, max_results: int = 10) -> list[SearchResult]:
        if not self._key:
            return []
        results = []
        try:
            self._ratelimit.acquire("yandex_xml")
            resp = requests.get("https://yandex.com/search/xml", params={
                "user": self._user, "key": self._key, "query": query,
                "groupby": f"attr=d.mode=flat.groups-on-page={max_results}",
            }, timeout=20)
            if resp.status_code == 200:
                from xml.etree import ElementTree
                root = ElementTree.fromstring(resp.content)
                ns = {"y": "http://yandex.com/xml"}
                for doc in root.findall(".//y:doc", ns):
                    results.append(SearchResult(
                        title=doc.findtext("y:title", "", ns),
                        url=doc.findtext("y:url", "", ns),
                        snippet=doc.findtext("y:headline", "", ns),
                        source="yandex",
                    ))
        except Exception as e:
            log.warning("Yandex.XML failed: %s", e)
        return results


# ─── Page Fetcher ────────────────────────────────────────────────

class PageFetcher:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit

    def fetch(self, url: str, timeout: int = 15) -> Optional[str]:
        if not url:
            return None
        try:
            self._ratelimit.acquire("page_fetch")
            resp = requests.get(url, headers={"User-Agent": "WATERS Scout/2.0"},
                                timeout=timeout, allow_redirects=True)
            if resp.status_code != 200:
                return None
            try:
                import trafilatura
                text = trafilatura.extract(resp.text, include_comments=False, include_tables=False)
                if text and len(text) > 50:
                    return text
            except ImportError:
                pass
            return resp.text[:10000]
        except Exception:
            return None


# ─── YandexGPT Client ────────────────────────────────────────────

class YandexGPTClient:
    API_URL = "https://llm.api.cloud.yandex.net/foundationModels/v1/completion"

    def __init__(self, state: StateManager, ratelimit: RateLimiter):
        self._state = state
        self._ratelimit = ratelimit
        self._api_key = _load_secret("yandexgpt_api_key")

    def _catalog_id(self) -> Optional[str]:
        key = self._api_key
        if key and key.startswith("AQVN"):
            return "b1g3hv7p7jqk9r8l2m4n"
        return None

    def _call(self, prompt: str, max_tokens: int = 300) -> Optional[str]:
        catalog = self._catalog_id()
        if not catalog or not self._api_key:
            return None
        if not self._state.can_use("yandexgpt"):
            return None
        try:
            self._ratelimit.acquire("yandexgpt")
            resp = requests.post(self.API_URL, json={
                "modelUri": f"gpt://{catalog}/yandexgpt-lite",
                "completionOptions": {"stream": False, "maxTokens": max_tokens},
                "messages": [{"role": "user", "text": prompt}],
            }, headers={
                "Authorization": f"Api-Key {self._api_key}",
                "Content-Type": "application/json",
            }, timeout=30)
            if resp.status_code == 200:
                data = resp.json()
                usage = data.get("usage", {})
                cost = (usage.get("inputTextTokens", 0) + usage.get("outputTextTokens", 0)) * 0.00000164
                self._state.log_usage("yandexgpt", "cost", cost)
                alt = data.get("alternatives", [])
                if alt:
                    return alt[0].get("message", {}).get("text", "")
        except Exception as e:
            log.warning("YandexGPT call failed: %s", e)
        return None

    def expand_query(self, text: str) -> list[str]:
        prompt = (
            f"Пользователь написал: '{text}'. Сгенерируй 3 коротких поисковых запроса "
            f"на русском и английском для поисковика. Верни только запросы, каждый на новой строке."
        )
        result = self._call(prompt, 200)
        if result:
            lines = [l.strip().strip("-*") for l in result.split("\n") if l.strip()]
            return lines[:4]
        return [text]

    def validate_snippet(self, snippet: str, query: str) -> dict:
        prompt = (
            f"Запрос: '{query}'. Сниппет: '{snippet[:500]}'. "
            f"Этот сниппет годен/мусор? Ответь одним словом и через пробел краткую выжимку."
        )
        result = self._call(prompt, 150)
        if result:
            verdict = "годно" if result.lower().startswith("годно") else "мусор"
            summary = result.split(" ", 1)[1] if " " in result else ""
            return {"verdict": verdict, "summary": summary[:500]}
        return {"verdict": "мусор", "summary": ""}


# ─── NotebookLM Validator ────────────────────────────────────────

class NotebookLMValidator:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit
        self._client = None

    async def _get_client(self):
        if self._client is None:
            try:
                from notebooklm import NotebookLMClient
                self._client = await NotebookLMClient.from_storage()
            except Exception:
                pass
        return self._client

    async def validate(self, snippet: str, query: str) -> dict:
        result = {"verdict": "мусор", "summary": ""}
        client = await self._get_client()
        if not client:
            return result
        try:
            self._ratelimit.acquire("notebooklm")
            nb = await asyncio.wait_for(client.notebooks.create("V"), timeout=30)
            await asyncio.wait_for(client.sources.add_text(nb.id, snippet[:3000]), timeout=30)
            v = await asyncio.wait_for(
                client.chat.ask(nb.id, f"Сниппет релевантен '{query}'? годно/мусор? Одно слово."),
                timeout=30)
            s = await asyncio.wait_for(
                client.chat.ask(nb.id, "Выжимка сниппета: 1 предложение."), timeout=30)
            await client.notebooks.delete(nb.id)
            vtext = (v.answer or "").strip().lower()
            result["verdict"] = "годно" if "годно" in vtext else "мусор"
            result["summary"] = (s.answer or "")[:500]
        except (TimeoutError, Exception) as e:
            log.warning("NotebookLM failed: %s", e)
        return result


# ─── Validator Router ─────────────────────────────────────────────

class ValidatorRouter:
    def __init__(self, state: StateManager, ratelimit: RateLimiter):
        self._notebooklm = NotebookLMValidator(ratelimit)
        self._yandexgpt = YandexGPTClient(state, ratelimit)
        self._state = state

    async def validate(self, snippet: str, query: str) -> dict:
        if self._state.can_use("notebooklm"):
            result = await self._notebooklm.validate(snippet, query)
            self._state.log_usage("notebooklm", "requests")
            if result["verdict"] != "мусор" or self._state.can_use("notebooklm"):
                return result
        if self._state.can_use("yandexgpt"):
            result = self._yandexgpt.validate_snippet(snippet, query)
            return result
        return {"verdict": "годно", "summary": "Лимит проверок исчерпан"}


# ─── Search Engine ───────────────────────────────────────────────

class SearchEngine:
    def __init__(self, ratelimit: RateLimiter, state: StateManager):
        self._yacy = YaCySearch(ratelimit)
        self._ddg = DuckDuckGoSearch(ratelimit)
        self._youtube = YouTubeTranscriptSearch(ratelimit)
        self._yandex = YandexXMLSearch(ratelimit)
        self._page = PageFetcher(ratelimit)
        self._state = state
        self._engines = {
            "yacy": self._yacy.search,
            "duckduckgo": self._ddg.search,
            "youtube": self._youtube.search,
            "yandex_xml": self._yandex.search,
        }

    def search(self, task: Task) -> list[SearchResult]:
        all_results = []
        seen = set()
        max_total = min(task.max_results * 3, 50)
        engine_order = [e for e in SEARCH_ENGINES.get(SCOUT_REGION, ["yacy"]) if e in self._engines]

        for engine_name in engine_order:
            if len(all_results) >= max_total:
                break
            if not self._state.can_use(engine_name):
                log.info("Skipping %s (limit exhausted)", engine_name)
                continue
            try:
                results = self._engines[engine_name](task.query, task.max_results)
                for r in results:
                    if r.url and r.url not in seen:
                        seen.add(r.url)
                        r.source_rating = self._state.get_source_rating(r.url)
                        all_results.append(r)
                if engine_name in ("yacy", "yandex_xml"):
                    self._state.log_usage(engine_name, "requests", 1)
            except Exception as e:
                log.warning("Engine %s error: %s", engine_name, e)
        return all_results

    def fetch_full_text(self, result: SearchResult) -> str:
        if result.content:
            return result.content
        text = self._page.fetch(result.url)
        return text or ""


# ─── File Saver ──────────────────────────────────────────────────

class FileSaver:
    def __init__(self):
        self._agent_dir = f"{RAW_BASE}/scout"

    def save(self, task: Task, results: list[SearchResult]) -> list[dict]:
        date_str = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        save_dir = f"{self._agent_dir}/{date_str}"
        _ensure_dir(save_dir)
        saved = []
        for i, r in enumerate(results):
            cs = _checksum(r.snippet, r.url, r.title)
            fpath = f"{save_dir}/{task.task_id}_{i:03d}_{r.source}.json"
            with open(fpath, "w", encoding="utf-8") as f:
                json.dump({
                    "task_id": task.task_id, "query": task.query, "source": r.source,
                    "title": r.title, "url": r.url, "snippet": r.snippet,
                    "content": r.content, "source_rating": r.source_rating,
                    "fetched_at": datetime.now(timezone.utc).isoformat(),
                    "checksum_sha256": cs,
                }, f, ensure_ascii=False, indent=2)
            saved.append({"path": fpath, "checksum_sha256": cs, "source": r.source,
                          "url": r.url, "title": r.title, "source_rating": r.source_rating})
        return saved


# ─── Kafka Manager ───────────────────────────────────────────────

class KafkaManager:
    def __init__(self, state: StateManager):
        self._state = state
        self._producer = None
        self._consumer = None

    def _get_producer(self):
        if self._producer is None:
            try:
                from kafka import KafkaProducer
                self._producer = KafkaProducer(
                    bootstrap_servers=KAFKA_BROKER,
                    value_serializer=lambda v: json.dumps(v, ensure_ascii=False).encode(),
                    acks="all", retries=3,
                )
            except Exception as e:
                log.error("Kafka producer: %s", e)
        return self._producer

    def _get_consumer(self):
        if self._consumer is None:
            try:
                from kafka import KafkaConsumer
                self._consumer = KafkaConsumer(
                    "tasks.assigned.v1", "delivery.ack.v1", "ratings.update.v1",
                    bootstrap_servers=KAFKA_BROKER,
                    group_id="field-agent",
                    value_deserializer=lambda v: json.loads(v.decode()),
                    auto_offset_reset="latest", enable_auto_commit=True,
                )
            except Exception as e:
                log.error("Kafka consumer: %s", e)
        return self._consumer

    def send(self, topic: str, msg: dict):
        p = self._get_producer()
        if p:
            try:
                p.send(topic, msg)
                p.flush()
            except Exception as e:
                log.error("Kafka send to %s: %s", topic, e)

    def send_file_ready(self, task: Task, file_info: dict, validation: dict):
        self.send("planners.answers.v1", {
            "type": "file_ready", "region": SCOUT_REGION, "agent": "scout",
            "task_id": task.task_id, "query": task.query, "requester": task.requester,
            "path": file_info["path"], "source": file_info["source"],
            "url": file_info["url"], "title": file_info["title"],
            "checksum_sha256": file_info["checksum_sha256"],
            "source_rating": file_info.get("source_rating", 0.5),
            "verdict": validation.get("verdict", "unknown"),
            "summary": validation.get("summary", ""),
            "timestamp": datetime.now(timezone.utc).isoformat(),
        })

    def send_task_summary(self, task: Task, valid_count: int, total_count: int):
        self.send("planners.answers.v1", {
            "type": "task_summary", "region": SCOUT_REGION, "agent": "scout",
            "task_id": task.task_id, "query": task.query, "requester": task.requester,
            "total_files": total_count, "valid_files": valid_count,
        })

    def send_heartbeat(self, stats: dict):
        self.send("events.system.v1", {
            "type": "heartbeat", "region": SCOUT_REGION, "agent": AGENT_ID,
            "status": "alive", "timestamp": datetime.now(timezone.utc).isoformat(),
            "stats": stats,
        })

    def send_error(self, message: str, details: dict = None):
        self.send("events.system.v1", {
            "type": "scout_error", "region": SCOUT_REGION, "agent": AGENT_ID,
            "message": message, "details": details or {},
            "timestamp": datetime.now(timezone.utc).isoformat(),
        })

    def listen(self, callback, stop_event: threading.Event):
        consumer = self._get_consumer()
        if not consumer:
            return
        for msg in consumer:
            if stop_event.is_set():
                break
            try:
                data = msg.value
                if not isinstance(data, dict):
                    continue
                if msg.topic == "delivery.ack.v1":
                    if data.get("path"):
                        self._state.mark_delivered(data["path"])
                elif msg.topic == "ratings.update.v1":
                    domain = data.get("domain", "")
                    delta = data.get("delta", 0)
                    reason = data.get("reason", "")
                    if domain:
                        self._state.update_source_rating(domain, delta, reason)
                elif msg.topic == "tasks.assigned.v1" and data.get("query"):
                    callback(data)
            except Exception as e:
                log.error("Kafka msg error: %s", e)


# ─── Telegram Bot ────────────────────────────────────────────────

class TelegramBot:
    def __init__(self, callback):
        self._callback = callback
        self._token = _load_secret("telegram_token")
        self._allowed = set()

    def _load_users(self):
        p = os.path.join(SECRET_DIR, ".secret_telegram_users")
        if os.path.exists(p):
            for line in open(p).read().strip().splitlines():
                self._allowed.add(line.strip())

    def start(self, stop: threading.Event):
        if not self._token:
            return
        self._load_users()
        try:
            from telegram.ext import Application, CommandHandler, MessageHandler, filters
            app = Application.builder().token(self._token).build()

            async def start_cmd(upd, ctx):
                uid = str(upd.effective_user.id)
                if self._allowed and uid not in self._allowed:
                    await upd.message.reply_text("Доступ запрещён.")
                    return
                await upd.message.reply_text("Scout RU. search: <текст> | !<текст> (без разбора) | video: <URL>")

            async def handle_msg(upd, ctx):
                uid = str(upd.effective_user.id)
                if self._allowed and uid not in self._allowed:
                    return
                text = (upd.message.text or "").strip()
                if text.lower().startswith("search:"):
                    q = text[7:].strip()
                    if q:
                        no_expand = q.startswith("!")
                        query = q[1:] if no_expand else q
                        task = Task(task_id=str(uuid.uuid4()), query=query,
                                    requester=f"telegram:{uid}", priority=10)
                        self._callback(task, no_expand)
                        await upd.message.reply_text(f"Задача {task.task_id} принята.")
                elif text.lower().startswith("video:"):
                    url = text[6:].strip()
                    vid = self._extract_video_id(url)
                    if vid:
                        task = Task(task_id=str(uuid.uuid4()), query=f"video:{vid}",
                                    requester=f"telegram:{uid}", priority=10)
                        self._callback(task, no_expand=True)
                        await upd.message.reply_text(f"Видео {task.task_id} принято.")

            app.add_handler(CommandHandler("start", start_cmd))
            app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, handle_msg))
            app.run_polling(stop_signals=[], close_loop=False)
        except Exception as e:
            log.error("Telegram: %s", e)

    @staticmethod
    def _extract_video_id(url: str) -> Optional[str]:
        m = re.search(r'(?:v=|youtu\.be/)([a-zA-Z0-9_-]{11})', url)
        return m.group(1) if m else None


# ─── Healthcheck ─────────────────────────────────────────────────

class Healthcheck:
    @staticmethod
    def run() -> bool:
        ok = True
        checks = []

        try:
            requests.get("https://yacy.searchlab.eu/yacysearch.json?query=test&maximumRecords=1", timeout=10)
            checks.append(("YaCy", True))
        except Exception:
            checks.append(("YaCy", False))

        try:
            with open("/home/waters-data/scout_state.db"):
                checks.append(("SQLite", True))
        except Exception:
            checks.append(("SQLite", False))

        st = os.statvfs("/home/waters-data")
        free_gb = st.f_frsize * st.f_bavail / (1024**3)
        checks.append(("Disk", free_gb > 0.5))

        try:
            from kafka import KafkaProducer
            p = KafkaProducer(bootstrap_servers=KAFKA_BROKER)
            p.close()
            checks.append(("Kafka", True))
        except Exception:
            checks.append(("Kafka", False))

        for name, status in checks:
            if not status:
                log.error("Healthcheck FAIL: %s", name)
                ok = False
            else:
                log.info("Healthcheck OK: %s", name)
        return ok


# ─── Field Agent ─────────────────────────────────────────────────

class FieldAgent:
    def __init__(self):
        self._state = StateManager()
        self._ratelimit = RateLimiter()
        self._engine = SearchEngine(self._ratelimit, self._state)
        self._yandexgpt = YandexGPTClient(self._state, self._ratelimit)
        self._validator = ValidatorRouter(self._state, self._ratelimit)
        self._saver = FileSaver()
        self._kafka = KafkaManager(self._state)
        self._cleanup = CleanupScheduler(self._state)
        self._stop = threading.Event()
        self._youtube = YouTubeTranscriptSearch(self._ratelimit)

    def _run_heartbeat(self):
        while not self._stop.wait(HEARTBEAT_INTERVAL):
            try:
                stats = self._state.get_stats()
                self._kafka.send_heartbeat(stats)
            except Exception as e:
                log.error("Heartbeat: %s", e)

    def _report_error(self, msg: str, details: dict = None):
        self._kafka.send_error(msg, details)
        log.error("ERROR: %s %s", msg, details or "")

    def _process_task(self, data: dict, no_expand: bool = False):
        requester = data.get("requester", "unknown") if isinstance(data, dict) else data
        query = data.get("query", "") if isinstance(data, dict) else data
        task_id = data.get("task_id", str(uuid.uuid4())) if isinstance(data, dict) else str(uuid.uuid4())
        priority = PRIORITY_MAP.get(requester, PRIORITY_MAP.get("default", 5))

        task = Task(task_id=task_id, query=query, requester=requester, priority=priority)
        self._state.create_task(task.task_id, task.query, task.requester, task.priority)
        self._state.update_task_status(task.task_id, "processing")

        try:
            final_query = query
            expanded = False
            if not no_expand:
                expanded_queries = self._yandexgpt.expand_query(query)
                if expanded_queries:
                    final_query = " ".join(expanded_queries[:3])
                    expanded = True

            results = self._engine.search(Task(
                task_id=task_id, query=final_query, requester=requester,
                priority=priority,
            ))
            if not results:
                self._state.update_task_status(task.task_id, "done")
                log.info("No results for %s", task_id)
                return

            saved = self._saver.save(task, results)
            valid_files = []

            for sf in saved:
                cs = sf["checksum_sha256"]
                if self._state.is_duplicate(cs):
                    try:
                        os.remove(sf["path"])
                    except OSError:
                        pass
                    continue

                snippet = ""
                try:
                    with open(sf["path"]) as f:
                        data = json.load(f)
                        snippet = data.get("snippet", "") or data.get("content", "")[:500]
                except Exception:
                    pass

                rating = sf.get("source_rating", 0.5)
                self._state.add_file(task_id, sf["path"], cs, sf["source"],
                                      sf["url"], sf["title"], rating)

                if rating >= 0.80:
                    validation = {"verdict": "годно", "summary": "Доверенный источник"}
                else:
                    validation = asyncio.run(self._validator.validate(snippet, query))

                self._state.update_notebooklm_result(sf["path"], validation.get("verdict", "мусор"),
                                                      validation.get("summary", ""))

                if validation.get("verdict") == "годно":
                    self._kafka.send_file_ready(task, sf, validation)
                    valid_files.append(sf)
                else:
                    try:
                        os.remove(sf["path"])
                    except OSError:
                        pass

            self._kafka.send_task_summary(task, len(valid_files), len(saved))
            self._state.update_task_status(task.task_id, "done")
            log.info("Task %s done: %d/%d valid", task_id, len(valid_files), len(saved))

        except Exception as e:
            log.error("Task %s failed: %s", task_id, e)
            self._state.update_task_status(task.task_id, "failed", str(e))

    def _kafka_callback(self, data: dict):
        self._process_task(data, no_expand=False)

    def run(self):
        _ensure_dir("/home/waters-data/logs")
        _ensure_dir(RAW_BASE)
        _ensure_dir(SECRET_DIR)

        if not Healthcheck.run():
            self._report_error("Healthcheck failed at startup")
            return

        self._state.retry_pending_on_startup()
        self._cleanup.start()

        threading.Thread(target=self._run_heartbeat, daemon=True, name="heartbeat").start()
        threading.Thread(target=self._kafka.listen, args=(self._kafka_callback, self._stop),
                         daemon=True, name="kafka").start()
        threading.Thread(target=TelegramBot(self._process_task).start,
                         args=(self._stop,), daemon=True, name="telegram").start()

        try:
            while not self._stop.is_set():
                time.sleep(1)
        except KeyboardInterrupt:
            self._stop.set()


if __name__ == "__main__":
    FieldAgent().run()
