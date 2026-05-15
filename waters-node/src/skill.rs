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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBookmark {
    pub description: String,
    pub test: String,
    pub expected: String,
}

pub struct Skill {
    pub manifest: SkillManifest,
    pub prompt: String,
    pub path: PathBuf,
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

    /// Загрузить все скилы из директории
    pub fn load_from(&mut self, dir: &Path) -> usize {
        if !dir.exists() {
            info!("Skills directory not found: {:?}", dir);
            return 0;
        }

        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }

                let skill_path = path.join("SKILL.md");
                let manifest_path = path.join("skill.json");

                if !skill_path.exists() {
                    // Попробовать прочитать как TUI-формат (один SKILL.md с frontmatter)
                    let single_file = path.join("SKILL.md");
                    if single_file.exists() {
                        if let Some(skill) = Self::load_tui_format(&single_file, &path) {
                            self.skills.insert(skill.manifest.name.clone(), skill);
                            count += 1;
                        }
                    }
                    continue;
                }

                if !manifest_path.exists() {
                    // Есть SKILL.md, но нет manifest — создаём базовый
                    if let Some(skill) = Self::load_skill_only(&skill_path, &path) {
                        let name = skill.manifest.name.clone();
                        self.skills.insert(name, skill);
                        count += 1;
                    }
                    continue;
                }

                // Полный формат: SKILL.md + skill.json
                match SkillRegistry::load_skills20(&manifest_path, &skill_path) {
                    Some(skill) => {
                        let name = skill.manifest.name.clone();
                        self.skills.insert(name, skill);
                        count += 1;
                    }
                    None => continue,
                }
            }
        }

        info!("Loaded {} skills from {:?}", count, dir);
        count
    }

    /// Загрузить Skills 2.0 формат (SKILL.md + skill.json)
    fn load_skills20(manifest_path: &Path, skill_path: &Path) -> Option<Skill> {
        let manifest_content = std::fs::read_to_string(manifest_path).ok()?;
        let manifest: SkillManifest = serde_json::from_str(&manifest_content).ok()?;
        let prompt = std::fs::read_to_string(skill_path).ok()?;

        Some(Skill {
            manifest,
            prompt,
            path: skill_path.to_path_buf(),
        })
    }

    /// TUI-формат: один SKILL.md с YAML frontmatter
    fn load_tui_format(file_path: &Path, skill_dir: &Path) -> Option<Skill> {
        let content = std::fs::read_to_string(file_path).ok()?;

        // Парсим YAML frontmatter (--- ... ---)
        if !content.starts_with("---") {
            return None;
        }

        let end = content[3..].find("---")?;
        let yaml_part = &content[3..3 + end];
        let prompt = content[3 + end + 3..].trim().to_string();

        // Простой парсинг YAML (без библиотеки)
        let name = extract_yaml(yaml_part, "name").unwrap_or_else(|| {
            skill_dir.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string()
        });
        let description = extract_yaml(yaml_part, "description").unwrap_or_default();

        let manifest = SkillManifest {
            name,
            version: extract_yaml(yaml_part, "version").unwrap_or_else(|| "1.0.0".into()),
            description,
            author: extract_yaml(yaml_part, "author"),
            tags: vec![],
            dependencies: vec![],
            bridges: vec![],
            bookmarks: vec![],
        };

        Some(Skill {
            manifest,
            prompt,
            path: file_path.to_path_buf(),
        })
    }

    /// Только SKILL.md без manifest
    fn load_skill_only(skill_path: &Path, skill_dir: &Path) -> Option<Skill> {
        let prompt = std::fs::read_to_string(skill_path).ok()?;
        let name = skill_dir.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();

        let manifest = SkillManifest {
            name,
            version: "1.0.0".into(),
            description: String::new(),
            author: None,
            tags: vec![],
            dependencies: vec![],
            bridges: vec![],
            bookmarks: vec![],
        };

        Some(Skill {
            manifest,
            prompt,
            path: skill_path.to_path_buf(),
        })
    }

    /// Получить скил по имени
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Получить промпт скила
    pub fn get_prompt(&self, name: &str) -> Option<&str> {
        self.skills.get(name).map(|s| s.prompt.as_str())
    }

    /// Список всех скилов
    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// Скилы по тегу
    pub fn by_tag(&self, tag: &str) -> Vec<&Skill> {
        self.skills.values().filter(|s| s.manifest.tags.contains(&tag.to_string())).collect()
    }

    /// Скилы по бриджу
    pub fn by_bridge(&self, bridge: &str) -> Vec<&Skill> {
        self.skills.values().filter(|s| s.manifest.bridges.contains(&bridge.to_string())).collect()
    }

    /// Добавить скил программно
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
        };
        let skill = Skill {
            manifest,
            prompt: prompt.to_string(),
            path: PathBuf::new(),
        };
        self.skills.insert(name.to_string(), skill);
    }
}

/// Простой парсинг YAML ключа (без библиотеки)
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

/// Конвертер TUI-скила в наш формат
pub fn convert_tui_to_our(tui_skill_path: &Path) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(tui_skill_path)?;
    if !content.starts_with("---") {
        return Err(anyhow::anyhow!("Not a TUI SKILL.md format"));
    }

    let end = content[3..].find("---")
        .ok_or_else(|| anyhow::anyhow!("No closing ---"))?;
    let yaml_part = &content[3..3 + end];

    let name = extract_yaml(yaml_part, "name")
        .ok_or_else(|| anyhow::anyhow!("No name in frontmatter"))?;
    let description = extract_yaml(yaml_part, "description").unwrap_or_default();
    let prompt = content[3 + end + 3..].trim().to_string();

    let skill_dir = tui_skill_path.parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine parent dir"))?;

    // Создаём manifest
    let manifest = SkillManifest {
        name: name.clone(),
        version: "1.0.0".into(),
        description,
        author: Some("converted".into()),
        tags: vec![],
        dependencies: vec![],
        bridges: vec![],
        bookmarks: vec![],
    };

    let manifest_json = serde_json::to_string_pretty(&manifest)?;

    // Записываем skill.json
    let json_path = skill_dir.join("skill.json");
    std::fs::write(&json_path, manifest_json)?;

    // Переименовываем SKILL.md → инструкция остаётся
    info!("Converted TUI skill '{}' to Skills 2.0 format", name);
    Ok(())
}
