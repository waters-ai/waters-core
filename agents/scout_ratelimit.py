"""scout_ratelimit.py — Rate limiter с exponential backoff."""

import time
import threading
import logging

log = logging.getLogger("scout.ratelimit")


class TokenBucket:
    def __init__(self, rate: float, burst: int, name: str = "default"):
        self.rate = rate
        self.burst = burst
        self.name = name
        self._tokens = float(burst)
        self._last_refill = time.monotonic()
        self._lock = threading.Lock()

    def _refill(self):
        now = time.monotonic()
        elapsed = now - self._last_refill
        self._tokens = min(self.burst, self._tokens + elapsed * self.rate)
        self._last_refill = now

    def acquire(self, tokens: float = 1.0, block: bool = True) -> bool:
        if tokens > self.burst:
            log.warning("%s: request %f > burst %d, clamping", self.name, tokens, self.burst)
            tokens = float(self.burst)

        with self._lock:
            self._refill()
            if self._tokens >= tokens:
                self._tokens -= tokens
                return True
            if not block:
                return False
            wait_time = (tokens - self._tokens) / self.rate
            log.debug("%s: waiting %.2fs for token", self.name, wait_time)
        time.sleep(wait_time)
        with self._lock:
            self._refill()
            self._tokens -= tokens
            return True


class RateLimiter:
    def __init__(self):
        self._buckets = {
            "duckduckgo": TokenBucket(rate=0.5, burst=2, name="ddg"),
            "youtube": TokenBucket(rate=0.2, burst=1, name="youtube"),
            "yandex": TokenBucket(rate=1.0, burst=5, name="yandex"),
            "page_fetch": TokenBucket(rate=0.33, burst=3, name="page_fetch"),
            "notebooklm": TokenBucket(rate=0.1, burst=1, name="notebooklm"),
        }
        self._retry_counters = {}

    def acquire(self, source: str, tokens: float = 1.0) -> bool:
        bucket = self._buckets.get(source)
        if not bucket:
            return True
        return bucket.acquire(tokens)

    def backoff_sleep(self, source: str, attempt: int, max_sleep: int = 120):
        sleep_time = min(2 ** attempt, max_sleep)
        log.warning("%s: backoff attempt %d, sleeping %ds", source, attempt, sleep_time)
        time.sleep(sleep_time)
