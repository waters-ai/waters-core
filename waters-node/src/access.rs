/// Role-based access control — кто и что может делать на ноде

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    Admin,      // полный доступ
    Developer,  // разрабатывает агентов и скилы
    Manager,    // управляет задачами, смотрит статус
    Programmer, // пишет код, запускает агентов
    Viewer,     // только чтение
    Farmer,     // полевой агент — свои команды
}

impl Role {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "admin" => Role::Admin,
            "developer" | "dev" => Role::Developer,
            "manager" | "mgr" => Role::Manager,
            "programmer" | "prog" => Role::Programmer,
            "viewer" | "view" => Role::Viewer,
            "farmer" | "field" => Role::Farmer,
            _ => Role::Viewer,
        }
    }

    pub fn can_manage_agents(&self) -> bool {
        matches!(self, Role::Admin | Role::Developer | Role::Programmer)
    }

    pub fn can_manage_skills(&self) -> bool {
        matches!(self, Role::Admin | Role::Developer)
    }

    pub fn can_manage_peers(&self) -> bool {
        matches!(self, Role::Admin | Role::Manager)
    }

    pub fn can_use_voice(&self) -> bool {
        matches!(self, Role::Admin | Role::Developer | Role::Manager | Role::Programmer | Role::Farmer)
    }

    pub fn can_view(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessUser {
    pub name: String,
    pub role: Role,
    pub token_hash: String,
    pub channels: Vec<String>,
    pub allowed_ips: Vec<String>,
}

pub struct AccessControl {
    users: HashMap<String, AccessUser>,
    tokens: HashMap<String, String>, // token -> username
    path: PathBuf,
    dashboard_password: String,
}

impl AccessControl {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("access.json");
        let (users, tokens, dashboard_password) = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                        let u: HashMap<String, AccessUser> = serde_json::from_value(data["users"].clone()).unwrap_or_default();
                        let t: HashMap<String, String> = serde_json::from_value(data["tokens"].clone()).unwrap_or_default();
                        let pwd = data["dashboard_password"].as_str().unwrap_or("admin").to_string();
                        (u, t, pwd)
                    } else {
                        (HashMap::new(), HashMap::new(), "admin".into())
                    }
                }
                Err(_) => (HashMap::new(), HashMap::new(), "admin".into()),
            }
        } else {
            let mut u = HashMap::new();
            let mut t = HashMap::new();
            u.insert("admin".into(), AccessUser {
                name: "admin".into(),
                role: Role::Admin,
                token_hash: "admin-token".into(),
                channels: vec!["*".into()],
                allowed_ips: vec!["127.0.0.1".into(), "::1".into()],
            });
            t.insert("admin-token".into(), "admin".into());
            (u, t, "admin".into())
        };

        AccessControl { users, tokens, path, dashboard_password }
    }

    pub fn authenticate(&self, token: &str) -> Option<&AccessUser> {
        self.tokens.get(token).and_then(|name| self.users.get(name))
    }

    pub fn check_dashboard(&self, password: &str) -> bool {
        password == self.dashboard_password
    }

    pub fn add_user(&mut self, name: &str, role: &str, token: &str, channels: &[String]) {
        let user = AccessUser {
            name: name.to_string(),
            role: Role::parse(role),
            token_hash: token.to_string(),
            channels: channels.to_vec(),
            allowed_ips: vec![],
        };
        self.users.insert(name.to_string(), user);
        self.tokens.insert(token.to_string(), name.to_string());
        self.save();
        info!("AccessControl: added user '{}' as {:?}", name, Role::parse(role));
    }

    pub fn can_access_channel(&self, username: &str, channel: &str) -> bool {
        if let Some(user) = self.users.get(username) {
            user.channels.contains(&"*".to_string()) || user.channels.contains(&channel.to_string())
        } else {
            false
        }
    }

    pub fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let data = serde_json::json!({
            "users": self.users,
            "tokens": self.tokens,
            "dashboard_password": self.dashboard_password,
        });
        let _ = fs::write(&self.path, serde_json::to_string_pretty(&data).unwrap_or_default());
    }

    pub fn summary(&self) -> String {
        let mut out = format!("🔐 Доступ: {} пользователей\n", self.users.len());
        for (name, user) in &self.users {
            out.push_str(&format!("  {} — {:?} (каналы: {})\n",
                name, user.role, user.channels.join(", ")));
        }
        out
    }
}
