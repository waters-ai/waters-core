#!/usr/bin/env python3
"""
Field Agent (Scout) v2.0 — DMZ-based information gathering subagent.
Подчиняется Integrator. Работает на 167, без MCP-доступа.

Каналы ввода:
  - Telegram bot (прямые запросы CEO)
  - Kafka tasks.assigned.v1 (задания от Integrator)
  - Kafka delivery_ack (подтверждение доставки от Integrator DM)

Поисковые движки:
  - DuckDuckGo (бесплатно)
  - YouTube Transcript (бесплатно)
  - Yandex.XML (опционально, RU-fallback)

Валидация: NotebookLM (бесплатно) → вердикт: годно/мусор + выжимка

Архитектура:
  167 (Scout): поиск → валидация → file_ready → Kafka
  238 (Integrator DM): SCP с 167 → agents/{agent}/data/ → delivery_ack

YASA compliance:
  - Секреты в ~/.secrets/.secret_* (YASA-DUTY-5)
  - Никаких ключей в коде (YASA-PROH-5)
  - Комментарии на русском, нейминг на английском (YASA-FMT-5)
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
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

import requests

from agents.scout_state import StateManager
from agents.scout_ratelimit import RateLimiter
from agents.scout_cleanup import CleanupScheduler

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
SECRET_DIR = Path(os.path.expanduser("~")) / ".secrets"
KAFKA_BROKER = "171.22.180.238:9092"

HIGH_AUTHORITY_DOMAINS = {
    "edu", "gov", "mil", "arxiv.org", "nature.com", "science.org",
    "wikipedia.org", "scholar.google.com", "nih.gov", "nasa.gov",
    "europa.eu", "who.int", "ieee.org", "acm.org",
}
MEDIUM_AUTHORITY_DOMAINS = {
    "habr.com", "vc.ru", "medium.com", "techcrunch.com", "theverge.com",
    "github.com", "gitlab.com", "stackoverflow.com", "reddit.com",
    "news.ycombinator.com", "habr.com", "dtf.ru", "tproger.ru",
}


def _load_secret(name: str) -> Optional[str]:
    path = SECRET_DIR / f".secret_{name}"
    if path.exists():
        return path.read_text().strip()
    return os.environ.get(f"WATERS_{name.upper()}")


def _ensure_dir(path: str):
    Path(path).mkdir(parents=True, exist_ok=True)


def _domain_authority(url: str) -> str:
    if not url:
        return "unknown"
    try:
        from urllib.parse import urlparse
        domain = urlparse(url).netloc.lower()
        if domain.startswith("www."):
            domain = domain[4:]
        for d in HIGH_AUTHORITY_DOMAINS:
            if d in domain or domain.endswith("." + d):
                return "high"
        for d in MEDIUM_AUTHORITY_DOMAINS:
            if d in domain:
                return "medium"
        return "low"
    except Exception:
        return "unknown"


def _checksum(text: str, url: str = "", title: str = "") -> str:
    return hashlib.sha256((title + url + text).encode()).hexdigest()


# ─── Dataclasses ─────────────────────────────────────────────────

@dataclass
class Task:
    task_id: str
    query: str
    requester: str
    source_engines: list[str] = field(default_factory=lambda: ["ddg", "youtube"])
    max_results: int = 10
    language: str = "ru"
    created_at: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())


@dataclass
class SearchResult:
    title: str
    url: str
    content: str
    snippet: str = ""
    source: str = ""
    language: str = ""
    domain_authority: str = "unknown"


# ─── QueryExpander ────────────────────────────────────────────────

class QueryExpander:
    EXPANSIONS = {
        "ru": [
            lambda q: q,
            lambda q: q + " 2026",
            lambda q: f"site:habr.com {q}" if "habr" not in q else q,
            lambda q: q.replace("новости ", "").replace("последние ", ""),
        ],
        "en": [
            lambda q: q,
            lambda q: q + " 2026",
        ],
    }

    def expand(self, query: str, language: str = "ru") -> list[str]:
        results = []
        patterns = self.EXPANSIONS.get(language, self.EXPANSIONS["ru"])
        for fn in patterns:
            expanded = fn(query)
            if expanded and expanded not in results:
                results.append(expanded)
        if language == "ru":
            en_guess = query.replace("новости ", "news ").replace("искусственный интеллект", "AI")
            en_guess = en_guess.replace("последние ", "latest ").replace("новый", "new")
            if en_guess != query and en_guess not in results:
                results.append(en_guess)
        return results[:4]


# ─── DuckDuckGo Search ────────────────────────────────────────────

class DuckDuckGoSearch:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit
        try:
            from duckduckgo_search import DDGS
            self._ddgs = DDGS
        except ImportError:
            log.error("duckduckgo-search not installed")
            self._ddgs = None

    def search(self, query: str, max_results: int = 10,
               timeout: int = 20) -> list[SearchResult]:
        if not self._ddgs:
            return []
        results = []
        try:
            self._ratelimit.acquire("duckduckgo")
            with self._ddgs(timeout=timeout) as ddgs:
                for r in ddgs.text(query, max_results=max_results):
                    url = r.get("href", "")
                    results.append(SearchResult(
                        title=r.get("title", ""),
                        url=url,
                        snippet=r.get("body", ""),
                        content="",
                        source="duckduckgo",
                        domain_authority=_domain_authority(url),
                    ))
        except Exception as e:
            log.warning("DuckDuckGo search failed for '%s': %s", query[:50], e)
        return results


# ─── YouTube Transcript Search ────────────────────────────────────

class YouTubeTranscriptSearch:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit

    def search(self, query: str, max_results: int = 5) -> list[SearchResult]:
        results = []
        try:
            from youtube_transcript_api import YouTubeTranscriptApi
            from youtube_transcript_api.formatters import TextFormatter

            self._ratelimit.acquire("youtube")
            search_url = f"https://www.youtube.com/results?search_query={requests.utils.quote(query)}"
            resp = requests.get(search_url, timeout=15)
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
                        snippet=text[:300] if text else "",
                        content=text,
                        source="youtube",
                        domain_authority="medium",
                    ))
                except Exception:
                    continue
        except ImportError:
            log.error("youtube-transcript-api not installed")
        except Exception as e:
            log.warning("YouTube search failed: %s", e)
        return results


# ─── Yandex.XML Search (optional, RU-fallback) ────────────────────

class YandexXMLSearch:
    API_URL = "https://yandex.com/search/xml"

    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit
        self._api_key = _load_secret("yandex_api_key")
        self._user = _load_secret("yandex_user")
        if not self._api_key or not self._user:
            log.info("Yandex.XML not configured (no keys) — skipping")

    def search(self, query: str, max_results: int = 10) -> list[SearchResult]:
        if not self._api_key or not self._user:
            return []
        results = []
        try:
            self._ratelimit.acquire("yandex")
            resp = requests.get(self._API_URL, params={
                "user": self._user,
                "key": self._api_key,
                "query": query,
                "groupby": f"attr=d.mode=flat.groups-on-page={max_results}",
            }, timeout=20)
            if resp.status_code == 200:
                from xml.etree import ElementTree
                root = ElementTree.fromstring(resp.content)
                ns = {"y": "http://yandex.com/xml"}
                for doc in root.findall(".//y:doc", ns):
                    url = doc.findtext("y:url", "", ns)
                    title = doc.findtext("y:title", "", ns)
                    snippet = doc.findtext("y:headline", "", ns)
                    if url:
                        results.append(SearchResult(
                            title=title,
                            url=url,
                            snippet=snippet,
                            content="",
                            source="yandex",
                            domain_authority=_domain_authority(url),
                        ))
        except Exception as e:
            log.warning("Yandex search failed: %s", e)
        return results


# ─── PageFetcher (full-page text extraction) ──────────────────────

class PageFetcher:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit
        self._user_agents = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
            "WATERS Scout/1.0 (information gatherer; +https://waters.ai)",
        ]

    def fetch(self, url: str, timeout: int = 15) -> Optional[str]:
        if not url:
            return None
        try:
            self._ratelimit.acquire("page_fetch")
            ua = self._user_agents[hash(url) % len(self._user_agents)]
            resp = requests.get(url, headers={"User-Agent": ua},
                                timeout=timeout, allow_redirects=True)
            if resp.status_code != 200:
                return None
            try:
                import trafilatura
                text = trafilatura.extract(resp.text, include_comments=False,
                                            include_tables=False, no_fallback=False)
                if text and len(text) > 50:
                    return text
            except ImportError:
                pass
            return resp.text[:10000]
        except Exception as e:
            log.debug("Page fetch failed for %s: %s", url[:50], e)
            return None


# ─── Search Engine (orchestrator) ─────────────────────────────────

class SearchEngine:
    def __init__(self, ratelimit: RateLimiter):
        self._ddg = DuckDuckGoSearch(ratelimit)
        self._youtube = YouTubeTranscriptSearch(ratelimit)
        self._yandex = YandexXMLSearch(ratelimit)
        self._page_fetcher = PageFetcher(ratelimit)
        self._query_expander = QueryExpander()

        self._engines = {
            "ddg": self._ddg.search,
            "youtube": self._youtube.search,
            "yandex": self._yandex.search,
        }

    def search(self, task: Task) -> list[SearchResult]:
        all_results = []
        seen_urls = set()

        queries = self._query_expander.expand(task.query, task.language)
        log.info("Expanded '%s' → %d variants", task.query[:50], len(queries))

        for engine_name in task.source_engines:
            engine = self._engines.get(engine_name)
            if not engine:
                log.warning("Unknown engine: %s", engine_name)
                continue
            for q in queries:
                try:
                    log.info("Searching %s: '%s'", engine_name, q[:60])
                    if engine_name == "youtube":
                        results = engine(q, min(task.max_results, 5))
                    else:
                        results = engine(q, task.max_results)
                    for r in results:
                        if r.url and r.url not in seen_urls:
                            seen_urls.add(r.url)
                            all_results.append(r)
                except Exception as e:
                    log.warning("Engine %s failed for '%s': %s", engine_name, q[:30], e)

        if not all_results:
            log.warning("No results from primary engines, trying RU-fallback (yandex)...")
            fallback = self._yandex.search(task.query, task.max_results)
            for r in fallback:
                if r.url and r.url not in seen_urls:
                    all_results.append(r)

        log.info("Total unique results: %d", len(all_results))
        return all_results

    def fetch_full_text(self, results: list[SearchResult]) -> list[SearchResult]:
        enriched = []
        for r in results:
            if r.content:
                enriched.append(r)
                continue
            text = self._page_fetcher.fetch(r.url)
            if text:
                r.content = text[:15000]
            enriched.append(r)
        return enriched


# ─── NotebookLM Validator ─────────────────────────────────────────

class NotebookLMValidator:
    def __init__(self, ratelimit: RateLimiter):
        self._ratelimit = ratelimit
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
            "verdict": "мусор",
            "summary": "",
            "is_valid_source": False,
            "is_relevant": False,
            "issues": [],
        }
        client = await self._get_client()
        if not client:
            result["summary"] = "NotebookLM unavailable"
            return result

        try:
            self._ratelimit.acquire("notebooklm")
            nb = await client.notebooks.create(f"Validate: {Path(file_path).name}")
            await client.sources.add_file(nb.id, file_path, wait=True)

            valid_check = await client.chat.ask(
                nb.id,
                f"Оцени этот источник. Это научный/авторитетный источник, "
                f"любительский блог или откровенный фейк? Ответь одним словом: годно/мусор."
            )
            relevance_check = await client.chat.ask(
                nb.id,
                f"Этот документ релевантен запросу: '{query}'? Ответь: да/нет."
            )
            summary_check = await client.chat.ask(
                nb.id,
                f"Дай краткую выжимку (3-5 предложений) документа: "
                f"о чём он, какие ключевые факты, даты, цифры."
            )

            verdict_raw = (valid_check.answer or "").strip().lower()
            is_relevant_raw = (relevance_check.answer or "").strip().lower()
            summary = (summary_check.answer or "").strip()

            result["is_valid_source"] = "годно" in verdict_raw
            result["is_relevant"] = "да" in is_relevant_raw
            result["summary"] = summary[:1000]
            result["verdict"] = "годно" if (result["is_valid_source"] and result["is_relevant"]) else "мусор"

            if result["verdict"] == "мусор":
                reasons = []
                if not result["is_valid_source"]:
                    reasons.append("источник неавторитетный")
                if not result["is_relevant"]:
                    reasons.append("нерелевантен запросу")
                result["issues"] = reasons

            await client.notebooks.delete(nb.id)

        except Exception as e:
            log.error("NotebookLM validation failed for %s: %s", file_path, e)
            result["issues"].append(str(e))

        return result

    async def synthesize_task(self, file_paths: list[str], query: str) -> dict:
        if not file_paths:
            return {"summary": "Нет файлов для синтеза", "verdict": "empty"}
        client = await self._get_client()
        if not client:
            return {"summary": "NotebookLM unavailable", "verdict": "error"}

        try:
            nb = await client.notebooks.create(f"Synthesis: {query[:40]}")
            for fp in file_paths:
                try:
                    await client.sources.add_file(nb.id, fp, wait=True)
                except Exception:
                    continue

            synthesis = await client.chat.ask(
                nb.id,
                f"У тебя есть несколько документов по запросу '{query}'. "
                f"Составь общую сводку: какие ключевые факты, "
                f"есть ли противоречия между источниками, "
                f"что подтверждено несколькими источниками."
            )
            contradictions = await client.chat.ask(
                nb.id,
                f"Есть ли противоречия между этими документами? "
                f"Если есть — перечисли. Если нет — скажи 'противоречий нет'."
            )

            result = {
                "summary": (synthesis.answer or "")[:2000],
                "contradictions": (contradictions.answer or "")[:1000],
                "files_count": len(file_paths),
            }
            await client.notebooks.delete(nb.id)
            return result
        except Exception as e:
            log.error("NotebookLM synthesis failed: %s", e)
            return {"summary": "Synthesis failed", "verdict": "error"}


# ─── CrossSourceVerifier ──────────────────────────────────────────

class CrossSourceVerifier:
    def verify(self, files: list[dict]) -> dict:
        if len(files) < 2:
            return {"verified_claims": [], "contradictions": [], "confidence": "low"}

        summaries = [f.get("notebooklm_summary", "") for f in files if f.get("notebooklm_summary")]
        sources = [f.get("source", "unknown") for f in files]
        urls = [f.get("url", "") for f in files]

        domains = set()
        for url in urls:
            auth = _domain_authority(url)
            domains.add(auth)

        unique_sources = len(set(sources))
        high_authority_count = sum(1 for d in domains if d == "high")

        confidence = "low"
        if unique_sources >= 3 and high_authority_count >= 1:
            confidence = "high"
        elif unique_sources >= 2:
            confidence = "medium"

        return {
            "verified_claims": [],
            "contradictions": [],
            "confidence": confidence,
            "unique_sources": unique_sources,
            "high_authority_count": high_authority_count,
        }


# ─── FileSaver ────────────────────────────────────────────────────

class FileSaver:
    def __init__(self):
        self._agent_dir = f"{RAW_BASE}/scout"

    def save(self, task: Task, results: list[SearchResult]) -> list[dict]:
        date_str = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        save_dir = f"{self._agent_dir}/{date_str}"
        _ensure_dir(save_dir)
        saved = []

        for i, r in enumerate(results):
            cs = _checksum(r.content or r.snippet, r.url, r.title)
            filename = f"{task.task_id}_{i:03d}_{r.source}_{int(time.time())}.json"
            filepath = f"{save_dir}/{filename}"
            content = {
                "task_id": task.task_id,
                "query": task.query,
                "source": r.source,
                "title": r.title,
                "url": r.url,
                "snippet": r.snippet,
                "content": r.content,
                "domain_authority": r.domain_authority,
                "fetched_at": datetime.now(timezone.utc).isoformat(),
                "checksum_sha256": cs,
            }
            with open(filepath, "w", encoding="utf-8") as f:
                json.dump(content, f, ensure_ascii=False, indent=2)
            saved.append({
                "path": filepath,
                "checksum_sha256": cs,
                "source": r.source,
                "url": r.url,
                "title": r.title,
                "domain_authority": r.domain_authority,
            })

        log.info("Saved %d files to %s", len(saved), save_dir)
        return saved


# ─── KafkaManager ─────────────────────────────────────────────────

class KafkaManager:
    def __init__(self, state: StateManager):
        self._state = state
        self._producer = None
        self._consumer = None
        self._task_callback = None

    def set_task_callback(self, callback):
        self._task_callback = callback

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
                    "tasks.assigned.v1",
                    "delivery.ack.v1",
                    bootstrap_servers=KAFKA_BROKER,
                    group_id="field-agent",
                    value_deserializer=lambda v: json.loads(v.decode("utf-8")),
                    auto_offset_reset="latest",
                    enable_auto_commit=True,
                )
            except Exception as e:
                log.error("Failed to create Kafka consumer: %s", e)
        return self._consumer

    def send_file_ready(self, task: Task, file_info: dict, validation: dict):
        producer = self._get_producer()
        if not producer:
            return
        message = {
            "type": "file_ready",
            "agent": "scout",
            "task_id": task.task_id,
            "query": task.query,
            "requester": task.requester,
            "path": file_info["path"],
            "source": file_info["source"],
            "url": file_info["url"],
            "title": file_info["title"],
            "domain_authority": file_info["domain_authority"],
            "checksum_sha256": file_info["checksum_sha256"],
            "verdict": validation.get("verdict", "unknown"),
            "summary": validation.get("summary", ""),
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        try:
            producer.send("planners.answers.v1", message)
            producer.flush()
            log.info("Sent file_ready for %s", file_info["path"])
        except Exception as e:
            log.error("Failed to send file_ready: %s", e)

    def send_task_summary(self, task: Task, synthesis: dict,
                           valid_count: int, total_count: int):
        producer = self._get_producer()
        if not producer:
            return
        message = {
            "type": "task_summary",
            "agent": "scout",
            "task_id": task.task_id,
            "query": task.query,
            "requester": task.requester,
            "total_files": total_count,
            "valid_files": valid_count,
            "summary": synthesis.get("summary", ""),
            "contradictions": synthesis.get("contradictions", ""),
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        try:
            producer.send("planners.answers.v1", message)
            producer.flush()
            log.info("Sent task_summary for %s", task.task_id)
        except Exception as e:
            log.error("Failed to send task_summary: %s", e)

    def listen(self, stop_event: threading.Event):
        consumer = self._get_consumer()
        if not consumer:
            return
        log.info("Listening on tasks.assigned.v1 and delivery.ack.v1...")
        try:
            for msg in consumer:
                if stop_event.is_set():
                    break
                try:
                    data = msg.value
                    if not isinstance(data, dict):
                        continue
                    if msg.topic == "delivery.ack.v1":
                        self._handle_delivery_ack(data)
                    elif msg.topic == "tasks.assigned.v1" and self._task_callback:
                        self._handle_task(data)
                except Exception as e:
                    log.error("Error processing Kafka message: %s", e)
        except Exception as e:
            log.error("Kafka consumer error: %s", e)

    def _handle_task(self, data: dict):
        task = Task(
            task_id=data.get("task_id", str(uuid.uuid4())),
            query=data.get("query", ""),
            requester=data.get("requester", "kafka:integrator"),
            source_engines=data.get("source_engines", ["ddg", "youtube"]),
            max_results=data.get("max_results", 10),
        )
        if task.query:
            log.info("Received task from Kafka: %s", task.task_id)
            self._task_callback(task)

    def _handle_delivery_ack(self, data: dict):
        path = data.get("path", "")
        if path:
            self._state.mark_delivered(path)
            log.info("Delivery confirmed: %s", path)


# ─── TelegramBot ──────────────────────────────────────────────────

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
            from telegram.ext import Application, CommandHandler, MessageHandler, filters

            app = Application.builder().token(self._token).build()

            async def start_cmd(update, context):
                uid = str(update.effective_user.id)
                if self._allowed_users and uid not in self._allowed_users:
                    await update.message.reply_text("Доступ запрещён.")
                    return
                await update.message.reply_text(
                    "Scout Agent v2.0 ready.\n"
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
                            requester=f"telegram:{uid}",
                        )
                        log.info("Received task from Telegram: %s", task.task_id)
                        self._task_callback(task)
                        await update.message.reply_text(f"Задача принята: {task.task_id}")
                    else:
                        await update.message.reply_text("Пустой запрос.")
                else:
                    await update.message.reply_text("Используй формат: search: <запрос>")

            app.add_handler(CommandHandler("start", start_cmd))
            app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, handle_message))

            log.info("Telegram bot started")
            app.run_polling(stop_signals=[], close_loop=False)

        except ImportError:
            log.error("python-telegram-bot not installed")
        except Exception as e:
            log.error("Telegram bot error: %s", e)


# ─── Field Agent (orchestrator) ───────────────────────────────────

class FieldAgent:
    def __init__(self):
        self._state = StateManager()
        self._ratelimit = RateLimiter()
        self._cleanup = CleanupScheduler(self._state)
        self._engine = SearchEngine(self._ratelimit)
        self._validator = NotebookLMValidator(self._ratelimit)
        self._verifier = CrossSourceVerifier()
        self._saver = FileSaver()
        self._kafka = KafkaManager(self._state)
        self._stop_event = threading.Event()

    def _process_task(self, task: Task):
        log.info("=" * 60)
        log.info("Processing task %s: %s", task.task_id, task.query)
        log.info("=" * 60)

        self._state.create_task(task.task_id, task.query, task.requester, task.source_engines)
        self._state.update_task_status(task.task_id, "processing")

        try:
            raw_results = self._engine.search(task)
            if not raw_results:
                log.warning("No results for task %s", task.task_id)
                self._state.update_task_status(task.task_id, "done")
                return

            enriched = self._engine.fetch_full_text(raw_results)
            saved_files = self._saver.save(task, enriched)

            valid_files = []
            all_file_paths = []

            for sf in saved_files:
                cs = sf["checksum_sha256"]
                if self._state.is_duplicate(cs):
                    log.info("Dedup: %s already exists, skipping", cs[:12])
                    try:
                        os.remove(sf["path"])
                    except OSError:
                        pass
                    continue

                self._state.add_file(
                    task_id=task.task_id,
                    path=sf["path"],
                    checksum_sha256=cs,
                    source=sf["source"],
                    url=sf["url"],
                    title=sf["title"],
                    domain_authority=sf["domain_authority"],
                )

                validation = asyncio.run(self._validator.validate(sf["path"], task.query))
                self._state.update_notebooklm_result(
                    sf["path"], validation["verdict"],
                    validation.get("summary", ""),
                )

                if validation.get("verdict") == "годно":
                    self._kafka.send_file_ready(task, sf, validation)
                    valid_files.append(sf)
                    all_file_paths.append(sf["path"])
                else:
                    log.info("Rejected by NotebookLM: %s (verdict=мусор)", sf["path"])
                    try:
                        os.remove(sf["path"])
                    except OSError:
                        pass

            if valid_files:
                vfiles = self._state.get_valid_files_for_task(task.task_id)
                vpaths = [f["path"] for f in vfiles if os.path.exists(f.get("path", ""))]

                if vpaths:
                    synthesis = asyncio.run(
                        self._validator.synthesize_task(vpaths, task.query)
                    )
                else:
                    synthesis = {"summary": "Нет доступных файлов для синтеза", "verdict": "empty"}

                verification = self._verifier.verify(vfiles)

                self._kafka.send_task_summary(
                    task, synthesis,
                    valid_count=len(valid_files),
                    total_count=len(saved_files),
                )

                self._state.update_task_status(task.task_id, "done")
                log.info("Task %s done: %d/%d valid, confidence=%s",
                         task.task_id, len(valid_files), len(saved_files),
                         verification.get("confidence", "unknown"))
            else:
                self._state.update_task_status(task.task_id, "done")
                log.info("Task %s done: all %d files rejected by NotebookLM",
                         task.task_id, len(saved_files))

        except Exception as e:
            log.error("Task %s failed: %s", task.task_id, e)
            self._state.update_task_status(task.task_id, "failed", str(e))

    def run(self):
        log.info("=" * 60)
        log.info("Field Agent (Scout) v2.0 starting...")
        log.info("Agent ID: %s", AGENT_ID)
        log.info("Kafka: %s", KAFKA_BROKER)
        log.info("State DB: /home/waters-data/scout_state.db")
        log.info("=" * 60)

        self._cleanup.start()

        self._kafka.set_task_callback(self._process_task)
        kafka_thread = threading.Thread(
            target=self._kafka.listen,
            args=(self._stop_event,),
            daemon=True,
            name="kafka",
        )
        kafka_thread.start()

        telegram = TelegramBot(self._process_task)
        tgram_thread = threading.Thread(
            target=telegram.start,
            args=(self._stop_event,),
            daemon=True,
            name="telegram",
        )
        tgram_thread.start()

        try:
            while not self._stop_event.is_set():
                time.sleep(1)
        except KeyboardInterrupt:
            log.info("Shutting down...")
            self._stop_event.set()


if __name__ == "__main__":
    _ensure_dir("/home/waters-data/logs")
    _ensure_dir(RAW_BASE)
    _ensure_dir(str(SECRET_DIR))
    agent = FieldAgent()
    agent.run()
