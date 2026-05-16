use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::skill::{LlmConfig, SkillManifest};

/// Обнаруженный формат агента
#[derive(Debug, Clone, PartialEq)]
pub enum AgentFormat {
    Tui,     // SKILL.md с YAML frontmatter
    Claude,  // CLAUDE.md или .claude/rules/*.md
    Cursor,  // .cursorrules или .cursor/rules/*.md
    Waters,  // наш skill.json + SKILL.md
    Generic, // неизвестный markdown — эвристика
}

/// Результат парсинга агента
#[derive(Debug, Clone)]
pub struct ParsedAgent {
    pub format: AgentFormat,
    pub name: String,
    pub description: String,
    pub role: String,
    pub prompt: String,
    pub bridges: Vec<String>,
    pub tools: Vec<String>,
    pub llm_preferred: String,
    pub category: String,
    pub source_path: PathBuf,
}

/// Определить формат агента по файлу
pub fn detect_format(path: &Path) -> AgentFormat {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext != "md" && ext != "txt" && ext != "yml" && ext != "yaml" && ext != "json" {
        return AgentFormat::Generic;
    }

    match name.as_str() {
        "claude.md" | "claude.txt" => AgentFormat::Claude,
        ".cursorrules" | "cursorrules" => AgentFormat::Cursor,
        "skill.md" => AgentFormat::Tui,
        "skill.json" => AgentFormat::Waters,
        _ => {
            // Check parent dir name for hints
            if let Some(parent) = path.parent() {
                let pname = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if pname == ".cursor"
                    || pname == "rules"
                        && parent
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            == Some(".cursor")
                {
                    return AgentFormat::Cursor;
                }
                if pname == ".claude" || pname == "claude" {
                    return AgentFormat::Claude;
                }
            }
            // Read first line for frontmatter detection
            if let Ok(content) = std::fs::read_to_string(path) {
                if content.trim_start().starts_with("---") {
                    return AgentFormat::Tui;
                }
            }
            AgentFormat::Generic
        }
    }
}

/// Парсинг TUI SKILL.md
pub fn parse_tui(content: &str, path: &Path) -> Option<ParsedAgent> {
    if !content.starts_with("---") {
        return None;
    }
    let end = content[3..].find("---")?;
    let yaml = &content[3..3 + end];
    let prompt = content[3 + end + 3..].trim().to_string();

    let name = extract_yaml(yaml, "name").or_else(|| {
        path.file_stem()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    })?;
    let description = extract_yaml(yaml, "description").unwrap_or_default();
    let role = extract_yaml(yaml, "role").unwrap_or_else(|| "general".into());
    let model = extract_yaml(yaml, "model").unwrap_or_else(|| "deepseek-v4-flash".into());

    Some(ParsedAgent {
        format: AgentFormat::Tui,
        name,
        description,
        role,
        prompt,
        bridges: vec![],
        tools: vec![],
        llm_preferred: model,
        category: "imported/tui".into(),
        source_path: path.to_path_buf(),
    })
}

