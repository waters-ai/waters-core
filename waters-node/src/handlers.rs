use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::bridge::BridgePool;
use crate::convo::ConvoAction;
use crate::display::*;

pub async fn handle_slash(
    slash_cmd: &str, slash_arg: &str,
    cmd: &str,
    mode_engine: &mut crate::mode::ModeEngine,
    skill_reg: &crate::skill::SkillRegistry,
    bridge_pool: &mut BridgePool,
    gossip: &crate::gossip::GossipEngine,
    channel_mgr: &Arc<Mutex<crate::channel::ChannelManager>>,
    api_state: &Arc<crate::api::ApiState>,
    agent_journal: &crate::journal::AgentJournal,
    subagents: &mut crate::subagent::SubAgentManager,
    agent_mgr: &mut crate::agent::AgentManager,
    session_mgr: &mut crate::session::SessionManager,
    convo: &mut crate::convo::Convo,
    convo_path: &PathBuf,
    task_mgr: &mut crate::task::TaskManager,
    group_mgr: &mut crate::group::GroupManager,
    node: &mut crate::node::Node,
    state_path: &PathBuf,
) -> Result<bool, anyhow::Error> {
    match slash_cmd {
        "help" | "h" => {
            println!("{}Slash commands:{}", BOLD, RESET);
            println!("  /help         — this help");
            println!("  /skills       — list skills");
            println!("  /bridges      — list bridges with status");
            println!("  /priorities   — show/change bridge priorities");
            println!("  /priorities set <name> <1-5> — set priority");
            println!("  /priorities lock <name> — lock bridge (never offloaded)");
            println!("  /priorities unlock <name> — unlock bridge");
            println!("  /task         — /task create/assign/list/bind/done");
            println!("  /task create <title> <desc> [group] — create task");
            println!("  /task assign <id> <agent> — assign agent to task");
            println!("  /task list [group] — list tasks");
            println!("  /task bind <id> bridge|db|mcp <name> — bind resource");
            println!("  /task done <id> — complete task");
            println!("  /group        — /group create/list/invite");
            println!("  /group create <name> — create group");
            println!("  /group invite <name> <node> — invite node to group");
            println!("  /group mode <name> storm|hunt|synthesis|focus|watch — set group mode");
            println!("  /group next <name> — advance to next lifecycle mode");
            println!("  /agent        — /agent create <name> <skill> <node_id>");
            println!("  /status       — node status");
            println!("  /approvals    — show pending peer approval requests");
            println!("  /approve      — /approve <idx> to accept peer");
            println!("  /reject       — /reject <idx> to deny peer");
            println!("  /mode         — switch node mode (plan/execute/stop/log)");
            println!("  /groupmode    — switch group mode (storm/hunt/synthesis/focus/watch)");
            println!("  /chat         — send message: /chat <text>");
            println!("  /connect      — connect to peer: /connect <ip>");
            println!("  /sessions     — list sessions");
            println!("  /json         — output JSON format");
            println!("  /tui-agents   — list builtin TUI-converted agents");
            println!("  /exit         — shutdown");
        }
        "skills" => {
            let list = skill_reg.list();
            if list.is_empty() {
                println!("No skills loaded.");
            } else {
                println!("{}Skills ({}):{}", BOLD, list.len(), RESET);
                for s in &list {
                    let tags = s.manifest.tags.join(", ");
                    println!("  {} v{} — {} [{}]", s.manifest.name, s.manifest.version, s.manifest.description, tags);
                }
            }
        }
        "bridges" => {
            let bridges = bridge_pool.list_with_status();
            println!("{}Bridges ({}){}", BOLD, bridges.len(), RESET);
            for (name, enabled, reason) in &bridges {
                let icon = if *enabled { "✅" } else { "⛔" };
                if *enabled {
                    println!("  {} {}", icon, name);
                } else {
                    println!("  {} {} — {}", icon, name, reason);
                }
            }
        }
        "priorities" => {
            if slash_arg.is_empty() {
                // Show all bridges with priorities and status
                let bridges = bridge_pool.list_with_status();
                let mut msg = format!("{}Bridge priorities:{}", BOLD, RESET);
                for (name, enabled, reason) in &bridges {
                    let info = bridge_pool.info.get(name);
                    let prio = info.map(|i| i.priority).unwrap_or(3);
                    let bw = info.map(|i| i.bandwidth_kbps).unwrap_or(0);
                    let icon = if *enabled { "✅" } else { "⛔" };
                    msg.push_str(&format!("\n  {} {} — priority {}, {} Kbps", icon, name, prio, bw));
                    if !reason.is_empty() {
                        msg.push_str(&format!(" ({})", reason));
                    }
                }
                // Show governor status
                for (link_name, _) in &bridge_pool.governor.links {
                    msg.push_str(&format!("\n{}", bridge_pool.governor.status_message(&bridge_pool.info, link_name)));
                }
                println!("{}", msg);
            } else {
                // /priorities set <name> <priority>
                let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
                if parts.len() >= 3 && parts[0] == "set" {
                    let name = parts[1];
                    if let Ok(prio) = parts[2].parse::<u8>() {
                        if bridge_pool.set_priority(name, prio) {
                            println!("{}✓{} Bridge '{}' priority set to {}", GREEN, RESET, name, prio);
                            let changes = bridge_pool.governor.autoadjust(&mut bridge_pool.info);
                            for c in &changes { println!("  {}", c); }
                        } else { println!("Bridge '{}' not found.", name); }
                    }
                } else if parts.len() >= 2 && parts[0] == "lock" {
                    let name = parts[1];
                    if bridge_pool.lock(name) {
                        println!("{}🔒{} Bridge '{}' locked — never offloaded", GREEN, RESET, name);
                    } else { println!("Bridge '{}' not found.", name); }
                } else if parts.len() >= 2 && parts[0] == "unlock" {
                    let name = parts[1];
                    if bridge_pool.unlock(name) {
                        println!("{}🔓{} Bridge '{}' unlocked", YELLOW, RESET, name);
                    } else { println!("Bridge '{}' not found.", name); }
                }
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
            }
        }
        "task" => {
            let parts: Vec<&str> = slash_arg.splitn(4, ' ').collect();
            if parts.len() >= 3 && parts[0] == "create" {
                let title = parts[1];
                let desc = parts[2];
                let group = if parts.len() >= 4 { Some(parts[3]) } else { None };
                let t = task_mgr.create(title, desc, node.name(), group).await;
                println!("{}✓{} Task '{}' created (group: {})", GREEN, RESET, t.id, group.unwrap_or("none"));
                agent_journal.log("system", "task_created", &t.id);
            } else if parts.len() >= 3 && parts[0] == "assign" {
                let task_id = parts[1];
                let agent = parts[2];
                let node_id = if parts.len() >= 4 { parts[3] } else { "local" };
                match task_mgr.assign_agent(task_id, agent, node_id, "executor").await {
                    Some(t) => println!("{}✓{} Task {} assigned to {} @{}", GREEN, RESET, t.id, agent, node_id),
                    None => println!("Task '{}' not found.", task_id),
                }
            } else if parts.len() >= 2 && parts[0] == "list" {
                let filter = if parts.len() >= 3 { Some(parts[2]) } else { None };
                let tasks = if let Some(g) = filter { task_mgr.list_by_group(g).await } else { task_mgr.list().await };
                if tasks.is_empty() { println!("No tasks."); }
                else {
                    println!("{}Tasks ({}):{}", BOLD, tasks.len(), RESET);
                    for t in &tasks {
                        println!("  [{}] {} — {} [mode:{:?}] (group: {})",
                            t.id, t.title, t.status, t.mode, t.group.as_deref().unwrap_or("-"));
                        for e in &t.executors {
                            println!("    → {} @{} ({})", e.agent_id, e.node_id, e.role);
                        }
                    }
                }
            } else if parts.len() >= 3 && parts[0] == "bind" {
                let task_id = parts[1];
                let res_type = parts[2]; // "bridge", "db", "mcp"
                let name = if parts.len() >= 4 { parts[3] } else { "" };
                match task_mgr.bind_resource(task_id, res_type, name).await {
                    Some(t) => println!("{}✓{} {} bound to task {} {}", GREEN, RESET, res_type, t.id, name),
                    None => println!("Task '{}' not found.", task_id),
                }
            } else if parts.len() >= 2 && parts[0] == "done" {
                match task_mgr.complete(parts[1]).await {
                    Some(t) => println!("{}✓{} Task '{}' completed", GREEN, RESET, t.id),
                    None => println!("Task '{}' not found.", parts[1]),
                }
            } else { println!("Usage: /task create <title> <desc> [group] | assign <id> <agent> | list [group] | bind <id> <type> <name> | done <id>"); }
        }
        "group" => {
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            if parts.len() >= 2 && parts[0] == "create" {
                let name = parts[1];
                match group_mgr.create(name, "open") {
                    Ok(info) => {
                        println!("{}✓{} Group '{}' created (mode: {})", GREEN, RESET, name, info.mode);
                        gossip.add_group(name, &info.token).await;
                    }
                    Err(e) => println!("Error: {}", e),
                }
            } else if parts.len() >= 2 && parts[0] == "list" {
                let groups = group_mgr.list();
                if groups.is_empty() { println!("No groups."); }
                else {
                    println!("{}Groups:{}", BOLD, RESET);
                    for g in &groups {
                        println!("  {} — {} ({} members) [mode: {}]",
                            g.name, g.visibility, g.members.len(), g.mode);
                    }
                }
            } else if parts.len() >= 3 && parts[0] == "invite" {
                let name = parts[1];
                let node = parts[2];
                match group_mgr.add_member(name, node, "member") {
                    Ok(_) => println!("{}✓{} Node {} invited to group '{}'", GREEN, RESET, node, name),
                    Err(e) => println!("Error: {}", e),
                }
            } else if parts.len() >= 3 && parts[0] == "mode" {
                let name = parts[1];
                if let Some(mode) = crate::group::GroupMode::parse(parts[2]) {
                    match group_mgr.set_mode(name, mode) {
                        Ok(m) => println!("{}✓{} Group '{}' mode: {}", GREEN, RESET, name, m),
                        Err(e) => println!("Error: {}", e),
                    }
                } else { println!("Modes: storm, hunt, synthesis, focus, watch"); }
            } else if parts.len() >= 2 && parts[0] == "next" {
                let name = parts[1];
                match group_mgr.advance_mode(name) {
                    Ok(m) => println!("{}→{} Group '{}' advanced to {}", GREEN, RESET, name, m),
                    Err(e) => println!("Error: {}", e),
                }
            } else { println!("Usage: /group create|list|invite|mode|next"); }
        }
        "groupmode" => {
            if let Some(mode) = crate::group::GroupMode::parse(slash_arg) {
                // Apply to the first available group, or all groups
                let names = group_mgr.list_names();
                if names.is_empty() { println!("No groups available."); }
                else {
                    for name in &names {
                        group_mgr.set_mode(name, mode).ok();
                    }
                    println!("{}✓{} Group mode set to {} for {} groups", GREEN, RESET, mode, names.len());
                }
            } else { println!("Modes: storm, hunt, synthesis, focus, watch"); }
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
            match crate::tui_agent::assistant_chat(bridge_pool, slash_arg, session_mgr) {
                Ok(r) => println!("{}", r),
                Err(_) => { convo.handle(slash_arg); }
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
        "approvals" => {
            let pending = gossip.pending_list().await;
            if pending.is_empty() {
                println!("No pending peer approvals.");
            } else {
                println!("{}Pending approvals ({}):{}", BOLD, pending.len(), RESET);
                for (i, p) in pending.iter().enumerate() {
                    println!("  [{}] {} from {} — groups: {:?}",
                        i, p.node_name, p.address, p.groups);
                    println!("       /approve {} or /reject {}", i, i);
                }
            }
        }
        "approve" if !slash_arg.is_empty() => {
            if let Ok(idx) = slash_arg.parse::<usize>() {
                if let Some(peer) = gossip.approve_pending(idx).await {
                    // Try to connect to the approved peer
                    let addr = if peer.address.contains(":") {
                        let parts: Vec<&str> = peer.address.rsplitn(2, ':').collect();
                        let port_part = parts[0];
                        // The address format is IP:PORT from the TCP connection
                        format!("{}:{}", peer.address.trim_end_matches(&format!(":{}", port_part)), port_part)
                    } else {
                        peer.address.clone()
                    };
                    println!("{}✓{} Approved {} — connecting...", GREEN, RESET, peer.node_name);
                    gossip.direct_sync(&addr, channel_mgr.clone()).await.ok();
                    agent_journal.log("system", "peer_approved", &peer.node_name);
                } else {
                    println!("Invalid index.");
                }
            }
        }
        "reject" if !slash_arg.is_empty() => {
            if let Ok(idx) = slash_arg.parse::<usize>() {
                if let Some(peer) = gossip.reject_pending(idx).await {
                    println!("{}✗{} Rejected {} from {}", YELLOW, RESET, peer.node_name, peer.address);
                    agent_journal.log("system", "peer_rejected", &peer.node_name);
                } else {
                    println!("Invalid index.");
                }
            }
        }
        "json" => {
            println!("{{\"mode\":\"json\",\"status\":\"ok\",\"node\":\"{}\",\"bridges\":{}}}",
                node.name(), serde_json::to_string(&bridge_pool.list()).unwrap_or_default());
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
    bridge_pool: &BridgePool,
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
            println!("  dashboard         — open http://localhost:{}", api_port);
            println!("  exit              — shutdown");
        }
        "status" => {
            let peers = gossip.list_peers().await;
            let bridges = bridge_pool.list();
            println!("{0}Mode:{1}      {2}", BOLD, RESET, mode_engine.current);
            println!("{0}Node:{1}      {2}{3}{4}  ({5})", BOLD, RESET, CYAN, id_short, RESET, node.name());
            println!("{}Uptime:{}   {}s", BOLD, RESET, uptime);
            println!("{}Peers:{}   {}", BOLD, RESET, peers.len());
            for p in &peers {
                println!("  {}→{} {}{}{}", DIM, RESET, CYAN, p.node_name, RESET);
            }
            println!("{}Bridges:{} {}", BOLD, RESET, bridges.len());
            for b in &bridges {
                println!("  ✅ {}", b);
            }
            let groups = group_mgr.list();
            if !groups.is_empty() {
                println!("{}Groups:{}", BOLD, RESET);
                for g in &groups {
                    println!("  {} [{}] — {} members", g.name, g.mode, g.members.len());
                }
            }
            println!("{}API:{}     {}{}{}", BOLD, RESET, CYAN, format!("http://localhost:{}", api_port), RESET);
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
            match crate::tui_agent::assistant_chat(bridge_pool, text, session_mgr) {
                Ok(r) => println!("{}", r),
                Err(_) => demo_response(text),
            }
        }
        _ => {
            let text = cmd;
            match crate::tui_agent::assistant_chat(bridge_pool, text, session_mgr) {
                Ok(r) => println!("{}", r),
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
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(true)
}

pub async fn handle_convo(
    convo: &mut crate::convo::Convo, convo_path: &PathBuf, input: &str,
    task_mgr: &crate::task::TaskManager, agent_mgr: &crate::agent::AgentManager,
    group_mgr: &crate::group::GroupManager, gossip: &crate::gossip::GossipEngine,
    skill_reg: &crate::skill::SkillRegistry, _bridge_pool: &BridgePool,
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
                reply.push_str(&format!("  [{}] {} [mode:{:?}] — {}",
                    &t.id[..t.id.len().min(8)], t.title, t.mode, t.status));
                if let Some(ref agent) = t.assigned_to {
                    reply.push_str(&format!(" (назначен: {})", agent));
                }
                reply.push('\n');
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
            let agents_mine = agent_mgr.list_mine().len();
            let agents_peers = agent_mgr.list_from_peers().len();
            let peers = gossip.list_peers().await.len();
            let bridges = _bridge_pool.list().len();
            let tui_count = crate::tui_agent::builtin_tui_agents().len();

            let mut reply = format!("📊 Отчёт ноды {}:\n", convo.profile.name);
            reply.push_str(&format!("  Задачи: {} всего ({} выполнено, {} открыто)\n", tasks.len(), done, open));
            reply.push_str(&format!("  Агенты: {} своих + {} из других нод ({} TUI)\n", agents_mine, agents_peers, tui_count));
            reply.push_str(&format!("  Ноды: {} подключено\n", peers));
            reply.push_str(&format!("  Бриджи: {} зарегистрировано\n", bridges));
            return Some(reply);
        }
        ConvoAction::Setup => {
            let mut reply = "⚙️ Настройки ноды:\n".to_string();
            reply.push_str(&format!("  Имя: {}\n", convo.profile.name));
            reply.push_str(&format!("  NodeID: {}\n", node.id()));
            reply.push_str(&format!("  Бриджи: {}\n", _bridge_pool.list().join(", ")));
            reply.push_str(&format!("  TUI-агентов: {} встроено\n", crate::tui_agent::builtin_tui_agents().len()));
            return Some(reply);
        }
        ConvoAction::Response(text) => {
            return Some(text);
        }
    }
}
