"""scout_ratelimit.py — Rate limiter. 100% без бана."""

import time
import threading
import logging

from agents.config import RATE_LIMITS

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
            tokens = float(self.burst)
        with self._lock:
            self._refill()
            if self._tokens >= tokens:
                self._tokens -= tokens
                return True
            if not block:
                return False
            wait_time = (tokens - self._tokens) / self.rate if self.rate > 0 else 1.0
        time.sleep(min(wait_time, 30))
        with self._lock:
            self._refill()
            self._tokens -= tokens
            return True


class RateLimiter:
    def __init__(self):
        self._buckets = {}
        for name, cfg in RATE_LIMITS.items():
            self._buckets[name] = TokenBucket(
                rate=cfg["rate"], burst=cfg["burst"], name=name
            )

    def acquire(self, source: str, tokens: float = 1.0) -> bool:
        bucket = self._buckets.get(source)
        if not bucket:
            return True
        return bucket.acquire(tokens)

    def backoff_sleep(self, source: str, attempt: int, max_sleep: int = 120):
        sleep_time = min(2 ** attempt, max_sleep)
        log.warning("%s: backoff attempt %d, sleeping %ds", source, attempt, sleep_time)
        time.sleep(sleep_time)
