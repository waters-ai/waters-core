# WATERS Node v1.0.0 — Техническая спецификация

**Версия:** 1.0.0 | **Дата:** 2026-05-18 | **Автор:** agent.architect.v1
**Формат:** YASA Technical v2 | **Язык:** русский

---

## WATNODE-HYBRID: Гибридная LLM (hybrid_llm)

### WATNODE-HYBRID-1: Назначение
Прозрачная прослойка между LLM Router (BridgePool) и агентами. Автоматически переключает источник LLM-ответа: внешняя LLM (DeepSeek/Claude/GPT-4o/Ollama) или бортовая GGUF-модель 1-3B.

> **Следствие:** Агенты продолжают работать при обрыве связи. Нода не впадает в L4 safe mode при потере интернета.

### WATNODE-HYBRID-2: EdgeEngine
Загрузка GGUF-модели через `llama-cpp-rs` или `candle`. CPU-инференс, <500ms на RPi4. Конфиг: `[edge] model, cache_ttl, max_tokens`.

> **Следствие:** Raspberry Pi 4 получает полноценного LLM-агента без интернета.

### WATNODE-HYBRID-3: SwitchProtocol
Интеграция с AutonomyEngine (L0-L4): L0=Remote+validate, L1=Remote+fallback, L2=Edge(простые)/Queue(сложные), L3=Edge+DTN, L4=SOS.

> **Следствие:** Плавная деградация, а не бинарное "работает/не работает".

### WATNODE-HYBRID-4: PrefetchCache
Упреждающая загрузка ответов на базе активных задач. Хранит в Redis DB 14-15.

> **Следствие:** Типичные запросы обслуживаются мгновенно, без ожидания LLM.

---

## WATNODE-DISTR: Распределённый инференс (distributed_inference)

### WATNODE-DISTR-1: DistributedCache
hash(prompt) → gossip-запрос к 6 нодам. Поиск <200ms. До 80% повторных запросов без инференса.

> **Следствие:** 6 слабых нод работают как одна с большим LLM-кешем.

### WATNODE-DISTR-2: Pipeline Parallelism (feature-gate)
Модель по слоям на 6 нод, тензоры через CargoEngine. Отключено по умолчанию.

> **Следствие:** 6×4GB RAM = запуск 7B модели на слабых устройствах.

### WATNODE-DISTR-3: Token Federation
Единый пул DeepSeek Flash API. Распределение между нодами, затем полная автономия.

> **Следствие:** Один аккаунт API на всю группу, экономия средств.

### WATNODE-DISTR-4: DomainRouter
Роутинг запросов по доменам (агрономия→нода3, навигация→нода5). Берётся из ProfileConfig.

> **Следствие:** Каждая нода держит свою специализированную GGUF-модель.

---

## WATNODE-REVIEW: Конвейер ревью кода (code_review_pipeline)

### WATNODE-REVIEW-1: Три агента
Programmer (пишет код+тесты), SecurityReviewer (YASA+OWASP), OnboardManager (RAM/CPU/compat).

> **Следствие:** Ни один опасный или неэффективный код не попадёт в ноду без проверки.

### WATNODE-REVIEW-2: Голосование
Оба APPROVE → AutoDeploy. Один REJECT → эскалация хозяину. Оба REJECT → блокировка.

> **Следствие:** Человек участвует только в спорных случаях.

### WATNODE-REVIEW-3: CodeSandbox
Изолированный запуск в tempfile + unshare/docker. Таймаут 30s, память 256MB.

> **Следствие:** Тесты не могут повредить основную систему.

---

## WATNODE-MEMORY: Память (дополнение)

### WATNODE-MEMORY-2: Новые DB Redis
DB 14 — кеш бортовой LLM (EdgeCache). DB 15 — распределённый LLM-кеш (DistributedCache). DB 1-6 — группы без изменений.

> **Следствие:** Кеш не пересекается с системными данными.

---

## WATNODE-CONFIG: Конфигурация (новые поля)

### WATNODE-CONFIG-1: config.toml
```toml
[edge]
model = "qwen2.5-1.5b.q4_k_m.gguf"
cache_ttl = 300
max_tokens = 512

[distributed]
cache_enabled = true
pipeline_enabled = false
token_pool_size = 1000000
```

### WATNODE-CONFIG-2: features Cargo.toml
```toml
[features]
default = []
pipeline-parallel = []  # off by default
```

---

## WATNODE-SECURITY: Безопасность (дополнение)

### WATNODE-SECURITY-2: YASA-CMD-6
Новая заповедь: "Проверяй бортовую LLM — ответ может быть неточным без связи".

### WATNODE-SECURITY-3: EdgeGuard
Валидация ответов GGUF: нет опасных команд, нет утечки ключей. Prompt-инъекция → блокировка.

---

## WATNODE-INTEGRATION: Интеграция с существующим

### WATNODE-INTEGRATION-1: Неизменяемые модули
a2a.rs, mcp_store.rs, media_bridge.rs, голос, tamagotchi.rs — без изменений.

### WATNODE-INTEGRATION-2: Архив v0.3
Все документы `docs/waters-node/MASTER_SPEC_V3.md*` перемещены в `docs/waters-node/archive/`.

---

*Документ создан: agent.architect.v1 | Утверждено: 2026-05-18*
