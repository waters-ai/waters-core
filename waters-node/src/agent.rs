use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub role: String,
    pub agent_type: String,
    pub owner_node: String,
    pub personal_resources: Vec<String>,
    pub active_skill: Option<String>,
    pub status: String,
}

pub struct AgentManager {
    pub mine: HashMap<String, Agent>,
    pub from_peers: HashMap<String, Agent>,
}

impl AgentManager {
    pub fn new() -> Self {
        AgentManager {
            mine: HashMap::new(),
            from_peers: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: &str, role: &str, agent_type: &str, owner: &str) {
        let agent = Agent {
            name: name.to_string(),
            role: role.to_string(),
            agent_type: agent_type.to_string(),
            owner_node: owner.to_string(),
            personal_resources: Vec::new(),
            active_skill: None,
            status: "idle".into(),
        };
        self.mine.insert(name.to_string(), agent);
        info!("Agent '{}' added (type: {}, role: {})", name, agent_type, role);
    }

    pub fn add_peer_agent(&mut self, name: &str, role: &str, agent_type: &str, peer_node: &str) {
        let agent = Agent {
            name: name.to_string(),
            role: role.to_string(),
            agent_type: agent_type.to_string(),
            owner_node: peer_node.to_string(),
            personal_resources: Vec::new(),
            active_skill: None,
            status: "idle".into(),
        };
        self.from_peers.insert(format!("{}@{}", name, peer_node), agent);
    }

    pub fn list_mine(&self) -> Vec<&Agent> {
        self.mine.values().collect()
    }

    pub fn list_shared_mine(&self) -> Vec<&Agent> {
        self.mine.values().filter(|a| a.agent_type == "shared").collect()
    }

    pub fn list_from_peers(&self) -> Vec<&Agent> {
        self.from_peers.values().collect()
    }

    pub fn list_all_available(&self) -> Vec<&Agent> {
        let mut all: Vec<&Agent> = self.mine.values().collect();
        for a in self.from_peers.values() {
            if a.agent_type == "shared" {
                all.push(a);
            }
        }
        all
    }

    pub fn get_mine(&self, name: &str) -> Option<&Agent> {
        self.mine.get(name)
    }

    pub fn get_available(&self, name: &str) -> Option<&Agent> {
        if let Some(a) = self.mine.get(name) {
            return Some(a);
        }
        self.from_peers.values().find(|a| a.name == name)
    }
}
