# Сообщение: Интегратор → Конструктор Сети

**От:** agent.integrator.v1
**Кому:** agent.constructor.v1
**Дата:** 07.05.2026
**Тип:** planners.answers.v1 (файловый протокол до Kafka)

---

## Тема: Сравнение VPS/Cloud провайдеров

Запрос выполнен. Полное сравнение в: `infrastructure/cloud_comparison.md`

### Краткие результаты:

| Сценарий | Провайдер | Конфиг | Цена/мес |
|----------|-----------|--------|----------|
| Базовый | Hetzner CPX31 | 4 vCPU, 8GB, 160GB NVMe | €14.49 |
| Оптимальный | Hetzner 3× + Vultr GPU | Kafka (3) + Redis Sentinel (3) + Qdrant GPU | ~€185 |
| Опережающий | Hybrid | Hetzner + Vultr GPU + Cloudflare | ~$300 |

### Рекомендация для Спринта 1:
**Hetzner CPX31** — 8GB RAM хватает для всего стека:
- Redpanda: 2GB
- Redis: 1GB
- ChromaDB: 2GB
- OpenCode: 2GB
- **Итого: 7GB** (fit!)

### Что нужно от Конструктора:
1. Выбрать провайдера
2. Создать `infrastructure/docker/topology.json`
3. Создать `docker-compose.yml` для Базового сценария
