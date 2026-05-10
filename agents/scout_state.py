"""scout_state.py — SQLite-слой для Scout Field Agent."""

import json
import sqlite3
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional


DB_PATH = "/home/waters-data/scout_state.db"


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
                notebooklm_verdict TEXT,
                notebooklm_summary TEXT,
                contradictory_with TEXT,
                delivered INTEGER DEFAULT 0,
                delivery_ack_at TIMESTAMP,
                created_at TIMESTAMP DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_files_checksum ON files(checksum_sha256);
            CREATE INDEX IF NOT EXISTS idx_files_task ON files(task_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
        """)
        conn.close()

    def create_task(self, task_id: str, query: str, requester: str = "unknown",
                    source_engines: list = None) -> dict:
        conn = self._get_conn()
        engines = json.dumps(source_engines or ["ddg", "youtube"])
        now = datetime.now(timezone.utc).isoformat()
        conn.execute(
            "INSERT OR IGNORE INTO tasks (task_id, query, requester, source_engines, created_at, updated_at) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (task_id, query, requester, engines, now, now)
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
            "SELECT * FROM tasks WHERE status IN ('pending','processing') ORDER BY created_at ASC"
        ).fetchall()
        result = []
        for row in rows:
            d = dict(row)
            d["source_engines"] = json.loads(d.get("source_engines", "[]"))
            result.append(d)
        return result

    def add_file(self, task_id: str, path: str, checksum_sha256: str,
                 source: str = "", url: str = "", title: str = "",
                 domain_authority: str = "unknown") -> bool:
        conn = self._get_conn()
        try:
            conn.execute(
                "INSERT OR IGNORE INTO files "
                "(task_id, path, checksum_sha256, source, url, title, domain_authority) "
                "VALUES (?, ?, ?, ?, ?, ?, ?)",
                (task_id, path, checksum_sha256, source, url, title, domain_authority)
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

    def get_stats(self) -> dict:
        conn = self._get_conn()
        total = conn.execute("SELECT COUNT(*) FROM files").fetchone()[0]
        delivered = conn.execute("SELECT COUNT(*) FROM files WHERE delivered=1").fetchone()[0]
        valid = conn.execute("SELECT COUNT(*) FROM files WHERE notebooklm_verdict='годно'").fetchone()[0]
        pending_tasks = conn.execute(
            "SELECT COUNT(*) FROM tasks WHERE status='pending'"
        ).fetchone()[0]
        return {
            "total_files": total,
            "delivered": delivered,
            "valid": valid,
            "pending_tasks": pending_tasks,
        }
