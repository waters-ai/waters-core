import sys, os

with open(sys.argv[1]) as f:
    content = f.read()

# The function to inject
func_lines = r'''
async fn process_convo(
    convo: &mut convo::Convo, convo_path: &std::path::PathBuf, input: &str,
    task_mgr: &task::TaskManager, agent_mgr: &agent::AgentManager,
    group_mgr: &group::GroupManager, gossip: &gossip::GossipEngine,
    skill_reg: &skill::SkillRegistry, bridge_reg: &bridge::BridgeRegistry,
    node: &mut node::Node, state_path: &std::path::PathBuf,
    session_mgr: &mut session::SessionManager,
) {
    let response = convo.handle(input);
    if response.starts_with("CMD:") {
        let cmd = response.trim_start_matches("CMD:");
        match cmd {
            "exit" => { println!("Bye."); std::process::exit(0); }
            "tasks" => {
                let tasks = task_mgr.list().await;
                if tasks.is_empty() { println!("  No tasks."); return; }
                for t in &tasks {
                    let who = t.assigned_to.as_deref().unwrap_or("-");
                    println!("  [{}] {} - {} (by {})", &t.id[..8], t.title, t.status, who);
                }
            }
            "agents" => {
                let mine = agent_mgr.list_mine();
                if mine.is_empty() { println!("  No agents."); return; }
                for a in &mine {
                    println!("  {} {} - {} ({})", if a.agent_type == "shared" { "\xf0\x9f\x8c\x90" } else { "\xf0\x9f\x94\x92" }, a.name, a.role, a.owner_node);
                }
            }
            "groups" => {
                let groups = group_mgr.list();
                if groups.is_empty() { println!("  No groups."); return; }
                for g in &groups {
                    println!("  {} ({} members) {}", g.name, g.members.len(), g.visibility);
                }
            }
            "peers" => {
                let peers = gossip.list_peers().await;
                if peers.is_empty() { println!("  No peers."); return; }
                for p in &peers {
                    let addr = p.addresses.first().map(|a| a.as_str()).unwrap_or("?");
                    println!("  {} ({})", p.node_name, addr);
                }
            }
            "report" => {
                let tasks = task_mgr.list().await;
                let done = tasks.iter().filter(|t| t.status == "done").count();
                let total = tasks.len();
                let agents_mine = agent_mgr.list_mine().len();
                let peers = gossip.list_peers().await.len();
                let skills = skill_reg.list().len();
                println!("  Tasks: {} done / {}", done, total);
                println!("  Agents: {} | Peers: {} | Skills: {}", agents_mine, peers, skills);
            }
            "setup" => {
                println!("  Name: {}", convo.profile.name);
                println!("  Bridges:");
                for b in bridge_reg.list() {
                    println!("    {} {} - {}", if b.connected { "\xe2\x9c\x85" } else { "\xe2\xac\x9c" }, b.name, b.description);
                }
            }
            _ => println!("{}", response),
        }
    } else {
        println!("{}", response);
    }
}
'''

# Insert before demo_tools
marker = b'\nasync fn demo_tools('
idx = content.find(marker)
if idx >= 0:
    content = content[:idx] + func_lines + content[idx:]
    with open(sys.argv[1], 'w') as f:
        f.write(content)
    print("Inserted", len(func_lines), "chars")
else:
    print("Marker not found")
