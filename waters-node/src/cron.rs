use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub name: String,
    pub schedule: String,
    pub action: String,
    pub skill: Option<String>,
    pub agent: Option<String>,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronConfig {
    pub jobs: Vec<CronJob>,
}

impl Default for CronConfig {
    fn default() -> Self {
        CronConfig {
            jobs: vec![
                CronJob {
                    name: "daily-report".into(),
                    schedule: "0 8 * * *".into(),
                    action: "generate daily report".into(),
                    skill: Some("general".into()),
                    agent: None,
                    enabled: false,
                    last_run: None,
                    next_run: None,
                },
                CronJob {
                    name: "weekly-audit".into(),
                    schedule: "0 9 * * 1".into(),
                    action: "perform weekly security audit".into(),
                    skill: Some("general".into()),
                    agent: None,
                    enabled: false,
                    last_run: None,
                    next_run: None,
                },
            ],
        }
    }
}

pub struct CronEngine {
    config_path: PathBuf,
    config: CronConfig,
    running: bool,
}

impl CronEngine {
    pub fn new(config_dir: &Path) -> Self {
        let config_path = config_dir.join("cron.toml");
        let config = if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => toml::from_str(&content).unwrap_or_default(),
                Err(_) => CronConfig::default(),
            }
        } else {
            CronConfig::default()
        };

        CronEngine {
            config_path,
            config,
            running: false,
        }
    }

    pub fn load(&mut self) -> Result<()> {
        if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path)?;
            self.config = toml::from_str(&content)?;
            info!("CronEngine: loaded {} jobs", self.config.jobs.len());
        }
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(&self.config)?;
        fs::write(&self.config_path, content)?;
        info!("CronEngine: saved {} jobs", self.config.jobs.len());
        Ok(())
    }

    pub fn add_job(&mut self, job: CronJob) {
        self.config.jobs.push(job);
        let _ = self.save();
    }

    pub fn remove_job(&mut self, name: &str) {
        self.config.jobs.retain(|j| j.name != name);
        let _ = self.save();
    }

    pub fn enable_job(&mut self, name: &str) {
        for job in &mut self.config.jobs {
            if job.name == name {
                job.enabled = true;
                break;
            }
        }
        let _ = self.save();
    }

    pub fn disable_job(&mut self, name: &str) {
        for job in &mut self.config.jobs {
            if job.name == name {
                job.enabled = false;
                break;
            }
        }
        let _ = self.save();
    }

    pub fn list_jobs(&self) -> &[CronJob] {
        &self.config.jobs
    }

    pub fn start(&mut self) {
        self.running = true;
        info!("CronEngine: started");
    }

    pub fn stop(&mut self) {
        self.running = false;
        info!("CronEngine: stopped");
    }

    pub fn parse_cron(expr: &str) -> Option<(u32, u32, u32, u32, u32)> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return None;
        }
        let minute = parts[0].parse().ok()?;
        let hour = parts[1].parse().ok()?;
        let day_of_month = if parts[2] == "*" { 1u32 } else { parts[2].parse().ok()? };
        let month = if parts[3] == "*" { 1u32 } else { parts[3].parse().ok()? };
        let day_of_week = if parts[4] == "*" { 7u32 } else { parts[4].parse().ok()? };
        Some((minute, hour, day_of_month, month, day_of_week))
    }
}

pub fn default_cron_config() -> String {
    let config = CronConfig::default();
    toml::to_string_pretty(&config).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cron() {
        let r = CronEngine::parse_cron("0 8 * * *");
        assert!(r.is_some());
        let (m, h, _, _, _) = r.unwrap();
        assert_eq!(m, 0);
        assert_eq!(h, 8);
    }

    #[test]
    fn test_default_config() {
        let config = CronConfig::default();
        assert_eq!(config.jobs.len(), 2);
        assert_eq!(config.jobs[0].name, "daily-report");
    }

    #[test]
    fn test_add_remove_job() {
        let mut engine = CronEngine::new(Path::new("/tmp/waters-cron-test"));
        engine.add_job(CronJob {
            name: "test-job".into(),
            schedule: "*/5 * * * *".into(),
            action: "test action".into(),
            skill: Some("general".into()),
            agent: None,
            enabled: true,
            last_run: None,
            next_run: None,
        });
        assert_eq!(engine.config.jobs.len(), 3);
        engine.remove_job("test-job");
        assert_eq!(engine.config.jobs.len(), 2);
    }
}
