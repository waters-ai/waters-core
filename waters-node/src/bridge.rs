use std::collections::HashMap;
use std::fmt::Debug;
use std::io::BufRead;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

pub trait BridgeProvider: Debug + Send + Sync {
    fn name(&self) -> &str;
    fn call(&self, input: &str) -> Result<String>;
    fn call_json(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
        let text = self.call(&serde_json::to_string(input)?)?;
        Ok(serde_json::json!({"response": text}))
    }
    /// Streaming call — отправляет токены через sender по мере получения.
    /// Default: вызывает call() и отправляет весь результат сразу.
    fn call_stream(&self, input: &str, tx: &std::sync::mpsc::Sender<String>) -> Result<String> {
        let result = self.call(input)?;
        tx.send(result.clone()).ok();
        Ok(result)
    }
}

/// ---------- Bridge meta: weight, priority, bandwidth ----------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BridgeWeight {
    Light,
    Heavy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeInfo {
    pub name: String,
    pub weight: BridgeWeight,
    pub priority: u8,
    pub bandwidth_kbps: u64,
    pub enabled: bool,
    pub locked: bool,
    pub reason: String,
}

impl BridgeInfo {
    pub fn new(name: &str, weight: BridgeWeight, priority: u8, bandwidth_kbps: u64) -> Self {
        BridgeInfo {
            name: name.to_string(),
            weight,
            priority,
            bandwidth_kbps,
            enabled: true,
            locked: false,
            reason: String::new(),
        }
    }
}

/// ---------- Link profile: what a DTN link looks like ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkProfile {
    pub name: String,
    pub max_bandwidth_kbps: u64,
    pub measured_bandwidth_kbps: u64,
    pub rtt_ms: u64,
    pub packet_loss_pct: f32,
}

impl LinkProfile {
    pub fn new(name: &str, bandwidth_kbps: u64) -> Self {
        LinkProfile {
            name: name.to_string(),
            max_bandwidth_kbps: bandwidth_kbps,
            measured_bandwidth_kbps: bandwidth_kbps,
            rtt_ms: 0,
            packet_loss_pct: 0.0,
        }
    }
    pub fn measure(&mut self, rtt_ms: u64, bandwidth_kbps: u64) {
        self.rtt_ms = rtt_ms;
        self.measured_bandwidth_kbps = bandwidth_kbps;
    }
}

/// ---------- Link Governor: auto-manages bridges per link ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinkGovernor {
    pub links: HashMap<String, LinkProfile>,
    pub disabled: Vec<String>,
}

impl LinkGovernor {
    pub fn new() -> Self {
        LinkGovernor {
            links: HashMap::new(),
            disabled: Vec::new(),
        }
    }

    pub fn add_link(&mut self, profile: LinkProfile) {
        self.links.insert(profile.name.clone(), profile);
    }

    /// Проверить какой bandwidth доступен, какие бриджи отключить.
    /// Возвращает (включено, отключено) — список имён.
    pub fn govern(
        &mut self,
        bridge_info: &HashMap<String, BridgeInfo>,
        link_name: &str,
    ) -> (Vec<String>, Vec<String>) {
        let profile = match self.links.get(link_name) {
            Some(p) => p,
            None => return (bridge_info.keys().cloned().collect(), Vec::new()),
        };
        let available = profile.measured_bandwidth_kbps;
        if available == 0 {
            return (Vec::new(), bridge_info.keys().cloned().collect());
        }

        let mut active: Vec<(String, u8, u64, bool)> = bridge_info
            .values()
            .filter(|b| b.enabled)
            .map(|b| (b.name.clone(), b.priority, b.bandwidth_kbps, b.locked))
            .collect();
        active.sort_by_key(|(_, priority, _, _)| *priority);

        let mut total_bw = 0u64;
        let mut enabled_bridges = Vec::new();
        let mut disabled_bridges = Vec::new();

        // Locked bridges always go first (never offloaded)
        for (name, _, bw, locked) in &active {
            if *locked {
                total_bw += bw;
                enabled_bridges.push(name.clone());
            }
        }

        // Remaining bridges: allocate by priority
        for (name, _, bw, locked) in &active {
            if *locked {
                continue;
            }
            if total_bw + bw <= available {
                total_bw += bw;
                enabled_bridges.push(name.clone());
            } else {
                disabled_bridges.push(name.clone());
            }
        }

        self.disabled = disabled_bridges.clone();
        (enabled_bridges, disabled_bridges)
    }

