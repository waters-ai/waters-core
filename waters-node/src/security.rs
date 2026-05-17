use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ShareScope {
    #[default]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
        info!(
            "SharePolicy: sharing bridge '{}' with group approval",
            bridge
        );
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
        self.policies.get(group).cloned().unwrap_or_default()
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

// ═══════════════════════════════════════════════════════════════
// Security Learning Engine — агент секьюрити учится на опыте
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SecurityEventKind {
    ShareApproved,
    ShareDenied,
    PeerConnected,
    PeerRejected,
    BridgeAccessed,
    DangerCommandBlocked,
    RatingThresholdCrossed,
    AnomalyDetected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub timestamp: String,
    pub kind: SecurityEventKind,
    pub peer_id: String,
    pub resource: String,
    pub details: String,
    pub risk_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerTrust {
    pub peer_id: String,
    pub successful_interactions: u32,
    pub failed_interactions: u32,
    pub total_shares: u32,
    pub last_seen: String,
    pub trust_level: TrustLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    Unknown,
    Low,
    Medium,
    High,
    Trusted,
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TrustLevel::Unknown => write!(f, "❓ Unknown"),
            TrustLevel::Low => write!(f, "⚠️ Low"),
            TrustLevel::Medium => write!(f, "🔶 Medium"),
            TrustLevel::High => write!(f, "🟢 High"),
            TrustLevel::Trusted => write!(f, "✅ Trusted"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedRule {
    pub pattern: String,
    pub action: String,
    pub confidence: f64,
    pub based_on_events: u32,
    pub auto_apply: bool,
}

pub struct SecurityLearner {
    events: Vec<SecurityEvent>,
    trust_map: HashMap<String, PeerTrust>,
    learned_rules: Vec<LearnedRule>,
    data_path: PathBuf,
    anomaly_threshold: f64,
    whitelist: HashSet<String>,
    blacklist: HashSet<String>,
    whitelist_file: PathBuf,
    blacklist_file: PathBuf,
}

impl SecurityLearner {
    pub fn new(data_path: &Path) -> Self {
        let full_path = data_path.join("security_learned.json");
        let wl_file = data_path.join("allow.txt");
        let bl_file = data_path.join("block.txt");
        let (events, trust_map, learned_rules, mut whitelist, mut blacklist) =
            load_state(&full_path);

        // Load user-editable text files — они приоритетнее JSON
        whitelist.extend(Self::load_list_file(&wl_file));
        blacklist.extend(Self::load_list_file(&bl_file));

        // Sync back — дописать в JSON то, чего там не было
        let mut learner = SecurityLearner {
            events,
            trust_map,
            learned_rules,
            data_path: full_path,
            anomaly_threshold: 0.7,
            whitelist,
            blacklist,
            whitelist_file: wl_file,
            blacklist_file: bl_file,
        };
        let _ = learner.save();
        learner
    }

    fn load_list_file(path: &Path) -> HashSet<String> {
        if !path.exists() {
            return HashSet::new();
        }
        match fs::read_to_string(path) {
            Ok(content) => content
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect(),
            Err(e) => {
                warn!("SecurityLearner: cant load list from {:?}: {}", path, e);
                HashSet::new()
            }
        }
    }

    fn save_list_file(path: &Path, list: &HashSet<String>) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = list
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = fs::write(path, content + "\n");
    }

    /// Sync whitelist from the text file (allow.txt) — user edits it by hand
    pub fn sync_whitelist(&mut self) {
        let file_list = Self::load_list_file(&self.whitelist_file);
        for item in file_list {
            self.whitelist.insert(item);
        }
        info!(
            "SecurityLearner: whitelist synced, {} entries",
            self.whitelist.len()
        );
    }

    /// Sync blacklist from the text file (block.txt) — user edits it by hand
    pub fn sync_blacklist(&mut self) {
        let file_list = Self::load_list_file(&self.blacklist_file);
        for item in file_list {
            self.blacklist.insert(item);
        }
        info!(
            "SecurityLearner: blacklist synced, {} entries",
            self.blacklist.len()
        );
    }

    /// Add to whitelist — сразу и в память, и в файл
    pub fn add_whitelist(&mut self, peer: &str) {
        self.whitelist.insert(peer.to_string());
        Self::save_list_file(&self.whitelist_file, &self.whitelist);
        info!(
            "SecurityLearner: whitelist +{} (now {})",
            peer,
            self.whitelist.len()
        );
    }

    /// Remove from whitelist
    pub fn remove_whitelist(&mut self, peer: &str) {
        self.whitelist.remove(peer);
        Self::save_list_file(&self.whitelist_file, &self.whitelist);
        info!(
            "SecurityLearner: whitelist -{} (now {})",
            peer,
            self.whitelist.len()
        );
    }

    /// Add to blacklist — сразу и в память, и в файл
    pub fn add_blacklist(&mut self, peer: &str) {
        self.blacklist.insert(peer.to_string());
        Self::save_list_file(&self.blacklist_file, &self.blacklist);
        info!(
            "SecurityLearner: blacklist +{} (now {})",
            peer,
            self.blacklist.len()
        );
    }

    /// Remove from blacklist
    pub fn remove_blacklist(&mut self, peer: &str) {
        self.blacklist.remove(peer);
        Self::save_list_file(&self.blacklist_file, &self.blacklist);
        info!(
            "SecurityLearner: blacklist -{} (now {})",
            peer,
            self.blacklist.len()
        );
    }

    pub fn is_whitelisted(&self, peer: &str) -> bool {
        self.whitelist.contains(peer) || self.trust_level(peer) == TrustLevel::Trusted
    }

    pub fn is_blacklisted(&self, peer: &str) -> bool {
        self.blacklist.contains(peer)
    }

    pub fn get_whitelist(&self) -> &HashSet<String> {
        &self.whitelist
    }
    pub fn get_blacklist(&self) -> &HashSet<String> {
        &self.blacklist
    }

    /// Переопределяем should_block — чёрный список имеет приоритет
    pub fn should_block_ext(&self, peer: &str, risk_score: u8) -> bool {
        if self.is_blacklisted(peer) {
            return true;
        }
        if self.is_whitelisted(peer) {
            return false;
        }
        self.should_block(peer, risk_score)
    }
}

fn load_state(
    path: &Path,
) -> (
    Vec<SecurityEvent>,
    HashMap<String, PeerTrust>,
    Vec<LearnedRule>,
    HashSet<String>,
    HashSet<String>,
) {
    if !path.exists() {
        return (
            Vec::new(),
            HashMap::new(),
            Vec::new(),
            HashSet::new(),
            HashSet::new(),
        );
    }
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(data) => (
                serde_json::from_value(data["events"].clone()).unwrap_or_default(),
                serde_json::from_value(data["trust_map"].clone()).unwrap_or_default(),
                serde_json::from_value(data["learned_rules"].clone()).unwrap_or_default(),
                serde_json::from_value(data["whitelist"].clone()).unwrap_or_default(),
                serde_json::from_value(data["blacklist"].clone()).unwrap_or_default(),
            ),
            Err(e) => {
                warn!("SecurityLearner: parse error: {}", e);
                (
                    Vec::new(),
                    HashMap::new(),
                    Vec::new(),
                    HashSet::new(),
                    HashSet::new(),
                )
            }
        },
        Err(e) => {
            warn!("SecurityLearner: cant load state: {}", e);
            (
                Vec::new(),
                HashMap::new(),
                Vec::new(),
                HashSet::new(),
                HashSet::new(),
            )
        }
    }
}

impl SecurityLearner {
    pub fn record_event(
        &mut self,
        kind: SecurityEventKind,
        peer: &str,
        resource: &str,
        details: &str,
        risk: u8,
    ) {
        let event = SecurityEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            kind,
            peer_id: peer.to_string(),
            resource: resource.to_string(),
            details: details.to_string(),
            risk_score: risk,
        };
        info!(
            "SecurityEvent: {:?} peer={} resource={} risk={}",
            kind, peer, resource, risk
        );
        self.events.push(event);

        // Update peer trust
        let trust = self.trust_map.entry(peer.to_string()).or_insert(PeerTrust {
            peer_id: peer.to_string(),
            successful_interactions: 0,
            failed_interactions: 0,
            total_shares: 0,
            last_seen: chrono::Utc::now().to_rfc3339(),
            trust_level: TrustLevel::Unknown,
        });
        trust.last_seen = chrono::Utc::now().to_rfc3339();

        match kind {
            SecurityEventKind::ShareApproved | SecurityEventKind::PeerConnected => {
                trust.successful_interactions += 1;
                trust.total_shares += 1;
            }
            SecurityEventKind::ShareDenied
            | SecurityEventKind::PeerRejected
            | SecurityEventKind::DangerCommandBlocked => {
                trust.failed_interactions += 1;
            }
            SecurityEventKind::AnomalyDetected => {
                trust.failed_interactions += 2;
            }
            _ => {}
        }

        // Recalculate trust level
        self.recalc_trust(peer);

        // Learn rules if enough events
        if self.events.len() % 10 == 0 {
            self.learn();
        }

        // Auto-save every 20 events
        if self.events.len() % 20 == 0 {
            let _ = self.save();
        }
    }

    fn recalc_trust(&mut self, peer: &str) {
        if let Some(trust) = self.trust_map.get_mut(peer) {
            let total = trust.successful_interactions + trust.failed_interactions;
            if total == 0 {
                return;
            }
            let ratio = trust.successful_interactions as f64 / total as f64;

            trust.trust_level = if trust.successful_interactions >= 50 && ratio > 0.95 {
                TrustLevel::Trusted
            } else if trust.successful_interactions >= 20 && ratio > 0.85 {
                TrustLevel::High
            } else if trust.successful_interactions >= 5 && ratio > 0.7 {
                TrustLevel::Medium
            } else if total > 3 {
                TrustLevel::Low
            } else {
                TrustLevel::Unknown
            };
        }
    }

    pub fn trust_level(&self, peer: &str) -> TrustLevel {
        self.trust_map
            .get(peer)
            .map(|t| t.trust_level)
            .unwrap_or(TrustLevel::Unknown)
    }

    pub fn should_auto_approve(&self, peer: &str) -> bool {
        self.trust_level(peer) == TrustLevel::Trusted || self.trust_level(peer) == TrustLevel::High
    }

    pub fn should_block(&self, peer: &str, risk_score: u8) -> bool {
        let level = self.trust_level(peer);
        if level == TrustLevel::Trusted {
            return false;
        }
        if level == TrustLevel::Unknown && risk_score > 5 {
            return true;
        }
        if level == TrustLevel::Low && risk_score > 3 {
            return true;
        }
        false
    }

    fn learn(&mut self) {
        // Pattern: если пир успешно шарит > 10 раз без инцидентов — повысить доверие
        for (peer, trust) in &self.trust_map {
            if trust.successful_interactions >= 10 && trust.failed_interactions == 0 {
                let rule_name = format!("auto-trust-{}", peer);
                let exists = self.learned_rules.iter().any(|r| r.pattern == rule_name);
                if !exists {
                    let rn = rule_name.clone();
                    self.learned_rules.push(LearnedRule {
                        pattern: rule_name,
                        action: format!("auto_approve {}", peer),
                        confidence: 0.9,
                        based_on_events: trust.successful_interactions + trust.failed_interactions,
                        auto_apply: true,
                    });
                    info!(
                        "SecurityLearner: learned rule '{}' — {} OK interactions, 0 incidents",
                        rn, trust.successful_interactions
                    );
                }
            }
        }

        // Pattern: если пир часто отклоняется — понизить
        for (peer, trust) in &self.trust_map {
            if trust.failed_interactions >= 3 && trust.successful_interactions == 0 {
                let rule_name = format!("block-{}", peer);
                let exists = self.learned_rules.iter().any(|r| r.pattern == rule_name);
                if !exists {
                    let rn = rule_name.clone();
                    self.learned_rules.push(LearnedRule {
                        pattern: rule_name,
                        action: format!("block {}", peer),
                        confidence: 0.6,
                        based_on_events: trust.failed_interactions,
                        auto_apply: false,
                    });
                    warn!("SecurityLearner: learned rule '{}' — {} failed interactions, suggest block", rn, trust.failed_interactions);
                }
            }
        }

        // Pattern: аномалия — пир с >10 failed за короткое время
        let recent_fails: Vec<&SecurityEvent> = self
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    SecurityEventKind::AnomalyDetected | SecurityEventKind::DangerCommandBlocked
                )
            })
            .collect();
        if recent_fails.len() >= 5 {
            let peers: Vec<&str> = recent_fails.iter().map(|e| e.peer_id.as_str()).collect();
            for peer in peers {
                let rule_name = format!("anomaly-block-{}", peer);
                let exists = self.learned_rules.iter().any(|r| r.pattern == rule_name);
                if !exists {
                    self.learned_rules.push(LearnedRule {
                        pattern: rule_name,
                        action: format!("auto_block {}", peer),
                        confidence: 0.8,
                        based_on_events: recent_fails.len() as u32,
                        auto_apply: true,
                    });
                    info!(
                        "SecurityLearner: anomaly detected for {}, auto-block applied",
                        peer
                    );
                }
            }
        }
    }

    pub fn get_rules(&self) -> &[LearnedRule] {
        &self.learned_rules
    }

    pub fn get_peers(&self) -> Vec<(&String, &PeerTrust)> {
        let mut peers: Vec<_> = self.trust_map.iter().collect();
        peers.sort_by_key(|(_, t)| std::cmp::Reverse(t.successful_interactions));
        peers
    }

    pub fn recent_events(&self, count: usize) -> Vec<&SecurityEvent> {
        self.events.iter().rev().take(count).collect()
    }

    pub fn summary(&self) -> String {
        let mut out = format!(
            "SecurityLearner: {} events, {} learned rules\n",
            self.events.len(),
            self.learned_rules.len()
        );
        out.push_str(&format!("  Known peers: {}\n", self.trust_map.len()));
        for (id, trust) in &self.trust_map {
            out.push_str(&format!(
                "    {} — {} (ok:{}, fail:{}, shares:{})\n",
                id,
                trust.trust_level,
                trust.successful_interactions,
                trust.failed_interactions,
                trust.total_shares
            ));
        }
        if !self.learned_rules.is_empty() {
            out.push_str("  Learned rules:\n");
            for rule in &self.learned_rules {
                out.push_str(&format!(
                    "    {} → {} (conf:{:.1}, events:{}, auto:{})\n",
                    rule.pattern,
                    rule.action,
                    rule.confidence,
                    rule.based_on_events,
                    rule.auto_apply
                ));
            }
        }
        out
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.data_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let state = serde_json::json!({
            "events": self.events,
            "trust_map": self.trust_map,
            "learned_rules": self.learned_rules,
            "whitelist": self.whitelist,
            "blacklist": self.blacklist,
        });
        fs::write(&self.data_path, serde_json::to_string_pretty(&state)?)?;
        // Sync to text files so user can edit them
        let wl_path = self.whitelist_file.clone();
        let bl_path = self.blacklist_file.clone();
        let wl = self.whitelist.clone();
        let bl = self.blacklist.clone();
        Self::save_list_file(&wl_path, &wl);
        Self::save_list_file(&bl_path, &bl);
        info!("SecurityLearner: state saved to {:?}", self.data_path);
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════
// Channel Isolation — разделение каналов по группам безопасности
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelAccess {
    Public,
    GroupOnly(Vec<String>),
    PeerList(Vec<String>),
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPolicy {
    pub name: String,
    pub access: ChannelAccess,
    pub encrypt: bool,
    pub max_peers: u8,
    pub audit: bool,
}

pub struct ChannelIsolation {
    channels: HashMap<String, ChannelPolicy>,
}

impl ChannelIsolation {
    pub fn new() -> Self {
        let mut channels = HashMap::new();
        channels.insert(
            "chat".into(),
            ChannelPolicy {
                name: "chat".into(),
                access: ChannelAccess::GroupOnly(vec![]),
                encrypt: false,
                max_peers: 100,
                audit: true,
            },
        );
        channels.insert(
            "findings".into(),
            ChannelPolicy {
                name: "findings".into(),
                access: ChannelAccess::GroupOnly(vec![]),
                encrypt: false,
                max_peers: 100,
                audit: true,
            },
        );
        channels.insert(
            "voice".into(),
            ChannelPolicy {
                name: "voice".into(),
                access: ChannelAccess::Public,
                encrypt: false,
                max_peers: 6,
                audit: false,
            },
        );
        channels.insert(
            "admin".into(),
            ChannelPolicy {
                name: "admin".into(),
                access: ChannelAccess::PeerList(vec![]),
                encrypt: true,
                max_peers: 3,
                audit: true,
            },
        );
        ChannelIsolation { channels }
    }

    pub fn create_channel(&mut self, name: &str, access: ChannelAccess, max_peers: u8) {
        let info_msg = format!("{:?}", &access);
        self.channels.insert(
            name.to_string(),
            ChannelPolicy {
                name: name.to_string(),
                access,
                encrypt: false,
                max_peers,
                audit: true,
            },
        );
        info!(
            "ChannelIsolation: created channel '{}' ({})",
            name, info_msg
        );
    }

    pub fn can_access(&self, channel: &str, peer: &str, group_token: Option<&str>) -> bool {
        let Some(policy) = self.channels.get(channel) else {
            return false;
        };
        match &policy.access {
            ChannelAccess::Public => true,
            ChannelAccess::Private => false,
            ChannelAccess::GroupOnly(groups) => {
                if let Some(token) = group_token {
                    groups.is_empty() || groups.contains(&token.to_string())
                } else {
                    false
                }
            }
            ChannelAccess::PeerList(peers) => peers.contains(&peer.to_string()),
        }
    }

    pub fn add_peer_to_channel(&mut self, channel: &str, peer: &str) {
        if let Some(policy) = self.channels.get_mut(channel) {
            match &mut policy.access {
                ChannelAccess::PeerList(peers) => {
                    if !peers.contains(&peer.to_string()) {
                        peers.push(peer.to_string());
                        info!("ChannelIsolation: added {} to channel '{}'", peer, channel);
                    }
                }
                _ => warn!(
                    "ChannelIsolation: cannot add peer to non-peerlist channel '{}'",
                    channel
                ),
            }
        }
    }

    pub fn remove_peer_from_channel(&mut self, channel: &str, peer: &str) {
        if let Some(policy) = self.channels.get_mut(channel) {
            match &mut policy.access {
                ChannelAccess::PeerList(peers) => {
                    peers.retain(|p| p != peer);
                }
                _ => {}
            }
        }
    }

    pub fn list_channels(&self) -> Vec<&ChannelPolicy> {
        self.channels.values().collect()
    }

    pub fn channel_summary(&self) -> String {
        let mut out = "🔒 Channel Isolation:\n".to_string();
        for ch in self.channels.values() {
            let icon = match ch.access {
                ChannelAccess::Public => "🌐",
                ChannelAccess::GroupOnly(_) => "👥",
                ChannelAccess::PeerList(_) => "🔐",
                ChannelAccess::Private => "🔒",
            };
            out.push_str(&format!(
                "  {} {} (peers:{}, audit:{})\n",
                icon, ch.name, ch.max_peers, ch.audit
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_access() {
        let ci = ChannelIsolation::new();
        assert!(ci.can_access("chat", "peer1", Some("token123")));
        assert!(!ci.can_access("admin", "stranger", None));
        assert!(ci.can_access("voice", "anyone", None));
    }
}
