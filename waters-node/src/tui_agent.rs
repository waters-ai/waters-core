use serde::{Deserialize, Serialize};

use crate::cargo::OnboardLlm;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiAgent {
    pub name: String,
    pub source: String,
    pub native_skill: TuiSkillWrapper,
    pub json_capable: bool,
    pub onboard_llm: Option<OnboardLlm>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiSkillWrapper {
    pub tui_name: String,
    pub description: String,
    pub bridges: Vec<String>,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentJsonMessage {
    pub agent: String,
    pub version: String,
    pub msg_type: String,
    pub payload: serde_json::Value,
    pub confidence: Option<f64>,
    pub timestamp: String,
}

impl TuiAgent {
    pub fn new(tui_name: &str, description: &str, bridges: &[String], onboard: Option<OnboardLlm>) -> Self {
        TuiAgent {
            name: format!("tui-{}", tui_name),
            source: "tui".into(),
            native_skill: TuiSkillWrapper {
                tui_name: tui_name.to_string(),
                description: description.to_string(),
                bridges: bridges.to_vec(),
                prompt: format!("You are a TUI-converted agent '{}'. {}", tui_name, description),
            },
            json_capable: true,
            onboard_llm: onboard,
        }
    }

    pub fn to_json_message(&self, msg_type: &str, payload: serde_json::Value, confidence: Option<f64>) -> AgentJsonMessage {
        AgentJsonMessage {
            agent: self.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            msg_type: msg_type.to_string(),
            payload,
            confidence,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn to_agent_entry(&self) -> crate::agent::Agent {
        crate::agent::Agent {
            name: self.name.clone(),
            role: format!("tui-{}", self.native_skill.tui_name),
            agent_type: "tui_converted".into(),
            owner_node: "local".into(),
            personal_resources: self.native_skill.bridges.clone(),
            active_skill: Some(format!("tui-{}", self.native_skill.tui_name)),
            status: "idle".into(),
        }
    }
}

pub fn convert_tui_to_node(tui_name: &str, description: &str, bridges: &[String], onboard: Option<OnboardLlm>) -> (TuiAgent, crate::agent::Agent) {
    let agent = TuiAgent::new(tui_name, description, bridges, onboard);
    let node_agent = agent.to_agent_entry();
    (agent, node_agent)
}

/// 6 агентов (1 ассистент + 5 специалистов), каждый со своим бортовым LLM
pub fn builtin_tui_agents() -> Vec<TuiAgent> {
    vec![
        TuiAgent::new(
            "assistant",
            "Node setup assistant — conversational, helps manage tasks, agents, groups, bridges, settings",
            &["chat".into()],
            Some(OnboardLlm { model: "qwen2.5:1.5b".into(), quant: "Q4_K_M".into(), ctx_size: 4096, size_mb: 980 }),
        ),
        TuiAgent::new(
            "scout-us",
            "US/global search via DuckDuckGo",
            &["duckduckgo".into()],
            Some(OnboardLlm { model: "qwen2.5:1.5b".into(), quant: "Q4_K_M".into(), ctx_size: 4096, size_mb: 980 }),
        ),
        TuiAgent::new(
            "explorer",
            "General exploration and data collection, onboard LLM for field work",
            &["duckduckgo".into(), "mcp-nasa".into()],
            Some(OnboardLlm { model: "qwen2.5:1.5b".into(), quant: "Q4_K_M".into(), ctx_size: 4096, size_mb: 980 }),
        ),
        TuiAgent::new(
            "analyst",
            "Data analysis and pattern recognition, deep reasoning onboard",
            &[] as &[String],
            Some(OnboardLlm { model: "qwen2.5:3b".into(), quant: "Q4_K_M".into(), ctx_size: 8192, size_mb: 1800 }),
        ),
        TuiAgent::new(
            "geologist",
            "Geological analysis of celestial bodies, spectral data processing",
            &[] as &[String],
            Some(OnboardLlm { model: "gemma-2b".into(), quant: "Q4_K_M".into(), ctx_size: 4096, size_mb: 1200 }),
        ),
        TuiAgent::new(
            "cartographer",
            "Mapping, trajectory calculation, spatial reasoning",
            &["mcp-trajectory".into()],
            Some(OnboardLlm { model: "qwen2.5:1.5b".into(), quant: "Q4_K_M".into(), ctx_size: 4096, size_mb: 980 }),
        ),
    ]
}
