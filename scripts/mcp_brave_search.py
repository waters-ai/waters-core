#!/usr/bin/env python3
"""
MCP Brave Search Adapter
P0-приоритет: Основной поисковик DMZ (2000 запросов/мес бесплатно)
"""

import os
import json
import hashlib
import requests
from typing import List, Dict, Optional
from datetime import datetime


class BraveSearchAdapter:
    """Адаптер для работы с Brave Search API"""

    def __init__(self):
        self.api_key = os.getenv("BRAVE_API_KEY")
        if not self.api_key:
            raise ValueError("BRAVE_API_KEY not set in environment")

        self.endpoint = "https://api.search.brave.com/res/v1/web/search"
        self.headers = {
            "Accept": "application/json",
            "X-Subscription-Token": self.api_key
        }

        # Настройки Redis для кэширования
        self.redis_host = os.getenv("REDIS_HOST", "redis-dmz")
        self.redis_port = int(os.getenv("REDIS_PORT", "6380"))

        # Настройки ChromaDB
        self.chroma_host = os.getenv("CHROMA_HOST", "chroma-dmz")
        self.chroma_port = int(os.getenv("CHROMA_PORT", "8001"))

        # Kafka для публикации
        self.kafka_broker = os.getenv("KAFKA_BROKER", "kafka:9092")

    def _generate_cache_key(self, query: str, count: int) -> str:
        """Генерация ключа кэша"""
        normalized = f"{query}:{count}"
        hash_value = hashlib.sha256(normalized.encode()).hexdigest()[:16]
        return f"cache:brave:{hash_value}"

    def _check_cache(self, cache_key: str) -> Optional[List[Dict]]:
        """Проверка кэша в Redis"""
        try:
            import redis
            r = redis.Redis(host=self.redis_host, port=self.redis_port, decode_responses=True)
            cached = r.get(cache_key)
            if cached:
                return json.loads(cached)
        except Exception as e:
            print(f"Cache check failed: {e}")
        return None

    def _save_to_cache(self, cache_key: str, results: List[Dict], ttl: int = 3600):
        """Сохранение в кэш (TTL 1 час по умолчанию)"""
        try:
            import redis
            r = redis.Redis(host=self.redis_host, port=self.redis_port, decode_responses=True)
            r.setex(cache_key, ttl, json.dumps(results))
        except Exception as e:
            print(f"Cache save failed: {e}")

    def search(self, query: str, count: int = 10) -> List[Dict]:
        """
        Поиск через Brave Search API

        Args:
            query: Поисковый запрос
            count: Количество результатов (max 10 для бесплатного тарифа)

        Returns:
            Список статей в формате knowledge_article.schema.json
        """
        # Проверка кэша
        cache_key = self._generate_cache_key(query, count)
        cached_results = self._check_cache(cache_key)
        if cached_results:
            print(f"Cache hit for query: {query}")
            return cached_results

        # Запрос к API
        params = {
            "q": query,
            "count": min(count, 10),  # Brave лимит
            "search_lang": "ru",
            "country": "RU",
            "text_decorations": False,
            "spellcheck": True
        }

        try:
            response = requests.get(
                self.endpoint,
                headers=self.headers,
                params=params,
                timeout=30
            )
            response.raise_for_status()
            data = response.json()

            # Парсинг результатов
            articles = self._parse_results(data, query)

            # Сохранение в кэш
            self._save_to_cache(cache_key, articles)

            # Сохранение в ChromaDB и Kafka
            self._save_to_chromadb(articles)
            self._publish_to_kafka(articles)

            return articles

        except requests.exceptions.RequestException as e:
            raise Exception(f"Brave API request failed: {str(e)}")
        except Exception as e:
            raise Exception(f"Brave API error: {str(e)}")

    def _parse_results(self, data: dict, query: str) -> List[Dict]:
        """Парсинг результатов в формат knowledge_article.schema.json"""
        articles = []

        for result in data.get("web", {}).get("results", []):
            article = {
                "article_id": f"brave_{hash(result['url'])}",
                "title": result.get("title", ""),
                "content": result.get("description", ""),
                "url": result.get("url", ""),
                "source": "external",
                "author": "agent.integrator.v1",
                "tags": ["web", "brave", "search", query.replace(" ", "_").lower()],
                "created_at": datetime.utcnow().isoformat() + "Z",
                "language": "ru",
                "confidence": 0.9
            }
            articles.append(article)

        return articles

    def _save_to_chromadb(self, articles: List[Dict]):
        """Сохранение в ChromaDB (векторный поиск)"""
        try:
            import chromadb
            from chromadb.utils import embedding_functions

            client = chromadb.HttpClient(host=self.chroma_host, port=self.chroma_port)
            collection = client.get_or_create_collection(
                name="external_articles",
                embedding_function=embedding_functions.DefaultEmbeddingFunction()
            )

            if articles:
                collection.add(
                    documents=[a["content"] for a in articles],
                    metadatas=[{
                        "title": a["title"],
                        "url": a["url"],
                        "source": a["source"],
                        "author": a["author"]
                    } for a in articles],
                    ids=[a["article_id"] for a in articles]
                )
                print(f"Saved {len(articles)} articles to ChromaDB")

        except Exception as e:
            print(f"ChromaDB save failed: {e}")

    def _publish_to_kafka(self, articles: List[Dict]):
        """Публикация в топик knowledge.articles.v1"""
        try:
            from kafka import KafkaProducer

            producer = KafkaProducer(
                bootstrap_servers=self.kafka_broker,
                value_serializer=lambda v: json.dumps(v).encode('utf-8')
            )

            for article in articles:
                producer.send('knowledge.articles.v1', article)
                producer.send('external.responses.v1', {
                    "response_id": article["article_id"],
                    "source": "brave_search",
                    "article": article,
                    "timestamp": datetime.utcnow().isoformat() + "Z"
                })

            producer.flush()
            print(f"Published {len(articles)} articles to Kafka")

        except Exception as e:
            print(f"Kafka publish failed: {e}")


def main():
    """Точка входа для MCP-сервера"""
    import sys

    if len(sys.argv) < 2:
        print("Usage: mcp_brave_search.py <query> [count]")
        sys.exit(1)

    query = sys.argv[1]
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 10

    adapter = BraveSearchAdapter()
    try:
        results = adapter.search(query, count)
        print(json.dumps(results, indent=2, ensure_ascii=False))
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
