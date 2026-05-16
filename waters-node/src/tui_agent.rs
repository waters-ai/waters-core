use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::bridge::{BridgePool, BridgeProvider};
use crate::cargo::OnboardLlm;
use crate::store::KvStore;

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
    pub fn new(
        tui_name: &str,
        description: &str,
        bridges: &[String],
        onboard: Option<OnboardLlm>,
    ) -> Self {
        TuiAgent {
            name: format!("tui-{}", tui_name),
            source: "tui".into(),
            native_skill: TuiSkillWrapper {
                tui_name: tui_name.to_string(),
                description: description.to_string(),
                bridges: bridges.to_vec(),
                prompt: format!(
                    "You are a TUI-converted agent '{}'. {}",
                    tui_name, description
                ),
            },
            json_capable: true,
            onboard_llm: onboard,
        }
    }

    pub fn to_json_message(
        &self,
        msg_type: &str,
        payload: serde_json::Value,
        confidence: Option<f64>,
    ) -> AgentJsonMessage {
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

pub fn convert_tui_to_node(
    tui_name: &str,
    description: &str,
    bridges: &[String],
    onboard: Option<OnboardLlm>,
) -> (TuiAgent, crate::agent::Agent) {
    let agent = TuiAgent::new(tui_name, description, bridges, onboard);
    let node_agent = agent.to_agent_entry();
    (agent, node_agent)
}

fn assistant_stream_redis(
    bridge_pool: &BridgePool,
    kvstore: &KvStore,
    group_id: u8,
    input: &str,
    session_id: &str,
) -> Result<String> {
    let mut llm_bridges: Vec<(u8, String)> = bridge_pool
        .list()
        .iter()
        .filter(|n| n.starts_with("llm-"))
        .filter_map(|n| {
            let prio = bridge_pool.info.get(n).map(|i| i.priority).unwrap_or(5);
            if bridge_pool.info.get(n).map(|i| i.enabled).unwrap_or(true) {
                Some((prio, n.clone()))
            } else {
                None
            }
        })
        .collect();
    llm_bridges.sort_by_key(|(p, _)| *p);

    let stream_channel = format!("channel:stream:{}", session_id);
    let stream_key = format!("stream:tokens:{}", session_id);

    for (_, name) in &llm_bridges {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let input2 = input.to_string();
        let bridge_name = name.clone();

        if let Some(bridge) = bridge_pool.get(&bridge_name) {
            bridge.call_stream(&input2, &tx).ok();
        }

        let mut full = String::new();
        let mut reasoning = String::new();
        let mut tool_ev = String::new();

        for token in rx {
            if token == "__done__" {
                break;
            }

            let (event_type, display_text) = if let Some(r) = token.strip_prefix("__reasoning__") {
                reasoning.push_str(r);
                ("reasoning", r.to_string())
            } else if token.starts_with("__tool_call__") {
                tool_ev = token[13..].to_string();
                ("tool_call", token[13..].to_string())
            } else {
                full.push_str(&token);
                ("token", token.clone())
            };

            let payload = serde_json::json!({
                "type": event_type,
                "content": display_text,
                "ts": chrono::Utc::now().to_rfc3339(),
            });

            if kvstore.is_connected() {
                let _ = kvstore
                    .group_db(group_id)
                    .publish(&stream_channel, &payload.to_string());
                let _ = kvstore.xadd(
                    &stream_key,
                    &[("type", event_type), ("content", &display_text)],
                    10000,
                );
            }
        }

        if !full.is_empty() {
            let done_msg =
                serde_json::json!({"type": "done", "ts": chrono::Utc::now().to_rfc3339()});
            let _ = kvstore.publish(&stream_channel, &done_msg.to_string());
            return Ok(full);
        }
    }

    for (_, name) in &llm_bridges {
        match bridge_pool.call(name, input) {
            Ok(r) => {
                return Ok(r);
            }
            Err(_) => continue,
        }
    }

    if let Some(bridge) = bridge_pool.get("chat") {
        bridge
            .call(input)
            .or_else(|_| Ok("Assistant ready.".into()))
    } else {
        Ok("Assistant ready.".into())
    }
}

pub fn assistant_chat(
    bridge_pool: &BridgePool,
    kvstore: &KvStore,
    group_id: u8,
    input: &str,
    session_id: &str,
) -> Result<String> {
    let result = assistant_stream_redis(bridge_pool, kvstore, group_id, input, session_id)?;
    if kvstore.is_connected() {
        let _ = kvstore.hset("session:log", session_id, &result);
    }
    Ok(result)
}

pub fn builtin_tui_agents() -> Vec<TuiAgent> {
    vec![
        TuiAgent::new(
            "assistant",
            "Node setup assistant — conversational, helps manage tasks, agents, groups, bridges, settings. Switches to node LLM when available.",
            &["chat".into()],
            Some(OnboardLlm { model: "qwen2.5:0.5b".into(), quant: "Q4_K_M".into(), ctx_size: 2048, size_mb: 350 }),
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
