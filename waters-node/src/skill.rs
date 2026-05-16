use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub bridges: Vec<String>,
    #[serde(default)]
    pub bookmarks: Vec<SkillBookmark>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub output_types: Vec<String>,
    #[serde(default)]
    pub imported_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_llm_preferred")]
    pub preferred: String,
    #[serde(default = "default_llm_min")]
    pub min: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
}

fn default_llm_preferred() -> String {
    "deepseek-v4-flash".into()
}
fn default_llm_min() -> String {
    "0.5b".into()
}

impl Default for LlmConfig {
    fn default() -> Self {
        LlmConfig {
            preferred: default_llm_preferred(),
            min: default_llm_min(),
            alternatives: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBookmark {
    pub description: String,
    pub test: String,
    pub expected: String,
}

#[derive(Clone)]
pub struct Skill {
    pub manifest: SkillManifest,
    pub prompt: String,
    pub path: PathBuf,
}

impl Skill {
    pub fn summary_for_llm(&self) -> String {
        format!(
            "[{}] {} — {} (role: {}, llm: {}, bridges: {})",
            self.manifest.category,
            self.manifest.name,
            self.manifest.description,
            self.manifest.role,
            self.manifest.llm.preferred,
            self.manifest.bridges.join(", "),
        )
    }
}

#[derive(Clone)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        SkillRegistry {
            skills: HashMap::new(),
        }
    }

