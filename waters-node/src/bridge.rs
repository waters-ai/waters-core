use std::collections::HashMap;
use std::fmt::Debug;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

pub trait BridgeProvider: Debug + Send + Sync {
    fn name(&self) -> &str;
    fn call(&self, input: &str) -> Result<String>;
    fn call_json(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
        let text = self.call(&serde_json::to_string(input)?)?;
        Ok(serde_json::json!({"response": text}))
    }
}

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
pub struct BridgesFile {
    #[serde(default)]
    pub bridges: Vec<BridgeConfig>,
    #[serde(default)]
    pub llm: LlmBridgeConfig,
    #[serde(default)]
    pub chat: ChatBridgeConfig,
    #[serde(default)]
    pub voice: Option<VoiceBridgeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBridgeConfig {
    pub provider: String,
    pub model: String,
    pub url: String,
    pub api_key: String,
    pub system_prompt: String,
}

impl Default for LlmBridgeConfig {
    fn default() -> Self {
        LlmBridgeConfig {
            provider: "ollama".into(),
            model: "qwen2.5:14b".into(),
            url: "http://127.0.0.1:11434".into(),
            api_key: String::new(),
            system_prompt: "You are a helpful WATERS node assistant.".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatBridgeConfig {
    pub transport: String,
    pub token: String,
}

impl Default for ChatBridgeConfig {
    fn default() -> Self {
        ChatBridgeConfig {
            transport: "stdin".into(),
            token: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceBridgeConfig {
    pub stt_model: String,
    pub tts_model: String,
    pub url: String,
}

pub struct BridgePool {
    pub bridges: HashMap<String, Box<dyn BridgeProvider>>,
}

impl Debug for BridgePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgePool")
            .field("bridges", &self.bridges.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl BridgePool {
    pub fn new() -> Self {
        BridgePool {
            bridges: HashMap::new(),
        }
    }

    pub fn register(&mut self, bridge: Box<dyn BridgeProvider>) {
        let name = bridge.name().to_string();
        info!("Bridge registered: {}", name);
        self.bridges.insert(name, bridge);
    }

    pub fn call(&self, name: &str, input: &str) -> Result<String> {
        self.bridges.get(name)
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", name))
            .and_then(|b| b.call(input))
    }

    pub fn call_json(&self, name: &str, input: &serde_json::Value) -> Result<serde_json::Value> {
        self.bridges.get(name)
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", name))
            .and_then(|b| b.call_json(input))
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn BridgeProvider>> {
        self.bridges.get(name)
    }

    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self.bridges.keys().cloned().collect();
        names.sort();
        names
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

/// LLM as a bridge
#[derive(Debug)]
pub struct LlmBridge {
    name: String,
    provider: LlmProvider,
    system_prompt: String,
}

#[derive(Debug)]
enum LlmProvider {
    DeepSeek { api_key: String, model: String },
    Ollama { url: String, model: String },
    OpenAI { url: String, model: String, api_key: String },
}

impl LlmBridge {
    pub fn new(name: &str, cfg: &LlmBridgeConfig) -> Self {
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
            name: name.to_string(),
            provider,
            system_prompt: cfg.system_prompt.clone(),
        }
    }
}

impl BridgeProvider for LlmBridge {
    fn name(&self) -> &str { &self.name }

    fn call(&self, input: &str) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        match &self.provider {
            LlmProvider::DeepSeek { api_key, model } => {
                let body = serde_json::json!({
                    "model": model,
                    "messages": [
                        {"role": "system", "content": &self.system_prompt},
                        {"role": "user", "content": input}
                    ],
                    "stream": false
                });
                let resp = client
                    .post("https://api.deepseek.com/beta/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&body)
                    .send()?;
                let result: serde_json::Value = resp.json()?;
                Ok(result["choices"][0]["message"]["content"]
                    .as_str().unwrap_or("").to_string())
            }
            LlmProvider::Ollama { url, model } => {
                let body = serde_json::json!({
                    "model": model,
                    "system": &self.system_prompt,
                    "prompt": input,
                    "stream": false
                });
                let resp = client
                    .post(format!("{}/api/generate", url))
                    .json(&body)
                    .send()?;
                let result: serde_json::Value = resp.json()?;
                Ok(result["response"].as_str().unwrap_or("").to_string())
            }
            LlmProvider::OpenAI { url, model, api_key } => {
                let body = serde_json::json!({
                    "model": model,
                    "messages": [
                        {"role": "system", "content": &self.system_prompt},
                        {"role": "user", "content": input}
                    ]
                });
                let mut req = client
                    .post(format!("{}/v1/chat/completions", url));
                if !api_key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {}", api_key));
                }
                let resp = req.json(&body).send()?;
                let result: serde_json::Value = resp.json()?;
                Ok(result["choices"][0]["message"]["content"]
                    .as_str().unwrap_or("").to_string())
            }
        }
    }
}

/// Chat as a bridge (stdin / telegram)
#[derive(Debug)]
pub struct ChatBridge {
    name: String,
    transport: ChatTransport,
}

#[derive(Debug)]
enum ChatTransport {
    Stdin,
    Telegram { token: String, chat_id: Option<String> },
}

impl ChatBridge {
    pub fn new_stdin(name: &str) -> Self {
        ChatBridge { name: name.to_string(), transport: ChatTransport::Stdin }
    }

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
            ChatTransport::Telegram { token, chat_id: _ } => {
                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                let body = serde_json::json!({
                    "chat_id": "@waters_node",
                    "text": input,
                    "parse_mode": "Markdown"
                });
                let client = reqwest::blocking::Client::new();
                let resp = client.post(&url).json(&body).send()?;
                let result: serde_json::Value = resp.json()?;
                Ok(serde_json::to_string(&result)?)
            }
        }
    }
}

/// Voice bridge (Whisper STT stub)
#[derive(Debug)]
pub struct VoiceBridge {
    name: String,
    url: String,
}

impl VoiceBridge {
    pub fn new(name: &str, url: &str) -> Self {
        VoiceBridge { name: name.to_string(), url: url.to_string() }
    }
}

impl BridgeProvider for VoiceBridge {
    fn name(&self) -> &str { &self.name }

    fn call(&self, input: &str) -> Result<String> {
        let body = serde_json::json!({"audio": input, "model": "whisper-1"});
        let resp = reqwest::blocking::Client::new()
            .post(format!("{}/v1/audio/transcriptions", self.url))
            .json(&body)
            .send()?;
        let result: serde_json::Value = resp.json()?;
        Ok(result["text"].as_str().unwrap_or("").to_string())
    }
}
