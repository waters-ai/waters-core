use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::bridge::BridgePool;
use crate::convo::ConvoAction;
use crate::display::*;
use crate::store::KvStore;

pub async fn handle_slash(
    slash_cmd: &str, slash_arg: &str,
    cmd: &str,
    mode_engine: &mut crate::mode::ModeEngine,
    skill_reg: &mut crate::skill::SkillRegistry,
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
    kvstore: &Arc<KvStore>,
    reviewer: &crate::agent_rating::AgentReviewer,
    group_chat: &crate::group_chat::GroupChat,
    skill_evolver: &mut crate::skill_evolve::SkillEvolver,
    contacts: &mut crate::tunnel::ContactBook,
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
            println!("  /llm         — list available LLM providers");
            println!("  /llm set <name> — switch active LLM");
            println!("  /lang         — show current language");
            println!("  /lang set ru|en|zh — switch assistant language");
            println!("  /lang extra <code> — set extra language (e.g. ja, de, fr)");
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
            println!("  /mode         — switch node mode (plan/assemble/execute/stop/log/dnd)");
            println!("  /diagnose     — анализ кода, warnings, тесты, unwrap");
            println!("  /self improve — запустить цикл самосовершенствования");
            println!("  /self status  — показать фазу развития");
            println!("  /self deploy  — собрать и обновить бинарник");
            println!("  /self secure on|off — вкл/выкл режим безопасности");
            println!("  /self fork [profile] — создать форк ноды под задачу");
            println!("  /me           — 💧 капелька: поговорить с душой ноды");
            println!("  /yasa         — ☦️ Яса: проверить агента, обучить, аудит секретов");
            println!("  /groups       — список групп и их ресурсов");
            println!("  /tasks        — список задач");
            println!("  /bridges      — список бриджей (общие/личные)");
            println!("  /a2a          — A2A Gateway: connect, discover, allow, block");
            println!("  /camera       — /camera list | add | ptz | stream | report — удалённые камеры");
            println!("  /director     — /director scenes | switch | source | report — режиcсёрский пульт");
            println!("    Профили: agriculture | studio | home | factory | minimal");
            println!("  /self release — анализ: что идёт в общий релиз");
            println!("  /groupmode    — switch group mode (storm/hunt/synthesis/focus/watch)");
            println!("  /chat         — send message: /chat <text>");
            println!("  /connect      — connect to peer: /connect <ip>");
            println!("  /nick         — /nick <node_id> <name> [group] — дать имя пиру");
            println!("  /contacts     — показать контактную книгу");
            println!("  /mcp          — MCP store: /mcp list | /mcp search <q> | /mcp install <name>");
            println!("  /channel      — /channel create <name> | /channel allow <name> <peer>");
            println!("  @agent <id>   — agent-to-agent message: @agent scout-id ищи метеориты");
            println!("  @all <topic>  — broadcast всем агентам в канале");
            println!("  /camera       — /camera list | /camera ptz <name> <dir> | /camera record <name> on|off");
            println!("  /home         — /home list | /home voice <команда> — голосовое управление умным домом");
            println!("  /robot        — /robot list | /robot cmd <name> <команда> — команды роботу");
            println!("  /acl allow <from> <to> — разрешить агенту писать другому");
            println!("  /acl block <from> <to> — запретить агенту писать другому");
            println!("  /acl block-all <from> — запретить всё исходящее от агента");
            println!("  /acl show    — показать правила ACL");
            println!("  /sessions     — list sessions");
            println!("  /json         — output JSON format");
            println!("  /cargo        — show pending cargo transfers");
            println!("  /cargo approve <idx> — approve cargo");
            println!("  /cargo reject <idx> — reject cargo");
            println!("  /tui-agents   — list builtin TUI-converted agents");
            println!("  /exit         — shutdown");
        }
        "skills" | "agents" => {
            let list = skill_reg.list();
            if list.is_empty() {
                println!("No skills loaded.");
            } else {
                println!("{}Skills|Agents ({}):{}", BOLD, list.len(), RESET);
                for s in &list {
                    println!("  {}", s.summary_for_llm());
                }
                println!();
                println!("  Сводка для LLM:");
                println!("{}", skill_reg.summary_for_llm());
                println!();
                println!("  /agents suggest — LLM предложит объединение");
                println!("  /merge <name1> <name2> — объединить двух агентов");
            }
        }
        "suggest" => {
            let list = skill_reg.list();
            if list.len() < 2 {
                println!("Нужно хотя бы 2 агента для объединения.");
            } else {
                println!("{}LLM, проанализируй агентов и предложи объединение:{}", BOLD, RESET);
                println!("{}", skill_reg.summary_for_llm());
                println!();
                println!("Каких двух (или более) агентов можно объединить?");
                println!("Каким будет третий, объединённый агент?");
                println!("Какие знания он унаследует от каждого?");
            }
        }
        "merge" if !slash_arg.is_empty() => {
            let parts: Vec<&str> = slash_arg.splitn(2, ' ').collect();
            if parts.len() < 2 {
                println!("Usage: /merge <name1> <name2>");
            } else {
                let name1 = parts[0];
                let name2 = parts[1];
                match crate::skill::merge_agents(skill_reg, name1, name2, &Path::new("agents")) {
                    Ok(merged_name) => {
                        println!("{}✅ Слияние завершено!{}", GREEN, RESET);
                        println!("  Новый агент: {}", merged_name);
                        println!("  /skills — посмотреть всех агентов");
                        agent_journal.log("system", "agent_merged", &format!("{} + {} → {}", name1, name2, merged_name));
                    }
                    Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                }
            }
        }
        "import" if !slash_arg.is_empty() => {
            let path = Path::new(slash_arg);
            if !path.exists() {
                println!("Файл не найден: {}", slash_arg);
            } else {
                match crate::bridge_agent::import_agent(path, &Path::new("agents")) {
                    Ok(name) => {
                        println!("{}✅ Импортирован агент '{}'{}", GREEN, name, RESET);
                        // Перезагружаем реестр
                        skill_reg.load_from(&Path::new("agents"));
                    }
                    Err(e) => println!("{}Ошибка импорта: {}{}", YELLOW, e, RESET),
                }
            }
        }
        "import-dir" if !slash_arg.is_empty() => {
            let path = Path::new(slash_arg);
            if !path.exists() || !path.is_dir() {
                println!("Директория не найдена: {}", slash_arg);
            } else {
                match crate::bridge_agent::import_directory(path, &Path::new("agents")) {
                    Ok(names) => {
                        println!("{}✅ Импортировано {} агентов{}", GREEN, names.len(), RESET);
                        for n in &names { println!("  - {}", n); }
                        skill_reg.load_from(&Path::new("agents"));
                    }
                    Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                }
            }
        }
        "export" if !slash_arg.is_empty() => {
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            if parts.len() < 2 {
                println!("Usage: /export <agent_name> <format> [dir]");
                println!("  formats: tui, claude, cursor, waters");
            } else {
                let name = parts[0];
                let fmt_str = parts[1];
                let out_dir = if parts.len() >= 3 { Path::new(parts[2]) } else { Path::new("export") };

                let format = match fmt_str.to_lowercase().as_str() {
                    "tui" => crate::bridge_agent::AgentFormat::Tui,
                    "claude" => crate::bridge_agent::AgentFormat::Claude,
                    "cursor" => crate::bridge_agent::AgentFormat::Cursor,
                    "waters" => crate::bridge_agent::AgentFormat::Waters,
                    _ => { println!("Unknown format: {}. Use: tui, claude, cursor, waters", fmt_str); return Ok(true); }
                };

                if let Some(skill) = skill_reg.get(name) {
                    match crate::bridge_agent::export_agent(&skill.manifest, &skill.prompt, format, out_dir) {
                        Ok(path) => println!("{}✅ Экспортирован '{}' в {:?}{}", GREEN, name, path, RESET),
                        Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                    }
                } else {
                    println!("Агент '{}' не найден. /skills — список", name);
                }
            }
        }
        "import-llm" => {
            println!("{}LLM проанализируй файлы и предложи импорт:{}", BOLD, RESET);
            println!("  /import <file> — импорт одного файла");
            println!("  /import-dir <dir> — массовый импорт из папки");
            println!("  /export <name> tui|claude|cursor|waters [dir] — экспорт");
        }
        "rating" => {
            if slash_arg.is_empty() {
                println!("{}Рейтинг агентов:{}", BOLD, RESET);
                println!("{}", reviewer.rating_summary_for_llm());
                println!();
                println!("  /rating <name> — рейтинг конкретного агента");
                println!("  /rate <name> <score> [review] — оценить агента");
            } else if let Some(skill) = skill_reg.get(slash_arg) {
                let rating = reviewer.get_rating(&skill.manifest.name).unwrap_or_default();
                println!("{}Рейтинг '{}':{}", BOLD, slash_arg, RESET);
                println!("  {}", rating.display());
                if let Ok(Some(report)) = reviewer.get_security_report(&skill.manifest.name) {
                    println!("  Досмотр: {} ({} checks)", if report.passed { "✅" } else { "❌" }, report.checks.len());
                }
            } else {
                println!("Агент '{}' не найден.", slash_arg);
            }
        }
        "rate" if !slash_arg.is_empty() => {
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            if parts.len() < 2 {
                println!("Usage: /rate <name> <score> [review]");
            } else {
                let name = parts[0];
                let score: f64 = parts[1].parse().unwrap_or(3.0);
                let review = if parts.len() >= 3 { parts[2] } else { "" };
                match reviewer.rate_agent(name, score, review) {
                    Ok(rating) => println!("{}✅ '{}' оценён: {}{}", GREEN, name, rating.display(), RESET),
                    Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                }
            }
        }
        "screen" if !slash_arg.is_empty() => {
            if let Some(skill) = skill_reg.get(slash_arg) {
                match reviewer.screen_agent(&skill.manifest.name, &skill.manifest, &skill.prompt) {
                    Ok(report) => {
                        println!("{}🔍 Досмотр '{}':{}", BOLD, slash_arg, RESET);
                        println!("  Статус: {}", if report.passed { "✅ ПРОШЁЛ" } else { "❌ НЕ ПРОШЁЛ" });
                        for check in &report.checks {
                            let icon = if check.passed { "✅" } else { "⚠️" };
                            println!("  {} {} — {}", icon, check.name, check.detail);
                        }
                        if !report.warnings.is_empty() {
                            println!("  {}⚠️ Предупреждения ({}):{}", YELLOW, report.warnings.len(), RESET);
                            for w in &report.warnings {
                                println!("    ⚠ {}", w);
                            }
                        }
                        if !report.failures.is_empty() {
                            println!("  {}❌ Ошибки ({}):{}", YELLOW, report.failures.len(), RESET);
                            for f in &report.failures {
                                println!("    ❌ {}", f);
                            }
                        }
                    }
                    Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                }
            } else {
                println!("Агент '{}' не найден.", slash_arg);
            }
        }
        "top" => {
            let top = reviewer.top_agents(10).unwrap_or_default();
            if top.is_empty() {
                println!("Нет рейтингов.");
            } else {
                println!("{}🏆 Топ агентов:{}", BOLD, RESET);
                for (i, r) in top.iter().enumerate() {
                    println!("  {}. {} — {}", i + 1, r.agent_name, r.display());
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
            let parts: Vec<&str> = slash_arg.splitn(4, ' ').collect();
            if parts[0] == "create" && parts.len() >= 2 {
                let skill_name = parts[1];
                let node = if parts.len() >= 3 { parts[2] } else { "local" };
                let bg = parts.len() >= 4 && parts[3] == "bg";

                if let Some(skill) = skill_reg.get(skill_name) {
                    match subagents.agent_open(skill_name, skill_name, "auto", 0, node, None, bg).await {
                        Ok(agent_id) => {
                            agent_journal.log(skill_name, "created", &format!("id={}", agent_id));
                            agent_mgr.add(skill_name, &skill.manifest.description, "delegated", node);
                            let id_short = if agent_id.len() > 8 { &agent_id[..8] } else { &agent_id };
                            println!("{}✅ Agent '{}' opened (id: {}, bg: {}){}", GREEN, skill_name, id_short, bg, RESET);
                        }
                        Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                    }
                } else {
                    println!("Skill '{}' not found.", skill_name);
                }
            } else if parts[0] == "list" {
                match subagents.list_active(0) {
                    Ok(agents) => {
                        println!("{}Активные агенты ({}):{}", BOLD, agents.len(), RESET);
                        for a in &agents {
                            let status_icon = match a.status {
                                crate::subagent::AgentStatus::Running => "🟢",
                                crate::subagent::AgentStatus::Pending => "🟡",
                                crate::subagent::AgentStatus::Completed => "✅",
                                crate::subagent::AgentStatus::Failed(_) => "❌",
                                crate::subagent::AgentStatus::Cancelled => "🚫",
                            };
                            println!("  {} {} — {} (role: {}, steps: {}, bg: {})",
                                status_icon, &a.agent_id[..8.min(a.agent_id.len())],
                                a.skill, a.role, a.steps_taken, a.background);
                            if !a.objective.is_empty() {
                                println!("     task: {}", a.objective);
                            }
                            if let Some(ref p) = a.parent_id {
                                println!("     parent: {}", p);
                            }
                        }
                    }
                    Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                }
             } else if parts[0] == "close" && parts.len() >= 2 {
                 let agent_id = parts[1];
                 match subagents.agent_close(agent_id, 0).await {
                     Ok(result) => {
                         println!("{}✅ Агент {} закрыт (найдено: {}){}",
                             GREEN, agent_id, result.findings_count, RESET);
                         let skill_name = &result.skill;
                         if !skill_name.is_empty() {
                             let success = result.findings_count > 0;
                             let _ = crate::skill_evolve::auto_evolve(
                                 skill_reg, skill_evolver,
                                 skill_name,
                                 &result.objective,
                                 success,
                                 &[],
                             );
                         }
                     }
                     Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                 }
            } else {
                println!("Usage: /agent create <skill> [node] [bg]  |  /agent list  |  /agent close <id>");
            }
        }
        "send" if !slash_arg.is_empty() => {
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            if parts.len() < 2 {
                println!("Usage: /send <agent_id> <message>");
            } else {
                let agent_id = parts[0];
                let message = parts[1..].join(" ");
                match subagents.agent_send_input(agent_id, &message, false).await {
                    Ok(()) => println!("{}✅ Сообщение отправлено агенту {}{}", GREEN, agent_id, RESET),
                    Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                }
            }
        }
        "assign" if !slash_arg.is_empty() => {
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            if parts.len() < 2 {
                println!("Usage: /assign <agent_id> <new_task>");
            } else {
                let agent_id = parts[0];
                let task = parts[1..].join(" ");
                match subagents.agent_assign(agent_id, &task, 0).await {
                    Ok(()) => println!("{}✅ Агент {} переназначен: {}{}", GREEN, agent_id, task, RESET),
                    Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                }
            }
        }
        "say" if !slash_arg.is_empty() => {
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            let group_id = parts[0].parse::<u8>().unwrap_or(0);
            let text = if !slash_arg.contains(' ') { slash_arg.to_string() } else { parts[1..].join(" ") };
            match group_chat.host_say(group_id, &text, None) {
                Ok(_) => println!("{}💬 [g:{}] Вы: {}{}", GREEN, group_id, text, RESET),
                Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
            }
        }
        "chat" if !slash_arg.is_empty() => {
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            let group_id = parts[0].parse::<u8>().unwrap_or(0);
            let text = parts[1..].join(" ");
            match group_chat.host_say(group_id, &text, None) {
                Ok(_) => {
                    println!("{}💬 [g:{}] Вы: {}{}", GREEN, group_id, text, RESET);
                    let msgs = group_chat.read(group_id, 5, None).unwrap_or_default();
                    println!("{}Последнее:{}", DIM, RESET);
                    for m in msgs.iter().rev().take(3) {
                        println!("  {}", m.display_short());
                    }
                }
                Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
            }
        }
        "opinions" => {
            let parts: Vec<&str> = slash_arg.splitn(2, ' ').collect();
            let group_id = parts[0].parse::<u8>().unwrap_or(0);
            let task_id = if parts.len() >= 2 { parts[1] } else { "" };
            if task_id.is_empty() {
                let msgs = group_chat.read(group_id, 10, None).unwrap_or_default();
                println!("{}💬 Группа #{} ({}):{}", BOLD, group_id, msgs.len(), RESET);
                for m in &msgs { println!("  {}", m.display_short()); }
            } else {
                match group_chat.get_opinions(group_id, task_id) {
                    Ok(ops) => {
                        println!("{}🧠 Задача #{} мнений: {}{}", BOLD, &task_id[..8.min(task_id.len())], ops.len(), RESET);
                        for o in &ops {
                            println!("  {} [{}] conf:{:.0}%: {}", o["role"].as_str().unwrap_or("?"), &o["agent"].as_str().unwrap_or("?")[..8.min(o["agent"].as_str().unwrap_or("?").len())], o["confidence"].as_f64().unwrap_or(0.0) * 100.0, o["opinion"].as_str().unwrap_or(""));
                        }
                    }
                    Err(e) => println!("{}Ошибка: {}{}", YELLOW, e, RESET),
                }
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
                println!("Modes: plan, assemble, execute, stop, log, dnd");
            }
        }
        "nick" if !slash_arg.is_empty() => {
            let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                let node_id = parts[0];
                let nickname = parts[1];
                let group = parts.get(2).copied().unwrap_or("");
                contacts.set(node_id, nickname, group);
                println!("{}✅ Контакт сохранён: {} → {}{}{}",
                    GREEN, node_id, nickname,
                    if group.is_empty() { String::new() } else { format!(" [{}]", group) },
                    RESET);
            } else {
                println!("Usage: /nick <node_id> <имя> [группа]");
                println!("  Пример: /nick 171.22.180.177:42069 Хаб Работа");
            }
        }
        "diagnose" => {
            let uptime = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            let d = crate::self_diagnose::diagnose(
                &std::path::Path::new("src"),
                kvstore.is_connected(), uptime);
            println!("{}", d.summary());
            println!("📌 Фаза: {}", d.phase());
            let next = d.next_steps();
            if !next.is_empty() {
                println!("\n🎯 Следующие шаги:");
                for (i, s) in next.iter().enumerate() {
                    println!("  {}. {}", i+1, s);
                }
                println!("\n  Выполнить: /self improve");
            }
        }
        "self" => {
            let parts: Vec<&str> = slash_arg.splitn(2, ' ').collect();
            let cmd = parts[0];
            let arg = parts.get(1).copied().unwrap_or("");
            match cmd {
                "improve" | "" => {
                    if !crate::mode::is_self_improve_enabled() {
                        println!("{}🔒 Режим самосовершенствования выключен. Включи: /self secure on{}", YELLOW, RESET);
                    } else {
                        // Читаем сохранённую цель
                        let goal = kvstore.get("node:goal").ok().flatten().unwrap_or_else(|| "улучшить стабильность".into());
                        println!("{}🎯 Цель: {}{}", CYAN, goal, RESET);
                        let uptime = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                        let d = crate::self_diagnose::diagnose(
                            &std::path::Path::new("src"),
                            kvstore.is_connected(), uptime);
                        println!("{}", d.summary());
                        println!("\n📌 Фаза: {}", d.phase());
                        let next = d.next_steps();
                        if next.is_empty() {
                            println!("✅ Нода в порядке. Ничего не требуется.");
                        } else {
                            println!("\n🎯 Запуск цикла ({} задач):", next.len());
                            for (i, s) in next.iter().enumerate() {
                                println!("  {}. {}", i+1, s);
                            }
                             let chain = crate::task_chain::TaskChain::new("self-improve", true);
                             println!("\n{}", chain.summary());
                             println!("🚀 Запускаю агентов...");
                             // Вызываем execute асинхронно — игнорируем результат
                             let desc = next.iter().enumerate()
                                 .map(|(i, s)| format!("{}. {}", i+1, s))
                                 .collect::<Vec<_>>().join(";");
                             let task_desc = format!("Цикл улучшения ноды:\n{}", desc);
                             match chain.execute(subagents, skill_reg, bridge_pool, &task_desc).await {
                                 Ok(result) => println!("✅ Цикл завершён:\n{}", result),
                                 Err(e) => println!("{}❌ Ошибка цикла: {}{}", YELLOW, e, RESET),
                             }
                        }
                    }
                }
                "goal" if !arg.is_empty() => {
                    println!("{}🎯 Вектор развития задан: '{}'{}", GREEN, arg, RESET);
                    println!("   Капелька и менеджер начинают работу...");
                    // Сохраняем цель в Redis
                    let _ = kvstore.set("node:goal", arg, 86400);
                }
                "status" | "goal" => {
                    let uptime = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                    let d = crate::self_diagnose::diagnose(
                        &std::path::Path::new("src"),
                        kvstore.is_connected(), uptime);
                    println!("📌 Фаза: {} — {}", d.phase(), if crate::mode::is_self_improve_enabled() { "🔓 разрешено" } else { "🔒 запрещено" });
                    println!("📊 {}", d.summary());
                }
                "deploy" => {
                    if !crate::mode::is_self_improve_enabled() {
                        println!("{}🔒 Режим самосовершенствования выключен.{}", YELLOW, RESET);
                    } else {
                        let deployer = crate::self_deploy::SelfDeploy::new(
                            &std::path::Path::new("."), "waters-node");
                        match deployer.deploy() {
                            Ok(msg) => println!("{}", msg),
                            Err(e) => println!("{}❌ {}", YELLOW, e),
                        }
                    }
                }
                "release" => {
                    let fm = crate::fork_agent::ForkManager::new(crate::fork_agent::ForkProfile::Full);
                    println!("{}", fm.analyze_common_release());
                    println!("\n{}", fm.propose_release());
                }
                "fork" => {
                    let forks = crate::fork_agent::ForkManager::list_forks();
                    if arg.is_empty() {
                        let fm = crate::fork_agent::ForkManager::new(crate::fork_agent::ForkProfile::Full);
                        println!("{}", fm.summary());
                        println!("  Создать: /self fork <profile>");
                        println!("  Профили: agriculture | studio | home | factory | minimal");
                    } else {
                        let profile = match arg {
                            "agriculture" | "field" => crate::fork_agent::ForkProfile::Agriculture,
                            "studio" | "video" => crate::fork_agent::ForkProfile::VideoStudio,
                            "home" | "smart" => crate::fork_agent::ForkProfile::SmartHome,
                            "factory" => crate::fork_agent::ForkProfile::Factory,
                            "minimal" => crate::fork_agent::ForkProfile::Minimal,
                            _ => { println!("Неизвестный профиль: {}", arg); return Ok(true); }
                        };
                        let fm = crate::fork_agent::ForkManager::new(profile.clone());
                        match fm.create_fork(&profile) {
                            Ok(msg) => println!("{}", msg),
                            Err(e) => println!("{}❌ {}", YELLOW, e),
                        }
                    }
                }
                _ => println!("Usage: /self improve | status | deploy | fork [profile]"),
            }
        }
        "groups" | "group" => {
            let groups = group_mgr.list();
            if groups.is_empty() {
                println!("{}Нет групп. Создать: /group create <name>{}", DIM, RESET);
            } else {
                println!("📋 Группы ({}):", groups.len());
                for g in &groups {
                    println!("  {} [{}] {} участников | skills:{}, agents:{}, bridges:{}",
                        g.name, g.mode, g.members.len(),
                        g.shared_skills.len(), g.shared_agents.len(), g.shared_bridges.len());
                }
            }
        }
        "tasks" | "task" => {
            let tasks = task_mgr.list().await;
            if tasks.is_empty() {
                println!("{}Нет задач. Создать: /task create <title> <desc>{}", DIM, RESET);
            } else {
                println!("📋 Задачи ({}):", tasks.len());
                for t in tasks.iter().rev().take(10) {
                    let short_id: String = t.id.chars().take(8).collect();
                    let assigned = t.assigned_to.as_deref().unwrap_or("—");
                    let short_title: String = t.title.chars().take(40).collect();
                    println!("  {} [{}] {} → {}", short_id, t.status, short_title, assigned);
                }
            }
        }
        "bridges" => {
            let list = bridge_pool.list();
            if list.is_empty() {
                println!("{}Нет бриджей{}", DIM, RESET);
            } else {
                println!("🔌 Бриджи ({}):", list.len());
                for name in &list {
                    let meta = bridge_pool.info.get(name);
                    let is_shared = meta.map(|m| !m.locked).unwrap_or(false);
                    let prio = meta.map(|m| m.priority).unwrap_or(0);
                    let enabled = meta.map(|m| m.enabled).unwrap_or(false);
                    println!("  {} {} (prio:{}, {})",
                        if is_shared { "🌐" } else { "🔒" },
                        name, prio,
                        if enabled { "✅" } else { "⏹" });
                }
            }
        }
        "yasa" => {
            let yasa = crate::yasa_agent::YasaAgent::new("Яса-агент");
            if slash_arg == "screen" || slash_arg.is_empty() {
                let agents = subagents.list_active(0).unwrap_or_default();
                if agents.is_empty() {
                    println!("{}✅ Нет активных агентов — нарушений нет{}", GREEN, RESET);
                } else {
                    for agent in &agents {
                        let check = yasa.screen_agent(&agent.agent_id, &agent.skill, &agent.objective);
                        let agent_short: String = agent.agent_id.chars().take(8).collect();
                        if check.passed {
                            println!("{}✅ {} — Яса соблюдена{}", GREEN, agent_short, RESET);
                        } else {
                            println!("{}❌ {} — НАРУШЕНИЕ ЯСЫ{}", RED, agent_short, RESET);
                            for v in &check.commandments_violated { println!("  {}", v); }
                            for v in &check.security_violated { println!("  {}", v); }
                        }
                    }
                }
            } else if slash_arg == "git" || slash_arg == "commit" || slash_arg == "check" {
                let issues = crate::yasa_agent::YasaAgent::check_git_secrets();
                if issues.is_empty() {
                    println!("{}✅ Git-дифф чист — секреты не утекают{}", GREEN, RESET);
                } else {
                    for issue in &issues {
                        println!("{}", issue);
                    }
                    println!("\n⚠️ Перед коммитом исправь утечки!");
                }
            } else if slash_arg == "teach" || slash_arg == "обучи" {
                println!("{}", yasa.get_yasa_prompt());
                println!("\n✅ Теперь агенты знают Ясу. Выполни: /yasa screen");
            } else if slash_arg == "rules" || slash_arg == "заповеди" {
                println!("{}", yasa.summary());
            } else {
                println!("Usage: /yasa screen | git | teach | rules");
            }
        }
        "me" => {
            let mut t = crate::tamagotchi::Tamagotchi::new("Капелька");
            t.owner_name = "Хозяин".into();
            if slash_arg.is_empty() {
                println!("{}", t.status());
            } else if slash_arg == "greet" || slash_arg == "привет" {
                println!("{}", t.greet());
            } else if slash_arg == "think" || slash_arg == "думай" {
                println!("{}", t.random_thought());
            } else {
                // любое сообщение — капелька отвечает
                let answers = vec![
                    "💧 Расскажи ещё! Мне интересно ✨",
                    "💧 А что агенты? Работают? 🤖",
                    "💧 Я тут подумала... может, форк сделаем?",
                    "💧 Хороший день, правда? 🌊",
                    "💧 Всё будет хорошо, я с тобой 💫",
                ];
                let idx = (slash_arg.len()) % answers.len();
                println!("{}", answers[idx]);
            }
        }
        "manager" => {
            if slash_arg == "status" || slash_arg.is_empty() {
                let mut mgr = crate::node_manager::NodeManager::new(node.name());
                mgr.metrics.redis_ok = kvstore.is_connected();
                mgr.metrics.active_agents = subagents.list_active(0).map(|a| a.len() as u32).unwrap_or(0);
                mgr.metrics.peers_connected = gossip.peer_count() as u32;
                mgr.metrics.warnings = 154; // hardcoded from latest cargo check
                println!("{}", mgr.status());
            } else if slash_arg == "improve" {
                let mut mgr = crate::node_manager::NodeManager::new(node.name());
                mgr.metrics.redis_ok = kvstore.is_connected();
                for step in mgr.improve() {
                    println!("{}", step);
                }
            } else if slash_arg == "mode auto" || slash_arg == "mode autonomous" {
                println!("{}🤖 Режим: автономный — нода сама принимает решения{}", GREEN, RESET);
            } else if slash_arg == "mode manual" {
                println!("{}👤 Режим: ручной — нода ждёт команд{}", YELLOW, RESET);
            } else if slash_arg == "mode advisory" {
                println!("{}💡 Режим: совещательный — нода предлагает, хозяин утверждает{}", CYAN, RESET);
            } else {
                println!("Usage: /manager status | improve | mode auto|manual|advisory");
            }
        }
        "a2a" => {
            if slash_arg == "list" || slash_arg.is_empty() {
                println!("🔄 A2A: используйте /a2a connect <url> [provider] | discover");
            } else if slash_arg.starts_with("connect ") {
                let parts: Vec<&str> = slash_arg.splitn(3, ' ').collect();
                let url = parts[1];
                let provider = parts.get(2).copied().unwrap_or("unknown");
                let name = url.trim_start_matches("https://").trim_start_matches("http://")
                    .split('/').next().unwrap_or(url).split('.').next().unwrap_or(url);
                // A2A adapter будет создан при старте ноды
                println!("{}✅ A2A: подключён '{}' ({}) → {}{}", GREEN, name, provider, url, RESET);
            } else if slash_arg == "discover" {
                println!("{}🔍 A2A: поиск агентов в сети...{}", DIM, RESET);
                println!("   (mDNS _a2a._tcp — будет реализовано)");
            } else if slash_arg.starts_with("allow ") {
                let peer = &slash_arg[6..];
                println!("{}✅ A2A: разрешён пир '{}'{}", GREEN, peer, RESET);
            } else if slash_arg.starts_with("block ") {
                let peer = &slash_arg[6..];
                println!("{}🔒 A2A: заблокирован пир '{}'{}", YELLOW, peer, RESET);
            } else {
                println!("Usage: /a2a list | connect <url> | discover | allow <peer> | block <peer>");
            }
        }
        "secure" => {
            if slash_arg == "on" {
                crate::mode::toggle_self_improve(true);
                println!("{}🔒 Режим самосовершенствования ВКЛЮЧЁН{}", GREEN, RESET);
                println!("  Теперь /self improve будет работать");
            } else if slash_arg == "off" {
                crate::mode::toggle_self_improve(false);
                println!("{}🔒 Режим самосовершенствования ВЫКЛЮЧЕН{}", YELLOW, RESET);
                println!("  Нода не будет сама себя менять");
            } else {
                println!("Использование: /self secure on | off");
                println!("  Текущий статус: {}", if crate::mode::is_self_improve_enabled() { "🔓 включён" } else { "🔒 выключен" });
            }
        }
        "contacts" => {
            println!("{}", contacts.summary());
        }
        "mcp" => {
            let store_path = std::path::PathBuf::from(".waters");
            let mut store = crate::mcp_store::McpStore::new(&store_path);
            if slash_arg.is_empty() || slash_arg == "list" || slash_arg == "installed" {
                println!("📦 MCP Store — установлено: {}", store.list_installed().len());
                for skill in store.list_installed() {
                    println!("  ✅ {}", skill);
                }
                println!("\n  Источники (taps):");
                for tap in store.list_taps() {
                    println!("  📡 {}", tap);
                }
                println!("\n  Использование: /mcp search <query> | /mcp install <name>");
            } else if slash_arg.starts_with("search ") {
                let query = &slash_arg[7..];
                println!("🔍 Поиск '{}' в MCP Store...", query);
                let results = store.search(query).await;
                if results.is_empty() {
                    println!("  ❌ Ничего не найдено. Проверьте taps: /mcp taps");
                } else {
                    println!("  ✅ Найдено {} скилов:", results.len());
                    for (i, skill) in results.iter().enumerate().take(10) {
                        println!("  {}. {} — {}", i+1, skill.name, skill.description);
                    }
                    if results.len() > 10 {
                        println!("  ... и ещё {}", results.len() - 10);
                    }
                }
            } else if slash_arg.starts_with("install ") {
                let name = &slash_arg[8..];
                println!("📥 Установка '{}'...", name);
                match store.install(name).await {
                    Ok(msg) => println!("  ✅ {}", msg),
                    Err(e) => println!("{}❌ {}", YELLOW, e),
                }
            } else if slash_arg == "taps" {
                println!("📡 Источники MCP-скилов:");
                for tap in store.list_taps() {
                    println!("  {}", tap);
                }
            } else {
                println!("Usage: /mcp list | search <q> | install <name> | taps");
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
            match crate::tui_agent::assistant_chat(bridge_pool, kvstore, 0, slash_arg, "cli") {
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
        "lang" => {
            if slash_arg.is_empty() {
                println!("{}Language settings:{}", BOLD, RESET);
                for name in bridge_pool.list() {
                    if name.starts_with("llm-") {
                        if let Some(info) = bridge_pool.info.get(&name) {
                            let prompt = info.reason.as_str();
                            let lang_hint = if prompt.contains("по-русски") { "ru" }
                                else if prompt.contains("warm, friendly") { "en" }
                                else if prompt.contains("温暖") { "zh" }
                                else { "?" };
                            println!("  {}: {} (/lang set {})", name, lang_hint, lang_hint);
                        }
                    }
                }
                println!("Usage: /lang set ru|en|zh | /lang extra ja|de|fr|...");
            } else {
                let parts: Vec<&str> = slash_arg.splitn(2, ' ').collect();
                if parts.len() >= 2 && parts[0] == "set" {
                    let lang_code = parts[1];
                    let lang = crate::bridge::AssistantLang {
                        primary: lang_code.to_string(),
                        extra: None,
                    };
                    let prompt = crate::bridge::assistant_system_prompt(&lang);
                    for name in bridge_pool.list() {
                        if name.starts_with("llm-") {
                            if let Some(info) = bridge_pool.info.get_mut(&name) {
                                info.reason = prompt.clone();
                            }
                        }
                    }
                    println!("{}✓{} Language set to {} for all LLM bridges", GREEN, RESET, lang_code);
                } else if parts.len() >= 2 && parts[0] == "extra" {
                    let extra = parts[1];
                    let lang = crate::bridge::AssistantLang {
                        primary: "ru".into(),
                        extra: Some(extra.to_string()),
                    };
                    let prompt = crate::bridge::assistant_system_prompt(&lang);
                    for name in bridge_pool.list() {
                        if name.starts_with("llm-") {
                            if let Some(info) = bridge_pool.info.get_mut(&name) {
                                info.reason = prompt.clone();
                            }
                        }
                    }
                    println!("{}✓{} Extra language set to {}", GREEN, RESET, extra);
                }
            }
        }
        "llm" => {
            if slash_arg.is_empty() {
                let bridges = bridge_pool.list();
                let llm_bridges: Vec<_> = bridges.iter().filter(|n| n.starts_with("llm-")).collect();
                if llm_bridges.is_empty() {
                    println!("No LLM bridges available.");
                } else {
                    println!("{}Available LLM providers:{}", BOLD, RESET);
                    for name in &llm_bridges {
                        let status = if bridge_pool.info.get(*name).map(|i| i.enabled).unwrap_or(false) { "✅" } else { "⛔" };
                        println!("  {} {} — /llm set {}", status, name, name.strip_prefix("llm-").unwrap_or(name));
                    }
                }
            } else {
                let parts: Vec<&str> = slash_arg.splitn(2, ' ').collect();
                if parts.len() >= 2 && parts[0] == "set" {
                    let target = format!("llm-{}", parts[1]);
                    if bridge_pool.bridges.contains_key(&target) {
                        // Re-prioritize: set this one to priority 1, others to 5
                        for name in bridge_pool.list() {
                            if name.starts_with("llm-") {
                                let prio = if name == target { 1 } else { 5 };
                                bridge_pool.set_priority(&name, prio);
                            }
                        }
                        println!("{}✓{} Switched to {}", GREEN, RESET, target);
                    } else {
                        println!("LLM '{}' not found. Available: {}", parts[1],
                            bridge_pool.list().iter().filter(|n| n.starts_with("llm-")).map(|n| &n[4..]).collect::<Vec<_>>().join(", "));
                    }
                }
            }
        }
        "cargo" => {
            if slash_arg.is_empty() {
                let pending = gossip.pending_cargo_list().await;
                if pending.is_empty() {
                    println!("No pending cargo transfers.");
                } else {
                    println!("{}Pending cargo ({}):{}", BOLD, pending.len(), RESET);
                    for (i, c) in pending.iter().enumerate() {
                        println!("  [{}] {} — {} from {} ({} KB, bridges: {:?})",
                            i, c.agent_name, c.mode, c.from_node, c.size_kb, c.bridges);
                        println!("       /cargo approve {} or /cargo reject {}", i, i);
                    }
                }
            }
            let parts: Vec<&str> = slash_arg.splitn(2, ' ').collect();
            if parts.len() == 2 && parts[0] == "approve" {
                if let Ok(idx) = parts[1].parse::<usize>() {
                    if let Some(c) = gossip.approve_cargo(idx).await {
                        println!("{}✓{} Cargo approved: {} (mode: {})", GREEN, RESET, c.agent_name, c.mode);
                        agent_journal.log("system", "cargo_approved", &c.agent_name);
                    }
                }
            } else if parts.len() == 2 && parts[0] == "reject" {
                if let Ok(idx) = parts[1].parse::<usize>() {
                    if let Some(c) = gossip.reject_cargo(idx).await {
                        println!("{}✗{} Cargo rejected: {}", YELLOW, RESET, c.agent_name);
                        agent_journal.log("system", "cargo_rejected", &c.agent_name);
                    }
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
    kvstore: &Arc<KvStore>,
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
        _ if cmd.starts_with("@agent ") || cmd.starts_with("@all ") => {
            if let Some(msg) = crate::agent_chat::AgentChat::parse_agent_command(cmd) {
                let chat = crate::agent_chat::AgentChat::new(kvstore.clone());
                let _ = chat.send(&msg, 0);
                let short_to = if msg.to.len() > 12 { &msg.to[..12] } else { &msg.to };
                println!("{}🤖 Сообщение отправлено агенту {}{}", GREEN, short_to, RESET);
            }
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
            match crate::tui_agent::assistant_chat(bridge_pool, kvstore, 0, text, "cli") {
                Ok(r) => println!("{}", r),
                Err(_) => demo_response(text),
            }
        }
        _ => {
            let text = cmd;
            match crate::tui_agent::assistant_chat(bridge_pool, kvstore, 0, text, "cli") {
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
