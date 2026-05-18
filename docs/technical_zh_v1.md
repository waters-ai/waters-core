# WATERS Node v1.0.0 — 技术规范

**版本：** 1.0.0 | **日期：** 2026-05-18 | **作者：** agent.architect.v1
**格式：** YASA Technical v2 | **语言：** 中文

---

## WATNODE-HYBRID: 混合 LLM（hybrid_llm）

### WATNODE-HYBRID-1: 目的
LLM 路由器（BridgePool）和智能体之间的透明层。自动切换 LLM 响应源：远程 LLM（DeepSeek/Claude/GPT-4o/Ollama）或板载 GGUF 1-3B 模型。

> **结论：** 连接中断期间智能体继续工作。互联网中断时节点不会降至 L4 安全模式。

### WATNODE-HYBRID-2: EdgeEngine
通过 `llama-cpp-rs` 或 `candle` 加载 GGUF 模型。CPU 推理，RPi4 上 <500ms。配置：`[edge] model, cache_ttl, max_tokens`。

> **结论：** Raspberry Pi 4 无需互联网即可获得功能完整的 LLM 智能体。

### WATNODE-HYBRID-3: SwitchProtocol
与 AutonomyEngine（L0-L4）集成：L0=远程+验证，L1=远程+回退，L2=边缘（简单）/队列（复杂），L3=边缘+DTN，L4=SOS。

> **结论：** 优雅降级而非二元的"工作/不工作"。

### WATNODE-HYBRID-4: PrefetchCache
基于活跃任务预取响应。存储在 Redis DB 14-15 中。

> **结论：** 典型请求即时响应，无需等待 LLM。

---

## WATNODE-DISTR: 分布式推理（distributed_inference）

### WATNODE-DISTR-1: DistributedCache
hash(prompt) → 向 6 个节点发起 gossip 请求。查找 <200ms。高达 80% 的重复查询无需推理。

> **结论：** 6 个弱节点像一个大 LLM 缓存一样工作。

### WATNODE-DISTR-2: Pipeline Parallelism（功能门控）
模型按层分割到 6 个节点，张量通过 CargoEngine 传输。默认关闭。

> **结论：** 6×4GB RAM = 在弱设备上运行 7B 模型。

### WATNODE-DISTR-3: Token Federation
共享 DeepSeek Flash API 池。在节点间分配，然后完全自治。

> **结论：** 整个组使用一个 API 账户，节省成本。

### WATNODE-DISTR-4: DomainRouter
按领域路由请求（农业→节点3，导航→节点5）。从 ProfileConfig 读取。

> **结论：** 每个节点运行自己的专用 GGUF 模型。

---

## WATNODE-REVIEW: 代码审查管道（code_review_pipeline）

### WATNODE-REVIEW-1: 三个智能体
程序员（编写代码+测试）、安全审查员（YASA+OWASP）、板载管理器（RAM/CPU/兼容性）。

> **结论：** 未经审查，没有危险或低效的代码进入节点。

### WATNODE-REVIEW-2: 投票
两个 APPROVE → 自动部署。一个 REJECT → 升级到所有者。两个 REJECT → 阻止。

> **结论：** 仅在争议情况下需要人工参与。

### WATNODE-REVIEW-3: CodeSandbox
在 tempfile + unshare/docker 中隔离执行。30 秒超时，256MB 内存。

> **结论：** 测试不会损坏主机系统。

---

## WATNODE-MEMORY: 内存（新增）

### WATNODE-MEMORY-2: 新的 Redis 数据库
DB 14 — 边缘 LLM 缓存（EdgeCache）。DB 15 — 分布式 LLM 缓存（DistributedCache）。DB 1-6 — 组不变。

> **结论：** 缓存不与系统数据重叠。

---

## WATNODE-CONFIG: 配置（新字段）

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

### WATNODE-CONFIG-2: Cargo.toml 特性
```toml
[features]
default = []
pipeline-parallel = []
```

---

## WATNODE-SECURITY: 安全（新增）

### WATNODE-SECURITY-2: YASA-CMD-6
新戒律："检查板载 LLM — 没有连接时其响应可能不准确。"

### WATNODE-SECURITY-3: EdgeGuard
GGUF 响应验证：无危险命令，无密钥泄露。提示注入 → 阻止。

---

## WATNODE-INTEGRATION: 与现有代码集成

### WATNODE-INTEGRATION-1: 未更改的模块
a2a.rs、mcp_store.rs、media_bridge.rs、语音、tamagotchi.rs — 无更改。

### WATNODE-INTEGRATION-2: v0.3 归档
所有 `docs/waters-node/MASTER_SPEC_V3.md*` 文档已移至 `docs/waters-node/archive/`。

---

*文档创建者：agent.architect.v1 | 批准日期：2026-05-18*