/// Парсинг Claude CLAUDE.md / .claude/rules/*.md
/// Claude format: markdown с секциями, без строгого frontmatter
pub fn parse_claude(content: &str, path: &Path) -> Option<ParsedAgent> {
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("claude-agent")
        .to_string();

    // Извлекаем первую строку как заголовок
    let title = content
        .lines()
        .find(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .unwrap_or_else(|| name.clone());

    // Ищем описание после заголовка
    let description = content
        .lines()
        .skip_while(|l| l.starts_with('#') || l.trim().is_empty())
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .unwrap_or_default();

    // Извлекаем упоминания инструментов/API
    let mut bridges: Vec<String> = vec![];
    for line in content.lines() {
        let l = line.to_lowercase();
        if l.contains("api") || l.contains("search") || l.contains("fetch") {
            if l.contains("nasa") {
                bridges.push("mcp-nasa".into());
            }
            if l.contains("search") {
                bridges.push("duckduckgo".into());
            }
            if !l.contains('#') && l.contains("http") {
                // generic API reference
            }
        }
    }
    bridges.sort();
    bridges.dedup();

    Some(ParsedAgent {
        format: AgentFormat::Claude,
        name: name.clone(),
        description: format!("Claude agent: {}", title),
        role: "specialist".into(),
        prompt: content.to_string(),
        bridges,
        tools: vec!["read_file".into(), "web_search".into()],
        llm_preferred: "claude-sonnet-4".into(),
        category: "imported/claude".into(),
        source_path: path.to_path_buf(),
    })
}

/// Парсинг Cursor .cursorrules / .cursor/rules/*.md
/// Cursor format: YAML frontmatter + markdown, или plain markdown
pub fn parse_cursor(content: &str, path: &Path) -> Option<ParsedAgent> {
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("cursor-agent")
        .to_string();

    let (description, prompt) = if content.starts_with("---") {
        // Cursor frontmatter format (new)
        let end = content[3..].find("---")?;
        let yaml = &content[3..3 + end];
        let prompt = content[3 + end + 3..].trim().to_string();
        let desc = extract_yaml(yaml, "description")
            .or_else(|| extract_yaml(yaml, "name"))
            .unwrap_or_else(|| format!("Cursor agent: {}", name));
        (desc, prompt)
    } else {
        // Plain markdown (legacy .cursorrules)
        let desc = content
            .lines()
            .find(|l| l.starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .unwrap_or_else(|| format!("Cursor agent: {}", name));
        (desc, content.to_string())
    };

    Some(ParsedAgent {
        format: AgentFormat::Cursor,
        name,
        description,
        role: "general".into(),
        prompt,
        bridges: vec![],
        tools: vec![
            "read_file".into(),
            "write_file".into(),
            "grep_files".into(),
            "exec_shell".into(),
        ],
        llm_preferred: "deepseek-v4-flash".into(),
        category: "imported/cursor".into(),
        source_path: path.to_path_buf(),
    })
}

/// Парсинг любого файла — автоопределение формата
pub fn parse_agent_file(path: &Path) -> Result<ParsedAgent> {
    let fmt = detect_format(path);
    let content = std::fs::read_to_string(path)?;

    let parsed =
        match fmt {
            AgentFormat::Tui => parse_tui(&content, path)
                .ok_or_else(|| anyhow::anyhow!("Invalid TUI SKILL.md format"))?,
            AgentFormat::Claude => parse_claude(&content, path)
                .ok_or_else(|| anyhow::anyhow!("Invalid Claude format"))?,
            AgentFormat::Cursor => parse_cursor(&content, path)
                .ok_or_else(|| anyhow::anyhow!("Invalid Cursor format"))?,
            AgentFormat::Waters => {
                // skill.json — уже наш формат, просто читаем
                let manifest: SkillManifest = serde_json::from_str(&content)?;
                let skill_dir = path.parent().unwrap_or(Path::new("."));
                let md_path = skill_dir.join("SKILL.md");
                let prompt = if md_path.exists() {
                    std::fs::read_to_string(&md_path)?
                } else {
                    String::new()
                };
                ParsedAgent {
                    format: AgentFormat::Waters,
                    name: manifest.name.clone(),
                    description: manifest.description.clone(),
                    role: manifest.role.clone(),
                    prompt,
                    bridges: manifest.bridges.clone(),
                    tools: manifest.tools.clone(),
                    llm_preferred: manifest.llm.preferred.clone(),
                    category: format!("agents/{}", manifest.category),
                    source_path: path.to_path_buf(),
                }
            }
            AgentFormat::Generic => {
                // Эвристика: если есть # заголовок — считаем агентом
                if content.contains("---") && content.contains("name:") {
                    parse_tui(&content, path)
                        .or_else(|| {
                            Some(ParsedAgent {
                                format: AgentFormat::Generic,
                                name: path
                                    .file_stem()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("agent")
                                    .to_string(),
                                description: "Generic agent".into(),
                                role: "general".into(),
                                prompt: content.clone(),
                                bridges: vec![],
                                tools: vec![],
                                llm_preferred: "deepseek-v4-flash".into(),
                                category: "imported/generic".into(),
                                source_path: path.to_path_buf(),
                            })
                        })
                        .ok_or_else(|| anyhow::anyhow!("Unknown format"))?
                } else {
                    ParsedAgent {
                        format: AgentFormat::Generic,
                        name: path
                            .file_stem()
                            .and_then(|n| n.to_str())
                            .unwrap_or("agent")
                            .to_string(),
                        description: "Generic agent".into(),
                        role: "general".into(),
                        prompt: content.clone(),
                        bridges: vec![],
                        tools: vec![],
                        llm_preferred: "deepseek-v4-flash".into(),
                        category: "imported/generic".into(),
                        source_path: path.to_path_buf(),
                    }
                }
            }
        };

    info!(
        "Parsed agent '{}' from {:?} (format: {:?})",
        parsed.name, path, fmt
    );
    Ok(parsed)
}

/// Импорт агента в наш формат: создаёт skill.json + SKILL.md
pub fn import_agent(source: &Path, agents_dir: &Path) -> Result<String> {
    let parsed = parse_agent_file(source)?;

    let category = parsed.category.trim_start_matches("agents/").to_string();
    let target_dir = agents_dir.join(&category).join(&parsed.name);
    std::fs::create_dir_all(&target_dir)?;

    // Создаём наш манифест
    let manifest = SkillManifest {
        name: parsed.name.clone(),
        version: "1.0.0".into(),
        description: parsed.description.clone(),
        author: Some(format!("imported from {:?}", parsed.format)),
        tags: vec![format!("{:?}", parsed.format).to_lowercase()],
        dependencies: vec![],
        bridges: parsed.bridges.clone(),
        bookmarks: vec![],
        category: category.clone(),
        role: parsed.role.clone(),
        llm: LlmConfig {
            preferred: parsed.llm_preferred.clone(),
            min: "0.5b".into(),
            alternatives: vec![],
        },
        tools: parsed.tools.clone(),
        output_types: vec!["custom".into()],
        imported_from: Some(format!("{:?}", parsed.format)),
    };

    // skill.json
    let json_path = target_dir.join("skill.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&manifest)?)?;

    // SKILL.md — наш формат
    let md_path = target_dir.join("SKILL.md");
    let mut skill_md = format!(
        "---\nname: {}\ndescription: {}\nrole: {}\n---\n\n",
        parsed.name, parsed.description, parsed.role
    );
    skill_md.push_str(&parsed.prompt);
    std::fs::write(&md_path, &skill_md)?;

    // Если это TUI — сохраняем оригинал как TUI.SKILL.md для совместимости
    if parsed.format == AgentFormat::Tui {
        let tui_path = target_dir.join("TUI.SKILL.md");
        std::fs::write(&tui_path, &skill_md)?;
    }

    info!("Imported agent '{}' -> {:?}", parsed.name, target_dir);
    Ok(parsed.name)
}

/// Экспорт агента в указанный формат
pub fn export_agent(
    manifest: &SkillManifest,
    prompt: &str,
    target_format: AgentFormat,
    output_dir: &Path,
) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir)?;

    match target_format {
        AgentFormat::Tui => {
            let path = output_dir.join("SKILL.md");
            let content = format!(
                "---\nname: {}\ndescription: {}\nrole: {}\nmodel: {}\n---\n\n{}",
                manifest.name, manifest.description, manifest.role, manifest.llm.preferred, prompt
            );
            std::fs::write(&path, content)?;
            info!("Exported '{}' as TUI SKILL.md", manifest.name);
            Ok(path)
        }
        AgentFormat::Claude => {
            let path = output_dir.join("CLAUDE.md");
            let content = format!(
                "# {}\n\n{}\n\n## Skills\n- Bridges: {}\n- Tools: {}\n\n## Instructions\n\n{}",
                manifest.name,
                manifest.description,
                manifest.bridges.join(", "),
                manifest.tools.join(", "),
                prompt,
            );
            std::fs::write(&path, content)?;
            info!("Exported '{}' as Claude CLAUDE.md", manifest.name);
            Ok(path)
        }
        AgentFormat::Cursor => {
            let path = output_dir.join(".cursorrules");
            let content = format!(
                "---\ndescription: {}\n---\n\nYou are an agent with the following capabilities:\n- Bridges: {}\n- Tools: {}\n\n{}",
                manifest.description,
                manifest.bridges.join(", "),
                manifest.tools.join(", "),
                prompt,
            );
            std::fs::write(&path, content)?;
            info!("Exported '{}' as Cursor .cursorrules", manifest.name);
            Ok(path)
        }
        AgentFormat::Waters => {
            let json_path = output_dir.join("skill.json");
            std::fs::write(&json_path, serde_json::to_string_pretty(manifest)?)?;
            let md_path = output_dir.join("SKILL.md");
            let content = format!(
                "---\nname: {}\ndescription: {}\nrole: {}\n---\n\n{}",
                manifest.name, manifest.description, manifest.role, prompt
            );
            std::fs::write(&md_path, content)?;
            info!("Exported '{}' as WATERS format", manifest.name);
            Ok(md_path)
        }
        AgentFormat::Generic => {
            anyhow::bail!("Cannot export to generic format. Specify TUI, Claude, or Cursor.");
        }
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

/// Массовый импорт всех агентов из директории
pub fn import_directory(source_dir: &Path, agents_dir: &Path) -> Result<Vec<String>> {
    let mut imported = Vec::new();
    if !source_dir.exists() {
        return Ok(imported);
    }

    // Рекурсивно ищем файлы агентов
    let mut entries: Vec<PathBuf> = Vec::new();
    collect_agent_files(source_dir, &mut entries)?;

    for entry in &entries {
        match import_agent(entry, agents_dir) {
            Ok(name) => {
                imported.push(name);
            }
            Err(e) => {
                warn!("Failed to import {:?}: {}", entry, e);
            }
        }
    }

    info!("Imported {} agents from {:?}", imported.len(), source_dir);
    Ok(imported)
}

fn collect_agent_files(dir: &Path, entries: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        if is_agent_file(dir) {
            entries.push(dir.to_path_buf());
        }
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_agent_files(&path, entries)?;
        } else if is_agent_file(&path) {
            entries.push(path);
        }
    }
    Ok(())
}

fn is_agent_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(
        name.as_str(),
        "skill.md" | "skill.json" | "claude.md" | ".cursorrules" | "cursorrules"
    ) || name.ends_with(".skill.md")
        || (name.ends_with(".md") && !name.contains("readme"))
        || name == "claude.md"
}
