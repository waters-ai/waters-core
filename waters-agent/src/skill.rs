use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub dependencies: Vec<String>,
    pub kafka_topics: Option<SkillTopics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTopics {
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
}

pub struct Skill {
    pub manifest: SkillManifest,
    pub path: std::path::PathBuf,
}

pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        SkillRegistry {
            skills: HashMap::new(),
        }
    }

    pub fn load_from(&mut self, dir: &Path) -> Result<usize> {
        let mut count = 0;
        if !dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let skill_path = path.join("SKILL.md");
                let manifest_path = path.join("skill.json");
                if skill_path.exists() && manifest_path.exists() {
                    let content = std::fs::read_to_string(&manifest_path)?;
                    if let Ok(manifest) = serde_json::from_str::<SkillManifest>(&content) {
                        let name = manifest.name.clone();
                        self.skills.insert(name.clone(), Skill {
                            manifest,
                            path: skill_path,
                        });
                        count += 1;
                    }
                }
            }
        }

        info!("Loaded {} skills from {:?}", count, dir);
        Ok(count)
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn list(&self) -> Vec<&str> {
        self.skills.keys().map(|k| k.as_str()).collect()
    }

    pub fn find_by_topic(&self, topic: &str) -> Vec<&Skill> {
        self.skills.values().filter(|s| {
            s.manifest.kafka_topics.as_ref().map_or(false, |t| {
                t.consumes.contains(&topic.to_string()) || t.produces.contains(&topic.to_string())
            })
        }).collect()
    }
}
