use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShareScope {
    Personal,
    Group(String),
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePolicy {
    pub resource_type: String,
    pub resource_id: String,
    pub scope: ShareScope,
    pub max_peers: u8,
    pub require_approval: bool,
    pub audit_log: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharePolicy {
    pub node_id: String,
    pub group_token: String,
    pub skills: Vec<ResourcePolicy>,
    pub agents: Vec<ResourcePolicy>,
    pub bridges: Vec<ResourcePolicy>,
    pub services: Vec<ResourcePolicy>,
    pub data_channels: Vec<String>,
}

impl SharePolicy {
    pub fn new(node_id: &str, group_token: &str) -> Self {
        SharePolicy {
            node_id: node_id.to_string(),
            group_token: group_token.to_string(),
            skills: Vec::new(),
            agents: Vec::new(),
            bridges: Vec::new(),
            services: Vec::new(),
            data_channels: vec!["chat".into(), "findings".into()],
        }
    }

    pub fn share_skill(&mut self, skill: &str, scope: ShareScope) {
        self.skills.push(ResourcePolicy {
            resource_type: "skill".into(),
            resource_id: skill.to_string(),
            scope,
            max_peers: 6,
            require_approval: false,
            audit_log: true,
        });
        info!("SharePolicy: sharing skill '{}'", skill);
    }

    pub fn share_bridge(&mut self, bridge: &str) {
        self.bridges.push(ResourcePolicy {
            resource_type: "bridge".into(),
            resource_id: bridge.to_string(),
            scope: ShareScope::Group(self.group_token.clone()),
            max_peers: 3,
            require_approval: true,
            audit_log: true,
        });
        info!("SharePolicy: sharing bridge '{}' with group approval", bridge);
    }

    pub fn is_shared(&self, resource_type: &str, resource_id: &str) -> bool {
        let list = match resource_type {
            "skill" => &self.skills,
            "agent" => &self.agents,
            "bridge" => &self.bridges,
            "service" => &self.services,
            _ => return false,
        };
        list.iter().any(|r| r.resource_id == resource_id)
    }

    pub fn visibility(&self, resource_type: &str, resource_id: &str) -> &str {
        let list = match resource_type {
            "skill" => &self.skills,
            "agent" => &self.agents,
            "bridge" => &self.bridges,
            "service" => &self.services,
            _ => return "personal",
        };
        for r in list {
            if r.resource_id == resource_id {
                match r.scope {
                    ShareScope::Personal => return "personal",
                    ShareScope::Group(_) => return "group",
                    ShareScope::Public => return "public",
                }
            }
        }
        "personal"
    }
}

pub struct PrivacyEngine {
    policies: HashMap<String, SharePolicy>,
}

impl PrivacyEngine {
    pub fn new() -> Self {
        PrivacyEngine {
            policies: HashMap::new(),
        }
    }

    pub fn get(&self, group: &str) -> Option<&SharePolicy> {
        self.policies.get(group)
    }

    pub fn get_mut(&mut self, group: &str) -> Option<&mut SharePolicy> {
        self.policies.get_mut(group)
    }

    pub fn create_policy(&mut self, group: &str, node_id: &str, token: &str) -> SharePolicy {
        let policy = SharePolicy::new(node_id, token);
        info!("PrivacyEngine: policy created for group '{}'", group);
        self.policies.insert(group.to_string(), policy);
        self.policies.get(group).unwrap().clone()
    }

    pub fn list(&self) -> Vec<String> {
        self.policies.keys().cloned().collect()
    }

    pub fn summary(&self) -> String {
        let mut out = String::from("Политики доступа:\n");
        for (group, policy) in &self.policies {
            out.push_str(&format!("  Группа '{}':\n", group));
            out.push_str(&format!("    Skills shared: {}\n", policy.skills.len()));
            out.push_str(&format!("    Bridges shared: {}\n", policy.bridges.len()));
            out.push_str(&format!("    Agents shared: {}\n", policy.agents.len()));
        }
        out
    }
}
