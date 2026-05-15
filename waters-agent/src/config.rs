use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    #[serde(default)]
    pub kafka: Option<KafkaConfig>,
    #[serde(default)]
    pub ollama: Option<OllamaConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub mission_id: String,
    pub llm_provider: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KafkaConfig {
    pub brokers: String,
    pub group_id: String,
    pub topics: KafkaTopics,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KafkaTopics {
    pub orders: String,
    pub findings: String,
    pub agents: String,
    pub ranks: String,
    pub heartbeat: String,
    pub config: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_url")]
    pub url: String,
    #[serde(default = "default_ollama_model")]
    pub model: String,
}

fn default_ollama_url() -> String {
    "http://127.0.0.1:11434".into()
}

fn default_ollama_model() -> String {
    "qwen2.5:14b".into()
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Config {
            agent: AgentConfig {
                id: "agent.constructor.v1".into(),
                mission_id: "mission-1".into(),
                llm_provider: "ollama".into(),
            },
            kafka: None,
            ollama: Some(OllamaConfig {
                url: default_ollama_url(),
                model: default_ollama_model(),
            }),
        }
    }
}
