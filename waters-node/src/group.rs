use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use tokio::sync::mpsc;
use tracing::info;

/// Режимы групповой работы (GROUP_MODES.md)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupMode {
    Storm,      // параллельная работа, макс скорость
    Hunt,       // итеративный поиск, усиление лучшего направления
    Synthesis,  // глубокий анализ, синтез findings
    Focus,      // один исполнитель, остальные read-only
    Watch,      // фоновый мониторинг, триггер → Storm
}

impl fmt::Display for GroupMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GroupMode::Storm => write!(f, "⚡ Storm"),
            GroupMode::Hunt => write!(f, "🎯 Hunt"),
            GroupMode::Synthesis => write!(f, "🔬 Synthesis"),
            GroupMode::Focus => write!(f, "🎯 Focus"),
            GroupMode::Watch => write!(f, "👁 Watch"),
        }
    }
}

impl GroupMode {
    pub fn parse(input: &str) -> Option<GroupMode> {
        let lower = input.to_lowercase();
        if lower.contains("storm") || lower.contains("шторм") { Some(GroupMode::Storm) }
        else if lower.contains("hunt") || lower.contains("охот") { Some(GroupMode::Hunt) }
        else if lower.contains("synthesis") || lower.contains("синтез") || lower.contains("анализ") { Some(GroupMode::Synthesis) }
        else if lower.contains("focus") || lower.contains("фокус") { Some(GroupMode::Focus) }
        else if lower.contains("watch") || lower.contains("дежур") || lower.contains("watch") { Some(GroupMode::Watch) }
        else { None }
    }

    pub fn next_after_completion(&self) -> GroupMode {
        match self {
            GroupMode::Watch => GroupMode::Storm,
            GroupMode::Storm => GroupMode::Hunt,
            GroupMode::Hunt => GroupMode::Synthesis,
            GroupMode::Synthesis => GroupMode::Focus,
            GroupMode::Focus => GroupMode::Watch,
        }
    }
}

