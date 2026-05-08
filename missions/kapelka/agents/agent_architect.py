#!/usr/bin/env python3
"""agent_architect.py — Архитектор Kapelka

Получает заказ → проектирует план аудита (что проверять, в каком порядке, какими инструментами).

Реагирует на тип: "order"
Пишет: "audit_plan"
"""

from base_agent import BaseAgent, KAPELKA_WORKSPACE
from typing import Optional


class ArchitectAgent(BaseAgent):
    def __init__(self):
        super().__init__("Architect-Kapelka", "agent.architect.kapelka.v1")

    def process(self, data: dict) -> Optional[dict]:
        target = data.get("target_url", data.get("target", ""))
        scope = data.get("scope", "full")
        depth = data.get("depth", "quick")
        message = data.get("message", "")

        if not target:
            self.log.warning("Нет target_url в заказе")
            return None

        prompt = f"""Ты — Архитектор аудита сайта. Спроектируй план аудита.

Цель: {target}
Объём: {scope}
Глубина: {depth}
Дополнительно: {message}

Опиши структурированно:
1. Какие компоненты проверить (frontend, infra, integration/Solana)
2. Какие инструменты использовать
3. Ожидаемые артефакты и формат отчёта
4. Критерии оценки для каждой категории (A-F)
5. Риски и узкие места

Ответь на русском языке."""

        response = self.call_ollama(prompt)
        if not response:
            response = "Не удалось сгенерировать план (Ollama недоступна)"

        return {
            "type": "audit_plan",
            "target": target,
            "scope": scope,
            "depth": depth,
            "plan": response,
            "status": "ready",
        }


if __name__ == "__main__":
    agent = ArchitectAgent()
    agent.run()