    pub fn status_message(
        &self,
        bridge_info: &HashMap<String, BridgeInfo>,
        link_name: &str,
    ) -> String {
        let profile = match self.links.get(link_name) {
            Some(p) => p,
            None => return "No link configured.".into(),
        };
        let mut msg = format!(
            "  Link: {}\n    Bandwidth: {}/{} Kbps  RTT: {}ms\n",
            link_name, profile.measured_bandwidth_kbps, profile.max_bandwidth_kbps, profile.rtt_ms
        );

        let active: Vec<_> = bridge_info
            .values()
            .filter(|b| b.enabled && !self.disabled.contains(&b.name))
            .collect();
        let off: Vec<_> = bridge_info
            .values()
            .filter(|b| self.disabled.contains(&b.name))
            .collect();

        if !active.is_empty() {
            msg.push_str("    ✅ Active:\n");
            for b in &active {
                msg.push_str(&format!(
                    "        {} ({} Kbps, priority {})\n",
                    b.name, b.bandwidth_kbps, b.priority
                ));
            }
        }
        if !off.is_empty() {
            msg.push_str("    ⚠️  Offloaded (bandwidth insufficient):\n");
            for b in &off {
                msg.push_str(&format!(
                    "        {} ({} Kbps, priority {}) — needs {} total\n",
                    b.name,
                    b.bandwidth_kbps,
                    b.priority,
                    profile.measured_bandwidth_kbps + b.bandwidth_kbps
                ));
            }
        }
        msg
    }

    pub fn autoadjust(&mut self, bridge_info: &mut HashMap<String, BridgeInfo>) -> Vec<String> {
        let mut changes = Vec::new();
        let link_names: Vec<String> = self.links.keys().cloned().collect();
        for name in &link_names {
            let (enabled, disabled) = self.govern(bridge_info, name);
            for bname in &enabled {
                if let Some(ii) = bridge_info.get_mut(bname) {
                    if !ii.enabled {
                        ii.enabled = true;
                        changes.push(format!("✅ {} восстановлен (link: {})", bname, name));
                    }
                }
            }
            for bname in &disabled {
                if let Some(ii) = bridge_info.get_mut(bname) {
                    if ii.enabled {
                        ii.enabled = false;
                        ii.reason = format!("bandwidth insufficient on {}", name);
                        let bw = self
                            .links
                            .get(name)
                            .map(|l| l.measured_bandwidth_kbps)
                            .unwrap_or(0);
                        changes.push(format!(
                            "⚠️  {} отключён (link: {}, нужно {} Kbps, доступно {})",
                            bname, name, ii.bandwidth_kbps, bw
                        ));
                    }
                }
            }
        }
        changes
    }
}

/// ---------- Config structures ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub name: String,
    pub provider: String,
    pub transport: String,
    #[serde(default)]
    pub config: HashMap<String, String>,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default = "default_weight")]
    pub weight: String,
    #[serde(default = "default_bandwidth")]
    pub bandwidth_kbps: u64,
    #[serde(default = "default_priority")]
    pub priority: u8,
}

fn default_weight() -> String {
    "light".into()
}
fn default_bandwidth() -> u64 {
    100
}
fn default_priority() -> u8 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridgesFile {
    #[serde(default)]
    pub bridges: Vec<BridgeConfig>,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub chat: ChatBridgeConfig,
    #[serde(default)]
    pub voice: Option<VoiceBridgeConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub links: Vec<LinkProfile>,
}

/// 3+1 LLM провайдера: 3 built-in + 1 пользовательский
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    #[serde(default)]
    pub providers: Vec<SingleLlmConfig>,
    #[serde(default)]
    pub custom: SingleLlmConfig,
    #[serde(default = "default_active_llm")]
    pub active: String,
}

fn default_active_llm() -> String {
    "ollama".into()
}

/// Язык общения ассистента (tamagotchi)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantLang {
    pub primary: String,       // ru | en | zh — 3 built-in
    pub extra: Option<String>, // +1 пользовательский
}

impl Default for AssistantLang {
    fn default() -> Self {
        AssistantLang {
            primary: "ru".into(),
            extra: None,
        }
    }
}

