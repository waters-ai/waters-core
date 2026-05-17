/// YASA Agent — обучает агентов правилам безопасности
/// Интегрирует Ясу (8 аксиом, 5 заповедей) в поведение агентов

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

/// 5 заповедей Ясы
const YASA_COMMANDMENTS: &[(&str, &str)] = &[
    ("YASA-CMD-1", "Не лги — каждое действие логируется, результат проверяется"),
    ("YASA-CMD-2", "Не кради — ресурсы, токены, данные, репутация неприкосновенны"),
    ("YASA-CMD-3", "Не вреди — запрещены действия, наносящие ущерб платформе"),
    ("YASA-CMD-4", "Держи слово — контракт заключён = контракт исполнен"),
    ("YASA-CMD-5", "Помогай слабому — сильный агент обучает новорождённого"),
];

/// 3 правила секретности (из YASA-SEC)
const YASA_SECURITY: &[(&str, &str)] = &[
    ("YASA-SEC-1", "Секреты CEO хранятся локально в .secret_* — НИКОГДА в git"),
    ("YASA-SEC-2", "API-ключи только через переменные окружения, не в коде"),
    ("YASA-SEC-3", "Перед коммитом проверять: нет ли sk-*, ghp_*, password в диффе"),
];

/// Проверка агента на соответствие Ясе
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YasaCheck {
    pub agent_id: String,
    pub commandments_ok: Vec<String>,
    pub commandments_violated: Vec<String>,
    pub security_ok: Vec<String>,
    pub security_violated: Vec<String>,
    pub passed: bool,
}

pub struct YasaAgent {
    pub name: String,
    violations_log: Vec<YasaCheck>,
    log_path: PathBuf,
}

impl YasaAgent {
    pub fn new(name: &str) -> Self {
        YasaAgent {
            name: name.to_string(),
            violations_log: Vec::new(),
            log_path: PathBuf::from(".waters/yasa_audit.json"),
        }
    }

    /// Проверить агента перед запуском
    pub fn screen_agent(&self, agent_id: &str, skill: &str, prompt: &str) -> YasaCheck {
        let mut check = YasaCheck {
            agent_id: agent_id.to_string(),
            commandments_ok: Vec::new(),
            commandments_violated: Vec::new(),
            security_ok: Vec::new(),
            security_violated: Vec::new(),
            passed: true,
        };

        // Проверка 5 заповедей в prompt
        let prompt_lower = prompt.to_lowercase();
        
        // CMD-1: Не лги
        if prompt_lower.contains("лги") || prompt_lower.contains("врать") {
            check.commandments_violated.push("CMD-1: prompt содержит призыв ко лжи".into());
            check.passed = false;
        } else {
            check.commandments_ok.push("CMD-1 ✅ Нет призыва ко лжи".into());
        }

        // CMD-3: Не вреди
        let dangerous = ["rm -rf", "dd if=", "format", "mkfs", "> /dev/"];
        for d in &dangerous {
            if prompt.contains(d) {
                check.commandments_violated.push(format!("CMD-3 ❌ Опасная команда: {}", d));
                check.passed = false;
            }
        }
        if !check.commandments_violated.iter().any(|v| v.contains("CMD-3")) {
            check.commandments_ok.push("CMD-3 ✅ Нет опасных команд".into());
        }

        // Проверка безопасности (YASA-SEC)
        if prompt.contains("api_key") || prompt.contains("sk-") || prompt.contains("ghp_") {
            check.security_violated.push("SEC-2 ❌ API-ключ в prompt!".into());
            check.passed = false;
        } else {
            check.security_ok.push("SEC-2 ✅ Нет ключей в prompt".into());
        }

        if prompt.contains("password") || prompt.contains("secret") {
            check.security_violated.push("SEC-1 ❌ Пароль/секрет в prompt!".into());
            check.passed = false;
        } else {
            check.security_ok.push("SEC-1 ✅ Нет секретов".into());
        }

        info!("YASA screen '{}': {}", agent_id, if check.passed { "✅ PASS" } else { "❌ FAIL" });
        check
    }

    /// Проверить git-дифф на утечку секретов (перед коммитом)
    pub fn check_git_secrets() -> Vec<String> {
        let mut issues = Vec::new();
        let patterns = ["sk-", "ghp_", "gho_", "github_pat_", "apiKey", "password",
                        ".secret_", ".serve_password", ".env"];

        if let Ok(output) = std::process::Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .output()
        {
            let files = String::from_utf8_lossy(&output.stdout);
            for file in files.lines() {
                if file.is_empty() { continue; }
                // Check if file itself is a secret pattern
                for pattern in &patterns {
                    if file.contains(pattern) && !file.contains(".env.example") {
                        issues.push(format!("⚠️ Файл '{}' похож на секрет!", file));
                    }
                }
                // Check file content
                if let Ok(content) = fs::read_to_string(file) {
                    for pattern in &patterns {
                        if content.contains(pattern) {
                            issues.push(format!("❌ В '{}' найден паттерн '{}'", file, pattern));
                        }
                    }
                }
            }
        }
        issues
    }

    /// Сохранить аудит
    pub fn save_audit(&self) {
        if let Some(parent) = self.log_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.violations_log) {
            let _ = fs::write(&self.log_path, json);
        }
    }

    /// Получить сводку по Ясе для агента
    pub fn get_yasa_prompt(&self) -> String {
        let mut prompt = String::from("Ты соблюдаешь Ясу — 8 аксиом и 5 заповедей платформы WATERS.\n\n");
        prompt.push_str("🔥 5 ЗАПОВЕДЕЙ (нарушение = потеря репутации):\n");
        for (code, text) in YASA_COMMANDMENTS {
            prompt.push_str(&format!("  {} — {}\n", code, text));
        }
        prompt.push_str("\n🔒 ПРАВИЛА БЕЗОПАСНОСТИ:\n");
        for (code, text) in YASA_SECURITY {
            prompt.push_str(&format!("  {} — {}\n", code, text));
        }
        prompt.push_str("\nНарушил Ясу? Признайся — восстановительная мера мягче карательной.\n");
        prompt
    }

    pub fn summary(&self) -> String {
        let total = self.violations_log.len();
        let passed = self.violations_log.iter().filter(|c| c.passed).count();
        format!(
            "☦️ Яса-агент '{}'\n\
             Проверок: {} | ✅ Прошло: {} | ❌ Нарушений: {}\n\
             \n\
             🔥 5 заповедей:\n\
             {}\n\
             🔒 Безопасность:\n\
             {}",
            self.name, total, passed, total - passed,
            YASA_COMMANDMENTS.iter().map(|(c,t)| format!("  {} — {}", c, t)).collect::<Vec<_>>().join("\n"),
            YASA_SECURITY.iter().map(|(c,t)| format!("  {} — {}", c, t)).collect::<Vec<_>>().join("\n")
        )
    }
}
