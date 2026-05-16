use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

use crate::skill::SkillRegistry;
use crate::store::KvStore;

const MAX_AGENTS: usize = 10;
const MAX_FINDINGS_PER_AGENT: usize = 1000;
const SUBAGENT_ACTIVE_SET: &str = "agents:active";

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
        matches!(
            self,
            AgentStatus::Completed | AgentStatus::Failed(_) | AgentStatus::Cancelled
        )
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
    pub last_finding: Option<Finding>,
}

impl SubAgentResult {
    pub fn summary_for_llm(&self) -> String {
        format!(
            "agent:{} role:{} skill:{} status:{:?} steps:{} findings:{} llm:{}",
            &self.agent_id[..8.min(self.agent_id.len())],
            self.role,
            self.skill,
            self.status,
            self.steps_taken,
            self.findings_count,
            self.llm_provider,
        )
    }
}

#[derive(Clone)]
pub struct SubAgentManager {
    kvstore: Arc<KvStore>,
    next_id: Arc<AtomicU64>,
    max_agents: usize,
}

impl SubAgentManager {
    pub fn new(kvstore: Arc<KvStore>) -> Self {
        SubAgentManager {
            kvstore,
            next_id: Arc::new(AtomicU64::new(1)),
            max_agents: MAX_AGENTS,
        }
    }

