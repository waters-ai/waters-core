mod config;
mod node;
mod tools;
mod session;
mod subagent;
mod mcp;
mod autonomy;
mod dtn;
mod cargo;
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
mod store;
mod bridge;
mod journal;
mod offline;
mod display;
mod handlers;
mod tui_agent;

#[cfg(feature = "kafka-transport")]
mod kafka;

use anyhow::Result;
use clap::Parser;
use bridge::BridgePool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use display::*;

#[derive(Parser, Debug)]
#[command(name = "waters-node", version, about = "WATERS Node — distributed agent runtime")]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
    #[arg(short, long, default_value = "bridges.json")]
    bridges: PathBuf,
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

    // Init BridgePool from bridges.json
    let bridges_file = bridge::BridgePool::load_config(&args.bridges);
    let mut bridge_pool = bridge::BridgePool::new();

    // Load link profiles for DTN bandwidth management
    for link in &bridges_file.links {
        bridge_pool.governor.add_link(link.clone());
        info!("Link profile loaded: {} ({} Kbps)", link.name, link.max_bandwidth_kbps);
    }

    // Register LLM bridge
    let llm_name = format!("llm-{}", bridges_file.llm.provider);
    bridge_pool.register(
        &llm_name,
        Box::new(bridge::LlmBridge::new(&llm_name, &bridges_file.llm)),
        bridge::BridgeInfo::new(&llm_name, bridge::BridgeWeight::Heavy, 1, 50),
    );
    let llm_display = format!("{}{}{}", GREEN, llm_name, RESET);
    print_node_info(&id_short, node.name(), &llm_display);

    // Register Chat bridge
    match bridges_file.chat.transport.as_str() {
        "telegram" => {
            bridge_pool.register("chat",
                Box::new(bridge::ChatBridge::new_telegram("chat", &bridges_file.chat.token)),
                bridge::BridgeInfo::new("chat", bridge::BridgeWeight::Light, 1, 5));
        }
        "stdin" | _ => {
            bridge_pool.register("chat",
                Box::new(bridge::ChatBridge::new_stdin("chat")),
                bridge::BridgeInfo::new("chat", bridge::BridgeWeight::Light, 1, 5));
        }
    }

    // Register custom bridges from config
    for bcfg in &bridges_file.bridges {
        if !bcfg.enabled { continue; }
        match bcfg.provider.as_str() {
            "llm" => {
                let llm_cfg = bridge::LlmBridgeConfig {
                    provider: bcfg.config.get("provider").cloned().unwrap_or_default(),
                    model: bcfg.config.get("model").cloned().unwrap_or_default(),
                    url: bcfg.config.get("url").cloned().unwrap_or_default(),
                    api_key: bcfg.config.get("api_key").cloned().unwrap_or_default(),
                    system_prompt: bcfg.config.get("system_prompt").cloned().unwrap_or_default(),
                };
                bridge_pool.register(&bcfg.name,
                    Box::new(bridge::LlmBridge::new(&bcfg.name, &llm_cfg)),
                    bridge::BridgeInfo::new(&bcfg.name, bridge::BridgeWeight::Heavy, 2, 50));
            }
            "voice" => {
                let url = bcfg.config.get("url").cloned().unwrap_or_default();
                let mode = bcfg.config.get("mode").map(|s| s.as_str()).unwrap_or("stt");
                let vb = match mode {
                    "tts" => bridge::VoiceBridge::new_tts(&bcfg.name, &url),
                    _ => bridge::VoiceBridge::new_stt(&bcfg.name, &url),
                };
                bridge_pool.register(&bcfg.name, Box::new(vb),
                    bridge::BridgeInfo::new(&bcfg.name, bridge::BridgeWeight::Heavy, 3, 500));
            }
            _ => tracing::warn!("Unknown bridge provider: {}", bcfg.provider),
        }
    }

    // Parse MCP servers and register each tool as a bridge
    let mcp_client = Arc::new(std::sync::Mutex::new(mcp::McpClient::new()));
    for mcp_cfg in &bridges_file.mcp_servers {
        {
            let mut client = mcp_client.lock().unwrap();
            client.register(&mcp_cfg.name, "stdio", &mcp_cfg.command, &mcp_cfg.args);
        }
        let weight = if mcp_cfg.weight == "heavy" { bridge::BridgeWeight::Heavy } else { bridge::BridgeWeight::Light };
        for tool in &mcp_cfg.tools {
            let bridge_name = format!("{}-{}", mcp_cfg.name, tool);
            bridge_pool.register(&bridge_name,
                Box::new(bridge::McpBridge::new(&bridge_name, &mcp_cfg.name, tool, mcp_client.clone())),
                bridge::BridgeInfo::new(&bridge_name, weight, mcp_cfg.priority, mcp_cfg.bandwidth_kbps));
        }
        info!("MCP server registered: {} ({} tools, {} Kbps, priority {})",
            mcp_cfg.name, mcp_cfg.tools.len(), mcp_cfg.bandwidth_kbps, mcp_cfg.priority);
    }

    // Register builtin search bridges
    bridge_pool.register("duckduckgo",
        Box::new(bridge::ChatBridge::new_stdin("duckduckgo")),
        bridge::BridgeInfo::new("duckduckgo", bridge::BridgeWeight::Light, 3, 10));

    let mut skill_reg = skill::SkillRegistry::new();
    skill_reg.load_from(&std::path::Path::new("skills"));
    if skill_reg.list().len() > 0 {
        let skill_count = skill_reg.list().len();
        println!("  {0}{1}Skills{2}{3}   {4}{5}{6}", DIM, BOLD, RESET, DIM, CYAN, skill_count, RESET);
    }

    // Initialize KvStore (Redis or in-memory)
    let kvstore = {
        let redis_url = std::env::var("REDIS_URL").ok();
        Arc::new(store::KvStore::new(redis_url.as_deref()))
    };
    if kvstore.is_connected() {
        println!("  {}KvStore{}   ✅ Redis connected", BOLD, RESET);
    }

    let tools = Arc::new(tools::ToolRegistry::new());
    print_tools(&tools.list());

    let agent_journal = journal::AgentJournal::new(&std::path::Path::new(".waters/logs"), Some(kvstore.clone()));

    let convo_path = PathBuf::from(".waters/profile.json");
    let mut convo = convo::Convo::load(&convo_path);

    let mut session_mgr = session::SessionManager::new(&PathBuf::from(&cfg.node.session_dir));
    let mut offline_queue = offline::OfflineQueue::new(&std::path::Path::new(".waters"));

    // Crash recovery: check for checkpoint first
    if let Ok(Some(cp)) = session::SessionManager::resume_from_checkpoint() {
        println!("  {}⚠️  Found checkpoint — recovering from crash...{}", YELLOW, RESET);
        let restored_session = cp.session;
        let restored_node_id = restored_session.node_id.clone();
        let node_name = cp.node_state.get("node_name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        // Restore session from checkpoint
        session_mgr.restore_from(restored_session);
        // Restore node state from checkpoint
        node = node::Node::new(&node_name, Some(restored_node_id));
        // Flush any pending offline events
        if let Ok(events) = offline_queue.read_all() {
            if !events.is_empty() {
                println!("  {}📤 {} offline events pending{}", YELLOW, events.len(), RESET);
            }
        }
        session::SessionManager::clear_checkpoint()?;
        println!("  {}✓{} Recovery complete{}", GREEN, RESET, RESET);
    }

    if let Some(sid) = &args.resume {
        session_mgr.resume(sid)?;
    } else if session_mgr.current().is_none() {
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
        if bridge_pool.get("llm-ollama").is_some() || bridge_pool.get("llm-deepseek").is_some() {
            demo::demo_conversation(&bridge_pool).await;
        } else {
            demo::print_no_llm_help();
        }
        println!(" {}Open your browser:{} {}{}{}", BOLD, RESET, CYAN, format!("http://localhost:{}", api_port), RESET);
        return Ok(());
    }

    // One-shot mode
    if let Some(prompt_text) = args.prompt {
        session_mgr.add_message("user", &prompt_text);
        match bridge_pool.call("llm-ollama", &prompt_text)
            .or_else(|_| bridge_pool.call("llm-deepseek", &prompt_text))
        {
            Ok(r) => { println!("{}", r); session_mgr.add_message("assistant", &r); }
            Err(_) => demo_response(&prompt_text),
        }
        session_mgr.save()?;
        return Ok(());
    }

    // Interactive mode
    let has_llm = bridge_pool.list().iter().any(|n| n.starts_with("llm-"));
    if !convo.profile.greeted || convo.profile.name.is_empty() {
        print_welcome();
        println!("{}", convo.greet());
        println!("(напиши своё имя и нажми Enter)");
    } else {
        print_ready();
        if has_llm {
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

        // Save checkpoint before each step
        let node_state = serde_json::json!({
            "node_name": node.name(),
            "node_id": node.id(),
            "uptime": uptime,
            "peers": gossip.peer_count(),
        });
        session_mgr.save_checkpoint(&node_state)?;

        let continue_running = if cmd.starts_with("/") {
            let parts: Vec<&str> = cmd[1..].splitn(2, ' ').collect();
            handlers::handle_slash(
                parts[0], parts.get(1).copied().unwrap_or(""),
                cmd,
                &mut mode_engine, &skill_reg, &mut bridge_pool,
                &gossip, &channel_mgr, &api_state, &agent_journal,
                &mut subagents, &mut agent_mgr, &mut session_mgr,
                &mut convo, &convo_path,
                &mut task_mgr, &mut group_mgr, &mut node, &state_path,
            ).await?
        } else {
            handlers::handle_natural(
                cmd,
                &mut mode_engine, &gossip, &channel_mgr, &api_state, &agent_journal,
                &bridge_pool, &mut session_mgr, &mut node, &id_short, api_port,
                uptime, &state_path, &mut convo, &convo_path,
                &task_mgr, &agent_mgr, &group_mgr, &skill_reg,
            ).await?
        };

        if !continue_running {
            break;
        }

        // Clear checkpoint after successful step
        session::SessionManager::clear_checkpoint()?;
    }

    println!("{}Node {} stopped. Goodbye!{}", DIM, node.name(), RESET);
    session_mgr.save()?;
    node.save_state(&state_path)?;
    Ok(())
}
