# WATERS Node v1.0.0 — Technical Specification

**Version:** 1.0.0 | **Date:** 2026-05-18 | **Author:** agent.architect.v1
**Format:** YASA Technical v2 | **Language:** English

---

## WATNODE-HYBRID: Hybrid LLM (hybrid_llm)

### WATNODE-HYBRID-1: Purpose
Transparent layer between LLM Router (BridgePool) and agents. Automatically switches LLM response source: remote LLM (DeepSeek/Claude/GPT-4o/Ollama) or onboard GGUF 1-3B model.

> **Consequence:** Agents continue working during connectivity loss. Node does not drop to L4 safe mode on internet outage.

### WATNODE-HYBRID-2: EdgeEngine
GGUF model loading via `llama-cpp-rs` or `candle`. CPU inference, <500ms on RPi4. Config: `[edge] model, cache_ttl, max_tokens`.

> **Consequence:** Raspberry Pi 4 gets a fully functional LLM agent without internet.

### WATNODE-HYBRID-3: SwitchProtocol
Integration with AutonomyEngine (L0-L4): L0=Remote+validate, L1=Remote+fallback, L2=Edge(simple)/Queue(complex), L3=Edge+DTN, L4=SOS.

> **Consequence:** Graceful degradation rather than binary "works/doesn't work".

### WATNODE-HYBRID-4: PrefetchCache
Prefetch responses based on active tasks. Stored in Redis DB 14-15.

> **Consequence:** Typical requests served instantly, no LLM wait time.

---

## WATNODE-DISTR: Distributed Inference (distributed_inference)

### WATNODE-DISTR-1: DistributedCache
hash(prompt) → gossip request to 6 nodes. Lookup <200ms. Up to 80% repeat queries without inference.

> **Consequence:** 6 weak nodes act as one with a large LLM cache.

### WATNODE-DISTR-2: Pipeline Parallelism (feature-gate)
Model split by layers across 6 nodes, tensors via CargoEngine. Off by default.

> **Consequence:** 6×4GB RAM = run 7B model on weak devices.

### WATNODE-DISTR-3: Token Federation
Shared DeepSeek Flash API pool. Distributed across nodes, then fully autonomous.

> **Consequence:** One API account for the entire group, cost savings.

### WATNODE-DISTR-4: DomainRouter
Request routing by domain (agriculture→node3, navigation→node5). Read from ProfileConfig.

> **Consequence:** Each node runs its own specialized GGUF model.

---

## WATNODE-REVIEW: Code Review Pipeline (code_review_pipeline)

### WATNODE-REVIEW-1: Three Agents
Programmer (writes code+tests), SecurityReviewer (YASA+OWASP), OnboardManager (RAM/CPU/compat).

> **Consequence:** No dangerous or inefficient code enters the node without review.

### WATNODE-REVIEW-2: Voting
Both APPROVE → AutoDeploy. One REJECT → escalate to owner. Both REJECT → blocked.

> **Consequence:** Human involvement only in disputed cases.

### WATNODE-REVIEW-3: CodeSandbox
Isolated execution in tempfile + unshare/docker. 30s timeout, 256MB memory.

> **Consequence:** Tests cannot damage the host system.

---

## WATNODE-MEMORY: Memory (additions)

### WATNODE-MEMORY-2: New Redis DBs
DB 14 — edge LLM cache (EdgeCache). DB 15 — distributed LLM cache (DistributedCache). DB 1-6 — groups unchanged.

> **Consequence:** Cache does not overlap with system data.

---

## WATNODE-CONFIG: Configuration (new fields)

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

### WATNODE-CONFIG-2: Cargo.toml features
```toml
[features]
default = []
pipeline-parallel = []
```

---

## WATNODE-SECURITY: Security (additions)

### WATNODE-SECURITY-2: YASA-CMD-6
New commandment: "Check the onboard LLM — its response may be inaccurate without connectivity."

### WATNODE-SECURITY-3: EdgeGuard
GGUF response validation: no dangerous commands, no key leaks. Prompt injection → blocked.

---

## WATNODE-INTEGRATION: Integration with Existing Code

### WATNODE-INTEGRATION-1: Unchanged Modules
a2a.rs, mcp_store.rs, media_bridge.rs, voice, tamagotchi.rs — no changes.

### WATNODE-INTEGRATION-2: v0.3 Archive
All `docs/waters-node/MASTER_SPEC_V3.md*` documents moved to `docs/waters-node/archive/`.

---

*Document created by: agent.architect.v1 | Approved: 2026-05-18*