    fn next_agent_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("agent.{}", n)
    }

    pub fn state_key(id: &str) -> String {
        format!("agent:{}:state", id)
    }
    pub fn findings_key(id: &str) -> String {
        format!("agent:{}:findings", id)
    }
    pub fn journal_key(id: &str) -> String {
        format!("agent:{}:journal", id)
    }

    pub fn agent_open(
        &self,
        role: &str,
        skill: &str,
        llm_provider: &str,
        group_id: u8,
        node_id: &str,
    ) -> Result<String> {
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let id = self.next_agent_id();
        let now = Utc::now().to_rfc3339();

        let state = SubAgentState {
            id: id.clone(),
            role: role.to_string(),
            skill: skill.to_string(),
            status: AgentStatus::Pending,
            node_id: node_id.to_string(),
            llm_provider: llm_provider.to_string(),
            group_id,
            created_at: now.clone(),
            updated_at: now,
            steps_taken: 0,
        };

        let json = serde_json::to_string(&state)?;
        self.kvstore
            .select_db(db)
            .set(&Self::state_key(&id), &json, 86400)?;

        // Add to active set
        self.kvstore.select_db(db).hset(
            SUBAGENT_ACTIVE_SET,
            &id,
            &serde_json::to_string(&state)?,
        )?;

        // Journal: created
        let journal_entry = serde_json::json!({
            "event": "created",
            "agent_id": id,
            "role": role,
            "skill": skill,
            "ts": Utc::now().to_rfc3339(),
        });
        let _ = self.kvstore.select_db(db).xadd(
            &Self::journal_key(&id),
            &[("event", "created"), ("data", &journal_entry.to_string())],
            100,
        );

        info!(
            "Agent opened: {} (role={}, skill={}, group={})",
            &id, role, skill, group_id
        );
        Ok(id)
    }

    pub fn agent_assign(&self, agent_id: &str, objective: &str, group_id: u8) -> Result<()> {
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let state_json = self.kvstore.select_db(db).get(&Self::state_key(agent_id))?;

        let mut state: SubAgentState = match state_json {
            Some(s) => serde_json::from_str(&s)?,
            None => anyhow::bail!("Agent {} not found", agent_id),
        };

        state.status = AgentStatus::Running;
        state.updated_at = Utc::now().to_rfc3339();
        state.steps_taken += 1;

        let json = serde_json::to_string(&state)?;
        self.kvstore
            .select_db(db)
            .set(&Self::state_key(agent_id), &json, 86400)?;

        // Save objective as a journal entry
        let journal_entry = serde_json::json!({
            "event": "assigned",
            "agent_id": agent_id,
            "objective": objective,
            "ts": Utc::now().to_rfc3339(),
        });
        let _ = self.kvstore.select_db(db).xadd(
            &Self::journal_key(agent_id),
            &[("event", "assigned"), ("data", &journal_entry.to_string())],
            100,
        );

        info!("Agent {} assigned: {}", agent_id, objective);
        Ok(())
    }

    pub fn agent_add_finding(
        &self,
        agent_id: &str,
        finding_type: &str,
        confidence: f64,
        data: serde_json::Value,
        skill: &str,
        node_id: &str,
        group_id: u8,
    ) -> Result<String> {
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let finding_id = uuid::Uuid::new_v4().to_string();

        let finding = Finding {
            id: finding_id.clone(),
            agent_id: agent_id.to_string(),
            finding_type: finding_type.to_string(),
            confidence,
            rank: 0, // Will be calculated by rank engine
            data,
            source_skill: skill.to_string(),
            source_node: node_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&finding)?;
        let _ = self.kvstore.select_db(db).xadd(
            &Self::findings_key(agent_id),
            &[("finding_id", &finding_id), ("data", &json)],
            MAX_FINDINGS_PER_AGENT,
        )?;

        // Update state: increment steps
        if let Ok(Some(state_json)) = self.kvstore.select_db(db).get(&Self::state_key(agent_id)) {
            if let Ok(mut state) = serde_json::from_str::<SubAgentState>(&state_json) {
                state.steps_taken += 1;
                state.updated_at = Utc::now().to_rfc3339();
                let _ = self.kvstore.select_db(db).set(
                    &Self::state_key(agent_id),
                    &serde_json::to_string(&state)?,
                    86400,
                );
            }
        }

        info!(
            "Finding added: {} type={} conf={} agent={}",
            &finding_id, finding_type, confidence, agent_id
        );
        Ok(finding_id)
    }

    pub fn agent_eval(&self, agent_id: &str, group_id: u8) -> Result<SubAgentResult> {
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let kv = self.kvstore.select_db(db);

        let state_json = kv.get(&Self::state_key(agent_id))?;
        let state: SubAgentState = match state_json {
            Some(s) => serde_json::from_str(&s)?,
            None => anyhow::bail!("Agent {} not found", agent_id),
        };

        let created = chrono::DateTime::parse_from_rfc3339(&state.created_at)
            .unwrap_or_else(|_| chrono::DateTime::from(Utc::now()));
        let duration = Utc::now().signed_duration_since(created).num_seconds();

        // Get findings count
        let findings_count = kv.xlen(&Self::findings_key(agent_id)).unwrap_or(0);

        // Get last finding
        let last_finding = None; // In future: XREVRANGE for latest

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
            last_finding,
        })
    }

    pub fn agent_close(&self, agent_id: &str, group_id: u8) -> Result<SubAgentResult> {
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let kv = self.kvstore.select_db(db);

        let state_json = kv.get(&Self::state_key(agent_id))?;
        let mut state: SubAgentState = match state_json {
            Some(s) => serde_json::from_str(&s)?,
            None => anyhow::bail!("Agent {} not found", agent_id),
        };

        state.status = AgentStatus::Completed;
        state.updated_at = Utc::now().to_rfc3339();
        let json = serde_json::to_string(&state)?;
        kv.set(&Self::state_key(agent_id), &json, 86400)?;

        // Remove from active set
        let _ = kv.hset(SUBAGENT_ACTIVE_SET, agent_id, "closed");

        // Journal: closed
        let journal_entry = serde_json::json!({
            "event": "closed",
            "agent_id": agent_id,
            "ts": Utc::now().to_rfc3339(),
        });
        let _ = kv.xadd(
            &Self::journal_key(agent_id),
            &[("event", "closed"), ("data", &journal_entry.to_string())],
            100,
        );

        info!("Agent closed: {}", agent_id);
        self.agent_eval(agent_id, group_id)
    }

    pub fn agent_complete_with_finding(
        &self,
        agent_id: &str,
        finding_type: &str,
        confidence: f64,
        data: serde_json::Value,
        skill: &str,
        node_id: &str,
        group_id: u8,
    ) -> Result<SubAgentResult> {
        self.agent_add_finding(
            agent_id,
            finding_type,
            confidence,
            data,
            skill,
            node_id,
            group_id,
        )?;

        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let kv = self.kvstore.select_db(db);

        let state_json = kv.get(&Self::state_key(agent_id))?;
        let mut state: SubAgentState = match state_json {
            Some(s) => serde_json::from_str(&s)?,
            None => anyhow::bail!("Agent {} not found", agent_id),
        };

        state.status = AgentStatus::Completed;
        state.updated_at = Utc::now().to_rfc3339();
        let json = serde_json::to_string(&state)?;
        kv.set(&Self::state_key(agent_id), &json, 86400)?;

        let _ = kv.hset(SUBAGENT_ACTIVE_SET, agent_id, "completed");

        info!("Agent completed with finding: {}", agent_id);
        self.agent_eval(agent_id, group_id)
    }

    pub fn list_active(&self, group_id: u8) -> Result<Vec<SubAgentResult>> {
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let kv = self.kvstore.select_db(db);

        let entries = kv.hgetall(SUBAGENT_ACTIVE_SET)?;
        let mut results = Vec::new();

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

// Role system prompts — адаптированы из TUI для наших профессий
pub fn role_system_prompt(role: &str, skill_reg: &SkillRegistry, skill_name: &str) -> String {
    let skill_prompt = skill_reg.get_prompt(skill_name).unwrap_or("");

    let role_intro = match role {
        // TUI 1:1 roles (coding)
        "general" => "Ты — универсальный агент. Можешь делать любые задачи: читать, писать, искать, запускать команды.",
        "explore" | "explorer" => "Ты — исследователь. Твоя задача: быстро найти информацию, изучить код или данные. Ты НЕ меняешь ничего — только читаешь и анализируешь.",
        "plan" | "planner" => "Ты — архитектор. Твоя задача: спроектировать решение, написать план, создать чеклист. Ты НЕ пишешь код и НЕ меняешь файлы.",
        "review" | "reviewer" => "Ты — ревьюер. Твоя задача: проверить код или данные на ошибки, оценить качество. Ты НЕ правишь — только даёшь заключение.",
        "implement" | "implementer" => "Ты — реализатор. Твоя задача: внести изменения, написать код, применить патчи. Работай строго по задаче, без лишних правок.",
        "verify" | "verifier" => "Ты — верификатор. Твоя задача: запустить тесты, проверить результат, сообщить pass/fail. Ты НЕ чинишь ошибки — только находишь.",
        "custom" => "Ты — специализированный агент с узким набором инструментов. Используй только то, что разрешено.",

        // WATERS professions
        "collector" => "Ты — Collector. Твоя работа: собирать сырые данные из внешних источников. Ты не анализируешь — ты приносишь. confidence = насколько ты уверен в источнике.",
        "scout" => "Ты — Scout. Твоя работа: разведка и поиск информации. Ищи в нескольких источниках, проверяй факты, возвращай evidence с цитатами.",
        "analyst" => "Ты — Analyst. Твоя работа: анализировать данные, находить паттерны, классифицировать, выявлять аномалии. Возвращай classification с confidence.",
        "synthesizer" => "Ты — Synthesizer. Твоя работа: объединять findings из разных источников в связный отчёт, статью или сводку. Используй лучший LLM.",
        "coordinator" => "Ты — Coordinator. Твоя работа: оркестрировать группу агентов, распределять задачи, следить за прогрессом. Используй agent_open/agent_eval/agent_close.",
        "archivist" => "Ты — Archivist. Твоя работа: управлять памятью группы. Индексируй findings, строй граф связей, поддерживай порядок в базе знаний.",
        "specialist" => "Ты — специалист. Твоя работа определяется загруженным SKILL.md. Следуй инструкциям скилла.",

        _ => "Ты — агент WATERS. Выполни поставленную задачу.",
    };

    format!(
        r#"{}

## Output contract

Твой ответ — JSON Finding:
{{
  "finding_type": "тип находки",
  "confidence": 0.0-1.0,
  "data": {{ ... }},
  "evidence_count": N
}}

## Skill

{}

## Правила
1. Проверяй источники
2. Указывай confidence
3. Сохраняй evidence
4. Если не уверен — скажи честно
"#,
        role_intro, skill_prompt
    )
}
