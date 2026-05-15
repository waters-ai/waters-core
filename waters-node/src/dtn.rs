use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtnProfile {
    pub name: String,
    pub delay_ms: u32,
    pub jitter_ms: u32,
    pub loss_percent: f32,
}

pub struct DtnEngine {
    profiles: HashMap<String, DtnProfile>,
    current_profile: Option<String>,
    interface: String,
}

impl DtnEngine {
    pub fn new(interface: &str) -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(
            "field".to_string(),
            DtnProfile {
                name: "field".into(),
                delay_ms: 100,
                jitter_ms: 10,
                loss_percent: 0.0,
            },
        );
        profiles.insert(
            "lunar".to_string(),
            DtnProfile {
                name: "lunar".into(),
                delay_ms: 1300,
                jitter_ms: 50,
                loss_percent: 0.1,
            },
        );
        profiles.insert(
            "martian".to_string(),
            DtnProfile {
                name: "martian".into(),
                delay_ms: 300_000,
                jitter_ms: 60_000,
                loss_percent: 1.0,
            },
        );
        profiles.insert(
            "iss".to_string(),
            DtnProfile {
                name: "iss".into(),
                delay_ms: 500,
                jitter_ms: 100,
                loss_percent: 0.1,
            },
        );

        DtnEngine {
            profiles,
            current_profile: None,
            interface: interface.to_string(),
        }
    }

    pub fn apply(&mut self, profile_name: &str) -> Result<()> {
        let profile = self
            .profiles
            .get(profile_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown DTN profile: {}", profile_name))?;

        let status = std::process::Command::new("tc")
            .args([
                "qdisc",
                "replace",
                "dev",
                &self.interface,
                "root",
                "netem",
                "delay",
                &format!("{}ms", profile.delay_ms),
                &format!("{}ms", profile.jitter_ms),
                "loss",
                &format!("{}%", profile.loss_percent),
            ])
            .status()?;

        if status.success() {
            self.current_profile = Some(profile_name.to_string());
            info!(
                "DTN profile '{}' applied: {}ms delay, {}ms jitter, {}% loss",
                profile_name, profile.delay_ms, profile.jitter_ms, profile.loss_percent
            );
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to apply tc-netem (need root): {}",
                profile_name
            ))
        }
    }

    pub fn remove(&mut self) -> Result<()> {
        let status = std::process::Command::new("tc")
            .args(["qdisc", "delete", "dev", &self.interface, "root"])
            .status()?;

        if status.success() {
            info!("DTN removed from {}", self.interface);
            self.current_profile = None;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to remove tc-netem"))
        }
    }

    #[allow(dead_code)]
    pub fn current(&self) -> Option<&str> {
        self.current_profile.as_deref()
    }

    #[allow(dead_code)]
    pub fn profiles(&self) -> &HashMap<String, DtnProfile> {
        &self.profiles
    }
}
