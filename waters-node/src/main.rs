mod config;
mod node;
mod tools;
mod session;
mod subagent;
mod llm;
mod mcp;
mod autonomy;
mod dtn;
mod cargo;
mod chat;
mod api;
mod channel;
mod group;
mod gossip;
mod demo;
mod convo;
mod task;
mod mode;
mod agent;
mod skill;
mod bridge;
mod journal;

#[cfg(feature = "kafka-transport")]
mod kafka;

use anyhow::Result;
use clap::Parser;
use convo::ConvoAction;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(name = "waters-node", version, about = "WATERS Node — distributed agent runtime")]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    #[arg(short, long)]
    verbose: bool,

    #[arg(long)]
    resume: Option<String>,

    #[arg(long)]
    connect: Option<String>,

    #[arg(long, default_value = "general")]
    role: String,

    #[arg(short, long)]
    prompt: Option<String>,

    #[arg(short = 'P', long, default_value_t = 42069)]
    port: u16,

    #[arg(long)]
    demo: bool,

    #[cfg(feature = "kafka-transport")]
    #[arg(long)]
    kafka: bool,
}

const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(
                    if args.verbose { "debug" } else { "info" }
                )),
        )
        .init();

    let cfg = if args.config.exists() {
        config::Config::from_file(&args.config)?
    } else {
        print!("{}", BOLD);
        println!("No config found, creating default...");
        print!("{}", RESET);
        config::Config::default()
    };

    let state_path = PathBuf::from(".waters/node.json");
    let existing_id = node::Node::load_state(&state_path).ok().flatten();
    let mut node = node::Node::new(&cfg.node.name, existing_id);

    // ─── BEAUTIFUL BANNER ───────────────────────
    println!();
    println!("{}        _                       _   ", CYAN);
    println!("       | |                     | |  ");
    println!("  __ _| |__  __ ___   ___  ___| |_ ");
    println!(" / _` | '_ \\/ _` \\ \\ / / |/ __| __|");
    println!("| (_| | | | | (_| |\\ V /| | (__| |_ ");
    println!(" \\__,_|_| |_|\\__,_| \\_/ |_|\\___|\\__| v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", RESET);
    println!("{}🌊  distributed agent runtime{}", BOLD, RESET);
    println!();

    // Identity
    let id_short = node.id()[..8].to_string();
    println!("  {0}{1}Node{2}       {3}{4}{5}  {6}(name: {7}){8}",
        BOLD, RESET, DIM, CYAN, id_short, RESET, DIM, node.name(), RESET);
    println!("  {0}{1}Version{2}   {3}0.2.0{4}", BOLD, RESET, DIM, GREEN, RESET);
    println!("  {0}{1}Session{2}   {3}active{4}", BOLD, RESET, DIM, GREEN, RESET);

    // LLM — auto-detect: DeepSeek Chat > Ollama > none
    let (llm_client, llm_name) = demo::deepseek_or_ollama().await;
    let llm_display = if llm_client.is_some() {
        format!("{}{}{}", GREEN, llm_name, RESET)
    } else {
        format!("{}{}{}", YELLOW, llm_name, RESET)
    };
    println!("  {0}{1}LLM{2}       {3}",
        BOLD, RESET, DIM, llm_display);
    println!();

    // Skills (загружаем скилы из skills/ директории)
    let mut skill_reg = skill::SkillRegistry::new();
    skill_reg.load_from(Path::new("skills"));
    let skill_count = skill_reg.list().len();
    if skill_count > 0 {
        let skill_msg = format!("{}Skills{}", BOLD, RESET);
        println!("  {0}{1}{2}   {3}{4}{5}", DIM, skill_msg, RESET, CYAN, skill_count, RESET);
    }

    // Bridges (список разъёмов)
    let mut bridge_reg = bridge::BridgeRegistry::new();

    // Agent Journal (бортовые журналы)
    let agent_journal = journal::AgentJournal::new(&Path::new(".waters/logs"));

    // Conversation (always loads, works without LLM)
    let convo_path = PathBuf::from(".waters/profile.json");
    let mut convo = convo::Convo::load(&convo_path);

    // Tools
    let tools = Arc::new(tools::ToolRegistry::new());
    println!("  {0}{1}Tools{2}{3}", BOLD, RESET, DIM, RESET);
    for t in tools.list() {
        println!("    {}- {}{}{}", DIM, CYAN, t, RESET);
    }
    println!();

    // Session
    let mut session_mgr = session::SessionManager::new(&PathBuf::from(&cfg.node.session_dir));
    if let Some(sid) = &args.resume {
        session_mgr.resume(sid)?;
    } else {
        session_mgr.start(node.id(), &cfg.node.name,
            "You are the WATERS Node interface. Help the user.");
    }

    // Mode Engine (5 режимов)
    let mut mode_engine = mode::ModeEngine::new();

    // Task Manager
    let mut task_mgr = task::TaskManager::new();

    // Agent Manager
    let mut agent_mgr = agent::AgentManager::new();

    // SubAgent Manager
    let mut subagents = subagent::SubAgentManager::new();

    // Autonomy
    let mut _autonomy = autonomy::AutonomyEngine::new();
    let start = std::time::Instant::now();

    // API server
    let api_state = Arc::new(api::ApiState::new(node.id(), node.name()));
    let api_state_clone = api_state.clone();
    let api_port = args.port;
    tokio::spawn(async move {
        let _ = api::serve(api_port, api_state_clone).await;
    });
    println!("  {0}{1}API{2}       {3}http://localhost:{4}{5}", BOLD, RESET, DIM, CYAN, api_port, RESET);
    println!();

    // Channel Manager
    let channel_path = PathBuf::from(".waters/channels");
    let channel_mgr = Arc::new(Mutex::new(channel::ChannelManager::new(&channel_path, node.id())));
    {
        let mut cm_lock = channel_mgr.lock().await;
        cm_lock.create("discovery.v1", "open").ok();
        cm_lock.create("heartbeat.v1", "open").ok();
        cm_lock.create("orders.public", "open").ok();
        cm_lock.create("findings.public", "open").ok();
    }
    agent_journal.log("system", "channels_ready", "4 system channels created");

    // Group Manager
    let mut group_mgr = group::GroupManager::new(node.id());

    // Gossip Engine
    let gossip = gossip::GossipEngine::new(node.id(), node.name(), api_port);
    for ch in &["discovery.v1", "heartbeat.v1", "orders.public", "findings.public"] {
        gossip.add_channel(ch).await;
    }
    gossip.start_mdns_listener().await.ok();
    gossip.start_mdns_broadcast(30).await.ok();
    gossip.start_tcp_listener(channel_mgr.clone()).await.ok();
    gossip.start_periodic_sync(channel_mgr.clone(), 60).await;

    // Connect if specified
    if let Some(ref peer) = args.connect {
        println!("  {0}→{1} Connecting to {2}{3}{4}...", BOLD, RESET, CYAN, peer, RESET);
        gossip.direct_sync(peer, channel_mgr.clone()).await.ok();
        api_state.nodes.lock().await.push(serde_json::json!({
            "peer": peer, "connected_at": chrono::Utc::now().to_rfc3339(),
        }));
        agent_journal.log("system", "peer_connected", peer);
        println!("  {0}✓{1} Connected!{2}", GREEN, RESET, RESET);
        println!();
    }

    // ═════  DEMO MODE  ═══════════════════════════
    if args.demo {
        println!("{}╔══════════════════════════════════════╗{}", CYAN, RESET);
        println!("{}║    waters-node DEMO                  ║{}", CYAN, RESET);
        println!("{}║    5 Programmers + 5 Chefs           ║{}", CYAN, RESET);
        println!("{}╚══════════════════════════════════════╝{}", CYAN, RESET);
        println!();

        if let Some(ref l) = llm_client {
            let chat_log = demo::demo_conversation(l).await;
            for line in &chat_log {
                println!("{}", line);
            }
        } else {
            println!("  {}", "\x1b[1mNo LLM connected.\x1b[0m");
            println!("  Set DEEPSEEK_API_KEY to see the full demo with 10 agents.");
            println!();
            demo::print_no_llm_help();
        }

        println!(" {}Open your browser:{} {}{}{}", BOLD, RESET, CYAN, format!("http://localhost:{}", api_port), RESET);
        println!();
        return Ok(());
    }

    // ═════  ONE-SHOT MODE  ═══════════════════════
    if let Some(prompt_text) = args.prompt {
        session_mgr.add_message("user", &prompt_text);
        if let Some(ref l) = llm_client {
            let mut ch = chat::ChatInterface::new(l.clone(), &session_mgr);
            match ch.process(&prompt_text).await {
                Ok(r) => { println!("{}", r); session_mgr.add_message("assistant", &r); }
                Err(e) => tracing::error!("Chat error: {}", e),
            }
        } else {
            demo_response(&prompt_text);
        }
        session_mgr.save()?;
        return Ok(());
    }

    // ═════  INTERACTIVE MODE  ════════════════════
    // First interaction — always use built-in Convo
    if !convo.profile.greeted || convo.profile.name.is_empty() {
        println!();
        println!("{}╔══════════════════════════════════════╗{}", CYAN, RESET);
        println!("{}║    🌊  Добро пожаловать!            ║{}", CYAN, RESET);
        println!("{}╚══════════════════════════════════════╝{}", CYAN, RESET);
        println!();
        println!("{}", convo.greet());
        println!();
        println!("(напиши своё имя и нажми Enter)");
        println!();
    } else {
        println!("{}╔══════════════════════════════════════╗{}", GREEN, RESET);
        println!("{}║    Node is READY                     ║{}", GREEN, RESET);
        println!("{}╚══════════════════════════════════════╝{}", GREEN, RESET);
        println!();
        if llm_client.is_some() {
            println!(" Try:");
            println!("  {0}chat ...{1}   — LLM command", DIM, RESET);
        } else {
            println!("  {0}chat ...{1}   — поговорить со мной", DIM, RESET);
        }
        println!("  {0}status{1}    — node info", DIM, RESET);
        println!();
    }

    use tokio::io::{AsyncBufReadExt, BufReader};

    loop {
        let uptime = start.elapsed().as_secs();

        // Show peer count and uptime in prompt
        let peer_count = gossip.peer_count();
        let peer_str = if peer_count > 0 { format!(" {}peers:{}", GREEN, peer_count) } else { String::new() };
        print!("{}{}>{} {}{}{} ",
            BOLD, if peer_count > 0 { GREEN } else { DIM }, RESET,
            DIM, peer_str, RESET);
        std::io::Write::flush(&mut std::io::stdout())?;

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        let cmd = line.trim();
        if cmd.is_empty() { continue; }

        // ─── Slash-команды ───────────────────────────
        if cmd.starts_with("/") {
            let parts: Vec<&str> = cmd[1..].splitn(2, ' ').collect();
            let slash_cmd = parts[0];
            let slash_arg = parts.get(1).copied().unwrap_or("");

            match slash_cmd {
                "help" | "h" => {
                    println!("{}Slash commands:{}", BOLD, RESET);
                    println!("  /help       — this help");
                    println!("  /skills     — list skills");
                    println!("  /models     — list available models (nodes)");
                    println!("  /agent      — /agent create <name> <skill> <node_id>");
                    println!("  /bridges    — list bridges");
                    println!("  /status     — node status");
                    println!("  /mode       — switch mode (plan/execute/stop/log)");
                    println!("  /connect    — connect to peer: /connect <ip>");
                    println!("  /chat       — send message: /chat <text>");
                    println!("  /sessions   — list sessions");
                    println!("  /resume     — resume session: /resume <id>");
                    println!("  /exit       — shutdown");
                }
                "skills" => {
                    let list = skill_reg.list();
                    if list.is_empty() {
                        println!("No skills loaded.");
                    } else {
                        println!("{}Skills ({}):{}", BOLD, list.len(), RESET);
                        for s in &list {
                            let tags = s.manifest.tags.join(", ");
                            let deps = s.manifest.dependencies.join(", ");
                            println!("  {} v{} — {}", s.manifest.name, s.manifest.version, s.manifest.description);
                            if !deps.is_empty() { println!("    deps: {}", deps); }
                            if !tags.is_empty() { println!("    tags: {}", tags); }
                            if !s.manifest.bookmarks.is_empty() {
                                println!("    tests: {}", s.manifest.bookmarks.len());
                            }
                        }
                    }
                }
                "models" => {
                    let peers = gossip.list_peers().await;
                    println!("{}Available models (nodes):{}", BOLD, RESET);
                    // Local model
                    println!("  {} — local (this node){}",
                        if llm_client.is_some() { "✅" } else { "⬜" },
                        if llm_client.is_some() { " active" } else { "" });
                    // Connected peers
                    for p in &peers {
                        println!("  {} — {} ({})", "⬜", p.node_name, p.node_id);
                    }
                    println!("\nUsage: /agent create <name> <skill> <node_id>");
                }
                "bridges" => {
                    let list = bridge_reg.list();
                    println!("{}Bridges:{}", BOLD, RESET);
                    for b in &list {
                        let icon = if b.connected { "✅" } else { "⬜" };
                        let keys = if b.config_keys.is_empty() { "free".into() } else { b.config_keys.join(", ") };
                        println!("  {} {} — {} [{}]", icon, b.name, b.description, keys);
                    }
                }
                "agent" => {
                    // /agent create <name> <skill> <node_id>
                    let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
                    if parts.len() >= 3 && parts[0] == "create" {
                        let name = parts[1];
                        let skill_name = parts[2];
                        let node = if parts.len() >= 4 { parts[3] } else { "local" };

                        if let Some(skill) = skill_reg.get(skill_name) {
                            let prompt = skill.prompt.clone();
                            let id = subagents.spawn(skill_name);
                            agent_journal.log(name, "created", &format!("skill={}, node={}", skill_name, node));
                        agent_mgr.add(name, &skill.manifest.description, "delegated", node);
                            println!("{}✓{} Agent '{}' created with skill '{}' on node '{}'", GREEN, RESET, name, skill_name, node);
                        } else {
                            println!("Skill '{}' not found. Available: {}", skill_name,
                                skill_reg.list().iter().map(|s| s.manifest.name.as_str()).collect::<Vec<_>>().join(", "));
                        }
                    } else {
                        println!("Usage: /agent create <name> <skill> <node_id>");
                        println!("  /models — show available nodes");
                        println!("  /skills — show available skills");
                    }
                }
                "mode" => {
                    if let Some(new_mode) = mode::ModeEngine::parse_mode(slash_arg) {
                        let msg = mode_engine.switch(new_mode);
                        println!("{}", msg);
                    } else {
                        println!("Modes: plan, assemble, execute, stop, log");
                    }
                }
                "connect" if !slash_arg.is_empty() => {
                    gossip.direct_sync(slash_arg, channel_mgr.clone()).await.ok();
                    api_state.nodes.lock().await.push(serde_json::json!({
                        "peer": slash_arg, "connected_at": chrono::Utc::now().to_rfc3339(),
                    }));
                    agent_journal.log("system", "slash_connect", slash_arg);
                    println!("{}✓{} Connected to {}", GREEN, RESET, slash_arg);
                }
                "chat" if !slash_arg.is_empty() => {
                    session_mgr.add_message("user", slash_arg);
                    if let Some(ref l) = llm_client {
                        let mut ci = chat::ChatInterface::new(l.clone(), &session_mgr);
                        match ci.process(slash_arg).await {
                            Ok(r) => { println!("{}", r); session_mgr.add_message("assistant", &r); }
                            Err(e) => println!("Error: {}", e),
                        }
                    } else {
                        if let Some(reply) = handle_convo(&mut convo, &convo_path, slash_arg, &task_mgr, &agent_mgr, &group_mgr, &gossip, &skill_reg, &bridge_reg, &mut node, &state_path, &mut session_mgr).await {
                            println!("{}", reply);
                        } else { break; }
                    }
                }
                "sessions" | "resume" => {
                    let sessions = session_mgr.list_sessions();
                    if sessions.is_empty() {
                        println!("No saved sessions.");
                    } else {
                        println!("{}Sessions:{}", BOLD, RESET);
                        for s in &sessions {
                            println!("  {}", s);
                        }
                    }
                    if slash_cmd == "resume" && !slash_arg.is_empty() {
                        if session_mgr.resume(slash_arg).ok().unwrap_or(false) {
                            println!("{}✓{} Session resumed: {}", GREEN, RESET, slash_arg);
                        } else {
                            println!("Session '{}' not found.", slash_arg);
                        }
                    }
                }
                "exit" | "quit" => break,
                _ => {
                    println!("Unknown slash command: /{}. Try /help", slash_cmd);
                }
            }
            continue;
        }

        match cmd {
            "exit" | "quit" | "q" => {
                println!("{}Shutting down...{}", DIM, RESET);
                session_mgr.save()?;
                node.save_state(&state_path)?;
                break;
            }
            "help" | "?" => {
                println!("{0}Commands:{1}", BOLD, RESET);
                println!("  help              — this help");
                println!("  chat <text>       — LLM-powered command");
                println!("  find              — discover peers");
                println!("  status            — node info");
                println!("  connect <ip>      — join a peer");
                println!("  dashboard         — open {}http://localhost:{}{}", CYAN, api_port, RESET);
                println!("  exit              — shutdown");
                println!();
                println!("{}Examples:{}", DIM, RESET);
                println!("  chat create group kapelka for audit");
                println!("  chat find all explorer nodes in the network");
            }
            "status" => {
                let peers = gossip.list_peers().await;
                println!("{0}Mode:{1}      {2}", BOLD, RESET, mode_engine.current);
                println!("{0}Node:{1}      {2}{3}{4}  ({5})", BOLD, RESET, CYAN, id_short, RESET, node.name());
                println!("{}Uptime:{}   {}s", BOLD, RESET, uptime);
                println!("{}Peers:{}   {}", BOLD, RESET, peers.len());
                for p in &peers {
                    println!("  {}→{} {}{}{}", DIM, RESET, CYAN, p.node_name, RESET);
                }
                println!("{}LLM:{}     {}", BOLD, RESET, if llm_client.is_some() { "connected" } else { "none" });
                println!("{}API:{}     {}{}{}", BOLD, RESET, CYAN, format!("http://localhost:{}", api_port), RESET);
                let agent_list = agent_journal.list_agents();
                println!("{}Journals:{} {} agents logged", DIM, RESET, agent_list.len());
                let cmds = mode_engine.available_commands();
                println!("{}Commands:{} {}", DIM, RESET, cmds.join(", "));
            }
            "find" | "nodes" => {
                let peers = gossip.list_peers().await;
                if peers.is_empty() {
                    println!("{}No peers found.{}", DIM, RESET);
                    println!("  Use '{}connect <ip>{}' to join a peer.", CYAN, RESET);
                } else {
                    println!("{}Peers ({}){}", GREEN, peers.len(), RESET);
                    for p in &peers {
                        println!("  {}→{} {}{}{}  {}(channels: {}){}",
                            DIM, RESET, CYAN, p.node_name, RESET, DIM, p.channels.len(), RESET);
                    }
                }
            }
            "dashboard" => {
                println!("Opening {}http://localhost:{}{}", CYAN, api_port, RESET);
                println!("  (open in your browser)");
            }
            text if text.to_lowercase().starts_with("режим ") => {
                let mode_name = text[6..].trim();
                if let Some(new_mode) = mode::ModeEngine::parse_mode(mode_name) {
                    let msg = mode_engine.switch(new_mode);
                    println!("{}", msg);
                } else {
                    println!("Неизвестный режим. Доступны: план, сбор, выполнение, стоп, журнал");
                }
            }
            _ if cmd.starts_with("connect ") => {
                let peer = cmd.trim_start_matches("connect ");
                println!("Connecting to {}...", peer);
                gossip.direct_sync(peer, channel_mgr.clone()).await.ok();
                api_state.nodes.lock().await.push(serde_json::json!({
                    "peer": peer, "connected_at": chrono::Utc::now().to_rfc3339(),
                }));
                println!("{}✓{} Connected to {}", GREEN, RESET, peer);
            }
            text if text.to_lowercase().starts_with("chat ") => {
                let text = text[5..].trim();
                session_mgr.add_message("user", text);
                if let Some(ref l) = llm_client {
                    let mut ci = chat::ChatInterface::new(l.clone(), &session_mgr);
                    match ci.process(text).await {
                        Ok(r) => { println!("{}", r); session_mgr.add_message("assistant", &r); }
                        Err(e) => println!("Error: {}", e),
                    }
                } else {
                    demo_response(text);
                }
            }
            _ => {
                if let Some(ref l) = llm_client {
                    session_mgr.add_message("user", cmd);
                    let mut ci = chat::ChatInterface::new(l.clone(), &session_mgr);
                    match ci.process(cmd).await {
                        Ok(r) => { println!("{}", r); session_mgr.add_message("assistant", &r); }
                        Err(e) => {
                            match convo.handle(cmd) {
                                ConvoAction::Exit => {
                                    println!("Shutting down...");
                                    session_mgr.save()?;
                                    convo.save(&convo_path);
                                    node.save_state(&state_path)?;
                                    break;
                                }
                                ConvoAction::Response(text) => println!("{}", text),
                                _ => {},
                            }
                        }
                    }
                } else {
                    if let Some(reply) = handle_convo(&mut convo, &convo_path, cmd, &task_mgr, &agent_mgr, &group_mgr, &gossip, &skill_reg, &bridge_reg, &mut node, &state_path, &mut session_mgr).await {
                        println!("{}", reply);
                    } else { break; }
                }
            }
    }
    }

    println!("{}Node {} stopped. Goodbye!{}", DIM, node.name(), RESET);
    Ok(())
}

