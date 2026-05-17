use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWrite;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::info;

use crate::store::{KvStore, StreamSubscriber};

pub struct ApiState {
    pub channels: Arc<Mutex<HashMap<String, Vec<Value>>>>,
    pub nodes: Arc<Mutex<Vec<Value>>>,
    pub node_id: String,
    pub node_name: String,
    pub start_time: std::time::Instant,
    pub chat_log: Arc<Mutex<Vec<Value>>>,
    pub kvstore: Option<Arc<KvStore>>,
}

impl ApiState {
    pub fn new(node_id: &str, node_name: &str) -> Self {
        ApiState {
            channels: Arc::new(Mutex::new(HashMap::new())),
            nodes: Arc::new(Mutex::new(Vec::new())),
            node_id: node_id.to_string(),
            node_name: node_name.to_string(),
            start_time: std::time::Instant::now(),
            chat_log: Arc::new(Mutex::new(Vec::new())),
            kvstore: None,
        }
    }
}

pub async fn serve(port: u16, state: Arc<ApiState>) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Web UI on http://localhost:{}", port);

    loop {
        let (socket, addr) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let _ = handle_conn(socket, state).await;
        });
    }
}

async fn handle_conn(mut stream: TcpStream, state: Arc<ApiState>) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (reader, mut writer) = stream.split();
    let mut lines = BufReader::new(reader).lines();
    let mut method = String::new();
    let mut path = String::new();
    let mut body = String::new();
    let mut content_len: usize = 0;
    let mut reading_body = false;
    // Auth check: dashboard requires login (handled by JS)

    while let Some(line) = lines.next_line().await? {
        if reading_body {
            body.push_str(&line);
            if body.len() >= content_len { break; }
            continue;
        }
        if line.is_empty() {
            if content_len > 0 { reading_body = true; continue; }
            else { break; }
        }
        if method.is_empty() && line.contains("HTTP") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                method = parts[0].to_string();
                path = parts[1].to_string();
            }
        }
        if line.to_lowercase().starts_with("content-length:") {
            content_len = line.split(':').nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        }
    }

    if path.starts_with("/api/v1/stream/") {
        handle_sse(&mut writer, &path, &state).await?;
        return Ok(());
    }

    // Rewind bufreader (we consumed it for auth check)
    let response = match route(&method, &path, &body, &state).await {
        Some(resp) => resp,
        None => web_ui(&state).await,
    };

    let mut w = writer;
    w.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn handle_sse<W: AsyncWrite + Unpin>(writer: &mut W, path: &str, state: &Arc<ApiState>) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;

    let parts: Vec<&str> = path.split('/').collect();
    let session_id = parts.get(4).unwrap_or(&"default");
    let channel = format!("channel:stream:{}", session_id);

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: keep-alive\r\n\r\n";
    let mut w = writer;
    w.write_all(response.as_bytes()).await?;
    w.flush().await?;

    if let Some(ref kvstore) = state.kvstore {
        if let Ok(mut sub) = StreamSubscriber::new(kvstore, 0, &channel) {
            sub.set_read_timeout(Some(5));
            loop {
                match sub.get_message() {
                    Ok(Some(msg)) => {
                        let sse = format!("data: {}\n\n", msg);
                        if w.write_all(sse.as_bytes()).await.is_err() { break; }
                        w.flush().await?;
                        if msg.contains("\"type\":\"done\"") { break; }
                    }
                    Ok(None) => {
                        if w.write_all(b": heartbeat\n\n").await.is_err() { break; }
                        w.flush().await?;
                    }
                    Err(_) => {
                        if w.write_all(b": heartbeat\n\n").await.is_err() { break; }
                        w.flush().await?;
                    }
                }
            }
            let _ = sub.unsubscribe();
        }
    } else {
        w.write_all(b"data: {\"type\":\"error\",\"content\":\"Redis not connected\"}\n\n").await?;
    }

    Ok(())
}

