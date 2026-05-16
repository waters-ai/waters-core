#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CargoMode {
    Full,
    Lite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CargoStatus {
    OfferSent,
    OfferRejected,
    AcceptancePending,
    Transferring,
    TransferPaused,
    Landed,
    Recalled,
    Expired,
    RequestSent,
    AwaitingSend,
}

impl CargoStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, CargoStatus::Landed | CargoStatus::Recalled | CargoStatus::Expired | CargoStatus::OfferRejected)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoManifest {
    pub agent_name: String,
    pub skills: Vec<String>,
    pub mode: CargoMode,
    pub required_bridges: Vec<String>,
    pub mission: String,
    pub sender_node: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot {
    pub name: String,
    pub version: String,
    pub prompt_hash: String,
    pub bridges: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub name: String,
    pub skills: Vec<SkillSnapshot>,
    pub memory: Option<Vec<u8>>,
    pub journal: Vec<CargoJournalEntry>,
    pub state: HashMap<String, String>,
    pub onboard_llm: Option<OnboardLlm>,
    pub task_memory: Option<TaskMemorySnapshot>,
}

/// Маленькая LLM на борту агента (1-3B, GGUF)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardLlm {
    pub model: String,
    pub quant: String,
    pub ctx_size: u32,
    pub size_mb: u32,
}

/// Снапшот памяти агента по конкретной задаче
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMemorySnapshot {
    pub task_id: String,
    pub findings: Vec<Finding>,
    pub context: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoJournalEntry {
    pub timestamp: String,
    pub event: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCargo {
    pub cargo_id: String,
    pub manifest: CargoManifest,
    pub payload: AgentSnapshot,
    pub ttl_secs: u64,
    pub reply_to: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoChunk {
    pub cargo_id: String,
    pub seq: u32,
    pub total: u32,
    pub data: Vec<u8>,
    pub checksum: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSession {
    pub session_id: String,
    pub last_seq: u64,
    pub reports: Vec<TaskReport>,
    pub new_tasks: Vec<TaskAssignment>,
    pub status_updates: Vec<AgentStatusUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReport {
    pub task_id: String,
    pub status: String,
    pub findings_count: u32,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: String,
    pub title: String,
    pub description: String,
    pub required_skills: Vec<String>,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusUpdate {
    pub agent_name: String,
    pub status: String,
    pub current_task: Option<String>,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultExchange {
    pub session_id: String,
    pub source_node: String,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub agent_id: String,
    pub task_id: String,
    pub finding_type: String,
    pub content: String,
    pub confidence: f64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum CargoGossipMessage {
    #[serde(rename = "cargo.offer")]
    CargoOffer {
        cargo_id: String,
        manifest: CargoManifest,
        timestamp: String,
    },
    #[serde(rename = "cargo.ack")]
    CargoAck {
        cargo_id: String,
        accepted: bool,
        reason: Option<String>,
        mode: Option<CargoMode>,
        timestamp: String,
    },
    #[serde(rename = "cargo.send")]
    CargoSend {
        cargo_id: String,
        payload: AgentCargo,
        timestamp: String,
    },
    #[serde(rename = "cargo.confirm")]
    CargoConfirm {
        cargo_id: String,
        status: CargoStatus,
        timestamp: String,
    },
    #[serde(rename = "cargo.request")]
    CargoRequest {
        agent_name: String,
        mode: CargoMode,
        requester: String,
        timestamp: String,
    },
    #[serde(rename = "cargo.chunk")]
    CargoChunkMsg {
        chunk: CargoChunk,
        timestamp: String,
    },
    #[serde(rename = "sync.start")]
    SyncStart {
        session_id: String,
        last_seq: u64,
        timestamp: String,
    },
    #[serde(rename = "sync.data")]
    SyncData {
        session: SyncSession,
        timestamp: String,
    },
    #[serde(rename = "sync.ack")]
    SyncAck {
        session_id: String,
        seq: u64,
        timestamp: String,
    },
    #[serde(rename = "xchange.data")]
    XchangeData {
        exchange: ResultExchange,
        timestamp: String,
    },
    #[serde(rename = "xchange.ack")]
    XchangeAck {
        session_id: String,
        received_count: u32,
        timestamp: String,
    },
}

pub struct CargoEngine {
    outgoing: HashMap<String, AgentCargo>,
    incoming: HashMap<String, AgentCargo>,
    statuses: HashMap<String, CargoStatus>,
}

impl CargoEngine {
    pub fn new() -> Self {
        CargoEngine {
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
            statuses: HashMap::new(),
        }
    }

    pub fn push_offer(&mut self, cargo: AgentCargo) {
        let id = cargo.cargo_id.clone();
        self.outgoing.insert(id.clone(), cargo);
        self.statuses.insert(id, CargoStatus::OfferSent);
    }

    pub fn accept_cargo(&mut self, cargo_id: &str) -> Option<&CargoStatus> {
        if let Some(status) = self.statuses.get_mut(cargo_id) {
            if *status == CargoStatus::AcceptancePending {
                *status = CargoStatus::Transferring;
            }
        }
        self.statuses.get(cargo_id)
    }

    pub fn reject_cargo(&mut self, cargo_id: &str) -> Option<&CargoStatus> {
        if let Some(status) = self.statuses.get_mut(cargo_id) {
            if *status == CargoStatus::AcceptancePending {
                *status = CargoStatus::OfferRejected;
            }
        }
        self.statuses.get(cargo_id)
    }

    pub fn confirm_landed(&mut self, cargo_id: &str) {
        self.statuses.insert(cargo_id.to_string(), CargoStatus::Landed);
    }

    pub fn set_paused(&mut self, cargo_id: &str) {
        if let Some(status) = self.statuses.get_mut(cargo_id) {
            if *status == CargoStatus::Transferring {
                *status = CargoStatus::TransferPaused;
            }
        }
    }

    pub fn expire(&mut self, cargo_id: &str) {
        self.statuses.insert(cargo_id.to_string(), CargoStatus::Expired);
        self.outgoing.remove(cargo_id);
        self.incoming.remove(cargo_id);
    }

    pub fn status(&self, cargo_id: &str) -> Option<&CargoStatus> {
        self.statuses.get(cargo_id)
    }

    pub fn list_active(&self) -> Vec<(&str, &CargoStatus)> {
        self.statuses.iter()
            .filter(|(_, s)| !s.is_terminal())
            .map(|(id, s)| (id.as_str(), s))
            .collect()
    }

    pub fn list_incoming(&self) -> Vec<(&str, &CargoStatus)> {
        self.incoming.keys()
            .filter_map(|id| self.statuses.get(id).map(|s| (id.as_str(), s)))
            .collect()
    }

    pub fn store_incoming(&mut self, cargo: AgentCargo) {
        let id = cargo.cargo_id.clone();
        self.incoming.insert(id.clone(), cargo);
        self.statuses.insert(id, CargoStatus::AcceptancePending);
    }
}
