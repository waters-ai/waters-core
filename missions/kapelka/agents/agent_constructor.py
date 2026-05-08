#!/usr/bin/env python3
"""agent_constructor.py — Конструктор Kapelka

Получает план аудита → готовит конфиги инфраструктуры: Docker-команды, CI/CD, deploy-шаблоны.

Реагирует на тип: "audit_plan"
Пишет: "infrastructure_config"
"""

from base_agent import BaseAgent
from typing import Optional


class ConstructorAgent(BaseAgent):
    def __init__(self):
        super().__init__("Constructor-Kapelka", "agent.constructor.kapelka.v1")

    def process(self, data: dict) -> Optional[dict]:
        target = data.get("target", "")
        scope = data.get("scope", "full")
        depth = data.get("depth", "quick")
        plan = data.get("plan", "")

        if data.get("type") != "audit_plan" or not target:
            return None

        prompt = f"""Ты — Конструктор инфраструктуры аудита. Подготовь конфиги.

План аудита: {plan[:1000] if plan else 'Не предоставлен'}
Цель: {target}
Объём: {scope}

Сгенерируй:
1. Docker-команды для запуска инструментов аудита (Lighthouse, npm audit, testssl, OWASP ZAP)
2. Структуру директорий для отчётов
3. Переменные окружения, которые понадобятся
4. Команды для быстрой проверки (curl, openssl)

Формат: структурированный список. Ответь на русском языке."""

        response = self.call_ollama(prompt)
        if not response:
            response = "Не удалось сгенерировать конфиги (Ollama недоступна)"

        return {
            "type": "infrastructure_config",
            "target": target,
            "config": response,
            "status": "ready",
        }


if __name__ == "__main__":
    agent = ConstructorAgent()
    agent.run()
