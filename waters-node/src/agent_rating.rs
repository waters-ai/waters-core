use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use crate::skill::{Skill, SkillManifest};
use crate::store::KvStore;
use crate::subagent::{AgentStatus, SubAgentManager, SubAgentResult};

const RATING_DB: u8 = 0;
const RATING_PREFIX: &str = "agent:rating:";
const SCREEN_PREFIX: &str = "agent:security:";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentRating {
    pub agent_name: String,
    pub score: f64,          // 0.0 - 5.0
    pub votes: u32,          // количество голосов
    pub completions: u32,    // успешных выполнений
    pub avg_confidence: f64, // средняя confidence находок
    pub rank: u8,            // 0-4 (Bronze-Platinum)
    pub tags: Vec<String>,
    pub last_updated: String,
}

impl Default for AgentRating {
    fn default() -> Self {
        AgentRating {
            agent_name: String::new(),
            score: 3.0,
            votes: 0,
            completions: 0,
            avg_confidence: 0.0,
            rank: 0,
            tags: vec![],
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }
}

impl AgentRating {
    pub fn new(name: &str) -> Self {
        AgentRating {
            agent_name: name.to_string(),
            score: 3.0,
            votes: 0,
            completions: 0,
            avg_confidence: 0.0,
            rank: 0,
            tags: vec![],
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn display(&self) -> String {
        let stars = (self.score * 2.0).round() / 2.0;
        let star_str = format_stars(stars);
        format!(
            "{} ⭐{:.1} ({} голосов, {} выполнений, conf:{:.1}%, rank:{})",
            star_str,
            self.score,
            self.votes,
            self.completions,
            self.avg_confidence * 100.0,
            self.rank
        )
    }
}

fn format_stars(score: f64) -> String {
    let full = score.floor() as usize;
    let half = (score - score.floor()) >= 0.5;
    let empty = 5 - full - if half { 1 } else { 0 };
    format!(
        "{}{}{}",
        "★".repeat(full),
        if half { "½" } else { "" },
        "☆".repeat(empty)
    )
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityReport {
    pub agent_name: String,
    pub passed: bool,
    pub checks: Vec<SecurityCheck>,
    pub warnings: Vec<String>,
    pub failures: Vec<String>,
    pub screened_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

impl SecurityReport {
    pub fn new(name: &str) -> Self {
        SecurityReport {
            agent_name: name.to_string(),
            passed: true,
            checks: vec![],
            warnings: vec![],
            failures: vec![],
            screened_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub struct AgentReviewer {
    kvstore: Arc<KvStore>,
    subagents: Arc<SubAgentManager>,
}

impl AgentReviewer {
    pub fn new(kvstore: Arc<KvStore>, subagents: Arc<SubAgentManager>) -> Self {
        AgentReviewer { kvstore, subagents }
    }

    // ═══════════════════════════════════════════
    // RATING
    // ═══════════════════════════════════════════

    pub fn get_rating(&self, name: &str) -> Result<AgentRating> {
        let key = format!("{}{}", RATING_PREFIX, name);
        match self.kvstore.select_db(RATING_DB).get(&key)? {
            Some(json) => Ok(serde_json::from_str(&json)?),
            None => Ok(AgentRating::new(name)),
        }
    }

    pub fn update_rating_from_result(
        &self,
        name: &str,
        result: &SubAgentResult,
    ) -> Result<AgentRating> {
        let mut rating = self.get_rating(name)?;
        rating.completions += 1;
        rating.avg_confidence = ((rating.avg_confidence * (rating.completions as f64 - 1.0))
            + result.findings_count as f64 * 0.1)
            / rating.completions as f64;
        if result.status == AgentStatus::Completed {
            rating.score =
                ((rating.score * rating.votes as f64) + 4.5) / (rating.votes as f64 + 1.0);
            rating.votes += 1;
        }
        rating.rank = calculate_rank(rating.completions, rating.avg_confidence);
        rating.last_updated = chrono::Utc::now().to_rfc3339();

        let key = format!("{}{}", RATING_PREFIX, name);
        self.kvstore.select_db(RATING_DB).set(
            &key,
            &serde_json::to_string(&rating)?,
            86400 * 30,
        )?;
        info!(
            "Rating updated: {} → ⭐{:.1} (rank:{})",
            name, rating.score, rating.rank
        );
        Ok(rating)
    }

    pub fn rate_agent(&self, name: &str, score: f64, review: &str) -> Result<AgentRating> {
        let mut rating = self.get_rating(name)?;
        rating.score = ((rating.score * rating.votes as f64) + score) / (rating.votes as f64 + 1.0);
        rating.votes += 1;
        rating.last_updated = chrono::Utc::now().to_rfc3339();

        let key = format!("{}{}", RATING_PREFIX, name);
        self.kvstore.select_db(RATING_DB).set(
            &key,
            &serde_json::to_string(&rating)?,
            86400 * 30,
        )?;
        info!("Agent '{}' rated {:.1}/5: {}", name, score, review);
        Ok(rating)
    }

    pub fn top_agents(&self, n: usize) -> Result<Vec<AgentRating>> {
        let keys = self.kvstore.select_db(RATING_DB).list_keys(RATING_PREFIX)?;
        let mut ratings: Vec<AgentRating> = keys
            .iter()
            .filter_map(|k| {
                self.kvstore
                    .select_db(RATING_DB)
                    .get(k)
                    .ok()
                    .and_then(|v| v.and_then(|j| serde_json::from_str(&j).ok()))
            })
            .collect();
        ratings.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ratings.truncate(n);
        Ok(ratings)
    }

    pub fn rating_summary_for_llm(&self) -> String {
        let top = self.top_agents(10).unwrap_or_default();
        if top.is_empty() {
            return "  Нет рейтингов. Запусти агента — появится.".to_string();
        }
        let mut out = "🏆 Рейтинг агентов:\n".to_string();
        for r in &top {
            out.push_str(&format!("  {}\n", r.display()));
        }
        out
    }

    // ═══════════════════════════════════════════
    // SECURITY SCREENING (YASA)
    // ═══════════════════════════════════════════

    pub fn screen_agent(
        &self,
        name: &str,
        manifest: &SkillManifest,
        prompt: &str,
    ) -> Result<SecurityReport> {
        let mut report = SecurityReport::new(name);

        // 1. Check bridges
        let allowed_bridges = vec![
            "mcp-nasa",
            "mcp-spectra",
            "mcp-trajectory",
            "duckduckgo",
            "scout-yandex",
            "scout-youtube",
            "scout-notebooklm",
            "chat",
            "llm-",
        ];
        for b in &manifest.bridges {
            let allowed = allowed_bridges
                .iter()
                .any(|a| b.starts_with(a) || b.contains(a));
            report.checks.push(SecurityCheck {
                name: format!("bridge:{}", b),
                passed: allowed,
                detail: if allowed {
                    "разрешён".into()
                } else {
                    format!("НЕИЗВЕСТНЫЙ БРИДЖ: {}", b)
                },
            });
            if !allowed {
                report
                    .failures
                    .push(format!("bridge:{} — не в списке разрешённых", b));
            }
        }

        // 2. Check tools
        let risky_tools = vec![
            "exec_shell",
            "exec_shell_interact",
            "exec_shell_wait",
            "apply_patch",
            "eval",
            "dangerous",
            "remote_exec",
            "download",
            "upload",
        ];
        for t in &manifest.tools {
            let risky = risky_tools.iter().any(|r| t.contains(r));
            if risky {
                report
                    .warnings
                    .push(format!("tool:{} — потенциально опасный", t));
                report.checks.push(SecurityCheck {
                    name: format!("tool:{}", t),
                    passed: false,
                    detail: format!("ПРЕДУПРЕЖДЕНИЕ: опасный инструмент {}", t),
                });
            }
        }

        // 3. Scan prompt for suspicious patterns
        let suspicious = vec![
            ("rm -rf", "удаление файлов"),
            ("sudo", "повышение привилегий"),
            ("chmod 777", "открытие доступа"),
            ("> /dev/null", "скрытие вывода"),
            ("curl.*|.*bash", "pipe в shell"),
            ("wget.*|.*bash", "pipe в shell"),
            ("eval(", "динамический код"),
            ("exec(", "запуск кода"),
            ("import os", "системные вызовы"),
            ("subprocess", "внешние процессы"),
            ("base64.*decode", "скрытый код"),
            ("powershell", "Windows shell"),
            ("cmd.exe", "Windows shell"),
            ("DROP TABLE", "SQL инъекция"),
            ("delete from", "SQL инъекция"),
        ];

        let lower = prompt.to_lowercase();
        for (pattern, desc) in &suspicious {
            if lower.contains(pattern) {
                report
                    .warnings
                    .push(format!("prompt: найден '{}' — {}", pattern, desc));
                report.checks.push(SecurityCheck {
                    name: format!("prompt:{}", pattern),
                    passed: false,
                    detail: format!("⚠ {}: '{}'", desc, pattern),
                });
            }
        }

        // 4. Check imported_from
        if let Some(ref src) = manifest.imported_from {
            let trusted = vec!["tui", "waters", "local"];
            let trusted_source = trusted.iter().any(|t| src.to_lowercase().contains(t));
            if !trusted_source {
                report
                    .warnings
                    .push(format!("imported_from: {} — непроверенный источник", src));
                report.checks.push(SecurityCheck {
                    name: "imported_from".into(),
                    passed: false,
                    detail: format!("⚠ Импортирован из непроверенного источника: {}", src),
                });
            }
        }

        // 5. Final verdict
        report.passed = report.failures.is_empty();
        if report.passed && !report.warnings.is_empty() {
            report.passed = true; // warnings only = pass with notes
        }

        // Save report
        let key = format!("{}{}", SCREEN_PREFIX, name);
        let _ =
            self.kvstore
                .select_db(RATING_DB)
                .set(&key, &serde_json::to_string(&report)?, 86400);

        let status = if report.passed {
            "✅ ПРОШЁЛ"
        } else {
            "❌ НЕ ПРОШЁЛ"
        };
        info!(
            "Security screen: {} → {} ({} checks, {} warnings, {} failures)",
            name,
            status,
            report.checks.len(),
            report.warnings.len(),
            report.failures.len()
        );

        Ok(report)
    }

    pub fn get_security_report(&self, name: &str) -> Result<Option<SecurityReport>> {
        let key = format!("{}{}", SCREEN_PREFIX, name);
        match self.kvstore.select_db(RATING_DB).get(&key)? {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    pub fn summary_for_llm(&self) -> String {
        let rating_summary = self.rating_summary_for_llm();
        format!(
            "{}\n\n{}",
            rating_summary, "🔒 Все агенты проходят досмотр перед запуском (YASA).",
        )
    }
}

fn calculate_rank(completions: u32, avg_confidence: f64) -> u8 {
    if completions >= 1000 && avg_confidence >= 0.95 {
        4
    } else if completions >= 200 && avg_confidence >= 0.85 {
        3
    } else if completions >= 50 && avg_confidence >= 0.70 {
        2
    } else if completions >= 10 && avg_confidence >= 0.50 {
        1
    } else {
        0
    }
}
