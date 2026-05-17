use std::collections::HashMap;
use std::path::Path;
use tracing::info;

pub struct NodeDiagnose {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub total_modules: u32,
    pub tested_modules: u32,
    pub uncovered_modules: Vec<String>,
    pub unwrap_calls: Vec<String>,
    pub unsafe_calls: Vec<String>,
    pub redis_connected: bool,
    pub uptime_secs: u64,
}

pub fn diagnose(src_dir: &Path, redis_ok: bool, uptime: u64) -> NodeDiagnose {
    let mut d = NodeDiagnose {
        warnings: Vec::new(),
        errors: Vec::new(),
        total_modules: 0,
        tested_modules: 0,
        uncovered_modules: Vec::new(),
        unwrap_calls: Vec::new(),
        unsafe_calls: Vec::new(),
        redis_connected: redis_ok,
        uptime_secs: uptime,
    };

    // Count .rs files
    if let Ok(entries) = std::fs::read_dir(src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                d.total_modules += 1;
                // Check for tests
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.contains("#[test]") {
                        d.tested_modules += 1;
                    } else {
                        d.uncovered_modules.push(
                            path.file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        );
                    }
                    // Find unwrap calls (rough count)
                    for (i, line) in content.lines().enumerate() {
                        let line = line.trim();
                        if line.contains(".unwrap()") && !line.starts_with("//") {
                            let fname = path.file_name().unwrap_or_default().to_string_lossy();
                            d.unwrap_calls.push(format!(
                                "{}:{}: {}",
                                fname,
                                i + 1,
                                line.chars().take(80).collect::<String>()
                            ));
                        }
                        if line.contains("unsafe ") && !line.starts_with("//") {
                            let fname = path.file_name().unwrap_or_default().to_string_lossy();
                            d.unsafe_calls.push(format!(
                                "{}:{}: {}",
                                fname,
                                i + 1,
                                line.chars().take(80).collect::<String>()
                            ));
                        }
                    }
                }
            }
        }
    }

    info!(
        "Diagnose: {} modules, {} tested, {} unwrap calls",
        d.total_modules,
        d.tested_modules,
        d.unwrap_calls.len()
    );
    d
}

impl NodeDiagnose {
    pub fn summary(&self) -> String {
        let coverage = if self.total_modules > 0 {
            (self.tested_modules as f64 / self.total_modules as f64) * 100.0
        } else {
            0.0
        };

        let mut out = format!("📊 Диагностика ноды:\n");
        out.push_str(&format!(
            "  Модулей: {} всего, {} с тестами ({:.0}%)\n",
            self.total_modules, self.tested_modules, coverage
        ));
        out.push_str(&format!(
            "  Предупреждений (warnings): {}\n",
            self.warnings.len()
        ));
        out.push_str(&format!("  Ошибок: {}\n", self.errors.len()));
        out.push_str(&format!(
            "  unwrap() вызовов: {}\n",
            self.unwrap_calls.len()
        ));
        out.push_str(&format!("  unsafe: {}\n", self.unsafe_calls.len()));
        out.push_str(&format!(
            "  Redis: {}\n",
            if self.redis_connected { "✅" } else { "❌" }
        ));
        out.push_str(&format!(
            "  Uptime: {}ч {}м\n",
            self.uptime_secs / 3600,
            (self.uptime_secs % 3600) / 60
        ));

        if !self.warnings.is_empty() {
            out.push_str(&format!("\n  ⚠️ Первые 5 warnings:\n"));
            for w in self.warnings.iter().take(5) {
                out.push_str(&format!("    • {}\n", w));
            }
        }
        if !self.uncovered_modules.is_empty() {
            out.push_str(&format!(
                "\n  🧪 Модули без тестов ({}):\n",
                self.uncovered_modules.len()
            ));
            for m in &self.uncovered_modules {
                out.push_str(&format!("    • {}.rs\n", m));
            }
        }
        if !self.unwrap_calls.is_empty() {
            out.push_str(&format!("\n  ⚠️ Первые 5 unwrap():\n"));
            for u in self.unwrap_calls.iter().take(5) {
                out.push_str(&format!("    • {}\n", u));
            }
        }
        out
    }

    pub fn phase(&self) -> &'static str {
        if self.warnings.len() > 10
            || (self.tested_modules as f64 / self.total_modules.max(1) as f64) < 0.8
            || !self.unwrap_calls.is_empty()
        {
            "1 — Качество кода"
        } else if !self.redis_connected {
            "2 — Стабильность"
        } else {
            "3 — Экосистема"
        }
    }

    pub fn next_steps(&self) -> Vec<String> {
        let mut steps = Vec::new();
        if !self.unwrap_calls.is_empty() {
            steps.push(format!("Убрать {} unwrap()", self.unwrap_calls.len()));
        }
        if !self.uncovered_modules.is_empty() {
            steps.push(format!(
                "Добавить тесты для {} модулей",
                self.uncovered_modules.len()
            ));
        }
        if !self.warnings.is_empty() {
            steps.push(format!("Исправить {} warnings", self.warnings.len()));
        }
        steps
    }
}
