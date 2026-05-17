use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::store::KvStore;

const MAX_AGENTS: usize = 10;
const MAX_FINDINGS_PER_AGENT: usize = 1000;
const SUBAGENT_ACTIVE_SET: &str = "agents:active";

// ═══════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

impl AgentStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, AgentStatus::Completed | AgentStatus::Failed(_) | AgentStatus::Cancelled)
    }
    pub fn is_running(&self) -> bool {
        matches!(self, AgentStatus::Pending | AgentStatus::Running)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentState {
    pub id: String,
    pub role: String,
    pub skill: String,
    pub status: AgentStatus,
    pub node_id: String,
    pub llm_provider: String,
    pub group_id: u8,
    pub created_at: String,
    pub updated_at: String,
    pub steps_taken: u32,
    pub objective: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub background: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub agent_id: String,
    pub finding_type: String,
    pub confidence: f64,
    pub rank: u8,
    pub data: serde_json::Value,
    pub source_skill: String,
    pub source_node: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    pub agent_id: String,
    pub role: String,
    pub skill: String,
    pub status: AgentStatus,
    pub llm_provider: String,
    pub steps_taken: u32,
    pub findings_count: u64,
    pub created_at: String,
    pub duration_secs: u64,
    pub objective: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub background: bool,
    pub last_finding: Option<String>,
}

impl SubAgentResult {
    pub fn summary_for_llm(&self) -> String {
        format!(
            "{} role:{} skill:{} {:?} steps:{} findings:{} obj:{} parent:{} bg:{}",
            &self.agent_id[..8.min(self.agent_id.len())],
            self.role, self.skill, self.status,
            self.steps_taken, self.findings_count,
            &self.objective[..20.min(self.objective.len())],
            self.parent_id.as_deref().unwrap_or("-"),
            self.background,
        )
    }
}

// ═══════════════════════════════════════════════════════
// AGENT HANDLE (runtime)
// ═══════════════════════════════════════════════════════

struct AgentRuntime {
    input_tx: tokio::sync::mpsc::Sender<String>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

// ═══════════════════════════════════════════════════════
// SUBAGENT MANAGER
// ═══════════════════════════════════════════════════════

pub struct SubAgentManager {
    kvstore: Arc<KvStore>,
    next_id: Arc<AtomicU64>,
    max_agents: usize,
    runtimes: Arc<Mutex<HashMap<String, AgentRuntime>>>,
}

impl Clone for SubAgentManager {
    fn clone(&self) -> Self {
        SubAgentManager {
            kvstore: self.kvstore.clone(),
            next_id: self.next_id.clone(),
            max_agents: self.max_agents,
            runtimes: self.runtimes.clone(),
        }
    }
}

impl SubAgentManager {
    pub fn new(kvstore: Arc<KvStore>) -> Self {
        SubAgentManager {
            kvstore,
            next_id: Arc::new(AtomicU64::new(1)),
            max_agents: MAX_AGENTS,
            runtimes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn next_agent_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("agent.{}", n)
    }

    pub fn state_key(id: &str) -> String { format!("agent:{}:state", id) }
    pub fn findings_key(id: &str) -> String { format!("agent:{}:findings", id) }
    pub fn journal_key(id: &str) -> String { format!("agent:{}:journal", id) }
    pub fn input_key(id: &str) -> String { format!("agent:{}:input", id) }

    fn db_for(group_id: u8) -> u8 {
        if group_id >= 1 && group_id <= 6 { group_id } else { 0 }
    }

    async fn running_count(&self) -> usize {
        let runtimes = self.runtimes.lock().await;
        runtimes.len()
    }

    // ═══════════════════════════════════════════════════
    // AGENT OPEN (с cap + background + cancellation)
    // ═══════════════════════════════════════════════════

    pub async fn agent_open(
        &self,
        role: &str,
        skill: &str,
        llm_provider: &str,
        group_id: u8,
        node_id: &str,
        parent_id: Option<String>,
        background: bool,
    ) -> Result<String> {
        // Concurrency cap
        let running = self.running_count().await;
        if running >= self.max_agents {
            anyhow::bail!("Sub-agent limit reached (max {}, running {}). Close an agent first.", self.max_agents, running);
        }

        let db = Self::db_for(group_id);
        let id = self.next_agent_id();
        let now = Utc::now().to_rfc3339();

        let state = SubAgentState {
            id: id.clone(),
            role: role.to_string(),
            skill: skill.to_string(),
            status: AgentStatus::Running,
            node_id: node_id.to_string(),
            llm_provider: llm_provider.to_string(),
            group_id,
            created_at: now.clone(),
            updated_at: now.clone(),
            steps_taken: 0,
            objective: String::new(),
            parent_id: parent_id.clone(),
            children: vec![],
            background,
        };

        // Save to Redis
        let json = serde_json::to_string(&state)?;
        self.kvstore.select_db(db).set(&Self::state_key(&id), &json, 86400)?;
        self.kvstore.select_db(db).hset(SUBAGENT_ACTIVE_SET, &id, &serde_json::to_string(&state)?)?;

        // Journal
        let journal = serde_json::json!({"event": "created", "role": role, "skill": skill, "background": background});
        let _ = self.kvstore.select_db(db).xadd(
            &Self::journal_key(&id), &[("event", "created"), ("data", &journal.to_string())], 100);

        // Cancel token: background gets independent, child gets parent-linked
        let cancel_token = if background {
            tokio_util::sync::CancellationToken::new()
        } else if let Some(ref parent) = parent_id {
            // To properly cascade, parent's token would need to be stored.
            // Simplified: child gets its own token but cancellation cascades manually
            tokio_util::sync::CancellationToken::new()
        } else {
            tokio_util::sync::CancellationToken::new()
        };

        // Input channel
        let (input_tx, mut input_rx) = tokio::sync::mpsc::channel::<String>(64);

        // Spawn agent task
        let task_id = id.clone();
        let db_clone = db;
        let kv = self.kvstore.clone();
        let mgr = self.clone();

        let handle = tokio::spawn(async move {
            // Agent listens for input messages while running
            loop {
                tokio::select! {
                    Some(msg) = input_rx.recv() => {
                        let truncated: String = msg.chars().take(60).collect();
                        info!("Agent {} received: {}", &task_id, truncated);

                        // Append input message as finding
                        let finding_id = uuid::Uuid::new_v4().to_string();
                        let finding = serde_json::json!({
                            "type": "input",
                            "content": msg,
                            "ts": Utc::now().to_rfc3339(),
                        });
                        let _ = kv.select_db(db_clone).xadd(
                            &Self::findings_key(&task_id),
                            &[("finding_id", &finding_id), ("data", &finding.to_string())],
                            MAX_FINDINGS_PER_AGENT,
                        );

                        // Update state
                        if let Ok(Some(s)) = kv.select_db(db_clone).get(&Self::state_key(&task_id)) {
                            if let Ok(mut st) = serde_json::from_str::<SubAgentState>(&s) {
                                st.steps_taken += 1;
                                st.updated_at = Utc::now().to_rfc3339();
                                let _ = kv.select_db(db_clone).set(
                                    &Self::state_key(&task_id),
                                    &serde_json::to_string(&st).unwrap_or_default(),
                                    86400,
                                );
                            }
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                        // Idle heartbeat — agent is alive
                        break; // In production: continue loop, break only on cancel
                    }
                }
            }
        });

        // Store runtime
        let runtime = AgentRuntime {
            input_tx,
            cancel_token: Some(cancel_token),
            task_handle: Some(handle),
        };
        self.runtimes.lock().await.insert(id.clone(), runtime);

        // Update parent's children list
        if let Some(ref pid) = parent_id {
            if let Ok(Some(s)) = self.kvstore.select_db(db).get(&Self::state_key(pid)) {
                if let Ok(mut st) = serde_json::from_str::<SubAgentState>(&s) {
                    st.children.push(id.clone());
                    st.updated_at = Utc::now().to_rfc3339();
                    let _ = self.kvstore.select_db(db).set(
                        &Self::state_key(pid), &serde_json::to_string(&st).unwrap_or_default(), 86400);
                }
            }
        }

        info!("Agent opened: {} (role={}, skill={}, bg={}, parent={:?})",
            &id, role, skill, background, parent_id);
        Ok(id)
    }

    // ═══════════════════════════════════════════════════
    // AGENT SEND INPUT (mid-flight)
    // ═══════════════════════════════════════════════════

    pub async fn agent_send_input(&self, agent_id: &str, message: &str, interrupt: bool) -> Result<()> {
        let runtimes = self.runtimes.lock().await;
        if let Some(rt) = runtimes.get(agent_id) {
            if interrupt {
                rt.cancel_token.as_ref().map(|ct| ct.cancel());
            }
            rt.input_tx.send(message.to_string()).await
                .map_err(|e| anyhow::anyhow!("Failed to send to agent {}: {}", agent_id, e))?;
            let message_short: String = message.chars().take(60).collect();
            info!("Sent input to agent {} (interrupt={}): {}", agent_id, interrupt, message_short);
            Ok(())
        } else {
            // Agent not in runtime — message via Redis
            let _ = self.kvstore.select_db(0).xadd(
                &Self::input_key(agent_id),
                &[("message", message), ("interrupt", &interrupt.to_string())],
                50,
            );
            info!("Queued input for agent {} (not running, saved to Redis)", agent_id);
            Ok(())
        }
    }

    // ═══════════════════════════════════════════════════
    // AGENT ASSIGN (change objective mid-flight)
    // ═══════════════════════════════════════════════════

    pub async fn agent_assign(&self, agent_id: &str, objective: &str, group_id: u8) -> Result<()> {
        let db = Self::db_for(group_id);
        let state_json = self.kvstore.select_db(db).get(&Self::state_key(agent_id))?;
        let mut state: SubAgentState = match state_json {
            Some(s) => serde_json::from_str(&s)?,
            None => anyhow::bail!("Agent {} not found", agent_id),
        };

        let old_obj = state.objective.clone();
        state.objective = objective.to_string();
        state.status = AgentStatus::Running;
        state.updated_at = Utc::now().to_rfc3339();
        state.steps_taken += 1;

        let json = serde_json::to_string(&state)?;
        self.kvstore.select_db(db).set(&Self::state_key(agent_id), &json, 86400)?;

        // Journal
        let journal = serde_json::json!({"event": "assigned", "old_objective": old_obj, "new_objective": objective});
        let _ = self.kvstore.select_db(db).xadd(
            &Self::journal_key(agent_id), &[("event", "assigned"), ("data", &journal.to_string())], 100);

        let old_obj_short: String = old_obj.chars().take(40).collect();
        info!("Agent {} assigned: {} → {}", agent_id, old_obj_short, objective);
        Ok(())
    }

    // ═══════════════════════════════════════════════════
    // AGENT EVAL
    // ═══════════════════════════════════════════════════

    pub fn agent_eval(&self, agent_id: &str, group_id: u8) -> Result<SubAgentResult> {
        let db = Self::db_for(group_id);
        let kv = self.kvstore.select_db(db);

        let state_json = kv.get(&Self::state_key(agent_id))?;
        let state: SubAgentState = match state_json {
            Some(s) => serde_json::from_str(&s)?,
            None => anyhow::bail!("Agent {} not found", agent_id),
        };

        let created = chrono::DateTime::parse_from_rfc3339(&state.created_at)
            .unwrap_or_else(|_| chrono::DateTime::from(Utc::now()));
        let duration = Utc::now().signed_duration_since(created).num_seconds();
        let findings_count = kv.xlen(&Self::findings_key(agent_id)).unwrap_or(0);

        Ok(SubAgentResult {
            agent_id: state.id.clone(),
            role: state.role.clone(),
            skill: state.skill.clone(),
            status: state.status,
            llm_provider: state.llm_provider.clone(),
            steps_taken: state.steps_taken,
            findings_count,
            created_at: state.created_at.clone(),
            duration_secs: duration as u64,
            objective: state.objective.clone(),
            parent_id: state.parent_id.clone(),
            children: state.children.clone(),
            background: state.background,
            last_finding: None,
        })
    }

    // ═══════════════════════════════════════════════════
    // AGENT CLOSE (cancellation cascade)
    // ═══════════════════════════════════════════════════

    pub async fn agent_close(&self, agent_id: &str, group_id: u8) -> Result<SubAgentResult> {
        let db = Self::db_for(group_id);
        let kv = self.kvstore.select_db(db);

        let state_json = kv.get(&Self::state_key(agent_id))?;
        let mut state: SubAgentState = match state_json {
            Some(s) => serde_json::from_str(&s)?,
            None => anyhow::bail!("Agent {} not found", agent_id),
        };

        let children = state.children.clone();

        // Close runtime if exists
        {
            let mut runtimes = self.runtimes.lock().await;
            if let Some(rt) = runtimes.remove(agent_id) {
                if let Some(ct) = rt.cancel_token {
                    ct.cancel();
                }
            }
        }

        state.status = AgentStatus::Completed;
        state.updated_at = Utc::now().to_rfc3339();
        let json = serde_json::to_string(&state)?;
        kv.set(&Self::state_key(agent_id), &json, 86400)?;
        let _ = kv.hset(SUBAGENT_ACTIVE_SET, agent_id, "closed");

        let journal = serde_json::json!({"event": "closed", "children_closed": children.len()});
        let _ = kv.xadd(&Self::journal_key(agent_id), &[("event", "closed"), ("data", &journal.to_string())], 100);

        info!("Agent closed: {} ({} children)", agent_id, children.len());

        // Cancellation cascade: close children iteratively (non-recursive)
        for child_id in &children {
            // Read child state, mark cancelled
            if let Ok(Some(child_json)) = kv.get(&Self::state_key(child_id)) {
                if let Ok(mut child_state) = serde_json::from_str::<SubAgentState>(&child_json) {
                    if child_state.status.is_running() {
                        child_state.status = AgentStatus::Cancelled;
                        child_state.updated_at = Utc::now().to_rfc3339();
                        let _ = kv.set(&Self::state_key(child_id),
                            &serde_json::to_string(&child_state).unwrap_or_default(), 86400);
                        let _ = kv.hset(SUBAGENT_ACTIVE_SET, child_id, "cancelled");

                        // Cancel runtime
                        let mut runtimes = self.runtimes.lock().await;
                        if let Some(rt) = runtimes.remove(child_id) {
                            if let Some(ct) = rt.cancel_token {
                                ct.cancel();
                            }
                        }
                        drop(runtimes);

                        info!("Cancellation cascade: child {} cancelled", child_id);
                    }
                }
            }
        }

        self.agent_eval(agent_id, group_id)
    }

    // ═══════════════════════════════════════════════════
    // FINDING + COMPLETE
    // ═══════════════════════════════════════════════════

    pub fn agent_add_finding(&self, agent_id: &str, finding_type: &str, confidence: f64,
            data: serde_json::Value, skill: &str, node_id: &str, group_id: u8) -> Result<String> {
        let db = Self::db_for(group_id);
        let finding_id = uuid::Uuid::new_v4().to_string();
        let finding = Finding {
            id: finding_id.clone(),
            agent_id: agent_id.to_string(),
            finding_type: finding_type.to_string(),
            confidence,
            rank: 0,
            data,
            source_skill: skill.to_string(),
            source_node: node_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };
        let json = serde_json::to_string(&finding)?;
        let _ = self.kvstore.select_db(db).xadd(
            &Self::findings_key(agent_id), &[("finding_id", &finding_id), ("data", &json)], MAX_FINDINGS_PER_AGENT)?;

        // Update steps
        if let Ok(Some(s)) = self.kvstore.select_db(db).get(&Self::state_key(agent_id)) {
            if let Ok(mut st) = serde_json::from_str::<SubAgentState>(&s) {
                st.steps_taken += 1;
                st.updated_at = Utc::now().to_rfc3339();
                let _ = self.kvstore.select_db(db).set(
                    &Self::state_key(agent_id), &serde_json::to_string(&st).unwrap_or_default(), 86400);
            }
        }
        info!("Finding added: {} type={} agent={}", &finding_id[..8], finding_type, agent_id);
        Ok(finding_id)
    }

    pub fn agent_complete_with_finding(&self, agent_id: &str, finding_type: &str,
            confidence: f64, data: serde_json::Value, skill: &str, node_id: &str, group_id: u8) -> Result<SubAgentResult> {
        self.agent_add_finding(agent_id, finding_type, confidence, data, skill, node_id, group_id)?;

        let db = Self::db_for(group_id);
        let state_json = self.kvstore.select_db(db).get(&Self::state_key(agent_id))?;
        let mut state: SubAgentState = match state_json {
            Some(s) => serde_json::from_str(&s)?,
            None => anyhow::bail!("Agent {} not found", agent_id),
        };
        state.status = AgentStatus::Completed;
        state.updated_at = Utc::now().to_rfc3339();
        let json = serde_json::to_string(&state)?;
        self.kvstore.select_db(db).set(&Self::state_key(agent_id), &json, 86400)?;
        let _ = self.kvstore.select_db(db).hset(SUBAGENT_ACTIVE_SET, agent_id, "completed");
        info!("Agent completed with finding: {}", agent_id);
        self.agent_eval(agent_id, group_id)
    }

    // ═══════════════════════════════════════════════════
    // LIST
    // ═══════════════════════════════════════════════════

    pub fn list_active(&self, group_id: u8) -> Result<Vec<SubAgentResult>> {
        let db = Self::db_for(group_id);
        let entries = self.kvstore.select_db(db).hgetall(SUBAGENT_ACTIVE_SET)?;
        let mut results: Vec<SubAgentResult> = Vec::new();
        for (agent_id, _) in &entries {
            if let Ok(result) = self.agent_eval(agent_id, group_id) {
                results.push(result);
            }
        }
        results.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(results)
    }

    pub fn summary_for_llm(&self, group_id: u8) -> String {
        let agents = self.list_active(group_id).unwrap_or_default();
        if agents.is_empty() {
            return "  Нет активных агентов.".to_string();
        }
        let mut out = format!("Активные агенты ({}):\n", agents.len());
        for a in &agents {
            out.push_str(&format!("  {}\n", a.summary_for_llm()));
        }
        out
    }
}

// ═══════════════════════════════════════════════════════
// ROLE SYSTEM PROMPTS
// ═══════════════════════════════════════════════════════

pub fn role_system_prompt(role: &str, skill_reg: &crate::skill::SkillRegistry, skill_name: &str) -> String {
    let skill_prompt = skill_reg.get_prompt(skill_name).unwrap_or("");
    let role_intro = match role {
        "general" => "Ты — универсальный агент. Можешь делать любые задачи.",
        "explore" | "explorer" => "Ты — исследователь. Только читаешь и анализируешь. Ничего не меняешь.",
        "plan" | "planner" => "Ты — архитектор. Проектируешь решение, пишешь план. Не меняешь файлы.",
        "review" | "reviewer" => "Ты — ревьюер. Проверяешь на ошибки. Не правишь.",
        "implement" | "implementer" => "Ты — реализатор. Вносишь изменения, пишешь код.",
        "verify" | "verifier" => "Ты — верификатор. Тестируешь, сообщаешь pass/fail.",
        "custom" => "Ты — специализированный агент с узким набором инструментов.",
        "collector" => "Ты — Collector. Собираешь сырые данные. Не анализируешь.",
        "scout" => "Ты — Scout. Разведка и поиск информации.",
        "analyst" => "Ты — Analyst. Анализируешь, классифицируешь, ищешь паттерны.",
        "synthesizer" => "Ты — Synthesizer. Объединяешь findings в отчёты. Нужен лучший LLM.",
        "coordinator" => "Ты — Coordinator. Оркестрируешь агентов, распределяешь задачи.",
        "archivist" => "Ты — Archivist. Управляешь памятью группы. Индексируешь findings.",
        "camera-operator" => "Ты — Camera Operator. Управляешь камерами, фото, видео, NDI, OBS.",
        "video-editor" => "Ты — Video Editor. Монтируешь, цветокоррекция, FFmpeg.",
        "lab-operator" => "Ты — Lab Operator. Управляешь приборами: спектрометры, микроскопы.",
        _ => "Ты — агент WATERS. Выполни поставленную задачу.",
    };

    format!("{}\n\n## Skill\n\n{}\n\n## Output\n\nFinding JSON: type, confidence, data", role_intro, skill_prompt)
}
