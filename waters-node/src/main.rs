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
mod display;
mod handlers;
mod tui_agent;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use display::*;

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
        config::Config::default()
    };

    let state_path = PathBuf::from(".waters/node.json");
    let existing_id = node::Node::load_state(&state_path).ok().flatten();
    let mut node = node::Node::new(&cfg.node.name, existing_id);

    print_banner(env!("CARGO_PKG_VERSION"));

    let id_short = node.id()[..8].to_string();
    let (llm_client, llm_name) = demo::deepseek_or_ollama().await;
    let llm_display = if llm_client.is_some() {
        format!("{}{}{}", GREEN, llm_name, RESET)
    } else {
        format!("{}{}{}", YELLOW, llm_name, RESET)
    };
    print_node_info(&id_short, node.name(), &llm_display);

    let mut skill_reg = skill::SkillRegistry::new();
    skill_reg.load_from(&std::path::Path::new("skills"));
    if skill_reg.list().len() > 0 {
        let skill_count = skill_reg.list().len();
        let skill_msg = format!("{}Skills{}", BOLD, RESET);
        println!("  {0}{1}{2}   {3}{4}{5}", DIM, skill_msg, RESET, CYAN, skill_count, RESET);
    }

    let tools = Arc::new(tools::ToolRegistry::new());
    print_tools(&tools.list());

    let mut bridge_reg = bridge::BridgeRegistry::new();
    let agent_journal = journal::AgentJournal::new(&std::path::Path::new(".waters/logs"));

    let convo_path = PathBuf::from(".waters/profile.json");
    let mut convo = convo::Convo::load(&convo_path);

    let mut session_mgr = session::SessionManager::new(&PathBuf::from(&cfg.node.session_dir));
    if let Some(sid) = &args.resume {
        session_mgr.resume(sid)?;
    } else {
        session_mgr.start(node.id(), &cfg.node.name,
            "You are the WATERS Node interface. Help the user.");
    }

    let mut mode_engine = mode::ModeEngine::new();
    let mut task_mgr = task::TaskManager::new();
    let mut agent_mgr = agent::AgentManager::new();
    let mut subagents = subagent::SubAgentManager::new();
    let start = std::time::Instant::now();

    let api_state = Arc::new(api::ApiState::new(node.id(), node.name()));
    let api_state_clone = api_state.clone();
    let api_port = args.port;
    tokio::spawn(async move {
        let _ = api::serve(api_port, api_state_clone).await;
    });
    print_api_info(api_port);

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

    let mut group_mgr = group::GroupManager::new(node.id());

    let gossip = gossip::GossipEngine::new(node.id(), node.name(), api_port);
    for ch in &["discovery.v1", "heartbeat.v1", "orders.public", "findings.public"] {
        gossip.add_channel(ch).await;
    }
    gossip.start_mdns_listener().await.ok();
    gossip.start_mdns_broadcast(30).await.ok();
    gossip.start_tcp_listener(channel_mgr.clone()).await.ok();
    gossip.start_periodic_sync(channel_mgr.clone(), 60).await;

    // Register builtin TUI agents
    for tui_agent in tui_agent::builtin_tui_agents() {
        let entry = tui_agent.to_agent_entry();
        agent_mgr.add(&entry.name, &entry.role, &entry.agent_type, &entry.owner_node);
    }
    agent_journal.log("system", "tui_agents_loaded", &format!("{} TUI agents", tui_agent::builtin_tui_agents().len()));

    // Connect if specified
    if let Some(ref peer) = args.connect {
        println!("  {}→{} Connecting to {}{}{}...", BOLD, RESET, CYAN, peer, RESET);
        gossip.direct_sync(peer, channel_mgr.clone()).await.ok();
        api_state.nodes.lock().await.push(serde_json::json!({
            "peer": peer, "connected_at": chrono::Utc::now().to_rfc3339(),
        }));
        agent_journal.log("system", "peer_connected", peer);
        println!("  {}✓{} Connected!{}", GREEN, RESET, RESET);
        println!();
    }

    // Demo mode
    if args.demo {
        println!("{}╔══════════════════════════════════════╗{}", CYAN, RESET);
        println!("{}║    waters-node DEMO                  ║{}", CYAN, RESET);
        println!("{}╚══════════════════════════════════════╝{}", CYAN, RESET);
        if let Some(ref l) = llm_client {
            for line in &demo::demo_conversation(l).await {
                println!("{}", line);
            }
        } else {
            demo::print_no_llm_help();
        }
        println!(" {}Open your browser:{} {}{}{}", BOLD, RESET, CYAN, format!("http://localhost:{}", api_port), RESET);
        return Ok(());
    }

    // One-shot mode
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

    // Interactive mode
    if !convo.profile.greeted || convo.profile.name.is_empty() {
        print_welcome();
        println!("{}", convo.greet());
        println!("(напиши своё имя и нажми Enter)");
    } else {
        print_ready();
        if llm_client.is_some() {
            println!(" Try:");
            println!("  {0}chat ...{1}   — LLM command", DIM, RESET);
        } else {
            println!("  {0}chat ...{1}   — поговорить со мной", DIM, RESET);
        }
        println!("  {0}status{1}    — node info", DIM, RESET);
    }

    // Main loop
    use tokio::io::{AsyncBufReadExt, BufReader};

    loop {
        let uptime = start.elapsed().as_secs();
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

        let continue_running = if cmd.starts_with("/") {
            let parts: Vec<&str> = cmd[1..].splitn(2, ' ').collect();
            handlers::handle_slash(
                parts[0], parts.get(1).copied().unwrap_or(""),
                cmd,
                &mut mode_engine, &skill_reg, &bridge_reg,
                &gossip, &channel_mgr, &api_state, &agent_journal,
                &mut subagents, &mut agent_mgr, &mut session_mgr,
                &llm_client, &mut convo, &convo_path,
                &task_mgr, &group_mgr, &mut node, &state_path,
            ).await?
        } else {
            handlers::handle_natural(
                cmd,
                &mut mode_engine, &gossip, &channel_mgr, &api_state, &agent_journal,
                &llm_client, &mut session_mgr, &mut node, &id_short, api_port,
                uptime, &state_path, &mut convo, &convo_path,
                &task_mgr, &agent_mgr, &group_mgr, &skill_reg, &bridge_reg,
            ).await?
        };

        if !continue_running {
            break;
        }
    }

    println!("{}Node {} stopped. Goodbye!{}", DIM, node.name(), RESET);
    session_mgr.save()?;
    node.save_state(&state_path)?;
    Ok(())
}
