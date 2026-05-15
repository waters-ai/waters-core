use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, warn};

pub struct SubAgentHandle {
    pub id: String,
    pub role: String,
    pub sender: mpsc::Sender<String>,
    pub receiver: mpsc::Receiver<String>,
}

impl SubAgentHandle {
    pub fn new(id: String, role: String) -> (Self, mpsc::Receiver<String>) {
        let (tx, rx) = mpsc::channel(64);
        let (tx_result, rx_result) = mpsc::channel(64);
        let handle = SubAgentHandle {
            id,
            role,
            sender: tx,
            receiver: rx_result,
        };
        (handle, rx)
    }
}

pub struct SubAgentManager {
    agents: HashMap<String, SubAgentHandle>,
    next_id: u64,
}

impl SubAgentManager {
    pub fn new() -> Self {
        SubAgentManager {
            agents: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn spawn(&mut self, role: &str) -> String {
        let id = format!("agent.{}.{}", role, self.next_id);
        self.next_id += 1;

        let (tx, mut rx) = mpsc::channel(64);
        let (tx_result, _rx_result) = mpsc::channel(64);

        let task_id = id.clone();
        let task_role = role.to_string();
        tokio::spawn(async move {
            info!("Sub-agent {} ({}) spawned", task_id, task_role);
            while let Some(msg) = rx.recv().await {
                info!("Sub-agent {} received: {}", task_id, msg);
                let result = format!("processed: {}", msg);
                if tx_result.send(result).await.is_err() {
                    warn!("Sub-agent {}: result channel closed", task_id);
                    break;
                }
            }
        });

        self.agents.insert(id.clone(), SubAgentHandle {
            id: id.clone(),
            role: role.to_string(),
            sender: tx,
            receiver: _rx_result,
        });
        info!("SubAgentManager: spawned {} ({})", id, role);
        id
    }

    pub fn send(&self, id: &str, msg: String) -> bool {
        if let Some(agent) = self.agents.get(id) {
            agent.sender.try_send(msg).is_ok()
        } else {
            false
        }
    }

    pub fn broadcast(&self, msg: &str) -> usize {
        let mut count = 0;
        for agent in self.agents.values() {
            if agent.sender.try_send(msg.to_string()).is_ok() {
                count += 1;
            }
        }
        count
    }

    pub fn count(&self) -> usize {
        self.agents.len()
    }

    pub fn list_roles(&self) -> Vec<(String, String)> {
        self.agents.values().map(|a| (a.id.clone(), a.role.clone())).collect()
    }
}