/// Обработать ConvoAction — подставить реальные данные из менеджеров
async fn handle_convo(
    convo: &mut convo::Convo, convo_path: &PathBuf, input: &str,
    task_mgr: &task::TaskManager, agent_mgr: &agent::AgentManager,
    group_mgr: &group::GroupManager, gossip: &gossip::GossipEngine,
    skill_reg: &skill::SkillRegistry, bridge_reg: &bridge::BridgeRegistry,
    node: &mut node::Node, state_path: &PathBuf,
    session_mgr: &mut session::SessionManager,
) -> Option<String> {
    use convo::ConvoAction;

    match convo.handle(input) {
        ConvoAction::Exit => {
            println!("Shutting down...");
            session_mgr.save().ok();
            convo.save(convo_path);
            node.save_state(state_path).ok();
            return None;
        }
        ConvoAction::Menu => {
            return Some(format!(
                "{}, выбирай:\n1. задачи\n2. агенты\n3. группы\n4. настройки",
                convo.profile.name
            ));
        }
        ConvoAction::Help => {
            return Some(
                "Команды:\n  задачи — список задач\n  агенты — список агентов\n  группы — список групп\n  ноды — подключённые ноды\n  отчёт — сводка\n  настройки — конфигурация".into()
            );
        }
        ConvoAction::ListTasks => {
            let tasks = task_mgr.list().await;
            if tasks.is_empty() {
                return Some("📋 Нет задач. Создай первую: chat создай задачу ...".into());
            }
            let mut reply = "📋 Задачи:\n".to_string();
            for t in &tasks {
                let agent = t.assigned_to.as_deref().unwrap_or("—");
                let node = t.assigned_node.as_deref().unwrap_or("");
                reply.push_str(&format!("  [{}] {} — {} (назначен: {}{})\n",
                    &t.id[..t.id.len().min(8)], t.title, t.status, agent,
                    if node.is_empty() { "".into() } else { format!(" @{}", node) }));
            }
            return Some(reply);
        }
        ConvoAction::ListAgents => {
            let mine = agent_mgr.list_mine();
            let peers = agent_mgr.list_from_peers();
            if mine.is_empty() && peers.is_empty() {
                return Some("🤖 Нет агентов. Создай: chat создай агента ...".into());
            }
            let mut reply = "🤖 Агенты:\n".to_string();
            for a in &mine {
                let icon = if a.agent_type == "shared" { "🌐" } else { "🔒" };
                reply.push_str(&format!("  {} {} — {} ({})\n", icon, a.name, a.role, a.owner_node));
            }
            if !peers.is_empty() {
                reply.push_str("  Из других нод:\n");
                for a in &peers {
                    reply.push_str(&format!("    🌐 {} — {} ({})\n", a.name, a.role, a.owner_node));
                }
            }
            return Some(reply);
        }
        ConvoAction::ListGroups => {
            let groups = group_mgr.list();
            if groups.is_empty() {
                return Some("🔗 Нет групп. Создай: chat создай группу ...".into());
            }
            let mut reply = "🔗 Группы:\n".to_string();
            for g in &groups {
                reply.push_str(&format!("  {} ({} участников, {} каналов, {})\n",
                    g.name, g.members.len(), g.channels.len(), g.visibility));
                if !g.shared_skills.is_empty() {
                    reply.push_str(&format!("    скилы: {}\n",
                        g.shared_skills.iter().map(|s| &s.name).cloned().collect::<Vec<_>>().join(", ")));
                }
                if !g.shared_bridges.is_empty() {
                    reply.push_str(&format!("    поиск: {}\n",
                        g.shared_bridges.iter().map(|b| &b.name).cloned().collect::<Vec<_>>().join(", ")));
                }
                if !g.shared_services.is_empty() {
                    reply.push_str(&format!("    сервисы: {}\n",
                        g.shared_services.iter().map(|s| &s.name).cloned().collect::<Vec<_>>().join(", ")));
                }
            }
            return Some(reply);
        }
        ConvoAction::ListPeers => {
            let peers = gossip.list_peers().await;
            if peers.is_empty() {
                return Some("🌍 Нет подключённых нод.".into());
            }
            let mut reply = format!("🌍 Ноды ({}):\n", peers.len());
            for p in &peers {
                let addr = p.addresses.first().map(|a| a.as_str()).unwrap_or("?");
                reply.push_str(&format!("  {} → {} ({})\n", p.node_name, addr, p.node_id));
            }
            return Some(reply);
        }
        ConvoAction::Report => {
            let tasks = task_mgr.list().await;
            let done = tasks.iter().filter(|t| t.status == "done").count();
            let open = tasks.iter().filter(|t| t.status == "open").count();
            let assigned = tasks.iter().filter(|t| t.status == "assigned").count();
            let agents_mine = agent_mgr.list_mine().len();
            let agents_peers = agent_mgr.list_from_peers().len();
            let peers = gossip.list_peers().await.len();
            let skills = skill_reg.list().len();

            let mut reply = format!(
                "📊 Отчёт ноды {}:\n", convo.profile.name);
            reply.push_str(&format!("  Задачи: {} всего ({} выполнено, {} в работе, {} открыто)\n",
                tasks.len(), done, assigned, open));
            reply.push_str(&format!("  Агенты: {} своих + {} из других нод\n", agents_mine, agents_peers));
            reply.push_str(&format!("  Ноды: {} подключено\n", peers));
            reply.push_str(&format!("  Скилы: {} загружено\n", skills));
            return Some(reply);
        }
        ConvoAction::Setup => {
            let mut reply = "⚙️ Настройки ноды:\n".to_string();
            reply.push_str(&format!("  Имя: {}\n", convo.profile.name));
            reply.push_str(&format!("  NodeID: {}\n", node.id()));
            reply.push_str("  Бриджи:\n");
            for b in bridge_reg.list() {
                let icon = if b.connected { "✅" } else { "⬜" };
                let keys = if b.config_keys.is_empty() { "бесплатно".into() } else { b.config_keys.join(", ") };
                reply.push_str(&format!("    {} {} — {} [{}]\n", icon, b.name, b.description, keys));
            }
            return Some(reply);
        }
        ConvoAction::Response(text) => {
            return Some(text);
        }
    }
}

async fn demo_tools(tools: &Arc<tools::ToolRegistry>, api_state: &Arc<api::ApiState>) {
    println!(" {}Tools{} available:", BOLD, RESET);
    for t in tools.list() {
        println!("  {}→{} {}{}{}", DIM, RESET, CYAN, t, RESET);
    }
    let ctx = tools::ToolContext {
        workspace: ".".into(),
        session_path: ".waters/sessions".into(),
    };
    // Run a simple shell demo
    if let Ok(result) = tools.call("exec_shell", &ctx,
        serde_json::json!({"command": "echo '🌊 waters-node: network ready. Demo OK.'"}))
    {
        if let Some(out) = result.get("stdout").and_then(|s| s.as_str()) {
            println!("  {}", out.trim());
        }
    }
    // Add demo peer
    api_state.nodes.lock().await.push(serde_json::json!({
        "peer": "demo.waters.ai:42069",
        "status": "simulated",
    }));
}

fn demo_response(input: &str) {
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
