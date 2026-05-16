use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::convo::ConvoAction;
use crate::display::*;

pub async fn handle_slash(
    slash_cmd: &str, slash_arg: &str, cmd: &str,
    mode_engine: &mut crate::mode::ModeEngine,
    skill_reg: &crate::skill::SkillRegistry,
    bridge_reg: &crate::bridge::BridgeRegistry,
    gossip: &crate::gossip::GossipEngine,
    channel_mgr: &Arc<Mutex<crate::channel::ChannelManager>>,
    api_state: &Arc<crate::api::ApiState>,
    agent_journal: &crate::journal::AgentJournal,
    subagents: &mut crate::subagent::SubAgentManager,
    agent_mgr: &mut crate::agent::AgentManager,
    session_mgr: &mut crate::session::SessionManager,
    llm_client: &Option<crate::llm::LlmClient>,
    convo: &mut crate::convo::Convo,
    convo_path: &PathBuf,
    task_mgr: &crate::task::TaskManager,
    group_mgr: &crate::group::GroupManager,
    node: &mut crate::node::Node,
    state_path: &PathBuf,
) -> Result<bool, anyhow::Error> {
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
            println!("  /json       — output JSON format");
            println!("  /tui-agents — list builtin TUI-converted agents");
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
            println!("  {} — local (this node){}",
                if llm_client.is_some() { "✅" } else { "⬜" },
                if llm_client.is_some() { " active" } else { "" });
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
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            if parts.len() >= 3 && parts[0] == "create" {
                let name = parts[1];
                let skill_name = parts[2];
                let node = if parts.len() >= 4 { parts[3] } else { "local" };
                if let Some(skill) = skill_reg.get(skill_name) {
                    subagents.spawn(skill_name);
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
            if let Some(new_mode) = crate::mode::ModeEngine::parse_mode(slash_arg) {
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
                let mut ci = crate::chat::ChatInterface::new(l.clone(), &*session_mgr);
                match ci.process(slash_arg).await {
                    Ok(r) => { println!("{}", r); session_mgr.add_message("assistant", &r); }
                    Err(e) => println!("Error: {}", e),
                }
            } else {
                if let Some(reply) = handle_convo(convo, convo_path, slash_arg, task_mgr, agent_mgr, group_mgr, gossip, skill_reg, bridge_reg, node, state_path, session_mgr).await {
                    println!("{}", reply);
                } else { return Ok(false); }
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
        "json" => {
            println!("{{\"mode\":\"json\",\"status\":\"ok\",\"node\":\"{}\"}}", node.name());
        }
        "tui-agents" => {
            let agents = crate::tui_agent::builtin_tui_agents();
            println!("{}TUI-converted agents:{}", BOLD, RESET);
            for a in &agents {
                println!("  {} [{}] — {} (bridges: {})",
                    a.name, a.source, a.native_skill.description,
                    a.native_skill.bridges.join(", "));
                let entry = a.to_agent_entry();
                agent_mgr.add(&entry.name, &entry.role, &entry.agent_type, &entry.owner_node);
            }
            println!("  {} agents registered", agents.len());
        }
        "exit" | "quit" => return Ok(false),
        _ => {
            println!("Unknown slash command: /{}. Try /help", slash_cmd);
        }
    }
    Ok(true)
}

pub async fn handle_natural(
    cmd: &str,
    mode_engine: &mut crate::mode::ModeEngine,
    gossip: &crate::gossip::GossipEngine,
    channel_mgr: &Arc<Mutex<crate::channel::ChannelManager>>,
    api_state: &Arc<crate::api::ApiState>,
    agent_journal: &crate::journal::AgentJournal,
    llm_client: &Option<crate::llm::LlmClient>,
    session_mgr: &mut crate::session::SessionManager,
    node: &mut crate::node::Node,
    id_short: &str,
    api_port: u16,
    uptime: u64,
    state_path: &PathBuf,
    convo: &mut crate::convo::Convo,
    convo_path: &PathBuf,
    task_mgr: &crate::task::TaskManager,
    agent_mgr: &crate::agent::AgentManager,
    group_mgr: &crate::group::GroupManager,
    skill_reg: &crate::skill::SkillRegistry,
    bridge_reg: &crate::bridge::BridgeRegistry,
) -> Result<bool, anyhow::Error> {
    match cmd {
        "exit" | "quit" | "q" => {
            println!("{}Shutting down...{}", DIM, RESET);
            session_mgr.save()?;
            node.save_state(state_path)?;
            return Ok(false);
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
        _ if cmd.to_lowercase().starts_with("режим ") => {
            let mode_name = cmd[6..].trim();
            if let Some(new_mode) = crate::mode::ModeEngine::parse_mode(mode_name) {
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
        _ if cmd.to_lowercase().starts_with("chat ") => {
            let text = cmd[5..].trim();
            session_mgr.add_message("user", text);
            if let Some(ref l) = llm_client {
                let mut ci = crate::chat::ChatInterface::new(l.clone(), &*session_mgr);
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
                let mut ci = crate::chat::ChatInterface::new(l.clone(), &*session_mgr);
                match ci.process(cmd).await {
                    Ok(r) => { println!("{}", r); session_mgr.add_message("assistant", &r); }
                    Err(_) => {
                        match convo.handle(cmd) {
                            ConvoAction::Exit => {
                                println!("Shutting down...");
                                session_mgr.save()?;
                                convo.save(convo_path);
                                node.save_state(state_path)?;
                                return Ok(false);
                            }
                            ConvoAction::Response(text) => println!("{}", text),
                            _ => {},
                        }
                    }
                }
            } else {
                if let Some(reply) = handle_convo(convo, convo_path, cmd, task_mgr, agent_mgr, group_mgr, gossip, skill_reg, bridge_reg, node, state_path, session_mgr).await {
                    println!("{}", reply);
                } else { return Ok(false); }
            }
        }
    }
    Ok(true)
}

pub async fn handle_convo(
    convo: &mut crate::convo::Convo, convo_path: &PathBuf, input: &str,
    task_mgr: &crate::task::TaskManager, agent_mgr: &crate::agent::AgentManager,
    group_mgr: &crate::group::GroupManager, gossip: &crate::gossip::GossipEngine,
    skill_reg: &crate::skill::SkillRegistry, bridge_reg: &crate::bridge::BridgeRegistry,
    node: &mut crate::node::Node, state_path: &PathBuf,
    session_mgr: &mut crate::session::SessionManager,
) -> Option<String> {
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
                reply.push_str(&format!("  [{}] {} [mode:{:?}] — {} (назначен: {}{})\n",
                    &t.id[..t.id.len().min(8)], t.title, t.mode, t.status, agent,
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
                let icon = if a.agent_type == "tui_converted" { "🔄" } else if a.agent_type == "shared" { "🌐" } else { "🔒" };
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
            let tui_count = crate::tui_agent::builtin_tui_agents().len();

            let mut reply = format!("📊 Отчёт ноды {}:\n", convo.profile.name);
            reply.push_str(&format!("  Задачи: {} всего ({} выполнено, {} в работе, {} открыто)\n",
                tasks.len(), done, assigned, open));
            reply.push_str(&format!("  Агенты: {} своих + {} из других нод ({} TUI-конвертированных)\n", agents_mine, agents_peers, tui_count));
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
            reply.push_str(&format!("  TUI-агентов: {} встроено\n", crate::tui_agent::builtin_tui_agents().len()));
            return Some(reply);
        }
        ConvoAction::Response(text) => {
            return Some(text);
        }
    }
}