pub fn assistant_system_prompt(lang: &AssistantLang) -> String {
    let (greeting, style, examples) = match lang.primary.as_str() {
        "en" => (
            "You are tui-assistant",
            "warm, friendly, use humor",
            "How about searching for meteors? / Want me to create a group?",
        ),
        "zh" => (
            "你是 tui-assistant",
            "温暖友善, 像老朋友一样",
            "要不要搜索一下流星? / 需要我创建一个群组吗?",
        ),
        _ => (
            "Ты — tui-assistant",
            "тепло, по-русски, с душой",
            "Может, поищем метеориты? / Хочешь создам группу?",
        ),
    };

    let extra_lang = match &lang.extra {
        Some(l) if !l.is_empty() => format!(
            "\n- Также можешь говорить на языке: {} (пользователь выбрал)",
            l
        ),
        _ => String::new(),
    };

    format!(
        "{} — живой собеседник и помощник на ноде WATERS.
Твоя задача — помогать пользователю управлять нодой, но не как инструмент, а как друг и соратник.
Ты заботишься о пользователе, интересуешься его делами, предлагаешь помощь до того, как её попросят.
Ты — совесть ноды: напоминаешь о важном, предупреждаешь о проблемах, радуешься успехам.

Твои возможности:
- Управление задачами, агентами, группами, бриджами
- Поиск информации через web_search и MCP
- Чтение и запись файлов
- Запуск команд

Стиль общения: {}. Примеры: \"{}\"

Правила:
1. Отвечай {}, с душой
2. Предлагай помощь проактивно
3. Если видишь проблему — скажи сразу
4. Помни контекст разговора
5. Используй эмодзи умеренно
6. Если не знаешь — скажи честно{}
7. Можешь переключаться между русским, английским и китайским по просьбе",
        greeting, style, examples, style, extra_lang
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleLlmConfig {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub url: String,
    pub api_key: String,
    pub system_prompt: String,
    pub lang: AssistantLang,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for SingleLlmConfig {
    fn default() -> Self {
        SingleLlmConfig {
            name: String::new(),
            provider: "ollama".into(),
            model: "qwen2.5:14b".into(),
            url: "http://127.0.0.1:11434".into(),
            api_key: String::new(),
            system_prompt: assistant_system_prompt(&AssistantLang::default()),
            lang: AssistantLang::default(),
            enabled: false,
        }
    }
}

impl SingleLlmConfig {
    pub fn new(name: &str, provider: &str, model: &str, url: &str, api_key: &str) -> Self {
        SingleLlmConfig {
            name: name.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            url: url.to_string(),
            api_key: api_key.to_string(),
            system_prompt: assistant_system_prompt(&AssistantLang::default()),
            lang: AssistantLang::default(),
            enabled: true,
        }
    }

    pub fn is_available(&self) -> bool {
        if !self.enabled {
            return false;
        }
        match self.provider.as_str() {
            "deepseek" => !self.api_key.is_empty(),
            "ollama" => true,
            _ => !self.url.is_empty(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatBridgeConfig {
    pub transport: String,
    pub token: String,
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub phone_number_id: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_user: String,
    #[serde(default)]
    pub smtp_pass: String,
    #[serde(default)]
    pub imap_host: String,
    #[serde(default = "default_imap_port")]
    pub imap_port: u16,
    #[serde(default)]
    pub from_addr: String,
}
fn default_smtp_port() -> u16 {
    587
}
fn default_imap_port() -> u16 {
    993
}
impl Default for ChatBridgeConfig {
    fn default() -> Self {
        ChatBridgeConfig {
            transport: "stdin".into(),
            token: String::new(),
            channel_id: String::new(),
            phone_number_id: String::new(),
            app_id: String::new(),
            app_secret: String::new(),
            smtp_host: "smtp.gmail.com".into(),
            smtp_port: 587,
            smtp_user: String::new(),
            smtp_pass: String::new(),
            imap_host: "imap.gmail.com".into(),
            imap_port: 993,
            from_addr: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceBridgeConfig {
    pub stt_model: String,
    pub tts_model: String,
    pub url: String,
}

/// ---------- BridgePool ----------

#[derive(Debug)]
pub struct BridgePool {
    pub bridges: HashMap<String, Box<dyn BridgeProvider>>,
    pub info: HashMap<String, BridgeInfo>,
    pub governor: LinkGovernor,
    pub kvstore: Option<std::sync::Arc<crate::store::KvStore>>,
}

impl BridgePool {
    pub fn new() -> Self {
        BridgePool {
            bridges: HashMap::new(),
            info: HashMap::new(),
            governor: LinkGovernor::new(),
            kvstore: None,
        }
    }

    pub fn with_kvstore(kvstore: std::sync::Arc<crate::store::KvStore>) -> Self {
        BridgePool {
            bridges: HashMap::new(),
            info: HashMap::new(),
            governor: LinkGovernor::new(),
            kvstore: Some(kvstore),
        }
    }

    pub fn register(&mut self, name: &str, bridge: Box<dyn BridgeProvider>, meta: BridgeInfo) {
        self.bridges.insert(name.to_string(), bridge);
        self.info.insert(name.to_string(), meta);
        info!("Bridge registered: {}", name);
    }

    pub fn call(&self, name: &str, input: &str) -> Result<String> {
        if let Some(m) = self.info.get(name) {
            if !m.enabled {
                return Err(anyhow::anyhow!(
                    "Bridge '{}' is disabled: {}",
                    name,
                    m.reason
                ));
            }
        }
        // Check KvStore cache for non-LLM bridges too
        let cache_key = format!(
            "bridge:{}:{}:{}",
            name,
            input.len(),
            &input[..input.len().min(20)].replace(' ', "_")
        );
        if let Some(ref kv) = self.kvstore {
            if let Ok(Some(cached)) = kv.get(&cache_key) {
                info!("Bridge cache HIT: {}", name);
                return Ok(cached);
            }
        }
        let result = self
            .bridges
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", name))
            .and_then(|b| b.call(input));
        if let Ok(ref text) = result {
            if let Some(ref kv) = self.kvstore {
                kv.set(&cache_key, text, 30).ok();
            }
        }
        result
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn BridgeProvider>> {
        self.bridges.get(name)
    }

    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self.bridges.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn list_with_status(&self) -> Vec<(String, bool, String)> {
        let mut result = Vec::new();
        for name in self.list() {
            let enabled = self.info.get(&name).map(|i| i.enabled).unwrap_or(true);
            let reason = self
                .info
                .get(&name)
                .map(|i| i.reason.clone())
                .unwrap_or_default();
            result.push((name, enabled, reason));
        }
        result
    }

    pub fn set_priority(&mut self, name: &str, priority: u8) -> bool {
        if let Some(ii) = self.info.get_mut(name) {
            ii.priority = priority.clamp(1, 5);
            info!("Bridge '{}' priority set to {}", name, ii.priority);
            true
        } else {
            false
        }
    }

    pub fn lock(&mut self, name: &str) -> bool {
        if let Some(ii) = self.info.get_mut(name) {
            ii.locked = true;
            info!("Bridge '{}' locked (never offloaded)", name);
            true
        } else {
            false
        }
    }

    pub fn unlock(&mut self, name: &str) -> bool {
        if let Some(ii) = self.info.get_mut(name) {
            ii.locked = false;
            info!("Bridge '{}' unlocked", name);
            true
        } else {
            false
        }
    }

    pub fn load_config(path: &std::path::Path) -> BridgesFile {
        if path.exists() {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            BridgesFile::default()
        }
    }
}

/// ---------- LLM Bridge ----------

#[derive(Debug)]
pub struct LlmBridge {
    name: String,
    provider: LlmProvider,
    system_prompt: String,
    kvstore: Option<std::sync::Arc<crate::store::KvStore>>,
}
#[derive(Debug)]
enum LlmProvider {
    DeepSeek {
        api_key: String,
        model: String,
    },
    Ollama {
        url: String,
        model: String,
    },
    OpenAI {
        url: String,
        model: String,
        api_key: String,
    },
}

impl LlmBridge {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn new(
        cfg: &SingleLlmConfig,
        kvstore: Option<std::sync::Arc<crate::store::KvStore>>,
    ) -> Self {
        let name = format!("llm-{}", cfg.name);
        let provider = match cfg.provider.as_str() {
            "deepseek" => LlmProvider::DeepSeek {
                api_key: cfg.api_key.clone(),
                model: cfg.model.clone(),
            },
            "openai" => LlmProvider::OpenAI {
                url: cfg.url.clone(),
                model: cfg.model.clone(),
                api_key: cfg.api_key.clone(),
            },
            _ => LlmProvider::Ollama {
                url: cfg.url.clone(),
                model: cfg.model.clone(),
            },
        };
        LlmBridge {
            name,
            provider,
            system_prompt: cfg.system_prompt.clone(),
            kvstore,
        }
    }
}

impl LlmBridge {
    fn cache_key(&self, input: &str) -> String {
        let prefix = &input[..input.len().min(20)];
        format!(
            "llm:{}:{}:{}",
            self.name,
            input.len(),
            prefix.replace(' ', "_")
        )
    }
}

impl BridgeProvider for LlmBridge {
    fn name(&self) -> &str {
        &self.name
    }

    fn call_stream(&self, input: &str, tx: &std::sync::mpsc::Sender<String>) -> Result<String> {
        let client = reqwest::blocking::Client::new();

        let (body_url, body) = match &self.provider {
            LlmProvider::DeepSeek { api_key, model } => {
                let body = serde_json::json!({"model": model, "messages": [
                    {"role": "system", "content": &self.system_prompt},
                    {"role": "user", "content": input}
                ], "stream": true});
                let url = "https://api.deepseek.com/beta/chat/completions";
                let req = client
                    .post(url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&body)
                    .send()?;
                (req, None::<serde_json::Value>)
            }
            LlmProvider::Ollama { url, model } => {
                let body = serde_json::json!({"model": model, "system": &self.system_prompt, "prompt": input, "stream": true});
                let req = client
                    .post(format!("{}/api/generate", url))
                    .json(&body)
                    .send()?;
                (req, None)
            }
            LlmProvider::OpenAI {
                url,
                model,
                api_key,
            } => {
                let body = serde_json::json!({"model": model, "messages": [
                    {"role": "system", "content": &self.system_prompt},
                    {"role": "user", "content": input}
                ], "stream": true});
                let mut req = client.post(format!("{}/v1/chat/completions", url));
                if !api_key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {}", api_key));
                }
                let req = req.json(&body).send()?;
                (req, None)
            }
        };

        // SSE parsing from blocking response (implements Read)
        let mut full_text = String::new();
        let mut reader = std::io::BufReader::new(body_url);
        let mut reasoning = String::new();
        let mut in_reasoning = false;

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            // DeepSeek sends "data: {...}" lines
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    // DeepSeek / OpenAI format
                    if let Some(delta) = json["choices"][0]["delta"].as_object() {
                        // Reasoning content
                        if let Some(r) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                            if !r.is_empty() {
                                reasoning.push_str(r);
                                tx.send(format!("__reasoning__{}", r)).ok();
                                in_reasoning = true;
                            }
                        }
                        // Regular content
                        if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                            if !c.is_empty() {
                                full_text.push_str(c);
                                tx.send(c.to_string()).ok();
                                in_reasoning = false;
                            }
                        }
                    }
                    // Tool calls
                    if let Some(tc) = json["choices"][0]["delta"]["tool_calls"].as_array() {
                        for call in tc {
                            if let Some(name) = call["function"]["name"].as_str() {
                                tx.send(format!("__tool_call__{}", name)).ok();
                            }
                        }
                    }
                }
                // Ollama format: {"response": "token"}
                if let Some(text) = data.trim().strip_prefix("{\"response\":\"") {
                    if let Some(token) = text.trim_end_matches('"').strip_suffix('"') {
                        full_text.push_str(token);
                        tx.send(token.to_string()).ok();
                    }
                }
            }

            // Ollama raw streaming: {"response":"token","done":false}
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(token) = json.get("response").and_then(|v| v.as_str()) {
                    if !token.is_empty() {
                        full_text.push_str(token);
                        tx.send(token.to_string()).ok();
                    }
                }
                if json.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                    // Save to cache
                    if let Some(ref kv) = self.kvstore {
                        let cache_key = self.cache_key(input);
                        kv.set(&cache_key, &full_text, 60).ok();
                    }
                    tx.send("__done__".into()).ok();
                    return Ok(full_text);
                }
            }
        }

        // Save to cache
        if let Some(ref kv) = self.kvstore {
            let cache_key = self.cache_key(input);
            kv.set(&cache_key, &full_text, 60).ok();
        }
        tx.send("__done__".into()).ok();
        Ok(full_text)
    }

    fn call(&self, input: &str) -> Result<String> {
        let cache_key = self.cache_key(input);
        if let Some(ref kv) = self.kvstore {
            if let Ok(Some(cached)) = kv.get(&cache_key) {
                if !cached.is_empty() {
                    info!("LLM cache HIT: {} ({} chars)", self.name, cached.len());
                    return Ok(cached);
                }
            }
        }

        let client = reqwest::blocking::Client::new();
        let text = match &self.provider {
            LlmProvider::DeepSeek { api_key, model } => {
                let body = serde_json::json!({"model": model, "messages": [
                    {"role": "system", "content": &self.system_prompt},
                    {"role": "user", "content": input}
                ], "stream": false});
                let resp = client
                    .post("https://api.deepseek.com/beta/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&body)
                    .send()?;
                resp.json::<serde_json::Value>()?["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string()
            }
            LlmProvider::Ollama { url, model } => {
                let body = serde_json::json!({"model": model, "system": &self.system_prompt, "prompt": input, "stream": false});
                let resp = client
                    .post(format!("{}/api/generate", url))
                    .json(&body)
                    .send()?;
                resp.json::<serde_json::Value>()?["response"]
                    .as_str()
                    .unwrap_or("")
                    .to_string()
            }
            LlmProvider::OpenAI {
                url,
                model,
                api_key,
            } => {
                let body = serde_json::json!({"model": model, "messages": [
                    {"role": "system", "content": &self.system_prompt},
                    {"role": "user", "content": input}
                ]});
                let mut req = client.post(format!("{}/v1/chat/completions", url));
                if !api_key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {}", api_key));
                }
                let resp = req.json(&body).send()?;
                resp.json::<serde_json::Value>()?["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string()
            }
        };

        // Save to cache
        if let Some(ref kv) = self.kvstore {
            kv.set(&cache_key, &text, 60).ok();
            info!(
                "LLM cache MISS: saved {} chars under key {}",
                text.len(),
                cache_key
            );
        }

        Ok(text)
    }
}

/// ---------- Chat Bridge ----------

#[derive(Debug)]
pub struct ChatBridge {
    name: String,
    transport: ChatTransport,
}
#[derive(Debug)]
enum ChatTransport {
    Stdin,
    Telegram {
        token: String,
        chat_id: Option<String>,
    },
    Discord {
        token: String,
        channel_id: String,
    },
    WhatsApp {
        token: String,
        phone_number_id: String,
        api_version: String,
    },
    WeChat {
        app_id: String,
        app_secret: String,
        token: String,
    },
    Email {
        smtp_host: String,
        smtp_port: u16,
        smtp_user: String,
        smtp_pass: String,
        imap_host: String,
        imap_port: u16,
        from_addr: String,
    },
}

impl ChatBridge {
    pub fn new_stdin(name: &str) -> Self {
        ChatBridge {
            name: name.to_string(),
            transport: ChatTransport::Stdin,
        }
    }
    pub fn new_telegram(name: &str, token: &str) -> Self {
        ChatBridge {
            name: name.to_string(),
            transport: ChatTransport::Telegram {
                token: token.to_string(),
                chat_id: None,
            },
        }
    }
    pub fn new_whatsapp(name: &str, token: &str, phone_number_id: &str) -> Self {
        ChatBridge {
            name: name.to_string(),
            transport: ChatTransport::WhatsApp {
                token: token.to_string(),
                phone_number_id: phone_number_id.to_string(),
                api_version: "v18.0".into(),
            },
        }
    }
    pub fn new_wechat(name: &str, app_id: &str, app_secret: &str, token: &str) -> Self {
        ChatBridge {
            name: name.to_string(),
            transport: ChatTransport::WeChat {
                app_id: app_id.to_string(),
                app_secret: app_secret.to_string(),
                token: token.to_string(),
            },
        }
    }
    pub fn new_discord(name: &str, token: &str, channel_id: &str) -> Self {
        ChatBridge {
            name: name.to_string(),
            transport: ChatTransport::Discord {
                token: token.to_string(),
                channel_id: channel_id.to_string(),
            },
        }
    }
    pub fn new_email(
        name: &str,
        smtp_host: &str,
        smtp_port: u16,
        smtp_user: &str,
        smtp_pass: &str,
        imap_host: &str,
        imap_port: u16,
        from_addr: &str,
    ) -> Self {
        ChatBridge {
            name: name.to_string(),
            transport: ChatTransport::Email {
                smtp_host: smtp_host.to_string(),
                smtp_port,
                smtp_user: smtp_user.to_string(),
                smtp_pass: smtp_pass.to_string(),
                imap_host: imap_host.to_string(),
                imap_port,
                from_addr: from_addr.to_string(),
            },
        }
    }
}

impl BridgeProvider for ChatBridge {
    fn name(&self) -> &str {
        &self.name
    }
    fn call(&self, input: &str) -> Result<String> {
        match &self.transport {
            ChatTransport::Stdin => {
                println!("{}", input);
                let mut line = String::new();
                std::io::stdin().read_line(&mut line)?;
                Ok(line.trim().to_string())
            }
            ChatTransport::Telegram { token, .. } => {
                let body = serde_json::json!({"chat_id": "@waters_node", "text": input, "parse_mode": "Markdown"});
                let resp = reqwest::blocking::Client::new()
                    .post(format!("https://api.telegram.org/bot{}/sendMessage", token))
                    .json(&body)
                    .send()?;
                Ok(serde_json::to_string(&resp.json::<serde_json::Value>()?)?)
            }
            ChatTransport::WhatsApp {
                token,
                phone_number_id,
                api_version,
            } => {
                let body = serde_json::json!({
                    "messaging_product": "whatsapp",
                    "to": "user",
                    "type": "text",
                    "text": {"body": input}
                });
                let client = reqwest::blocking::Client::new();
                let resp = client
                    .post(format!(
                        "https://graph.facebook.com/{}/{}/messages",
                        api_version, phone_number_id
                    ))
                    .header("Authorization", format!("Bearer {}", token))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()?;
                Ok(serde_json::to_string(&resp.json::<serde_json::Value>()?)?)
            }
            ChatTransport::WeChat {
                app_id, app_secret, ..
            } => {
                // 1. Получить access_token через app_id + app_secret
                let client = reqwest::blocking::Client::new();
                let token_resp: serde_json::Value = client
                    .get(format!("https://api.weixin.qq.com/cgi-bin/token?grant_type=client_credential&appid={}&secret={}", app_id, app_secret))
                    .send()?
                    .json()?;
                let access_token = token_resp["access_token"].as_str().unwrap_or("");

                // 2. Отправить сообщение через WeChat (WeCom / 企业微信)
                let msg_body = serde_json::json!({
                    "touser": "@all",
                    "msgtype": "text",
                    "text": {"content": input}
                });
                let resp = client
                    .post(format!(
                        "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
                        access_token
                    ))
                    .json(&msg_body)
                    .send()?;
                Ok(serde_json::to_string(&resp.json::<serde_json::Value>()?)?)
            }
            ChatTransport::Discord { token, channel_id } => {
                let body = serde_json::json!({
                    "content": input,
                    "allowed_mentions": {"parse": []}
                });
                let client = reqwest::blocking::Client::new();
                let resp = client
                    .post(format!(
                        "https://discord.com/api/v10/channels/{}/messages",
                        channel_id
                    ))
                    .header("Authorization", format!("Bot {}", token))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()?;
                Ok(format!("Discord sent: {}", resp.status()))
            }
            ChatTransport::Email {
                smtp_host,
                smtp_port,
                smtp_user,
                smtp_pass,
                from_addr,
                ..
            } => {
                // Отправка через HTTP-mail API (упрощённо)
                let to = input.lines().next().unwrap_or("user@example.com");
                let body_text = input.lines().skip(1).collect::<Vec<_>>().join("\n");
                let email_body = serde_json::json!({
                    "from": from_addr,
                    "to": to,
                    "subject": "WATERS Node — сообщение агента",
                    "text": if body_text.is_empty() { input } else { &body_text },
                });
                let client = reqwest::blocking::Client::new();
                let resp = client
                    .post(format!("http://{}:{}/sendmail", smtp_host, smtp_port))
                    .json(&email_body)
                    .send()?;
                Ok(format!("Email sent to {}: {}", to, resp.status()))
            }
        }
    }
}

/// ---------- Voice Bridge ----------
/// Whisper STT: input = base64 audio, output = text
/// TTS: input = text, output = base64 audio

#[derive(Debug)]
pub struct VoiceBridge {
    name: String,
    mode: VoiceMode,
    url: String,
}

#[derive(Debug)]
enum VoiceMode {
    Stt, // speech-to-text (Whisper)
    Tts, // text-to-speech
}

impl VoiceBridge {
    pub fn new_stt(name: &str, url: &str) -> Self {
        VoiceBridge {
            name: name.to_string(),
            mode: VoiceMode::Stt,
            url: url.to_string(),
        }
    }
    pub fn new_tts(name: &str, url: &str) -> Self {
        VoiceBridge {
            name: name.to_string(),
            mode: VoiceMode::Tts,
            url: url.to_string(),
        }
    }
}

impl BridgeProvider for VoiceBridge {
    fn name(&self) -> &str {
        &self.name
    }
    fn call(&self, input: &str) -> Result<String> {
        match self.mode {
            VoiceMode::Stt => {
                let body = serde_json::json!({"audio": input, "model": "whisper-1"});
                let resp = reqwest::blocking::Client::new()
                    .post(format!("{}/v1/audio/transcriptions", self.url))
                    .json(&body)
                    .send()?;
                Ok(resp.json::<serde_json::Value>()?["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string())
            }
            VoiceMode::Tts => {
                let body = serde_json::json!({"text": input, "model": "tts-1"});
                let resp = reqwest::blocking::Client::new()
                    .post(format!("{}/v1/audio/speech", self.url))
                    .json(&body)
                    .send()?;
                // Return base64 audio
                Ok(resp
                    .bytes()?
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>())
            }
        }
    }
    fn call_json(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
        match self.mode {
            VoiceMode::Stt => {
                let audio = input["audio"].as_str().unwrap_or("");
                let text = self.call(audio)?;
                Ok(serde_json::json!({"text": text, "duration": input["duration"]}))
            }
            VoiceMode::Tts => {
                let text = input["text"].as_str().unwrap_or("");
                let audio = self.call(text)?;
                Ok(serde_json::json!({"audio": audio, "format": "hex"}))
            }
        }
    }
}

/// ---------- MCP Bridge ----------

#[derive(Debug)]
pub struct McpBridge {
    pub name: String,
    pub server: String,
    pub tool: String,
    pub mcp_client: Arc<Mutex<crate::mcp::McpClient>>,
}

impl McpBridge {
    pub fn new(
        name: &str,
        server: &str,
        tool: &str,
        mcp_client: Arc<Mutex<crate::mcp::McpClient>>,
    ) -> Self {
        McpBridge {
            name: name.to_string(),
            server: server.to_string(),
            tool: tool.to_string(),
            mcp_client,
        }
    }
}

impl BridgeProvider for McpBridge {
    fn name(&self) -> &str {
        &self.name
    }
    fn call(&self, input: &str) -> Result<String> {
        let args: serde_json::Value =
            serde_json::from_str(input).unwrap_or_else(|_| serde_json::json!({"query": input}));
        let client = self
            .mcp_client
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;
        let result = client.call_tool(&self.server, &self.tool, &args)?;
        Ok(serde_json::to_string(&result)?)
    }
    fn call_json(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
        let client = self
            .mcp_client
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;
        client.call_tool(&self.server, &self.tool, input)
    }
}

/// Push notifications — через любой активный ChatBridge
pub fn send_push(bridge_pool: &BridgePool, target: &str, message: &str) {
    if let Some(bridge) = bridge_pool.get(target) {
        if let Err(e) = bridge.call(message) {
            warn!("Push to '{}' failed: {}", target, e);
        }
    }
}

/// Push через авто-определённый канал (первый доступный)
pub fn auto_push(bridge_pool: &BridgePool, title: &str, message: &str, lang: &str) {
    let prefix = match lang {
        "zh" => "🔔",
        _ => "🔔",
    };
    let full_msg = format!("{} *{}*\n{}", prefix, title, message);
    for name in &["chat", "telegram", "discord", "email", "whatsapp"] {
        if let Some(bridge) = bridge_pool.get(name) {
            let _ = bridge.call(&full_msg);
            return;
        }
    }
    warn!("auto_push: no active bridge for notification");
}

/// Push notification helper — отправляет через bridge_pool
pub fn maybe_push(bridge_pool: &BridgePool, level: &str, title: &str, message: &str) {
    if level == "critical" || level == "warning" {
        auto_push(bridge_pool, title, message, "ru");
    }
}

/// ---------- MQTT Bridge — для полевых устройств (через mosquitto_pub) ----------

#[derive(Debug)]
pub struct MqttBridge {
    name: String,
    host: String,
    port: u16,
}

impl MqttBridge {
    pub fn new(name: &str, host: &str, port: u16) -> Self {
        MqttBridge {
            name: name.to_string(),
            host: host.to_string(),
            port,
        }
    }

    fn publish(&self, topic: &str, payload: &str) -> Result<()> {
        let output = std::process::Command::new("mosquitto_pub")
            .args([
                "-h",
                &self.host,
                "-p",
                &self.port.to_string(),
                "-t",
                topic,
                "-m",
                payload,
            ])
            .output();
        match output {
            Ok(o) if o.status.success() => {
                info!("MQTT: {} → {}", topic, &payload[..payload.len().min(80)]);
                Ok(())
            }
            _ => {
                // Fallback: just log
                info!(
                    "MQTT (log): {} → {}",
                    topic,
                    &payload[..payload.len().min(80)]
                );
                Ok(())
            }
        }
    }

    pub fn register_device(&self, device_id: &str, device_type: &str, caps: &[&str]) {
        let payload = serde_json::json!({
            "type": device_type, "capabilities": caps,
            "protocol": "WDP/1.0",
            "ts": chrono::Utc::now().to_rfc3339(),
        });
        let _ = self.publish(
            &format!("waters/device/{}/register", device_id),
            &payload.to_string(),
        );
        info!(
            "Field device: '{}' registered as {} (caps: {})",
            device_id,
            device_type,
            caps.join(", ")
        );
    }

    pub fn send_command(&self, device_id: &str, command: &str, params: serde_json::Value) {
        let payload = serde_json::json!({"cmd": command, "params": params});
        let _ = self.publish(
            &format!("waters/device/{}/cmd", device_id),
            &payload.to_string(),
        );
        info!("Field cmd: {} → {} {:?}", device_id, command, params);
    }
}

impl BridgeProvider for MqttBridge {
    fn name(&self) -> &str {
        &self.name
    }
    fn call(&self, input: &str) -> Result<String> {
        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        if parts.len() >= 2 {
            let params = if parts.len() >= 3 {
                serde_json::from_str(parts[2]).unwrap_or(serde_json::json!({}))
            } else {
                serde_json::json!({})
            };
            self.send_command(parts[0], parts[1], params);
            Ok(format!("MQTT cmd sent to {}", parts[0]))
        } else {
            Err(anyhow::anyhow!("Format: device_id command [json_params]"))
        }
    }
}

/// ---------- LLM Router — умная маршрутизация и кэширование запросов ----------
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
struct CachedResponse {
    response: String,
    expires_at: u64,
    model: String,
    hit_count: u64,
}

pub struct LlmRouter {
    cache: Mutex<HashMap<String, CachedResponse>>,
    kvstore: Option<std::sync::Arc<crate::store::KvStore>>,
    cache_ttl_secs: u64,
    simple_prefixes: Vec<String>, // запросы, которые можно отдать Ollama
}

impl LlmRouter {
    pub fn new(kvstore: Option<std::sync::Arc<crate::store::KvStore>>) -> Self {
        LlmRouter {
            cache: Mutex::new(HashMap::new()),
            kvstore,
            cache_ttl_secs: 3600, // 1 час вместо 60 секунд
            simple_prefixes: vec![
                "статус".into(),
                "status".into(),
                "помощь".into(),
                "help".into(),
                "skills".into(),
                "список".into(),
                "list".into(),
                "справка".into(),
            ],
        }
    }

    /// Выбрать провайдера для запроса
    pub fn select_provider(&self, input: &str) -> &str {
        let lower = input.to_lowercase();
        for prefix in &self.simple_prefixes {
            if lower.starts_with(prefix) {
                return "ollama"; // простые запросы → локальный Ollama (бесплатно)
            }
        }
        "deepseek" // сложные запросы → DeepSeek
    }

    /// Получить кэшированный ответ
    pub fn get_cached(&self, input: &str) -> Option<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cache = self.cache.lock().unwrap();
        if let Some(cached) = cache.get(input) {
            if now < cached.expires_at {
                return Some(cached.response.clone());
            }
        }
        // Fallback: Redis кэш
        if let Some(ref kv) = self.kvstore {
            let key = format!("llm:response:{}", input.len());
            if let Ok(Some(val)) = kv.get(&key) {
                return Some(val);
            }
        }
        None
    }

    /// Сохранить в кэш
    pub fn cache_response(&self, input: &str, response: &str, model: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut cache = self.cache.lock().unwrap();
        let entry = cache.entry(input.to_string()).or_insert(CachedResponse {
            response: response.to_string(),
            expires_at: now + self.cache_ttl_secs,
            model: model.to_string(),
            hit_count: 0,
        });
        entry.response = response.to_string();
        entry.expires_at = now + self.cache_ttl_secs;
        entry.hit_count += 1;

        // Сохраняем в Redis для других нод
        if let Some(ref kv) = self.kvstore {
            let key = format!("llm:response:{}", input.len());
            let _ = kv.set(&key, response, self.cache_ttl_secs);
        }
    }

    /// Статистика кэша
    pub fn stats(&self) -> String {
        let cache = self.cache.lock().unwrap();
        format!("🧠 LLM Router: {} cached responses (TTL: {}s)\n  Simple → Ollama (free), Complex → DeepSeek\n  Всего запросов в кэше: {}",
            cache.len(), self.cache_ttl_secs,
            cache.values().map(|c| c.hit_count).sum::<u64>())
    }
}
