"""scout_cleanup.py — Планировщик очистки файлов старше 7 дней."""

import logging
import os
import threading
import time
from datetime import datetime, timezone
from pathlib import Path

log = logging.getLogger("scout.cleanup")


class CleanupScheduler:
    def __init__(self, state_manager, raw_base: str = "/home/waters-data/raw",
                 max_age_days: int = 7, interval_seconds: int = 3600):
        self._state = state_manager
        self._raw_base = raw_base
        self._max_age_days = max_age_days
        self._interval = interval_seconds
        self._stop_event = threading.Event()

    def start(self):
        thread = threading.Thread(target=self._run_loop, daemon=True, name="cleanup")
        thread.start()
        log.info("CleanupScheduler started (interval=%ds, max_age=%dd)",
                 self._interval, self._max_age_days)
        return thread

    def stop(self):
        self._stop_event.set()

    def _run_loop(self):
        while not self._stop_event.is_set():
            try:
                self.cleanup_once()
            except Exception as e:
                log.error("Cleanup error: %s", e)
            self._stop_event.wait(self._interval)

    def cleanup_once(self):
        candidates = self._state.get_cleanup_candidates(self._max_age_days)
        if not candidates:
            return

        deleted_count = 0
        for file_info in candidates:
            path = file_info.get("path", "")
            if not path or not os.path.exists(path):
                self._state.delete_file_record(file_info["id"])
                continue
            try:
                os.remove(path)
                log.info("Deleted old file: %s", path)
            except OSError as e:
                log.warning("Failed to delete %s: %s", path, e)
                continue
            self._state.delete_file_record(file_info["id"])
            deleted_count += 1

        if deleted_count:
            log.info("Cleanup: removed %d files older than %d days",
                     deleted_count, self._max_age_days)
