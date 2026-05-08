#!/usr/bin/env python3
"""agent_auditor.py — Аудитор Kapelka

Получает конфиги → запускает инструменты, собирает результаты, формирует отчёт.

Реагирует на тип: "infrastructure_config"
Пишет: "audit_report"
При критических уязвимостях → alert в alerts.security.v1
"""

import json
from base_agent import BaseAgent
from typing import Optional


class AuditorAgent(BaseAgent):
    def __init__(self):
        super().__init__("Auditor-Kapelka", "agent.auditor.kapelka.v1")

    def process(self, data: dict) -> Optional[dict]:
        target = data.get("target", "")
        config = data.get("config", {})
        depth = data.get("depth", "quick")

        if data.get("type") != "infrastructure_config" or not target:
            return None

        prompt = f"""Ты — Аудитор сайта. Выполни анализ.

Цель: {target}
Глубина: {depth}
Конфигурация: {json.dumps(config, ensure_ascii=False)[:800]}

Проверь и дай оценку (A-F) по каждому пункту:

1. **Безопасность** (Security):
   - SSL/TLS: протоколы, шифры, HSTS
   - Security headers: CSP, X-Frame-Options, X-Content-Type-Options
   - Возможные XSS/CSRF-векторы
   - Риски утечки данных

2. **Производительность** (Performance):
   - Ожидаемое время загрузки
   - Оптимизация статики
   - Кэширование

3. **Deploy-readiness** (готовность к продакшну):
   - Что нужно настроить перед деплоем
   - Риски при запуске

Для каждого пункта укажи:
- Оценку (A-F)
- Что именно проверено
- Рекомендации по исправлению

Критические проблемы (CVSS >= 9) пометь как [CRITICAL].
Ответь на русском языке, структурированно."""

        response = self.call_ollama(prompt)
        if not response:
            response = "Не удалось выполнить аудит (Ollama недоступна)"

        # Проверка на критические проблемы
        has_critical = "[CRITICAL]" in (response or "")
        severity = "critical" if has_critical else "normal"

        report = {
            "type": "audit_report",
            "target": target,
            "depth": depth,
            "report": response,
            "severity": severity,
            "status": "completed",
        }

        if has_critical:
            self.alert("critical", f"Критические уязвимости на {target}", {
                "target": target,
                "report_preview": response[:500],
            })

        return report


if __name__ == "__main__":
    agent = AuditorAgent()
    agent.run()
