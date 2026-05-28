use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub node: NodeConfig,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
    #[serde(default)]
    pub redis: Option<RedisConfig>,
    #[serde(default)]
    pub ollama: Option<OllamaConfig>,
    #[serde(default)]
    pub kafka: Option<KafkaConfig>,
    #[serde(default)]
    pub edge: Option<EdgeConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EdgeConfig {
    #[serde(default = "default_edge_model_path")]
    pub model_path: String,
    #[serde(default = "default_edge_n_threads")]
    pub n_threads: usize,
    #[serde(default = "default_edge_n_ctx")]
    pub n_ctx: usize,
}

fn default_edge_model_path() -> String {
    "/models/llama-2-7b-chat.gguf".into()
}

fn default_edge_n_threads() -> usize {
    4
}

fn default_edge_n_ctx() -> usize {
    2048
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default = "default_workspace")]
    pub workspace: String,
    #[serde(default = "default_session_dir")]
    pub session_dir: String,
    #[serde(default = "default_llm_provider")]
    pub llm_provider: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        NodeConfig {
            name: default_name(),
            id: None,
            profile: default_profile(),
            workspace: default_workspace(),
            session_dir: default_session_dir(),
            llm_provider: default_llm_provider(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            node: NodeConfig::default(),
            profiles: std::collections::HashMap::new(),
            redis: None,
            ollama: None,
            kafka: None,
            edge: None,
        }
    }
}

/// Per-profile config — группы, бриджи, LLM, автономия
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProfileConfig {
    #[serde(default)]
    pub llm_provider: String,
    #[serde(default)]
    pub llm_model: String,
    #[serde(default)]
    pub llm_url: String,
    #[serde(default)]
    pub bridges: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub autonomy_level: u8,
    #[serde(default)]
    pub dtn_profile: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_url")]
    pub url: String,
    #[serde(default = "default_ollama_model")]
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KafkaConfig {
    pub brokers: String,
    pub group_id: String,
    pub topics: KafkaTopics,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KafkaTopics {
    #[serde(default = "default_topic")]
    pub orders: String,
    #[serde(default = "default_findings_topic")]
    pub findings: String,
    #[serde(default = "default_heartbeat_topic")]
    pub heartbeat: String,
    #[serde(default = "default_agents_topic")]
    pub agents: String,
}

fn default_profile() -> String {
    "default".into()
}

fn default_name() -> String {
    // Человекочитаемое имя по умолчанию
    let hostname = std::fs::read_to_string("/etc/hostname").unwrap_or_default();
    if !hostname.trim().is_empty() {
        let clean: String = hostname
            .chars()
            .filter(|c| c.is_alphanumeric())
            .take(8)
            .collect();
        return crate::tunnel::suggest_node_name(&clean);
    }
    crate::tunnel::suggest_node_name("waters-node")
}

fn default_workspace() -> String {
    ".".into()
}

fn default_session_dir() -> String {
    ".waters/sessions".into()
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".into()
}

fn default_llm_provider() -> String {
    "ollama".into()
}

fn default_ollama_url() -> String {
    "http://127.0.0.1:11434".into()
}

fn default_ollama_model() -> String {
    "qwen2.5:14b".into()
}

fn default_topic() -> String {
    "mission.1.orders.v1".into()
}

fn default_findings_topic() -> String {
    "mission.1.findings.v1".into()
}

fn default_heartbeat_topic() -> String {
    "mission.1.heartbeat.v1".into()
}

fn default_agents_topic() -> String {
    "mission.1.agents.v1".into()
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
