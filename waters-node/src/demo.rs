use crate::bridge::BridgePool;
use crate::display::{BOLD, RESET};

pub fn setup_llm() -> Option<String> {
    if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
        if !key.is_empty() {
            return Some("deepseek".to_string());
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let config_path = std::path::Path::new(&home).join(".deepseek").join("config.toml");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if content.contains("api_key") {
                    return Some("deepseek".to_string());
                }
            }
        }
    }
    // Check Ollama
    let resp = reqwest::blocking::get("http://127.0.0.1:11434/api/tags");
    if resp.is_ok() {
        return Some("ollama".to_string());
    }
    None
}

pub fn print_no_llm_help() {
    println!("  {}", "\x1b[1mNo LLM connected.\x1b[0m");
    println!("  The node runs in convo-only mode.");
    println!("  To enable LLM:");
    println!("    - Set DEEPSEEK_API_KEY in environment");
    println!("    - Or edit bridges.json with your LLM config");
    println!("    - Or run: docker run -d -p 11434:11434 ollama/ollama");
    println!();
    println!("  Without LLM, you can still use:");
    println!("    - P2P networking (connect to peers)");
    println!("    - Task management");
    println!("    - Group management");
    println!("    - Agent management");
    println!("    - All slash commands (/skills, /bridges, /status)");
}

pub async fn demo_conversation(bridge_pool: &BridgePool) {
    println!("Running waters-node demo with LLM...\n");
    
    let llm_names: Vec<String> = bridge_pool.list().into_iter()
        .filter(|n| n.starts_with("llm-"))
        .collect();
    
    if llm_names.is_empty() {
        println!("No LLM bridges available.");
        return;
    }
    
    let llm_name = &llm_names[0];
    println!("Using LLM bridge: {}\n", llm_name);

    let prompts = vec![
        "Hello! What can you do?",
        "Create a group for meteorite hunters",
        "List my skills",
    ];

    for prompt in &prompts {
        println!("{}>{} {}", BOLD, RESET, prompt);
        match bridge_pool.call(llm_name, prompt) {
            Ok(response) => println!("  {}\n", response),
            Err(e) => println!("  Error: {}\n", e),
        }
    }
}