    pub fn load_from(&mut self, dir: &Path) -> usize {
        if !dir.exists() {
            info!("Skills directory not found: {:?}", dir);
            return 0;
        }

        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                count += self.load_dir(&path, dir);
            }
        }

        info!("Loaded {} skills from {:?}", count, dir);
        count
    }

    fn load_dir(&mut self, dir: &Path, root: &Path) -> usize {
        let mut count = 0;

        let manifest_path = dir.join("skill.json");
        let skill_path = dir.join("SKILL.md");

        if manifest_path.exists() && skill_path.exists() {
            if let Some(skill) = Self::load_skills20(&manifest_path, &skill_path, root, dir) {
                let name = skill.manifest.name.clone();
                self.skills.insert(name.clone(), skill);
                count += 1;
            }
        } else if skill_path.exists() {
            if let Some(skill) = Self::load_tui_format(&skill_path, dir, root) {
                let name = skill.manifest.name.clone();
                self.skills.insert(name.clone(), skill);
                count += 1;
            }
        } else {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let sub = entry.path();
                    if sub.is_dir() {
                        count += self.load_dir(&sub, root);
                    }
                }
            }
        }

        count
    }

    fn load_skills20(
        manifest_path: &Path,
        skill_path: &Path,
        root: &Path,
        dir: &Path,
    ) -> Option<Skill> {
        let manifest_content = std::fs::read_to_string(manifest_path).ok()?;
        let mut manifest: SkillManifest = serde_json::from_str(&manifest_content).ok()?;
        let prompt = std::fs::read_to_string(skill_path).ok()?;

        if manifest.category.is_empty() {
            manifest.category = dir
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
        }

        Some(Skill {
            manifest,
            prompt,
            path: skill_path.to_path_buf(),
        })
    }

    fn load_tui_format(file_path: &Path, dir: &Path, root: &Path) -> Option<Skill> {
        let content = std::fs::read_to_string(file_path).ok()?;
        if !content.starts_with("---") {
            return None;
        }

        let end = content[3..].find("---")?;
        let yaml_part = &content[3..3 + end];
        let prompt = content[3 + end + 3..].trim().to_string();

        let name = extract_yaml(yaml_part, "name").unwrap_or_else(|| {
            dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });
        let description = extract_yaml(yaml_part, "description").unwrap_or_default();
        let role = extract_yaml(yaml_part, "role").unwrap_or_else(|| "general".into());

        let category = dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let manifest = SkillManifest {
            name,
            version: extract_yaml(yaml_part, "version").unwrap_or_else(|| "1.0.0".into()),
            description,
            author: extract_yaml(yaml_part, "author"),
            tags: vec![],
            dependencies: vec![],
            bridges: vec![],
            bookmarks: vec![],
            category,
            role,
            llm: LlmConfig::default(),
            tools: vec![],
            output_types: vec![],
            imported_from: Some("tui".into()),
        };

        Some(Skill {
            manifest,
            prompt,
            path: file_path.to_path_buf(),
        })
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn get_prompt(&self, name: &str) -> Option<&str> {
        self.skills.get(name).map(|s| s.prompt.as_str())
    }

    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.manifest.tags.contains(&tag.to_string()))
            .collect()
    }

    pub fn by_bridge(&self, bridge: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.manifest.bridges.contains(&bridge.to_string()))
            .collect()
    }

    pub fn by_category(&self, category: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.manifest.category == category)
            .collect()
    }

    pub fn by_role(&self, role: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.manifest.role == role)
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<&Skill> {
        let q = query.to_lowercase();
        self.skills
            .values()
            .filter(|s| {
                s.manifest.name.to_lowercase().contains(&q)
                    || s.manifest.description.to_lowercase().contains(&q)
                    || s.manifest
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&q))
                    || s.manifest.role.contains(&q)
                    || s.manifest.category.contains(&q)
            })
            .collect()
    }

    pub fn summary_for_llm(&self) -> String {
        let mut out = String::from("Доступные агенты (SKILL.md):\n");
        let mut skills: Vec<&Skill> = self.skills.values().collect();
        skills.sort_by_key(|s| format!("{}/{}", s.manifest.category, s.manifest.name));
        for s in &skills {
            out.push_str(&format!("  {}\n", s.summary_for_llm()));
        }
        out
    }

    pub fn add(&mut self, name: &str, prompt: &str, description: &str) {
        let manifest = SkillManifest {
            name: name.to_string(),
            version: "1.0.0".into(),
            description: description.to_string(),
            author: Some("user".into()),
            tags: vec![],
            dependencies: vec![],
            bridges: vec![],
            bookmarks: vec![],
            category: String::new(),
            role: String::new(),
            llm: LlmConfig::default(),
            tools: vec![],
            output_types: vec![],
            imported_from: None,
        };
        let skill = Skill {
            manifest,
            prompt: prompt.to_string(),
            path: PathBuf::new(),
        };
        self.skills.insert(name.to_string(), skill);
    }

    pub fn create_from_manifest(&mut self, manifest: SkillManifest, prompt: &str) {
        let name = manifest.name.clone();
        let skill = Skill {
            manifest,
            prompt: prompt.to_string(),
            path: PathBuf::new(),
        };
        self.skills.insert(name, skill);
    }
}

