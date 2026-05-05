# 🏛️ Системный анализ архитектуры WATERS

**Статус:** Сводный черновик Системного Архитектора. Фаза 0.
**Дата:** 05.05.2026

---

## Модель мозга: Дельфин (однополушарный сон)
## Матрица выбора: Что берём сейчас, что откладываем

| Компонент | Фаза 0 (сейчас) | Фаза 3 (1000 агентов) | Фаза 5 (10000+ агентов) |
|:---|:---|:---|:---|
| Векторная БД | ChromaDB | Qdrant (GPU) | Qdrant Cluster |
| Графовая БД | LightRAG | LightRAG | LightRAG + Qdrant |
| Брокер | Redpanda | Kafka | Kafka Cluster |
| Кэш/стейт | Redis | Redis Cluster | Redis Enterprise |
| LLM | DeepSeek API | DeepSeek + Claude | DeepSeek + Локальные |
| Песочница | Docker | Docker Swarm | Kubernetes |
| ОС | Ubuntu 26.04 LTS | osModa/NixOS | osModa/NixOS |
| Обучение | TextGrad | LoRA + Unsloth | Axolotl |

## План миграции

**ChromaDB → Qdrant:**
1. Экспорт эмбеддингов из ChromaDB
2. Развёртывание Qdrant параллельно
3. Импорт эмбеддингов в Qdrant
4. Переключение агентов через MCP-конфиг

**Redpanda → Kafka:**
- Код не меняется (API совместим)
- Разворачиваем Kafka-кластер, перенаправляем DNS

**DeepSeek → гибрид:**
- Критичные задачи: Claude 4 Sonnet
- Тесты в песочнице: локальная Qwen2.5-7B
- Основной поток: DeepSeek остаётся