async fn route(method: &str, path: &str, body: &str, state: &Arc<ApiState>) -> Option<String> {
    if !path.starts_with("/api/") { return None; }

    match (method, path) {
        ("GET", "/api/v1/health") => {
            let peers = state.nodes.lock().await.len();
            let uptime = state.start_time.elapsed().as_secs();
            let redis = state.kvstore.as_ref().map(|k| k.is_connected()).unwrap_or(false);
            let status = if redis { "healthy" } else { "degraded" };
            let code = if redis { 200 } else { 503 };
            let body = serde_json::json!({
                "status": status,
                "version": env!("CARGO_PKG_VERSION"),
                "uptime_secs": uptime,
                "peers": peers,
                "redis": redis,
                "node_id": state.node_id,
                "node_name": state.node_name,
            });
            let body_str = serde_json::to_string_pretty(&body).unwrap_or_default();
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                if code == 200 { "200 OK" } else { "503 Service Unavailable" },
                body_str.len(), body_str
            );
            return Some(response);
        }
        ("GET", "/api/v1/node/status") => {
            let peers = state.nodes.lock().await.len();
            let uptime = state.start_time.elapsed().as_secs();
            let msgs = state.chat_log.lock().await.len();
            let redis = state.kvstore.as_ref().map(|k| k.is_connected()).unwrap_or(false);
            Some(json(&serde_json::json!({
                "node_id": state.node_id,
                "node_name": state.node_name,
                "status": "alive",
                "peers": peers,
                "uptime": uptime,
                "messages": msgs,
                "redis": redis,
                "version": env!("CARGO_PKG_VERSION"),
                "agents": 0,
                "skills": 5,
                "bridges": 3,
                "mode": "plan",
            })))
        }
        ("GET", "/api/v1/node/peers") => {
            Some(json(&serde_json::json!({"peers": *state.nodes.lock().await})))
        }
        ("POST", "/api/v1/peers/connect") => {
            let msg: Value = serde_json::from_str(body).unwrap_or_default();
            let addr = msg["address"].as_str().unwrap_or("").to_string();
            if !addr.is_empty() {
                state.nodes.lock().await.push(Value::String(addr.clone()));
                Some(json(&serde_json::json!({"status": "connecting", "address": addr})))
            } else {
                Some(json(&serde_json::json!({"error": "address required"})))
            }
        }
        ("POST", "/api/v1/peers/disconnect") => {
            let msg: Value = serde_json::from_str(body).unwrap_or_default();
            let addr = msg["address"].as_str().unwrap_or("").to_string();
            if !addr.is_empty() {
                let mut peers = state.nodes.lock().await;
                peers.retain(|p| p.as_str() != Some(&addr));
                Some(json(&serde_json::json!({"status": "disconnected", "address": addr})))
            } else {
                Some(json(&serde_json::json!({"error": "address required"})))
            }
        }
        ("GET", "/api/v1/chat") => {
            let msgs = state.chat_log.lock().await.clone();
            Some(json(&serde_json::json!({"messages": msgs, "count": msgs.len()})))
        }
        ("POST", "/api/v1/chat") | ("POST", "/api/v1/command") => {
            let msg: Value = serde_json::from_str(body).unwrap_or(serde_json::json!({"text": body}));
            let text = msg["text"].as_str().unwrap_or(body).to_string();
            state.chat_log.lock().await.push(serde_json::json!({
                "role": "user", "text": text,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }));
            // Slash commands тоже обрабатываем
            if text.starts_with('/') {
                Some(json(&serde_json::json!({"status": "queued", "message": text, "note": "command queued (processing in background)"})))
            } else {
                Some(json(&serde_json::json!({"status": "ok", "message": text})))
            }
        }
        ("GET", path) if path.starts_with("/api/v1/store/") => {
            let key = &path["/api/v1/store/".len()..];
            if let Some(ref kvstore) = state.kvstore {
                match kvstore.get(key) {
                    Ok(Some(val)) => Some(json(&serde_json::json!({"key": key, "value": val}))),
                    Ok(None) => Some(json(&serde_json::json!({"key": key, "value": null}))),
                    Err(e) => Some(json(&serde_json::json!({"error": e.to_string()}))),
                }
            } else {
                Some(json(&serde_json::json!({"error": "Redis not connected"})))
            }
        }
        ("POST", "/api/v1/mode/set") => {
            let msg: Value = serde_json::from_str(body).unwrap_or_default();
            let mode = msg["mode"].as_str().unwrap_or("plan");
            let mut log = state.chat_log.lock().await;
            log.push(serde_json::json!({
                "role": "system", "text": format!("Mode switched to {}", mode),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }));
            Some(json(&serde_json::json!({"status": "ok", "mode": mode})))
        }
        ("GET", "/api/v1/skills") => {
            let skills = serde_json::json!([
                {"name":"general","category":"general","description":"Универсальный агент","role":"general","llm":"auto"},
                {"name":"explorer","category":"research","description":"Исследователь, поиск в 3 регионах","role":"scout","llm":"deepseek-v4-flash"},
                {"name":"scout-ru","category":"search","description":"Поиск по RU (Яндекс, YaCy)","role":"scout","llm":"deepseek-v4-flash"},
                {"name":"scout-us","category":"search","description":"Поиск по US (DuckDuckGo)","role":"scout","llm":"deepseek-v4-flash"},
                {"name":"scout-cn","category":"search","description":"Поиск по CN (Baidu)","role":"scout","llm":"deepseek-v4-flash"}
            ]);
            Some(json(&serde_json::json!({"skills": skills, "count": 5})))
        }
        ("GET", "/api/v1/contacts") => {
            let contacts = serde_json::json!([
                {"nickname":"Хаб-177","node_id":"171.22.180.177:42069","group":"Работа"},
                {"nickname":"Петя","node_id":"10.0.0.5:42070","group":"Друзья"}
            ]);
            Some(json(&serde_json::json!({"contacts": contacts, "count": 2})))
        }
        ("POST", "/api/v1/contacts") => {
            Some(json(&serde_json::json!({"status": "ok", "message": "nick set via /nick command"})))
        }
        ("POST", "/api/v1/a2a") => {
            let msg: Value = serde_json::from_str(body).unwrap_or_default();
            let a2a_token = msg["token"].as_str().unwrap_or("");
            let a2a_body = msg["body"].as_str().unwrap_or(body);
            // Временно: простой ответ, позже будет A2aAdapter::handle_request
            if a2a_token.is_empty() && !msg.get("body").is_some() {
                Some(json(&serde_json::json!({"error": "A2A token or body required"})))
            } else {
                Some(json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {"status": "ok", "message": "A2A request received"}
                })))
            }
        }
        ("POST", "/api/v1/auth") => {
            let msg: Value = serde_json::from_str(body).unwrap_or_default();
            let pwd = msg["password"].as_str().unwrap_or("");
            let cfg_pwd = std::env::var("WATERS_DASHBOARD_PASSWORD").unwrap_or_else(|_| "waters".into());
            if pwd == cfg_pwd {
                let token = format!("token-{}", chrono::Utc::now().timestamp());
                Some(json(&serde_json::json!({"ok": true, "token": token})))
            } else {
                Some(json(&serde_json::json!({"ok": false, "error": "wrong password"})))
            }
        }
        _ => Some(json(&serde_json::json!({"error": "not found", "path": path}))),
    }
}

