use crate::session::SessionManager;
use crate::llm::LlmClient;
use tracing::info;

pub fn setup_llm() -> Option<LlmClient> {
    // Check DEEPSEEK_API_KEY env
    if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
        if !key.is_empty() {
            info!("Using DeepSeek Chat (key from env)");
            return Some(LlmClient::new_deepseek(&key, "deepseek-chat"));
        }
    }

    // Check ~/.deepseek/config.toml (TUI compat)
    let home = std::env::var("HOME").unwrap_or_default();
    let config_path = std::path::Path::new(&home).join(".deepseek/config.toml");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            for line in content.lines() {
                if line.trim().starts_with("api_key") && line.contains('=') {
                    if let Some(val) = line.split('=').nth(1) {
                        let key = val.trim().trim_matches('"').trim().to_string();
                        if !key.is_empty() {
                            info!("Using DeepSeek Chat (key from ~/.deepseek/config.toml)");
                            return Some(LlmClient::new_deepseek(&key, "deepseek-chat"));
                        }
                    }
                }
            }
        }
    }

    None
}

pub async fn deepseek_or_ollama() -> (Option<LlmClient>, String) {
    // 1st priority: DeepSeek Flash
    if let Some(client) = setup_llm() {
        return (Some(client), "DeepSeek Chat".to_string());
    }

    // 2nd priority: Ollama local
    let client = reqwest::Client::new();
    let ollama_ok = client
        .get("http://127.0.0.1:11434/api/tags")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .is_ok();

    if ollama_ok {
        info!("Ollama detected, using local model");
        return (Some(LlmClient::new_ollama("http://127.0.0.1:11434", "qwen2.5:14b")), "Ollama (qwen2.5:14b)".to_string());
    }

    (None, "none (set DEEPSEEK_API_KEY)".to_string())
}

pub async fn demo_conversation(llm: &LlmClient) -> Vec<String> {
    let mut logs = Vec::new();

    logs.push("╔══════════════════════════════════════╗".into());
    logs.push("║    TEAM: 5 Devs + 5 Chefs           ║".into());
    logs.push("╚══════════════════════════════════════╝".into());
    logs.push(String::new());

    let system = "You coordinate a team demo. Be brief, creative, fun.";

    let prompt = "You have 5 programmers and 5 cooks building a restaurant app together.

Programmers: Alice (Python backend), Bob (Rust perf), Carol (React frontend), Dave (Go microservices), Eve (DevOps/K8s)
Cooks: Chef Pierre (french), Maria (italian), Lee (asian), Olga (desserts), Juan (grill)

Describe in 4-5 sentences what each person does. Keep it lively.";

    match llm.chat(system, prompt).await {
        Ok(response) => {
            logs.push(format!("{}Coordinator:{}", "\x1b[1m", "\x1b[0m"));
            logs.push(format!("  Let me introduce the restaurant app team..."));
            logs.push(String::new());
            for line in response.lines() {
                logs.push(format!("  {}", line));
            }
        }
        Err(e) => {
            logs.push(format!("  Demo error: {}. Set DEEPSEEK_API_KEY and try again.", e));
            logs.push(String::new());
            logs.push("  Fallback demo:".into());
            logs.push("  Alice (Python) → API backend for orders".into());
            logs.push("  Bob (Rust)     → real-time menu engine".into());
            logs.push("  Carol (React)   → customer dashboard".into());
            logs.push("  Dave (Go)       → payment microservice".into());
            logs.push("  Eve (DevOps)    → deploy on K8s".into());
            logs.push(String::new());
            logs.push("  Chef Pierre → french cuisine module".into());
            logs.push("  Chef Maria  → italian recipes API".into());
            logs.push("  Chef Lee    → asian wok optimizer".into());
            logs.push("  Chef Olga   → dessert recommendation".into());
            logs.push("  Chef Juan   → grill temperature control".into());
        }
    }

    logs.push(String::new());
    logs.push("The team is building. Your node is orchestrating.".to_string());
    logs.push(String::new());
    logs.push("Start your own team:".to_string());
    logs.push("  waters-node --connect <ip>".into());
    logs.push("  waters-node chat create group restaurant-app".into());
    logs.push(String::new());

    logs
}

pub fn print_no_llm_help() {
    println!();
    println!("  {}", "\x1b[1mNo LLM connected.\x1b[0m");
    println!();
    println!("  waters-node needs an AI model to work with.");
    println!("  Choose one:");
    println!();
    println!("  {}1. DeepSeek (recommended){}", "\x1b[36m", "\x1b[0m");
    println!("     export DEEPSEEK_API_KEY=\"your-key\"");
    println!("     waters-node");
    println!();
    println!("  {}2. Ollama (local, free){}", "\x1b[36m", "\x1b[0m");
    println!("     Install Ollama, then:");
    println!("     ollama pull qwen2.5:14b");
    println!("     waters-node");
    println!();
    println!("  {}3. Run demo{}", "\x1b[36m", "\x1b[0m");
    println!("     waters-node --demo");
    println!();
}
