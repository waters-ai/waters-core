use std::collections::HashMap;
use std::fmt::Debug;
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
            name: name.to_string(), weight, priority, bandwidth_kbps,
            enabled: true, locked: false, reason: String::new(),
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
            rtt_ms: 0, packet_loss_pct: 0.0,
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
    pub fn new() -> Self { LinkGovernor { links: HashMap::new(), disabled: Vec::new() } }

    pub fn add_link(&mut self, profile: LinkProfile) {
        self.links.insert(profile.name.clone(), profile);
    }

    /// Проверить какой bandwidth доступен, какие бриджи отключить.
    /// Возвращает (включено, отключено) — список имён.
    pub fn govern(&mut self, bridge_info: &HashMap<String, BridgeInfo>, link_name: &str) -> (Vec<String>, Vec<String>) {
        let profile = match self.links.get(link_name) {
            Some(p) => p,
            None => return (bridge_info.keys().cloned().collect(), Vec::new()),
        };
        let available = profile.measured_bandwidth_kbps;
        if available == 0 { return (Vec::new(), bridge_info.keys().cloned().collect()); }

        let mut active: Vec<(String, u8, u64, bool)> = bridge_info.values()
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
            if *locked { continue; }
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

    pub fn status_message(&self, bridge_info: &HashMap<String, BridgeInfo>, link_name: &str) -> String {
        let profile = match self.links.get(link_name) {
            Some(p) => p,
            None => return "No link configured.".into(),
        };
        let mut msg = format!("  Link: {}\n    Bandwidth: {}/{} Kbps  RTT: {}ms\n",
            link_name, profile.measured_bandwidth_kbps, profile.max_bandwidth_kbps, profile.rtt_ms);

        let active: Vec<_> = bridge_info.values().filter(|b| b.enabled && !self.disabled.contains(&b.name)).collect();
        let off: Vec<_> = bridge_info.values().filter(|b| self.disabled.contains(&b.name)).collect();

        if !active.is_empty() {
            msg.push_str("    ✅ Active:\n");
            for b in &active {
                msg.push_str(&format!("        {} ({} Kbps, priority {})\n", b.name, b.bandwidth_kbps, b.priority));
            }
        }
        if !off.is_empty() {
            msg.push_str("    ⚠️  Offloaded (bandwidth insufficient):\n");
            for b in &off {
                msg.push_str(&format!("        {} ({} Kbps, priority {}) — needs {} total\n",
                    b.name, b.bandwidth_kbps, b.priority, profile.measured_bandwidth_kbps + b.bandwidth_kbps));
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
                        let bw = self.links.get(name).map(|l| l.measured_bandwidth_kbps).unwrap_or(0);
                        changes.push(format!("⚠️  {} отключён (link: {}, нужно {} Kbps, доступно {})",
                            bname, name, ii.bandwidth_kbps, bw));
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

fn default_weight() -> String { "light".into() }
fn default_bandwidth() -> u64 { 100 }
fn default_priority() -> u8 { 3 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridgesFile {
    #[serde(default)]
    pub bridges: Vec<BridgeConfig>,
    #[serde(default)]
    pub llm: LlmBridgeConfig,
    #[serde(default)]
    pub chat: ChatBridgeConfig,
    #[serde(default)]
    pub voice: Option<VoiceBridgeConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub links: Vec<LinkProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBridgeConfig {
    pub provider: String, pub model: String, pub url: String,
    pub api_key: String, pub system_prompt: String,
}
impl Default for LlmBridgeConfig {
    fn default() -> Self { LlmBridgeConfig {
        provider: "ollama".into(), model: "qwen2.5:14b".into(),
        url: "http://127.0.0.1:11434".into(), api_key: String::new(),
        system_prompt: "You are a helpful WATERS node assistant.".into(),
    }}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatBridgeConfig { pub transport: String, pub token: String }
impl Default for ChatBridgeConfig {
    fn default() -> Self { ChatBridgeConfig { transport: "stdin".into(), token: String::new() }}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceBridgeConfig { pub stt_model: String, pub tts_model: String, pub url: String }

/// ---------- BridgePool ----------

#[derive(Debug)]
pub struct BridgePool {
    pub bridges: HashMap<String, Box<dyn BridgeProvider>>,
    pub info: HashMap<String, BridgeInfo>,
    pub governor: LinkGovernor,
}

impl BridgePool {
    pub fn new() -> Self {
        BridgePool { bridges: HashMap::new(), info: HashMap::new(), governor: LinkGovernor::new() }
    }

    pub fn register(&mut self, name: &str, bridge: Box<dyn BridgeProvider>, meta: BridgeInfo) {
        self.bridges.insert(name.to_string(), bridge);
        self.info.insert(name.to_string(), meta);
        info!("Bridge registered: {}", name);
    }

    pub fn call(&self, name: &str, input: &str) -> Result<String> {
        // Check if bridge is disabled by governor
        if let Some(m) = self.info.get(name) {
            if !m.enabled {
                return Err(anyhow::anyhow!("Bridge '{}' is disabled: {}", name, m.reason));
            }
        }
        self.bridges.get(name)
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", name))
            .and_then(|b| b.call(input))
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
            let reason = self.info.get(&name).map(|i| i.reason.clone()).unwrap_or_default();
            result.push((name, enabled, reason));
        }
        result
    }

    pub fn set_priority(&mut self, name: &str, priority: u8) -> bool {
        if let Some(ii) = self.info.get_mut(name) {
            ii.priority = priority.clamp(1, 5);
            info!("Bridge '{}' priority set to {}", name, ii.priority);
            true
        } else { false }
    }

    pub fn lock(&mut self, name: &str) -> bool {
        if let Some(ii) = self.info.get_mut(name) {
            ii.locked = true;
            info!("Bridge '{}' locked (never offloaded)", name);
            true
        } else { false }
    }

    pub fn unlock(&mut self, name: &str) -> bool {
        if let Some(ii) = self.info.get_mut(name) {
            ii.locked = false;
            info!("Bridge '{}' unlocked", name);
            true
        } else { false }
    }

    pub fn load_config(path: &std::path::Path) -> BridgesFile {
        if path.exists() {
            std::fs::read_to_string(path)
                .ok().and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else { BridgesFile::default() }
    }
}

/// ---------- LLM Bridge ----------

#[derive(Debug)]
pub struct LlmBridge { name: String, provider: LlmProvider, system_prompt: String }
#[derive(Debug)]
enum LlmProvider {
    DeepSeek { api_key: String, model: String },
    Ollama { url: String, model: String },
    OpenAI { url: String, model: String, api_key: String },
}

impl LlmBridge {
    pub fn new(name: &str, cfg: &LlmBridgeConfig) -> Self {
        let provider = match cfg.provider.as_str() {
            "deepseek" => LlmProvider::DeepSeek { api_key: cfg.api_key.clone(), model: cfg.model.clone() },
            "openai" => LlmProvider::OpenAI { url: cfg.url.clone(), model: cfg.model.clone(), api_key: cfg.api_key.clone() },
            _ => LlmProvider::Ollama { url: cfg.url.clone(), model: cfg.model.clone() },
        };
        LlmBridge { name: name.to_string(), provider, system_prompt: cfg.system_prompt.clone() }
    }
}

impl BridgeProvider for LlmBridge {
    fn name(&self) -> &str { &self.name }
    fn call(&self, input: &str) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        match &self.provider {
            LlmProvider::DeepSeek { api_key, model } => {
                let body = serde_json::json!({"model": model, "messages": [
                    {"role": "system", "content": &self.system_prompt},
                    {"role": "user", "content": input}
                ], "stream": false});
                let resp = client.post("https://api.deepseek.com/beta/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key)).json(&body).send()?;
                Ok(resp.json::<serde_json::Value>()?["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
            }
            LlmProvider::Ollama { url, model } => {
                let body = serde_json::json!({"model": model, "system": &self.system_prompt, "prompt": input, "stream": false});
                let resp = client.post(format!("{}/api/generate", url)).json(&body).send()?;
                Ok(resp.json::<serde_json::Value>()?["response"].as_str().unwrap_or("").to_string())
            }
            LlmProvider::OpenAI { url, model, api_key } => {
                let body = serde_json::json!({"model": model, "messages": [
                    {"role": "system", "content": &self.system_prompt},
                    {"role": "user", "content": input}
                ]});
                let mut req = client.post(format!("{}/v1/chat/completions", url));
                if !api_key.is_empty() { req = req.header("Authorization", format!("Bearer {}", api_key)); }
                let resp = req.json(&body).send()?;
                Ok(resp.json::<serde_json::Value>()?["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
            }
        }
    }
}

/// ---------- Chat Bridge ----------

#[derive(Debug)]
pub struct ChatBridge { name: String, transport: ChatTransport }
#[derive(Debug)]
enum ChatTransport { Stdin, Telegram { token: String, chat_id: Option<String> } }

impl ChatBridge {
    pub fn new_stdin(name: &str) -> Self { ChatBridge { name: name.to_string(), transport: ChatTransport::Stdin } }
    pub fn new_telegram(name: &str, token: &str) -> Self {
        ChatBridge { name: name.to_string(), transport: ChatTransport::Telegram { token: token.to_string(), chat_id: None } }
    }
}

impl BridgeProvider for ChatBridge {
    fn name(&self) -> &str { &self.name }
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
                    .json(&body).send()?;
                Ok(serde_json::to_string(&resp.json::<serde_json::Value>()?)?)
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
    Stt,  // speech-to-text (Whisper)
    Tts,  // text-to-speech
}

impl VoiceBridge {
    pub fn new_stt(name: &str, url: &str) -> Self {
        VoiceBridge { name: name.to_string(), mode: VoiceMode::Stt, url: url.to_string() }
    }
    pub fn new_tts(name: &str, url: &str) -> Self {
        VoiceBridge { name: name.to_string(), mode: VoiceMode::Tts, url: url.to_string() }
    }
}

impl BridgeProvider for VoiceBridge {
    fn name(&self) -> &str { &self.name }
    fn call(&self, input: &str) -> Result<String> {
        match self.mode {
            VoiceMode::Stt => {
                let body = serde_json::json!({"audio": input, "model": "whisper-1"});
                let resp = reqwest::blocking::Client::new()
                    .post(format!("{}/v1/audio/transcriptions", self.url))
                    .json(&body).send()?;
                Ok(resp.json::<serde_json::Value>()?["text"].as_str().unwrap_or("").to_string())
            }
            VoiceMode::Tts => {
                let body = serde_json::json!({"text": input, "model": "tts-1"});
                let resp = reqwest::blocking::Client::new()
                    .post(format!("{}/v1/audio/speech", self.url))
                    .json(&body).send()?;
                // Return base64 audio
                Ok(resp.bytes()?.iter().map(|b| format!("{:02x}", b)).collect::<String>())
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
    pub fn new(name: &str, server: &str, tool: &str, mcp_client: Arc<Mutex<crate::mcp::McpClient>>) -> Self {
        McpBridge { name: name.to_string(), server: server.to_string(), tool: tool.to_string(), mcp_client }
    }
}

impl BridgeProvider for McpBridge {
    fn name(&self) -> &str { &self.name }
    fn call(&self, input: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(input)
            .unwrap_or_else(|_| serde_json::json!({"query": input}));
        let client = self.mcp_client.lock().map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;
        let result = client.call_tool(&self.server, &self.tool, &args)?;
        Ok(serde_json::to_string(&result)?)
    }
    fn call_json(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
        let client = self.mcp_client.lock().map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;
        client.call_tool(&self.server, &self.tool, input)
    }
}