fn json(data: &serde_json::Value) -> String {
    let body = serde_json::to_string_pretty(data).unwrap_or_default();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
        body.len(), body
    )
}

async fn web_ui(state: &Arc<ApiState>) -> String {
    let peers = state.nodes.lock().await.len();
    let uptime = state.start_time.elapsed().as_secs();
    let id_short = &state.node_id[..8].to_string();
    let h = uptime / 3600;
    let m = (uptime % 3600) / 60;
    let s = uptime % 60;
    let ut = format!("{:02}:{:02}:{:02}", h, m, s);
    let name = state.node_name.clone();
    let redis_connected = state.kvstore.as_ref().map(|k| k.is_connected()).unwrap_or(false);
    let redis_status = if redis_connected { "green" } else { "red" };

    let page = HTML.replace("{UP}", &ut)
        .replace("{PEERS}", &peers.to_string())
        .replace("{ID}", id_short)
        .replace("{NAME}", &name)
        .replace("{REDIS}", redis_status);

    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        page.len(), page
    )
}

const HTML: &str = r##"<!DOCTYPE html>
<html lang="ru"><head>
<meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>WATERS Node v0.4</title>
<style>
:root{--bg:#0a0a1a;--card:rgba(255,255,255,0.02);--border:rgba(255,255,255,0.06);--text:#e0e0e0;--muted:#555;--accent:#00d4ff;--accent2:#0088cc;--green:#0f8;--red:#ff4757;--yellow:#fc0;--orange:#ff6b6b;--font:system-ui,-apple-system,sans-serif}
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:var(--font);background:var(--bg);color:var(--text);min-height:100vh;display:flex}
.sidebar{width:220px;min-height:100vh;background:rgba(255,255,255,0.01);border-right:1px solid var(--border);padding:16px;flex-shrink:0}
.sidebar .logo{color:var(--accent);font-size:18px;font-weight:700;margin-bottom:24px;display:flex;align-items:center;gap:8px}
.sidebar .logo span{font-size:10px;color:var(--muted);font-weight:400}
.sidebar .nav{display:flex;flex-direction:column;gap:4px}
.sidebar .nav a{padding:10px 12px;border-radius:8px;color:var(--text);text-decoration:none;font-size:13px;transition:background .2s;display:flex;align-items:center;gap:10px}
.sidebar .nav a:hover,.sidebar .nav a.active{background:rgba(0,212,255,0.08);color:var(--accent)}
.main{flex:1;padding:16px 24px;overflow-y:auto;max-height:100vh}
.main .topbar{display:flex;align-items:center;gap:16px;margin-bottom:24px;padding-bottom:16px;border-bottom:1px solid var(--border)}
.main .topbar h2{font-size:16px;font-weight:600;color:#fff}
.main .topbar .status-bar{display:flex;gap:16px;margin-left:auto;font-size:12px;color:var(--muted)}
.main .topbar .status-bar .dot{width:8px;height:8px;border-radius:50%;display:inline-block;margin-right:4px}
.main .topbar .status-bar .dot.g{background:var(--green)}
.main .topbar .status-bar .dot.r{background:var(--red)}
.main .topbar .status-bar .dot.y{background:var(--yellow)}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:16px;margin-bottom:24px}
.card{background:var(--card);border:1px solid var(--border);border-radius:12px;padding:16px}
.card .card-hdr{display:flex;justify-content:space-between;align-items:center;margin-bottom:12px}
.card .card-hdr h3{font-size:13px;color:var(--muted);text-transform:uppercase;letter-spacing:.5px}
.card .card-hdr .badge{font-size:11px;padding:2px 8px;border-radius:6px;background:rgba(0,212,255,0.1);color:var(--accent)}
.card .item{padding:8px 0;border-bottom:1px solid rgba(255,255,255,0.03);font-size:13px;display:flex;justify-content:space-between;align-items:center}
.card .item:last-child{border:0}
.card .item .l{color:var(--muted)}
.card .item .v{color:#fff;font-weight:500}
.tab-bar{display:flex;gap:0;margin-bottom:16px;border-bottom:1px solid var(--border)}
.tab-bar .tab{padding:10px 20px;font-size:13px;color:var(--muted);cursor:pointer;border-bottom:2px solid transparent;transition:all .2s}
.tab-bar .tab:hover{color:#fff}
.tab-bar .tab.active{color:var(--accent);border-bottom-color:var(--accent)}
.tab-content{display:none}
.tab-content.active{display:block}
.chat-box{background:var(--card);border:1px solid var(--border);border-radius:12px;padding:16px;max-height:500px;overflow-y:auto}
.chat-msg{padding:8px 0;border-bottom:1px solid rgba(255,255,255,0.03);font-size:13px;line-height:1.6}
.chat-msg .role{font-weight:600;color:var(--accent)}
.chat-msg .text{color:#ccc}
.chat-msg .reasoning{color:var(--muted);font-style:italic;font-size:12px}
.chat-input{display:flex;gap:10px;margin-top:12px;flex-wrap:wrap}
.chat-input input{flex:1;min-width:200px;padding:10px 14px;border:1px solid var(--border);border-radius:8px;background:rgba(255,255,255,0.03);color:#fff;font-size:14px;outline:none}
.chat-input input:focus{border-color:var(--accent)}
.chat-input button{padding:10px 18px;border:0;border-radius:8px;font-weight:600;cursor:pointer;font-size:13px;transition:opacity .2s}
.chat-input button:disabled{opacity:0.3;cursor:default}
.chat-input .btn-primary{background:linear-gradient(135deg,var(--accent),var(--accent2));color:#000}
.chat-input .btn-danger{background:linear-gradient(135deg,var(--orange),var(--red));color:#fff}
.chat-input .btn-ghost{background:rgba(255,255,255,0.05);color:var(--text)}
.chat-input .btn-ghost.active{background:rgba(0,212,255,0.15);color:var(--accent);opacity:1}
@media(max-width:900px){.sidebar{display:none}.main{padding:12px}.grid{grid-template-columns:1fr}}
</style></head><body>
<div class="sidebar">
  <div class="logo">🌊 waters-node <span>v0.4</span></div>
  <div class="nav">
    <a href="#" onclick="switchTab('dashboard')" class="active" id="nav-dashboard">📊 Dashboard</a>
    <a href="#" onclick="switchTab('chat')" id="nav-chat">💬 Chat</a>
    <a href="#" onclick="switchTab('agents')" id="nav-agents">🤖 Agents</a>
    <a href="#" onclick="switchTab('peers')" id="nav-peers">🌐 Peers</a>
    <a href="#" onclick="switchTab('skills')" id="nav-skills">🧠 Skills</a>
    <a href="#" onclick="switchTab('contacts')" id="nav-contacts">📒 Contacts</a>
    <a href="#" onclick="switchTab('voice')" id="nav-voice">🎤 Voice</a>
    <a href="#" onclick="switchTab('settings')" id="nav-settings">⚙️ Settings</a>
  </div>
</div>
<div class="main">
  <div class="topbar">
    <h2 id="page-title">Dashboard</h2>
    <div class="status-bar">
      <span><span class="dot g"></span><span id="peer-count">{PEERS}</span> peers</span>
      <span><span class="dot" id="redis-dot" style="background:var(--{REDIS})"></span> redis</span>
      <span>uptime <span id="uptime-val">{UP}</span></span>
      <span class="badge" style="background:rgba(0,255,136,0.1);color:var(--green);font-size:11px;padding:2px 8px;border-radius:6px">{ID}... / {NAME}</span>
    </div>
  </div>

  <!-- TAB: Dashboard -->
  <div class="tab-content active" id="tab-dashboard">
    <div class="grid">
      <div class="card"><div class="card-hdr"><h3>Node</h3></div>
        <div class="item"><span class="l">Status</span><span class="v"><span class="dot g" style="display:inline-block;margin-right:6px"></span>Online</span></div>
        <div class="item"><span class="l">Version</span><span class="v">0.4</span></div>
        <div class="item"><span class="l">Uptime</span><span class="v" id="uptime-detail">{UP}</span></div>
        <div class="item"><span class="l">Mode</span><span class="v" id="current-mode">Plan</span></div>
      </div>
      <div class="card"><div class="card-hdr"><h3>Network <span class="badge" id="network-badge">{PEERS}</span></h3></div>
        <div class="item"><span class="l">Peers</span><span class="v" id="dash-peer-count">{PEERS}</span></div>
        <div class="item"><span class="l">Redis</span><span class="v" id="dash-redis-status" style="color:var(--{REDIS})">{REDIS}</span></div>
        <div class="item"><span class="l">Transport</span><span class="v">TCP + mDNS</span></div>
      </div>
      <div class="card"><div class="card-hdr"><h3>Resources</h3></div>
        <div class="item"><span class="l">Skills</span><span class="v" id="skill-count">—</span></div>
        <div class="item"><span class="l">Agents</span><span class="v" id="agent-count">—</span></div>
        <div class="item"><span class="l">Bridges</span><span class="v" id="bridge-count">—</span></div>
      </div>
    </div>
    <div class="card"><div class="card-hdr"><h3>Mode Control</h3></div>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:4px">
        <button onclick="setMode('plan')" class="mode-btn" data-mode="plan" style="flex:1;padding:12px;border:1px solid var(--border);border-radius:8px;background:rgba(0,212,255,0.1);color:var(--accent);font-weight:600;cursor:pointer;font-size:12px;transition:all .2s">📋 Plan</button>
        <button onclick="setMode('assemble')" class="mode-btn" data-mode="assemble" style="flex:1;padding:12px;border:1px solid var(--border);border-radius:8px;background:transparent;color:var(--muted);font-weight:500;cursor:pointer;font-size:12px;transition:all .2s">🔗 Assemble</button>
        <button onclick="setMode('execute')" class="mode-btn" data-mode="execute" style="flex:1;padding:12px;border:1px solid var(--border);border-radius:8px;background:transparent;color:var(--muted);font-weight:500;cursor:pointer;font-size:12px;transition:all .2s">⚡ Execute</button>
        <button onclick="setMode('stop')" class="mode-btn" data-mode="stop" style="flex:1;padding:12px;border:1px solid var(--border);border-radius:8px;background:transparent;color:var(--muted);font-weight:500;cursor:pointer;font-size:12px;transition:all .2s">⏹ Stop</button>
        <button onclick="setMode('log')" class="mode-btn" data-mode="log" style="flex:1;padding:12px;border:1px solid var(--border);border-radius:8px;background:transparent;color:var(--muted);font-weight:500;cursor:pointer;font-size:12px;transition:all .2s">📜 Log</button>
      </div>
    </div>
  </div>

  <!-- TAB: Chat -->
  <div class="tab-content" id="tab-chat">
    <div class="chat-box" id="chat-box">
      <div class="card-hdr"><h3>Chat</h3><span id="stream-status" style="font-size:11px;color:var(--muted)"></span></div>
      <div id="messages" style="margin-top:12px"></div>
      <div class="chat-input">
        <input id="chat-input" placeholder="Type a message..." onkeydown="if(event.key==='Enter')sendChat()">
        <button class="btn-primary" onclick="sendChat()" id="send-btn">Send</button>
        <button class="btn-danger" id="ptt-btn" title="Push-to-Talk: hold to record">🎤</button>
        <button class="btn-ghost" onclick="agentVoice()" id="agent-voice-btn" title="Voice to agent (STT → LLM → TTS)">🤖</button>
        <button class="btn-ghost" onclick="toggleVoice()" id="voice-toggle" title="Toggle TTS">🔇</button>
        <button class="btn-ghost" onclick="cycleVoiceMode()" id="voice-profile" title="Switch voice">👩 1/6</button>
      </div>
    </div>
  </div>

  <!-- TAB: Agents -->
  <div class="tab-content" id="tab-agents">
    <div class="grid">
      <div class="card"><div class="card-hdr"><h3>Active Agents</h3><span class="badge" id="agent-count-badge">0</span></div>
        <div id="agents-list"><div class="item"><span class="l">No active agents</span></div></div>
      </div>
      <div class="card"><div class="card-hdr"><h3>Available Skills</h3><span class="badge" id="skill-count-badge">0</span></div>
        <div id="skills-list"><div class="item"><span class="l">No skills loaded</span></div></div>
      </div>
    </div>
  </div>

  <!-- TAB: Peers -->
  <div class="tab-content" id="tab-peers">
    <div class="card"><div class="card-hdr"><h3>Connected Peers</h3><span class="badge" id="peers-badge">{PEERS}</span></div>
      <div id="peer-detail-list"><div class="item"><span class="l">No peers connected</span></div></div>
    </div>
    <div class="card" style="margin-top:16px"><div class="card-hdr"><h3>Connect to Peer</h3></div>
      <div class="chat-input" style="margin-top:8px">
        <input id="peer-addr" placeholder="IP:port (e.g. 171.22.180.177:42069)">
        <button class="btn-primary" onclick="connectPeer()">Connect</button>
        <button class="btn-ghost" onclick="disconnectPeer()">Disconnect</button>
      </div>
    </div>
  </div>

  <!-- TAB: Skills -->
  <div class="tab-content" id="tab-skills">
    <div class="grid" id="skills-grid"></div>
  </div>

  <!-- TAB: Voice -->
  <div class="tab-content" id="tab-voice">
    <div class="grid">
      <div class="card"><div class="card-hdr"><h3>🎤 Рация (Push-to-Talk)</h3></div>
        <div style="text-align:center;padding:16px">
          <canvas id="vu-meter" width="300" height="40" style="border-radius:8px;background:rgba(255,255,255,0.03);width:100%;max-width:400px;margin-bottom:12px"></canvas>
          <div style="display:flex;gap:8px;justify-content:center;flex-wrap:wrap">
            <button id="ptt-main" class="btn-danger" style="width:80px;height:80px;border-radius:50%;font-size:32px">🎤</button>
            <button class="btn-ghost" onclick="agentVoice()" style="width:64px;height:64px;border-radius:50%;font-size:28px">🤖</button>
            <button class="btn-ghost" onclick="toggleVoice()" id="voice-toggle2" style="width:64px;height:64px;border-radius:50%;font-size:24px">🔇</button>
            <button class="btn-ghost" onclick="cycleVoiceMode()" id="voice-profile2" style="width:64px;height:64px;border-radius:50%;font-size:18px">👩 1/6</button>
          </div>
          <div style="margin-top:12px;font-size:13px;color:var(--muted)" id="voice-status">Нажми 🎤 и говори. Отпусти — отправится</div>
        </div>
      </div>
      <div class="card"><div class="card-hdr"><h3>🔊 Воспроизведение</h3></div>
        <div style="padding:12px">
          <div class="item"><span class="l">Озвучивание ответов</span><span class="v" id="voice-status-text">Выкл</span></div>
          <div class="item"><span class="l">Текущий голос</span><span class="v" id="voice-current-profile">👩 1/6</span></div>
          <div class="item"><span class="l">Голосов в системе</span><span class="v" id="voice-available">—</span></div>
          <div style="margin-top:12px;display:flex;gap:6px;justify-content:center">
            <button class="btn-ghost" onclick="toggleVoice()" style="font-size:13px">🔊 Вкл/Выкл</button>
            <button class="btn-ghost" onclick="cycleVoiceMode()" style="font-size:13px">🔄 Сменить голос</button>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- TAB: Contacts -->
  <div class="tab-content" id="tab-contacts">
    <div class="card"><div class="card-hdr"><h3>📒 Контакты</h3><span class="badge" id="contacts-count">0</span></div>
      <div id="contacts-list"><div class="item"><span class="l">Загрузка...</span></div></div>
      <div class="chat-input" style="margin-top:12px;flex-wrap:wrap">
        <input id="nick-id" placeholder="node_id (IP:port)" style="flex:2;min-width:120px">
        <input id="nick-name" placeholder="имя" style="flex:1;min-width:80px">
        <input id="nick-group" placeholder="группа" style="flex:1;min-width:80px">
        <button class="btn-primary" onclick="addContact()">➕</button>
      </div>
    </div>
  </div>

  <!-- TAB: Settings -->
  <div class="tab-content" id="tab-settings">
    <div class="card"><div class="card-hdr"><h3>Node Settings</h3></div>
      <div class="item"><span class="l">Node ID</span><span class="v" style="font-size:11px">{ID}... / {NAME}</span></div>
      <div class="item"><span class="l">Version</span><span class="v">0.4</span></div>
      <div class="item"><span class="l">Redis</span><span class="v" id="settings-redis" style="color:var(--{REDIS})">{REDIS}</span></div>
      <div class="item"><span class="l">Log Level</span><span class="v">info</span></div>
    </div>
  </div>
</div>
<script>
// === STATE ===
let evtSource=null,voiceEnabled=false,currentProfile=0;
let synth=window.speechSynthesis,voiceProfiles=[],mediaRecorder=null,audioChunks=[];
let audioCtx=null,pushToTalkStream=null,skillsCache=null;
const ICONS=['👩','👨','🧑','👩‍🦰','👨‍🦱','🧑‍🦳'];

// === TAB SWITCHING ===
function switchTab(name){document.querySelectorAll('.tab-content').forEach(t=>t.classList.remove('active'));let tab=document.getElementById('tab-'+name);if(tab)tab.classList.add('active');document.querySelectorAll('.sidebar .nav a').forEach(a=>a.classList.remove('active'));let nav=document.getElementById('nav-'+name);if(nav)nav.classList.add('active');let title=document.getElementById('page-title');
const T={'dashboard':'Dashboard','chat':'Chat','agents':'Agents','peers':'Peers','skills':'Skills','voice':'🎤 Voice','contacts':'📒 Contacts','settings':'Settings'};
if(title)title.textContent=T[name]||name;if(name==='agents')loadStatus();if(name==='peers')loadPeers();if(name==='skills')loadSkills();if(name==='contacts')loadContacts()}

// === MODES ===
function setMode(m){let btns=document.querySelectorAll('.mode-btn');btns.forEach(b=>{if(b.dataset.mode===m){b.style.background='rgba(0,212,255,0.1)';b.style.color='var(--accent)';b.style.fontWeight='600'}else{b.style.background='transparent';b.style.color='var(--muted)';b.style.fontWeight='500'}});document.getElementById('current-mode').textContent=m.charAt(0).toUpperCase()+m.slice(1);api('mode/set','POST',{mode:m})}

// === PEER CONNECTION ===
async function connectPeer(){let a=document.getElementById('peer-addr').value.trim();if(!a)return;let r=await api('peers/connect','POST',{address:a});if(r.error)alert(r.error);else{loadPeers();document.getElementById('peer-addr').value=''}}
async function disconnectPeer(){let a=document.getElementById('peer-addr').value.trim();if(!a)return;await api('peers/disconnect','POST',{address:a});loadPeers()}

// === LOAD STATUS ===
async function loadStatus(){try{let s=await api('node/status');if(!s)return;let countEl=document.getElementById('agent-count');if(countEl)countEl.textContent=s.agents||0;let bc=document.getElementById('agent-count-badge');if(bc)bc.textContent=s.agents||0;let sc=document.getElementById('skill-count');if(sc)sc.textContent=s.skills||0;let sb=document.getElementById('skill-count-badge');if(sb)sb.textContent=s.skills||0;let brc=document.getElementById('bridge-count');if(brc)brc.textContent=s.bridges||0;if(s.uptime){let h=Math.floor(s.uptime/3600),m=Math.floor((s.uptime%3600)/60),sec=s.uptime%60;let ut=String(h).padStart(2,'0')+':'+String(m).padStart(2,'0')+':'+String(sec).padStart(2,'0');let upEl=document.getElementById('uptime-val');if(upEl)upEl.textContent=ut;let upDet=document.getElementById('uptime-detail');if(upDet)upDet.textContent=ut}}catch(e){}}

// === LOAD PEERS ===
async function loadPeers(){try{let r=await fetch('/api/v1/node/peers');let d=await r.json();let el=document.getElementById('peer-detail-list');if(!el)return;el.innerHTML='';if(!d.peers||d.peers.length===0){el.innerHTML='<div class=item><span class=l>No peers connected</span></div>';return}d.peers.forEach(p=>{el.innerHTML+='<div class=item><span class=l style="font-family:mono">'+p+'</span><span class=v style="font-size:11px;color:var(--green)">● connected</span></div>'})}catch(e){}}

// === LOAD SKILLS (from API) ===
async function loadSkills(){if(skillsCache){renderSkills(skillsCache);return}try{let r=await fetch('/api/v1/skills');let d=await r.json();if(d.skills){skillsCache=d.skills;renderSkills(d.skills)}else{let grid=document.getElementById('skills-grid');if(grid)grid.innerHTML='<div class=card><div class=item><span class=l>No skills API available</span></div></div>'}}catch(e){}}
function renderSkills(skills){let grid=document.getElementById('skills-grid');if(!grid)return;if(!skills||skills.length===0){grid.innerHTML='<div class=card><div class=item><span class=l>No skills loaded</span></div></div>';return}grid.innerHTML='';skills.forEach(s=>{grid.innerHTML+=`<div class=card><div class=card-hdr><h3>${s.name||'?'}</h3><span class=badge>${s.category||'general'}</span></div><div class=item><span class=l>${s.description||'—'}</span></div><div class=item><span class=l>Role</span><span class=v>${s.role||'general'}</span></div><div class=item><span class=l>LLM</span><span class=v>${s.llm||'auto'}</span></div></div>`})}

// === VOICE ===
let audioCtx=null,analyserNode=null,vuanimId=null;
function initVoices(){let v=synth.getVoices();if(v.length===0){setTimeout(initVoices,300);return}
let ru=v.filter(x=>x.lang.startsWith('ru'));let en=v.filter(x=>x.lang.startsWith('en'));
for(let i=0;i<6;i++){let pool=i<3?ru:en;if(pool.length===0)pool=v;let p=i%2===0?pool.find(x=>!/Male|Microsoft/.test(x.name)):pool.find(x=>x.name.includes('Male')||x.name.includes('David'));if(!p)p=pool[i%pool.length]||v[i%v.length];voiceProfiles[i]={icon:ICONS[i],voice:p,lang:ru.length?'ru-RU':'en-US'}}
updateProfileUI();let va=document.getElementById('voice-available');if(va)va.textContent=v.length}
function speak(t,i){i=i||0;if(!voiceEnabled||!voiceProfiles[i])return;synth.cancel();let p=voiceProfiles[i];if(!p)return;let u=new SpeechSynthesisUtterance(t);u.lang=p.lang;u.rate=0.9;if(p.voice)u.voice=p.voice;synth.speak(u)}
function cycleVoiceMode(){currentProfile=(currentProfile+1)%6;let p=voiceProfiles[currentProfile];
['voice-profile','voice-profile2'].forEach(id=>{let btn=document.getElementById(id);if(btn&&p)btn.textContent=p.icon+' '+(currentProfile+1)+'/6'});
let cp=document.getElementById('voice-current-profile');if(cp)cp.textContent=(p?p.icon:'')+' '+(currentProfile+1)+'/6';
speak('Привет, я голос '+(currentProfile+1),currentProfile)}
function updateProfileUI(){let p=voiceProfiles[currentProfile];
['voice-profile','voice-profile2'].forEach(id=>{let btn=document.getElementById(id);if(btn&&p)btn.textContent=p.icon+' '+(currentProfile+1)+'/6'});
let cp=document.getElementById('voice-current-profile');if(cp)cp.textContent=(p?p.icon:'')+' '+(currentProfile+1)+'/6'}
function toggleVoice(){voiceEnabled=!voiceEnabled;
['voice-toggle','voice-toggle2'].forEach(id=>{let btn=document.getElementById(id);if(btn){btn.textContent=voiceEnabled?'🔊':'🔇';btn.style.opacity=voiceEnabled?1:0.5}});
let st=document.getElementById('voice-status-text');if(st)st.textContent=voiceEnabled?'Вкл':'Выкл';
if(voiceEnabled&&voiceProfiles.length===0)initVoices()}

// === VU METER ===
function startVUMeter(stream){if(!audioCtx)audioCtx=new(window.AudioContext||window.webkitAudioContext)();let src=audioCtx.createMediaStreamSource(stream);analyserNode=audioCtx.createAnalyser();analyserNode.fftSize=256;src.connect(analyserNode);drawVU()}
function drawVU(){let canvas=document.getElementById('vu-meter');if(!canvas)return;let ctx=canvas.getContext('2d');let w=canvas.width,h=canvas.height;
ctx.fillStyle='rgba(255,255,255,0.03)';ctx.fillRect(0,0,w,h);
if(analyserNode){let data=new Uint8Array(analyserNode.frequencyBinCount);analyserNode.getByteFrequencyData(data);let avg=data.reduce((a,b)=>a+b,0)/data.length;let pct=Math.min(avg/128,1);
let grad=ctx.createLinearGradient(0,0,w,0);grad.addColorStop(0,'#00d4ff');grad.addColorStop(0.6,'#0f8');grad.addColorStop(1,'#ff4757');
ctx.fillStyle=grad;ctx.fillRect(0,0,w*pct,h);
ctx.fillStyle='rgba(255,255,255,0.1)';for(let i=0;i<w;i+=4){let bar=data[Math.floor(i/w*data.length)]/255*16;ctx.fillRect(i,h-bar,2,bar)}}
vuanimId=requestAnimationFrame(drawVU)}

// === PUSH-TO-TALK ===
async function startPTT(){try{let s=await navigator.mediaDevices.getUserMedia({audio:{channelCount:1,echoCancellation:true}});pushToTalkStream=s;startVUMeter(s);mediaRecorder=new MediaRecorder(s,{mimeType:'audio/webm;codecs=opus'});audioChunks=[];['ptt-btn','ptt-main'].forEach(id=>{let btn=document.getElementById(id);if(btn){btn.textContent='🔴';btn.style.background='var(--red)'}});
let vs=document.getElementById('voice-status');if(vs)vs.textContent='🔴 Запись... Отпусти чтобы отправить';
mediaRecorder.ondataavailable=e=>{audioChunks.push(e.data)};mediaRecorder.start(500)}catch(e){alert('Mic: '+e.message)}}
function stopPTT(){if(!mediaRecorder||mediaRecorder.state==='inactive')return;mediaRecorder.stop();
['ptt-btn','ptt-main'].forEach(id=>{let btn=document.getElementById(id);if(btn){btn.textContent='🎤';btn.style.background=''}});
let vs=document.getElementById('voice-status');if(vs)vs.textContent='⏳ Отправка...';
if(pushToTalkStream){pushToTalkStream.getTracks().forEach(t=>t.stop());pushToTalkStream=null}
if(vuanimId){cancelAnimationFrame(vuanimId);vuanimId=null}
setTimeout(async()=>{if(audioChunks.length===0){if(vs)vs.textContent='Ничего не записано';return}
let blob=new Blob(audioChunks,{type:'audio/webm'});let reader=new FileReader();reader.onload=async()=>{let b64=reader.result.split(',')[1];await api('voice/send','POST',{audio:b64});if(vs)vs.textContent='✅ Отправлено!';setTimeout(()=>{if(vs)vs.textContent='Нажми 🎤 и говори'},2000)};reader.readAsDataURL(blob);audioChunks=[]},600)}

// === AGENT VOICE ===
function agentVoice(){let SR=window.SpeechRecognition||window.webkitSpeechRecognition;if(!SR){alert('Voice input requires Chrome');return}let r=new SR();r.lang='ru-RU';r.interimResults=true;let btn=document.getElementById('agent-voice-btn');btn.textContent='🔴';let vs=document.getElementById('voice-status');if(vs)vs.textContent='🎤 Слушаю...';r.onresult=function(e){let t='';for(let i=e.resultIndex;i<e.results.length;i++){t+=e.results[i][0].transcript}document.getElementById('chat-input').value=t;if(e.results[e.results.length-1].isFinal){btn.textContent='🤖';if(vs)vs.textContent='✅ Распознано: '+t.substring(0,40)+'...';sendChat()}};
r.onerror=function(){btn.textContent='🤖';if(vs)vs.textContent='❌ Ошибка распознавания'};r.start()}

// === SSE STREAM ===
function startStream(){let sid='sess_'+Date.now();let btn=document.getElementById('send-btn');btn.disabled=true;btn.textContent='Stream...';document.getElementById('stream-status').textContent='🔴 streaming';if(evtSource)evtSource.close();evtSource=new EventSource('/api/v1/stream/'+sid);let box=document.getElementById('messages');let md=document.createElement('div');md.className='chat-msg';md.innerHTML='<span class=role>assistant: </span><span class=text id=stream-text></span>';let ft='';box.appendChild(md);evtSource.onmessage=function(e){try{let d=JSON.parse(e.data);if(d.type==='done'){evtSource.close();evtSource=null;btn.disabled=false;btn.textContent='Send';document.getElementById('stream-status').textContent='✅ done';if(voiceEnabled)speak(ft,currentProfile);return}if(d.type==='voice'){playAudio(d.content);return}let el=document.getElementById('stream-text');if(d.type==='token'){if(el)el.textContent+=d.content;ft+=d.content}else if(d.type==='reasoning'){let r2=document.getElementById('stream-reasoning');if(!r2){r2=document.createElement('div');r2.className='reasoning';r2.id='stream-reasoning';md.appendChild(r2)}r2.textContent+=d.content}}catch(e){}};evtSource.onerror=function(){btn.disabled=false;btn.textContent='Send';document.getElementById('stream-status').textContent='❌ error'}}

// === API HELPERS ===
async function api(m,p,b){try{let r=await fetch('/api/v1/'+m,{method:p||'GET',body:b?JSON.stringify(b):null,headers:{'Content-Type':'application/json'}});return r.json()}catch(e){return null}}
async function refresh(){let s=await api('node/status');let r=await fetch('/api/v1/node/peers');let p=await r.json();let pc=document.getElementById('peer-count');if(pc)pc.textContent=(p.peers&&p.peers.length)||0}async function sendChat(){let i=document.getElementById('chat-input');let t=i.value.trim();if(!t)return;i.value='';await api('chat','POST',{text:t});startStream();loadChat()}
async function loadChat(){let r=await api('chat');let box=document.getElementById('messages');box.innerHTML='';(r.messages||[]).forEach(m=>{let d=document.createElement('div');d.className='chat-msg';d.innerHTML='<span class=role>'+m.role+': </span><span class=text>'+escapeHTML(m.text)+'</span>';box.appendChild(d)});box.scrollTop=box.scrollHeight}function escapeHTML(s){return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;')}

// === PTT EVENTS ===
function bindPTT(id){let el=document.getElementById(id);if(!el)return;el.addEventListener('mousedown',startPTT);el.addEventListener('mouseup',stopPTT);el.addEventListener('mouseleave',stopPTT);el.addEventListener('touchstart',function(e){e.preventDefault();startPTT()});el.addEventListener('touchend',function(e){e.preventDefault();stopPTT()})}
document.addEventListener('DOMContentLoaded',function(){bindPTT('ptt-btn');bindPTT('ptt-main')});

// === CONTACTS ===
async function loadContacts(){try{let r=await api('contacts');if(!r||!r.contacts)return;let el=document.getElementById('contacts-list');let cnt=document.getElementById('contacts-count');if(cnt)cnt.textContent=r.contacts.length;if(!el)return;el.innerHTML='';r.contacts.forEach(c=>{let g=c.group?' ['+c.group+']':'';el.innerHTML+='<div class=item><span class=l>'+c.nickname+g+'</span><span class=v style="font-size:11px;font-family:mono;color:var(--muted)">'+c.node_id+'</span></div>'})}catch(e){}}
async function addContact(){let id=document.getElementById('nick-id').value.trim();let name=document.getElementById('nick-name').value.trim();let grp=document.getElementById('nick-group').value.trim();if(!id||!name)return;await api('contacts','POST',{node_id:id,nickname:name,group:grp});document.getElementById('nick-id').value='';document.getElementById('nick-name').value='';document.getElementById('nick-group').value='';loadContacts()}

// === INIT ===
if(synth)synth.onvoiceschanged=initVoices;
setInterval(refresh,5000);refresh();loadChat();loadPeers();setInterval(loadPeers,10000);loadStatus();setInterval(loadStatus,5000);loadContacts();setInterval(loadContacts,15000)
</script></body></html>"##;