/// Групповые ресурсы (SECTION 4 GROUP_MODES.md)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroupResources {
    pub llm_budget: HashMap<String, LlmAllocation>,
    pub active_bridges: Vec<String>,
    pub databases: Vec<DbConnection>,
    pub agents: Vec<AgentAssignment>,
    pub task_storage: Option<StorageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAllocation {
    pub node_id: String,
    pub model: String,
    pub priority: u8,
    pub boost: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAssignment {
    pub agent_id: String,
    pub node_id: String,
    pub role_in_task: String,
    pub personal_resources: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConnection {
    pub name: String,
    pub db_type: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub storage_type: String,
    pub uri: String,
}

/// Сообщение в групповом mpsc канале
#[derive(Debug, Clone)]
pub struct GroupMessage {
    pub from_node: String,
    pub msg_type: String,
    pub content: String,
}

/// Ресурс группы (скил или агент, разрешённый как общий)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedResource {
    pub resource_type: String,
    pub name: String,
    pub owner_node: String,
    pub description: String,
    pub allowed_groups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub node_id: String,
    pub role: String,
    pub joined_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub name: String,
    pub visibility: String,
    pub token: String,
    pub mode: GroupMode,
    pub created_at: String,
    pub created_by: String,
    pub members: Vec<Member>,
    pub channels: Vec<String>,
    pub resources: GroupResources,
    #[serde(default)]
    pub shared_skills: Vec<SharedResource>,
    #[serde(default)]
    pub shared_agents: Vec<SharedResource>,
    #[serde(default)]
    pub shared_bridges: Vec<SharedResource>,
    #[serde(default)]
    pub shared_services: Vec<SharedResource>,
}

/// Группа с mpsc каналом для in-process общения
pub struct Group {
    pub info: GroupInfo,
    pub tx: mpsc::Sender<GroupMessage>,
    pub rx: mpsc::Receiver<GroupMessage>,
    /// Участвует ли нода в общем mpsc группы
    pub shared: bool,
}

pub struct GroupManager {
    groups: HashMap<String, Group>,
    node_id: String,
}

/// Сколько mpsc каналов на одну группу:
///   1 tx/rx пара на группу (in-process внутри ноды)
///   1 gossip proxy (cross-node, опционально)
///
/// Топология:
///   Нода А                  Нода Б
///   ┌────────────────┐      ┌────────────────┐
///   │ Group "project"│      │ Group "project"│
///   │  tx ──mpsc──►rx│      │  tx ──mpsc──►rx│
///   │  │ shared=yes  │      │  │ shared=yes  │
///   │  │             │      │  │             │
///   │  └──gossip─────┼──────┼──►gossip       │
///   │     proxy      │      │    proxy       │
///   └────────────────┘      └────────────────┘
///
/// Если нода НЕ делится (shared=false):
///   - её mpsc работает только для локальных sub-agent'ов
///   - gossip proxy не пересылает её сообщения другим нодам
///   - её ресурсы не видны в общем пуле группы

impl GroupManager {
    pub fn new(node_id: &str) -> Self {
        GroupManager {
            groups: HashMap::new(),
            node_id: node_id.to_string(),
        }
    }

    pub fn create(&mut self, name: &str, visibility: &str) -> anyhow::Result<GroupInfo> {
        if self.groups.contains_key(name) {
            return Err(anyhow::anyhow!("Group '{}' already exists", name));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let token = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(256);

        let info = GroupInfo {
            name: name.to_string(),
            visibility: visibility.to_string(),
            token: token.clone(),
            mode: GroupMode::Watch,
            created_at: now.clone(),
            created_by: self.node_id.clone(),
            members: vec![Member {
                node_id: self.node_id.clone(),
                role: "lead".into(),
                joined_at: now.clone(),
            }],
            channels: vec![
                format!("{}.commands", name),
                format!("{}.data", name),
            ],
            resources: GroupResources::default(),
            shared_skills: vec![],
            shared_agents: vec![],
            shared_bridges: vec![],
            shared_services: vec![],
        };

        let g = Group { info: info.clone(), tx, rx, shared: false };
        info!("Group '{}' created (visibility: {}) by {}", name, visibility, self.node_id);
        self.groups.insert(name.to_string(), g);
        Ok(info)
    }

    /// Включить общий доступ ноды в группе (shared mpsc)
    pub fn enable_sharing(&mut self, group: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;
        g.shared = true;
        info!("Node {} enabled sharing in group '{}'", self.node_id, group);
        Ok(())
    }

    /// Отключить общий доступ ноды в группе
    pub fn disable_sharing(&mut self, group: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;
        g.shared = false;
        info!("Node {} disabled sharing in group '{}'", self.node_id, group);
        Ok(())
    }

    /// Проверить, делится ли нода в группе
    pub fn is_shared(&self, group: &str) -> bool {
        self.groups.get(group).map(|g| g.shared).unwrap_or(false)
    }

    /// Поделиться скилом с группой (только если shared=true)
    pub fn share_skill(&mut self, group: &str, skill_name: &str, description: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;

        let resource = SharedResource {
            resource_type: "skill".into(),
            name: skill_name.to_string(),
            owner_node: self.node_id.clone(),
            description: description.to_string(),
            allowed_groups: vec![group.to_string()],
        };

        if !g.info.shared_skills.iter().any(|s| s.name == skill_name) {
            g.info.shared_skills.push(resource.clone());
            let msg = GroupMessage {
                from_node: self.node_id.clone(),
                msg_type: "skill_shared".into(),
                content: skill_name.to_string(),
            };
            let _ = g.tx.try_send(msg);
            info!("Skill '{}' shared with group '{}'", skill_name, group);
        }
        Ok(())
    }

    /// Поделиться агентом с группой
    pub fn share_agent(&mut self, group: &str, agent_name: &str, role: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;

        let resource = SharedResource {
            resource_type: "agent".into(),
            name: agent_name.to_string(),
            owner_node: self.node_id.clone(),
            description: role.to_string(),
            allowed_groups: vec![group.to_string()],
        };

        if !g.info.shared_agents.iter().any(|a| a.name == agent_name) {
            g.info.shared_agents.push(resource.clone());
            let msg = GroupMessage {
                from_node: self.node_id.clone(),
                msg_type: "agent_shared".into(),
                content: agent_name.to_string(),
            };
            let _ = g.tx.try_send(msg);
            info!("Agent '{}' shared with group '{}'", agent_name, group);
        }
        Ok(())
    }

    /// Поделиться поисковым бриджем с группой (duckduckgo, yandex, baidu)
    pub fn share_bridge(&mut self, group: &str, bridge_name: &str, description: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;

        let resource = SharedResource {
            resource_type: "bridge".into(),
            name: bridge_name.to_string(),
            owner_node: self.node_id.clone(),
            description: description.to_string(),
            allowed_groups: vec![group.to_string()],
        };

        if !g.info.shared_bridges.iter().any(|b| b.name == bridge_name) {
            g.info.shared_bridges.push(resource.clone());
            let msg = GroupMessage {
                from_node: self.node_id.clone(),
                msg_type: "bridge_shared".into(),
                content: bridge_name.to_string(),
            };
            let _ = g.tx.try_send(msg);
            info!("Bridge '{}' shared with group '{}'", bridge_name, group);
        }
        Ok(())
    }

    /// Поделиться AI-сервисом с группой (notebooklm, chromadb, obsidian)
    pub fn share_service(&mut self, group: &str, service_name: &str, node_key: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;

        let resource = SharedResource {
            resource_type: "service".into(),
            name: service_name.to_string(),
            owner_node: self.node_id.clone(),
            description: format!("provided by {}: {}", self.node_id, node_key),
            allowed_groups: vec![group.to_string()],
        };

        if !g.info.shared_services.iter().any(|s| s.name == service_name) {
            g.info.shared_services.push(resource.clone());
            let msg = GroupMessage {
                from_node: self.node_id.clone(),
                msg_type: "service_shared".into(),
                content: format!("{} ({})", service_name, node_key),
            };
            let _ = g.tx.try_send(msg);
            info!("Service '{}' shared with group '{}' via {}", service_name, group, node_key);
        }
        Ok(())
    }

    /// Отправить сообщение в групповой mpsc
    pub fn send_to_group(&self, group: &str, msg: GroupMessage) -> bool {
        self.groups.get(group).map(|g| g.tx.try_send(msg).is_ok()).unwrap_or(false)
    }

    /// Получить tx канал группы (для клонирования в sub-agent'ы)
    pub fn get_tx(&self, group: &str) -> Option<mpsc::Sender<GroupMessage>> {
        self.groups.get(group).map(|g| g.tx.clone())
    }

    /// Set group mode (Storm/Hunt/Synthesis/Focus/Watch)
    pub fn set_mode(&mut self, group: &str, mode: GroupMode) -> anyhow::Result<GroupMode> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;
        let old = g.info.mode;
        g.info.mode = mode;
        info!("Group '{}' mode: {} → {}", group, old, mode);
        let msg = GroupMessage {
            from_node: self.node_id.clone(),
            msg_type: "mode_change".into(),
            content: format!("{} → {}", old, mode),
        };
        let _ = g.tx.try_send(msg);
        Ok(mode)
    }

    /// Advance to next mode in lifecycle (Watch→Storm→Hunt→Synthesis→Focus→Watch)
    pub fn advance_mode(&mut self, group: &str) -> anyhow::Result<GroupMode> {
        let mode = self.groups.get(group).map(|g| g.info.mode)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;
        let next = mode.next_after_completion();
        self.set_mode(group, next)
    }

    pub fn add_member(&mut self, group: &str, node_id: &str, role: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;

        if g.info.members.iter().any(|m| m.node_id == node_id) {
            return Err(anyhow::anyhow!("Node {} already in group '{}'", node_id, group));
        }

        g.info.members.push(Member {
            node_id: node_id.to_string(),
            role: role.to_string(),
            joined_at: chrono::Utc::now().to_rfc3339(),
        });

        info!("Node {} joined group '{}' as {}", node_id, group, role);
        Ok(())
    }

    pub fn remove_member(&mut self, group: &str, node_id: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;

        let len_before = g.info.members.len();
        g.info.members.retain(|m| m.node_id != node_id);

        if g.info.members.len() == len_before {
            return Err(anyhow::anyhow!("Node {} not in group '{}'", node_id, group));
        }

        info!("Node {} removed from group '{}'", node_id, group);
        Ok(())
    }

    pub fn set_role(&mut self, group: &str, node_id: &str, role: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;

        if let Some(member) = g.info.members.iter_mut().find(|m| m.node_id == node_id) {
            member.role = role.to_string();
            info!("Node {} role in '{}' set to {}", node_id, group, role);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Node {} not in group '{}'", node_id, group))
        }
    }

    pub fn add_channel(&mut self, group: &str, channel: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;
        if !g.info.channels.contains(&channel.to_string()) {
            g.info.channels.push(channel.to_string());
            info!("Channel '{}' added to group '{}'", channel, group);
        }
        Ok(())
    }

    pub fn set_visibility(&mut self, group: &str, visibility: &str) -> anyhow::Result<()> {
        let g = self.groups.get_mut(group)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group))?;
        g.info.visibility = visibility.to_string();
        info!("Group '{}' visibility set to {}", group, visibility);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&GroupInfo> {
        self.groups.get(name).map(|g| &g.info)
    }

    pub fn list(&self) -> Vec<GroupInfo> {
        self.groups.values().map(|g| g.info.clone()).collect()
    }

    pub fn list_names(&self) -> Vec<String> {
        self.groups.keys().cloned().collect()
    }

    pub fn exists(&self, name: &str) -> bool {
        self.groups.contains_key(name)
    }

    pub fn is_member(&self, group: &str, node_id: &str) -> bool {
        self.groups.get(group)
            .map(|g| g.info.members.iter().any(|m| m.node_id == node_id))
            .unwrap_or(false)
    }

    pub fn save_state(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let infos: HashMap<String, GroupInfo> = self.groups.iter().map(|(k, g)| (k.clone(), g.info.clone())).collect();
        let json = serde_json::to_string_pretty(&infos)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_state(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let infos: HashMap<String, GroupInfo> = serde_json::from_str(&content)?;
            for (name, info) in infos {
                let (tx, rx) = mpsc::channel(256);
                self.groups.insert(name, Group { info, tx, rx, shared: false });
            }
            info!("Groups loaded from {}", path.display());
        }
        Ok(())
    }

    pub fn channel_group(&self, channel: &str) -> Option<String> {
        for (name, g) in &self.groups {
            if g.info.channels.iter().any(|c| c == channel) {
                return Some(name.clone());
            }
        }
        None
    }
}
