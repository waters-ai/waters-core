use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub node: NodeConfig,
    #[serde(default)]
    pub redis: Option<RedisConfig>,
    #[serde(default)]
    pub ollama: Option<OllamaConfig>,
    #[serde(default)]
    pub kafka: Option<KafkaConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default = "default_workspace")]
    pub workspace: String,
    #[serde(default = "default_session_dir")]
    pub session_dir: String,
    #[serde(default = "default_llm_provider")]
    pub llm_provider: String,
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

fn default_name() -> String { "waters-node".into() }
fn default_workspace() -> String { ".".into() }
fn default_session_dir() -> String { ".waters/sessions".into() }
fn default_llm_provider() -> String { "ollama".into() }
fn default_ollama_url() -> String { "http://127.0.0.1:11434".into() }
fn default_ollama_model() -> String { "qwen2.5:14b".into() }
fn default_topic() -> String { "mission.1.orders.v1".into() }
fn default_findings_topic() -> String { "mission.1.findings.v1".into() }
fn default_heartbeat_topic() -> String { "mission.1.heartbeat.v1".into() }
fn default_agents_topic() -> String { "mission.1.agents.v1".into() }

impl Config {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn default() -> Self {
        Config {
            agent: AgentConfig {
                id: "agent.constructor.v1".into(),
                mission_id: "mission-1".into(),
                llm_provider: "ollama".into(),
            },
            kafka: None,
            redis: None,
            ollama: Some(OllamaConfig {
                url: default_ollama_url(),
                model: default_ollama_model(),
            }),
        }
    }
    }
}
