"""scout_state.py — SQLite-слой для Scout Field Agent."""

import json
import sqlite3
import threading
import time
from datetime import datetime, timezone, date
from pathlib import Path
from typing import Optional

from agents.config import DB_PATH, FREE_LIMITS, DAILY_BUDGET


class StateManager:
    def __init__(self, db_path: str = DB_PATH):
        self._db_path = db_path
        self._local = threading.local()
        self._init_db()

    def _get_conn(self) -> sqlite3.Connection:
        if not hasattr(self._local, "conn") or self._local.conn is None:
            self._local.conn = sqlite3.connect(self._db_path)
            self._local.conn.row_factory = sqlite3.Row
            self._local.conn.execute("PRAGMA journal_mode=WAL")
            self._local.conn.execute("PRAGMA synchronous=NORMAL")
        return self._local.conn

    def _init_db(self):
        conn = sqlite3.connect(self._db_path)
        conn.execute("PRAGMA journal_mode=WAL")
        conn.executescript("""
            CREATE TABLE IF NOT EXISTS tasks (
                task_id TEXT PRIMARY KEY,
                query TEXT NOT NULL,
                requester TEXT DEFAULT 'unknown',
                priority INTEGER DEFAULT 5,
                source_engines TEXT DEFAULT '["ddg","youtube"]',
                status TEXT DEFAULT 'pending',
                created_at TIMESTAMP DEFAULT (datetime('now')),
                updated_at TIMESTAMP DEFAULT (datetime('now')),
                completed_at TIMESTAMP,
                error TEXT
            );

            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL REFERENCES tasks(task_id),
                path TEXT NOT NULL,
                checksum_sha256 TEXT UNIQUE,
                source TEXT,
                url TEXT,
                title TEXT,
                domain_authority TEXT DEFAULT 'unknown',
                source_rating REAL DEFAULT 0.5,
                notebooklm_verdict TEXT,
                notebooklm_summary TEXT,
                contradictory_with TEXT,
                delivered INTEGER DEFAULT 0,
                delivery_ack_at TIMESTAMP,
                created_at TIMESTAMP DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS daily_usage (
                usage_date TEXT NOT NULL,
                service TEXT NOT NULL,
                metric TEXT NOT NULL,
                value REAL DEFAULT 0,
                PRIMARY KEY (usage_date, service, metric)
            );

            CREATE TABLE IF NOT EXISTS source_ratings (
                domain TEXT PRIMARY KEY,
                rating REAL DEFAULT 0.5,
                source_type TEXT DEFAULT 'new',
                total_files INTEGER DEFAULT 0,
                passed_files INTEGER DEFAULT 0,
                last_checked TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_files_checksum ON files(checksum_sha256);
            CREATE INDEX IF NOT EXISTS idx_files_task ON files(task_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority);
            CREATE INDEX IF NOT EXISTS idx_usage_date ON daily_usage(usage_date);
        """)
        conn.close()

    def retry_pending_on_startup(self):
        conn = self._get_conn()
        conn.execute("UPDATE tasks SET status='pending', updated_at=datetime('now') WHERE status='processing'")
        conn.commit()

    def create_task(self, task_id: str, query: str, requester: str = "unknown",
                    priority: int = 5, source_engines: list = None) -> dict:
        conn = self._get_conn()
        engines = json.dumps(source_engines or ["ddg", "youtube"])
        now = datetime.now(timezone.utc).isoformat()
        conn.execute(
            "INSERT OR IGNORE INTO tasks (task_id, query, requester, priority, source_engines, created_at, updated_at) "
            "VALUES (?, ?, ?, ?, ?, ?, ?)",
            (task_id, query, requester, priority, engines, now, now)
        )
        conn.commit()
        return self.get_task(task_id)

    def update_task_status(self, task_id: str, status: str, error: str = None):
        conn = self._get_conn()
        now = datetime.now(timezone.utc).isoformat()
        fields = {"status": status, "updated_at": now}
        if status in ("done", "failed"):
            fields["completed_at"] = now
        if error:
            fields["error"] = error
        set_clause = ", ".join(f"{k}=?" for k in fields)
        values = list(fields.values()) + [task_id]
        conn.execute(f"UPDATE tasks SET {set_clause} WHERE task_id=?", values)
        conn.commit()

    def get_task(self, task_id: str) -> Optional[dict]:
        conn = self._get_conn()
        row = conn.execute("SELECT * FROM tasks WHERE task_id=?", (task_id,)).fetchone()
        if row:
            d = dict(row)
            d["source_engines"] = json.loads(d.get("source_engines", "[]"))
            return d
        return None

    def get_pending_tasks(self) -> list[dict]:
        conn = self._get_conn()
        rows = conn.execute(
            "SELECT * FROM tasks WHERE status IN ('pending','processing') "
            "ORDER BY priority DESC, created_at ASC"
        ).fetchall()
        result = []
        for row in rows:
            d = dict(row)
            d["source_engines"] = json.loads(d.get("source_engines", "[]"))
            result.append(d)
        return result

    def add_file(self, task_id: str, path: str, checksum_sha256: str,
                 source: str = "", url: str = "", title: str = "",
                 source_rating: float = 0.5) -> bool:
        conn = self._get_conn()
        try:
            conn.execute(
                "INSERT OR IGNORE INTO files "
                "(task_id, path, checksum_sha256, source, url, title, source_rating) "
                "VALUES (?, ?, ?, ?, ?, ?, ?)",
                (task_id, path, checksum_sha256, source, url, title, source_rating)
            )
            conn.commit()
            return True
        except sqlite3.IntegrityError:
            return False

    def is_duplicate(self, checksum_sha256: str) -> bool:
        conn = self._get_conn()
        row = conn.execute(
            "SELECT 1 FROM files WHERE checksum_sha256=?", (checksum_sha256,)
        ).fetchone()
        return row is not None

    def update_notebooklm_result(self, path: str, verdict: str, summary: str,
                                  contradictory_with: str = None):
        conn = self._get_conn()
        conn.execute(
            "UPDATE files SET notebooklm_verdict=?, notebooklm_summary=?, "
            "contradictory_with=? WHERE path=?",
            (verdict, summary, contradictory_with, path)
        )
        conn.commit()

    def mark_delivered(self, path: str):
        conn = self._get_conn()
        now = datetime.now(timezone.utc).isoformat()
        conn.execute(
            "UPDATE files SET delivered=1, delivery_ack_at=? WHERE path=?",
            (now, path)
        )
        conn.commit()

    def get_cleanup_candidates(self, max_age_days: int = 7) -> list[dict]:
        conn = self._get_conn()
        rows = conn.execute(
            "SELECT * FROM files WHERE delivered=1 AND "
            "created_at < datetime('now', ?)",
            (f"-{max_age_days} days",)
        ).fetchall()
        return [dict(r) for r in rows]

    def get_task_files(self, task_id: str) -> list[dict]:
        conn = self._get_conn()
        rows = conn.execute(
            "SELECT * FROM files WHERE task_id=? ORDER BY created_at", (task_id,)
        ).fetchall()
        return [dict(r) for r in rows]

    def get_valid_files_for_task(self, task_id: str) -> list[dict]:
        conn = self._get_conn()
        rows = conn.execute(
            "SELECT * FROM files WHERE task_id=? AND notebooklm_verdict='годно' "
            "ORDER BY created_at", (task_id,)
        ).fetchall()
        return [dict(r) for r in rows]

    def delete_file_record(self, file_id: int):
        conn = self._get_conn()
        conn.execute("DELETE FROM files WHERE id=?", (file_id,))
        conn.commit()

    # ─── Source Ratings ───────────────────────────────────────────

    def get_source_rating(self, domain: str) -> float:
        conn = self._get_conn()
        row = conn.execute(
            "SELECT rating FROM source_ratings WHERE domain=?", (domain,)
        ).fetchone()
        return row["rating"] if row else 0.50

    def update_source_rating(self, domain: str, delta: float, reason: str = ""):
        conn = self._get_conn()
        now = datetime.now(timezone.utc).isoformat()
        conn.execute(
            "INSERT INTO source_ratings (domain, rating, source_type, total_files, last_checked) "
            "VALUES (?, ?, ?, 1, ?) "
            "ON CONFLICT(domain) DO UPDATE SET "
            "rating = MIN(1.0, MAX(0.0, rating + ?)), "
            "total_files = total_files + 1, "
            "source_type = CASE WHEN ? = 'content_passed_ollama' AND rating + ? > 0.80 THEN 'checked_by_238' ELSE source_type END, "
            "last_checked = ?",
            (domain, max(0.0, min(1.0, delta + 0.50)), reason, now,
             delta, reason, delta, now)
        )
        conn.commit()

    def get_source_ratings_summary(self) -> list[dict]:
        conn = self._get_conn()
        rows = conn.execute(
            "SELECT domain, rating, source_type, total_files, passed_files, last_checked "
            "FROM source_ratings ORDER BY rating DESC LIMIT 50"
        ).fetchall()
        return [dict(r) for r in rows]

    # ─── Daily Budgets ────────────────────────────────────────────

    def _today(self) -> str:
        return date.today().isoformat()

    def log_usage(self, service: str, metric: str, value: float = 1.0):
        today = self._today()
        conn = self._get_conn()
        conn.execute(
            "INSERT INTO daily_usage (usage_date, service, metric, value) "
            "VALUES (?, ?, ?, ?) "
            "ON CONFLICT(usage_date, service, metric) "
            "DO UPDATE SET value = value + ?",
            (today, service, metric, value, value)
        )
        conn.commit()

    def get_usage(self, service: str, metric: str = None) -> float:
        today = self._today()
        if service in FREE_LIMITS:
            m = metric or "requests"
        elif service in DAILY_BUDGET:
            m = metric or "cost"
        else:
            return 0.0
        conn = self._get_conn()
        row = conn.execute(
            "SELECT value FROM daily_usage WHERE usage_date=? AND service=? AND metric=?",
            (today, service, m)
        ).fetchone()
        return row["value"] if row else 0.0

    def can_use(self, service: str) -> bool:
        if service in ("yacy",):
            return True
        if service in FREE_LIMITS:
            used = self.get_usage(service, "requests")
            return used < FREE_LIMITS[service]
        if service in DAILY_BUDGET:
            used = self.get_usage(service, "cost")
            return used < DAILY_BUDGET[service]
        return True

    def get_all_usage(self) -> dict:
        result = {}
        for service, limit in FREE_LIMITS.items():
            used = self.get_usage(service, "requests")
            result[service] = {"used": used, "limit": limit, "remaining": max(0, limit - used), "metric": "requests"}
        for service, limit in DAILY_BUDGET.items():
            used = self.get_usage(service, "cost")
            result[service] = {"used": used, "limit": limit, "remaining": max(0, limit - used), "metric": "cost"}
        return result

    def get_stats(self) -> dict:
        conn = self._get_conn()
        total = conn.execute("SELECT COUNT(*) FROM files").fetchone()[0]
        delivered = conn.execute("SELECT COUNT(*) FROM files WHERE delivered=1").fetchone()[0]
        valid = conn.execute("SELECT COUNT(*) FROM files WHERE notebooklm_verdict='годно'").fetchone()[0]
        pending_tasks = conn.execute(
            "SELECT COUNT(*) FROM tasks WHERE status='pending'"
        ).fetchone()[0]
        usage = self.get_all_usage()
        return {
            "total_files": total,
            "delivered": delivered,
            "valid": valid,
            "pending_tasks": pending_tasks,
            "usage": usage,
        }
