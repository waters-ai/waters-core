use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiAgent {
    pub name: String,
    pub source: String,
    pub native_skill: TuiSkillWrapper,
    pub json_capable: bool,
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
    pub fn new(tui_name: &str, description: &str, bridges: &[String]) -> Self {
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

pub fn convert_tui_to_node(tui_name: &str, description: &str, bridges: &[String]) -> (TuiAgent, crate::agent::Agent) {
    let agent = TuiAgent::new(tui_name, description, bridges);
    let node_agent = agent.to_agent_entry();
    (agent, node_agent)
}

pub fn builtin_tui_agents() -> Vec<TuiAgent> {
    vec![
        TuiAgent::new("scout-us", "US/global search via DuckDuckGo", &["duckduckgo".into()]),
        TuiAgent::new("scout-ru", "Russian search via Yandex", &["yandex.search".into()]),
        TuiAgent::new("scout-cn", "Chinese search via Baidu", &["baidu.search".into()]),
        TuiAgent::new("explorer", "General exploration and data collection", &["duckduckgo".into()]),
        TuiAgent::new("analyst", "Data analysis and pattern recognition", &[] as &[String]),
        TuiAgent::new("writer", "Content writing and summarization", &[] as &[String]),
        TuiAgent::new("astronomer", "Astronomical observation and meteor tracking", &["mcp-nasa".into()]),
        TuiAgent::new("geologist", "Geological analysis of celestial bodies", &[] as &[String]),
        TuiAgent::new("cartographer", "Mapping and trajectory calculation", &["mcp-trajectory".into()]),
        TuiAgent::new("inspector", "Quality assurance and validation", &[] as &[String]),
    ]
}
