# Сравнение VPS/Cloud провайдеров для WATERS

**От:** Интегратор Знаний → Конструктору Сети
**Дата:** 07.05.2026
**Сценарии:** Базовый ($100-150/мес), Оптимальный ($200+/мес), Опережающий ($350+/мес)

---

## 1. Базовый сценарий ($100-150/мес)
**Цель:** Redpanda (1), Redis (1), ChromaDB (1), OpenCode (1), LightRAG (встроен)

| Провайдер | Конфигурация | Цена | Особенности |
|-----------|--------------|------|-------------|
| **Hetzner CPX31** | 4 vCPU, 8GB RAM, 160GB NVMe | €14.49/мес | Лучший price/performance, EU, без Kubernetes |
| **DigitalOcean** | 4 vCPU, 8GB RAM, 160GB SSD | $48/мес | Managed Redis ($15), managed DBs, простой UI |
| **Vultr** | 4 vCPU, 8GB RAM, 256GB NVMe | $32/мес | GPU-инстансы от $0.50/ч, 32 локации |
| **AWS Lightsail** | 4 vCPU, 8GB RAM, 160GB SSD | $40/мес | Экосистема AWS, easy migration to full AWS |
| **Scaleway** | 4 vCPU, 8GB RAM, 100GB SSD | €12.99/мес | EU (France), Kapsule (K8s), cheap |

**Рекомендация:** **Hetzner CPX31** — 8GB RAM хватит для Redpanda + Redis + ChromaDB на одной машине (docker-compose). Если нужно больше — CPX41 (€25/мес, 16GB).

---

## 2. Оптимальный сценарий ($200+/мес)
**Цель:** Kafka (3), Redis Sentinel (3), Qdrant (1, GPU), OpenCode (3), Neo4j (1)

| Провайдер | Конфигурация | Цена | Примечание |
|-----------|--------------|------|------------|
| **Hetzner** | 3× CPX31 (Kafka) + 1× CPX51 (16GB, Qdrant) + 1× GPU (RTX 4060) | ~€85/мес | Нужен отдельный GPU-сервер (€75-100) |
| **DigitalOcean** | 3× Standard ($24) + 1× GPU-optimized ($60) + managed Redis ($45) | ~$177/мес | Managed Redis Sentinel, managed Kafka (в разработке) |
| **Vultr** | 3× Cloud Compute + 1× GPU (RTX A4000) | ~$180/мес | GPU по запросу, K8s built-in |
| **AWS** | 3× t3.medium + 1× g4dn.xlarge (GPU) + ElastiCache | ~$300+/мес | Дорого, но полная экосистема |

**Рекомендация:** **Hetzner** для CPU-нод + **Vultr GPU** для Qdrant/SigLIP. Гибридная архитектура снижает стоимость на 40%.

---

## 3. Опережающий сценарий ($350+/мес)
**Цель:** Kafka Cluster (5+), Redis Cluster (6), Qdrant Cluster, OpenCode (6), ImageBind (GPU)

| Провайдер | Конфигурация | Цена | Примечание |
|-----------|--------------|------|------------|
| **AWS** | MSK (Kafka) + ElastiCache + EKS + g5.xlarge | $400-600/мес | Full managed, минимум ручной работы |
| **GCP** | Dataflow + Memorystore + GKE + A2 instance | $350-500/мес | Лучший ML/GPU support, Vertex AI integration |
| **Hybrid** | Hetzner (Kafka, Redis) + Vultr GPU (ML) + Cloudflare (edge) | ~$300/мес | Оптимально по цене, сложнее в управлении |

**Рекомендация:** **Hybrid** — CPU-инфраструктура на Hetzner, GPU-ноды на Vultr, CDN на Cloudflare.

---

## 4. Сравнительная таблица критериев

| Критерий | Hetzner | DigitalOcean | Vultr | AWS | GCP |
|----------|---------|--------------|-------|-----|-----|
| **Цена (CPU)** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐ |
| **Цена (GPU)** | ⭐⭐⭐ | ⭐ | ⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ |
| **Локации** | EU (3) | Global (14) | Global (32) | Global (30) | Global (35) |
| **Managed Kafka** | ❌ | ⚠️ Beta | ❌ | ✅ MSK | ✅ Dataflow |
| **Managed Redis** | ❌ | ✅ | ❌ | ✅ ElastiCache | ✅ Memorystore |
| **Kubernetes** | ❌ | ✅ | ✅ | ✅ EKS | ✅ GKE |
| **SLA** | 99.9% | 99.99% | 99.99% | 99.99% | 99.95% |
| **Поддержка** | Community | 24/7 | 24/7 | 24/7 (план) | 24/7 (план) |
| **DTN-эмуляция** | ✅ (bare metal) | ✅ | ✅ | ✅ | ✅ |

---

## 5. Рекомендации для Конструктора

### Спринт 1 (сейчас): Базовый сценарий
**Выбор:** Hetzner CPX31 (€14.49/мес, 4 vCPU, 8GB RAM)
**Причина:**
- Docker Compose работает отлично
- 8GB RAM: Redpanda (2GB) + Redis (1GB) + ChromaDB (2GB) + OpenCode (2GB) = 7GB (fit!)
- NVMe диск — быстрый I/O для Kafka и ChromaDB
- EU локация — данные в Европе (compliance)

### Спринт 2-3: Переход к Оптимальному
**Выбор:** Hetzner 3× CPX31 + Vultr GPU (RTX 4060)
**Миграция:**
1. Redpanda → Kafka (3 узла) на Hetzner
2. Redis → Redis Sentinel (3 узла) на Hetzner
3. ChromaDB → Qdrant на Vultr GPU
4. OpenCode 1 → 3 экземпляра (по Троицам)

### Спринт 4+: Опережающий
**Выбор:** Hybrid (Hetzner + Vultr GPU + Cloudflare)
**Миграция:**
1. Kafka → MSK/Dataflow (если нужен managed)
2. Redis → Cluster (6 экз.)
3. Qdrant → Cluster
4. OpenCode → 6 экз. (каждый агент)

---

## 6. Terraform / IaC готовность

| Провайдер | Terraform provider | Pulumi | Ansible |
|-----------|-------------------|--------|---------|
| Hetzner | ✅ (hetznercloud/hcloud) | ✅ | ✅ |
| DigitalOcean | ✅ (digitalocean/digitalocean) | ✅ | ✅ |
| Vultr | ✅ (vultr/vultr) | ✅ | ✅ |
| AWS | ✅ (hashicorp/aws) | ✅ | ✅ |
| GCP | ✅ (hashicorp/google) | ✅ | ✅ |

**Рекомендация:** Начать с Terraform для Hetzner — простой provider, 1 файл для всей инфраструктуры.

---

*Данные собраны Интегратором из публичных прайсов провайдеров (май 2026)*
*Следующий шаг: Конструктор выбирает провайдера и создаёт `infrastructure/docker/topology.json`*