fn extract_yaml(yaml: &str, key: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(val) = line.trim().strip_prefix(&format!("{}:", key)) {
            let val = val.trim().trim_matches('"');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

pub fn merge_agents(
    registry: &mut SkillRegistry,
    name1: &str,
    name2: &str,
    agents_dir: &Path,
) -> Result<String> {
    let s1 = registry
        .get(name1)
        .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", name1))?;
    let s2 = registry
        .get(name2)
        .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", name2))?;

    let m1 = &s1.manifest;
    let m2 = &s2.manifest;

    let merged_name = format!("merged-{}-{}", m1.name, m2.name);

    let mut merged_bridges = m1.bridges.clone();
    for b in &m2.bridges {
        if !merged_bridges.contains(b) {
            merged_bridges.push(b.clone());
        }
    }

    let mut merged_tools = m1.tools.clone();
    for t in &m2.tools {
        if !merged_tools.contains(t) {
            merged_tools.push(t.clone());
        }
    }

    let mut merged_outputs = m1.output_types.clone();
    for o in &m2.output_types {
        if !merged_outputs.contains(o) {
            merged_outputs.push(o.clone());
        }
    }

    let llm_preferred = if m1.llm.preferred.contains("pro") || m2.llm.preferred.contains("pro") {
        "deepseek-v4-pro".to_string()
    } else {
        m1.llm.preferred.clone()
    };

    let description = format!(
        "Объединение {} и {}. Умеет: {}. Вырос из: {} + {}.",
        m1.description,
        m2.description,
        merged_bridges.join(", "),
        m1.name,
        m2.name,
    );

    let merged_dir = agents_dir.join("merged").join(&merged_name);
    std::fs::create_dir_all(&merged_dir)?;

    let merged_manifest = SkillManifest {
        name: merged_name.clone(),
        version: "1.0.0".into(),
        description,
        author: Some("merged".into()),
        tags: {
            let mut t = m1.tags.clone();
            for tag in &m2.tags {
                if !t.contains(tag) {
                    t.push(tag.clone());
                }
            }
            t
        },
        dependencies: vec![m1.name.clone(), m2.name.clone()],
        bridges: merged_bridges,
        bookmarks: vec![],
        category: "agents/merged".into(),
        role: format!("merged"),
        llm: LlmConfig {
            preferred: llm_preferred,
            min: "0.5b".into(),
            alternatives: vec![],
        },
        tools: merged_tools,
        output_types: merged_outputs,
        imported_from: None,
    };

    let prompt = format!(
        r#"---
name: {}
role: merged
description: {}
---

# {} — объединённый агент

Ты создан из двух агентов: **{}** и **{}**.

## Что ты умеешь

- **{}**: {}
- **{}**: {}

## Твои инструменты

- Bridges: {}
- Tools: {}

## Output types

{}

## Наследие

Ты знаешь то, что знали твои предшественники.
Сохраняй их лучшие практики, объединяй их знания.
"#,
        merged_name,
        merged_manifest.description,
        merged_name,
        m1.name,
        m2.name,
        m1.name,
        m1.description,
        m2.name,
        m2.description,
        merged_manifest.bridges.join(", "),
        merged_manifest.tools.join(", "),
        merged_manifest.output_types.join(", "),
    );

    let json_path = merged_dir.join("skill.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&merged_manifest)?)?;

    let md_path = merged_dir.join("SKILL.md");
    std::fs::write(&md_path, &prompt)?;

    registry.create_from_manifest(merged_manifest, &prompt);

    info!(
        "Merged agents '{}' + '{}' → '{}'",
        name1, name2, merged_name
    );
    Ok(merged_name)
}

pub fn convert_tui_to_our(tui_skill_path: &Path) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(tui_skill_path)?;
    if !content.starts_with("---") {
        return Err(anyhow::anyhow!("Not a TUI SKILL.md format"));
    }

    let end = content[3..]
        .find("---")
        .ok_or_else(|| anyhow::anyhow!("No closing ---"))?;
    let yaml_part = &content[3..3 + end];

    let name =
        extract_yaml(yaml_part, "name").ok_or_else(|| anyhow::anyhow!("No name in frontmatter"))?;
    let description = extract_yaml(yaml_part, "description").unwrap_or_default();
    let role = extract_yaml(yaml_part, "role").unwrap_or_else(|| "general".into());
    let prompt = content[3 + end + 3..].trim().to_string();

    let skill_dir = tui_skill_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine parent dir"))?;

    let category = skill_dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let manifest = SkillManifest {
        name: name.clone(),
        version: "1.0.0".into(),
        description,
        author: Some("converted".into()),
        tags: vec![],
        dependencies: vec![],
        bridges: vec![],
        bookmarks: vec![],
        category,
        role,
        llm: LlmConfig::default(),
        tools: vec![],
        output_types: vec![],
        imported_from: Some("tui".into()),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    let json_path = skill_dir.join("skill.json");
    std::fs::write(&json_path, manifest_json)?;

    info!(
        "Converted TUI skill '{}' to Skills 2.0 format at {:?}",
        name, json_path
    );
    Ok(())
}
