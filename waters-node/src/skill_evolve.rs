use crate::skill::{LlmConfig, Skill, SkillManifest, SkillRegistry};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillReview {
    pub skill_name: String,
    pub task_summary: String,
    pub success_rating: u8,
    pub improvements: Vec<String>,
    pub new_tags: Vec<String>,
    pub weaknesses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolvedSkill {
    pub original_name: String,
    pub version: String,
    pub prompt_delta: String,
    pub new_description: String,
    pub tags: Vec<String>,
}

pub struct SkillEvolver {
    history: Vec<SkillReview>,
    evolve_dir: PathBuf,
}

impl SkillEvolver {
    pub fn new(evolve_dir: &Path) -> Self {
        SkillEvolver {
            history: Vec::new(),
            evolve_dir: evolve_dir.to_path_buf(),
        }
    }

    pub fn review(
        &mut self,
        skill_name: &str,
        task_summary: &str,
        success: bool,
        notes: &[&str],
    ) -> SkillReview {
        let rating = if success {
            std::cmp::min(10, 8 + notes.len() as u8)
        } else {
            3u8
        };

        let review = SkillReview {
            skill_name: skill_name.to_string(),
            task_summary: task_summary.to_string(),
            success_rating: rating,
            improvements: notes.iter().map(|n| n.to_string()).collect(),
            new_tags: Vec::new(),
            weaknesses: Vec::new(),
        };

        info!(
            "📊 SkillEvolver: reviewed '{}' — rating {}/10",
            skill_name, rating
        );
        self.history.push(review.clone());
        review
    }

    pub fn evolve(
        &self,
        registry: &mut SkillRegistry,
        review: &SkillReview,
    ) -> Result<Option<String>> {
        if review.success_rating < 5 {
            info!(
                "⏭️ SkillEvolver: '{}' rating {} < 5, skipping evolve",
                review.skill_name, review.success_rating
            );
            return Ok(None);
        }

        let original = match registry.get(&review.skill_name) {
            Some(s) => s,
            None => {
                warn!("Skill '{}' not found for evolve", review.skill_name);
                return Ok(None);
            }
        };

        let version_parts: Vec<&str> = original.manifest.version.split('.').collect();
        let minor: u32 = version_parts
            .get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let new_version = format!("1.{}.{}", minor + 1, review.success_rating);

        let new_name = format!("{}-v{}", review.skill_name, minor + 1);

        let improve_text = if review.improvements.is_empty() {
            String::new()
        } else {
            format!(
                "\n## Улучшения после ревью\n\n{}",
                review.improvements.join("\n")
            )
        };

        let mut new_prompt = format!(
            "---\nname: {new_name}\nversion: {new_version}\ndescription: {} — эволюционировал после ревью (рейтинг {}/10)\n---\n\n# {new_name}\n\nЭволюционировал из: **{}** v{}\n{}",
            original.manifest.description,
            review.success_rating,
            original.manifest.name,
            original.manifest.version,
            improve_text,
        );

        if !review.weaknesses.is_empty() {
            new_prompt.push_str(&format!(
                "\n\n## Известные слабости\n\n{}",
                review
                    .weaknesses
                    .iter()
                    .map(|w| format!("- {w}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        let mut manifest = original.manifest.clone();
        manifest.name = new_name.clone();
        manifest.version = new_version.clone();
        manifest.description = format!(
            "{} (эволюция v{})",
            original.manifest.description,
            minor + 1
        );
        manifest.author = Some("self-evolved".into());
        let mut all_tags = review.new_tags.clone();
        for t in &manifest.tags {
            if !all_tags.contains(t) {
                all_tags.push(t.clone());
            }
        }
        manifest.tags = all_tags;

        let evolve_skill_dir = self.evolve_dir.join(&new_name);
        std::fs::create_dir_all(&evolve_skill_dir)?;

        let md_path = evolve_skill_dir.join("SKILL.md");
        std::fs::write(&md_path, &new_prompt)?;

        registry.create_from_manifest(manifest, &new_prompt);

        info!(
            "🧬 SkillEvolver: '{}' evolved into '{}' (v{})",
            review.skill_name, new_name, new_version
        );

        Ok(Some(new_name))
    }

    pub fn get_history(&self) -> &[SkillReview] {
        &self.history
    }

    pub fn recent_reviews(&self, count: usize) -> Vec<&SkillReview> {
        self.history.iter().rev().take(count).collect()
    }

    pub fn evolve_dir(&self) -> &Path {
        &self.evolve_dir
    }
}

pub fn auto_evolve(
    registry: &mut SkillRegistry,
    evolver: &mut SkillEvolver,
    skill_name: &str,
    task_summary: &str,
    success: bool,
    notes: &[&str],
) -> Result<Option<String>> {
    let review = evolver.review(skill_name, task_summary, success, notes);
    evolver.evolve(registry, &review)
}
