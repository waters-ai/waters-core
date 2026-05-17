use std::sync::Arc;

pub const CYAN: &str = "\x1b[36m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";

pub fn print_banner(version: &str) {
    println!();
    println!("{}        _                       _   ", CYAN);
    println!("       | |                     | |  ");
    println!("  __ _| |__  __ ___   ___  ___| |_ ");
    println!(" / _` | '_ \\/ _` \\ \\ / / |/ __| __|");
    println!("| (_| | | | | (_| |\\ V /| | (__| |_ ");
    println!(" \\__,_|_| |_|\\__,_| \\_/ |_|\\___|\\__| v{}", version);
    println!("{}", RESET);
    println!("{}🌊  distributed agent runtime{}", BOLD, RESET);
    println!();
}

pub fn print_node_info(id_short: &str, name: &str, llm_display: &str) {
    println!("  {0}{1}Node{2}       {3}{4}{5}  {6}(name: {7}){8}",
        BOLD, RESET, DIM, CYAN, id_short, RESET, DIM, name, RESET);
    println!("  {0}{1}Version{2}   {3}0.3.0{4}", BOLD, RESET, DIM, GREEN, RESET);
    println!("  {0}{1}Session{2}   {3}active{4}", BOLD, RESET, DIM, GREEN, RESET);
    println!("  {0}{1}LLM{2}       {3}", BOLD, RESET, DIM, llm_display);
    println!();
}

pub fn print_tools(tools: &[&str]) {
    println!("  {0}{1}Tools{2}{3}", BOLD, RESET, DIM, RESET);
    for t in tools {
        println!("    {}- {}{}{}", DIM, CYAN, t, RESET);
    }
    println!();
}

pub fn print_api_info(port: u16) {
    println!("  {0}{1}API{2}       {3}http://localhost:{4}{5}", BOLD, RESET, DIM, CYAN, port, RESET);
    println!();
}

pub fn print_welcome() {
    println!();
    println!("{}╔══════════════════════════════════════╗{}", CYAN, RESET);
    println!("{}║    🌊  Добро пожаловать!            ║{}", CYAN, RESET);
    println!("{}╚══════════════════════════════════════╝{}", CYAN, RESET);
    println!();
}

pub fn print_ready() {
    println!("{}╔══════════════════════════════════════╗{}", GREEN, RESET);
    println!("{}║    Node is READY                     ║{}", GREEN, RESET);
    println!("{}╚══════════════════════════════════════╝{}", GREEN, RESET);
    println!();
}

pub fn demo_response(input: &str) {
    let lower = input.to_lowercase();
    println!();
    if lower.contains("group") || lower.contains("создай") {
        println!("  You asked to create a group.");
        println!("  → waters-node group create <name>");
    } else if lower.contains("hello") || lower.contains("hi") || lower.contains("привет") {
        println!("  {0}Welcome to WATERS!{1}", BOLD, RESET);
        println!("  This is your personal node in a distributed agent network.");
        println!("  Install Ollama for LLM-powered commands, or use the demo:");
        println!("  {0}  waters-node --demo{1}", DIM, RESET);
    } else if lower.contains("node") || lower.contains("нод") {
        println!("  Each waters-node is a full network participant.");
        println!("  {0}  features: channels, groups, P2P sync, tools, agents{1}", DIM, RESET);
        println!("  {0}  connect:   waters-node --connect <ip>{1}", DIM, RESET);
        println!("  {0}  dashboard: http://localhost:42069{1}", DIM, RESET);
    } else {
        println!("  {}Type 'help' for commands.{}", DIM, RESET);
        println!("  Or run {}waters-node --demo{} for a tour.", CYAN, RESET);
    }
    println!();
}

pub async fn demo_tools(tools: &Arc<crate::tools::ToolRegistry>, api_state: &Arc<crate::api::ApiState>) {
    println!(" {}Tools{} available:", BOLD, RESET);
    for t in tools.list() {
        println!("  {}→{} {}{}{}", DIM, RESET, CYAN, t, RESET);
    }
    let ctx = crate::tools::ToolContext {
        workspace: ".".into(),
        session_path: ".waters/sessions".into(),
        kvstore: None,
    };
    if let Ok(result) = tools.call("exec_shell", &ctx,
        serde_json::json!({"command": "echo '🌊 waters-node: network ready. Demo OK.'"}))
    {
        if let Some(out) = result.get("stdout").and_then(|s| s.as_str()) {
            println!("  {}", out.trim());
        }
    }
    api_state.nodes.lock().await.push(serde_json::json!({
        "peer": "demo.waters.ai:42069",
        "status": "simulated",
    }));
}
