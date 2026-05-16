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
        ("POST", "/api/v1/chat") => {
            let msg: Value = serde_json::from_str(body).unwrap_or(serde_json::json!({"text": body}));
            let text = msg["text"].as_str().unwrap_or(body).to_string();
            state.chat_log.lock().await.push(serde_json::json!({
                "role": "user", "text": text,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }));
            Some(json(&serde_json::json!({"status": "ok", "message": text})))
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
<title>waters-node</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:system-ui,-apple-system,sans-serif;background:#080818;color:#e0e0e0;min-height:100vh}
.header{padding:20px;border-bottom:1px solid rgba(255,255,255,0.06);display:flex;align-items:center;gap:12px}
.header h1{color:#00d4ff;font-size:20px;font-weight:600}
.header .sub{color:#555;font-size:13px}
.header .status{margin-left:auto;display:flex;gap:20px;font-size:13px;color:#888}
.header .status .val{color:#0f0}
.dash{display:grid;grid-template-columns:1fr 1fr 1fr;gap:16px;padding:20px;max-width:1200px;margin:0 auto}
.card{background:rgba(255,255,255,0.02);border:1px solid rgba(255,255,255,0.06);border-radius:12px;padding:16px}
.card h2{font-size:14px;color:#888;margin-bottom:12px;text-transform:uppercase;letter-spacing:0.5px}
.card .item{padding:8px 0;border-bottom:1px solid rgba(255,255,255,0.04);font-size:13px;display:flex;justify-content:space-between}
.card .item:last-child{border:0}
.card .label{color:#888}
.card .value{color:#fff}
.pill{display:inline-block;padding:2px 8px;border-radius:6px;font-size:11px;background:rgba(0,212,255,0.1);color:#00d4ff}
.pill.green{background:rgba(0,255,136,0.1);color:#0f8}
.pill.yellow{background:rgba(255,200,0,0.1);color:#fc0}
.chat-box{grid-column:1/-1;background:rgba(255,255,255,0.02);border:1px solid rgba(255,255,255,0.06);border-radius:12px;padding:16px;max-height:500px;overflow-y:auto}
.chat-msg{padding:8px 0;border-bottom:1px solid rgba(255,255,255,0.04);font-size:13px;line-height:1.5}
.chat-msg .role{font-weight:600;color:#00d4ff}
.chat-msg .text{color:#ccc}
.chat-msg .reasoning{color:#888;font-style:italic;font-size:12px}
.chat-input{display:flex;gap:10px;margin-top:12px}
.chat-input input{flex:1;padding:10px 14px;border:1px solid rgba(255,255,255,0.1);border-radius:8px;background:rgba(255,255,255,0.04);color:#fff;font-size:14px;outline:none}
.chat-input input:focus{border-color:#00d4ff}
.chat-input button{padding:10px 20px;border:0;border-radius:8px;background:linear-gradient(135deg,#00d4ff,#0088cc);color:#000;font-weight:600;cursor:pointer;font-size:14px}
.chat-input button:disabled{opacity:0.4;cursor:default}
@media(max-width:768px){.dash{grid-template-columns:1fr}}
</style></head><body>
<div class="header">
  <h1>WATERS node</h1>
  <span class="sub">{ID}... / {NAME}</span>
  <div class="status">
    <span>uptime <span class="val">{UP}</span></span>
    <span>peers <span class="val" id="peer-count">{PEERS}</span></span>
    <span>redis <span class="val" id="redis-status">{REDIS}</span></span>
    <span class="pill green">online</span>
  </div>
</div>
<div class="dash">
  <div class="card">
    <h2>Tasks</h2>
    <div id="tasks-list"><div class="item"><span class="label">No tasks yet</span></div></div>
  </div>
  <div class="card">
    <h2>Agents</h2>
    <div id="agents-list"><div class="item"><span class="label">No agents yet</span></div></div>
  </div>
  <div class="card">
    <h2>Peers</h2>
    <div id="peers-list"><div class="item"><span class="label">No peers yet</span></div></div>
  </div>
  <div class="chat-box" id="chat-box">
    <h2 style="font-size:14px;color:#888;margin-bottom:12px">Chat <span id="stream-status" style="font-size:11px;color:#555;font-weight:normal"></span></h2>
    <div id="messages"></div>
    <div class="chat-input">
      <input id="chat-input" placeholder="type a message..." onkeydown="if(event.key==='Enter')sendChat()">
      <button onclick="sendChat()" id="send-btn">Send</button>
    </div>
  </div>
</div>
<script>
let evtSource=null;
async function api(m,p,b){let r=await fetch('/api/v1/'+m,{method:p||'GET',body:b?JSON.stringify(b):null,headers:{'Content-Type':'application/json'}});return r.json()}
async function refresh(){let s=await api('node/status');let r=await fetch('/api/v1/node/peers');let p=await r.json();if(p.peers)document.getElementById('peer-count').textContent=p.peers.length||0}
async function sendChat(){let i=document.getElementById('chat-input');let t=i.value.trim();if(!t)return;i.value='';await api('chat','POST',{text:t});startStream();loadChat()}
async function loadChat(){let r=await api('chat');let box=document.getElementById('messages');box.innerHTML='';(r.messages||[]).forEach(m=>{let d=document.createElement('div');d.className='chat-msg';d.innerHTML='<span class=role>'+m.role+': </span><span class=text>'+m.text+'</span>';box.appendChild(d)});box.scrollTop=box.scrollHeight}
function startStream(){let sessionId='sess_'+Date.now();let btn=document.getElementById('send-btn');btn.disabled=true;btn.textContent='Stream...';document.getElementById('stream-status').textContent='streaming';if(evtSource)evtSource.close();evtSource=new EventSource('/api/v1/stream/'+sessionId);let box=document.getElementById('messages');let msgDiv=document.createElement('div');msgDiv.className='chat-msg';msgDiv.innerHTML='<span class=role>assistant: </span><span class=text id=stream-text></span>';box.appendChild(msgDiv);evtSource.onmessage=function(e){try{let d=JSON.parse(e.data);if(d.type==='done'){evtSource.close();evtSource=null;btn.disabled=false;btn.textContent='Send';document.getElementById('stream-status').textContent='done';return}let el=document.getElementById('stream-text');if(d.type==='token'){if(el)el.textContent+=d.content}else if(d.type==='reasoning'){let r=document.getElementById('stream-reasoning');if(!r){r=document.createElement('div');r.className='reasoning';r.id='stream-reasoning';msgDiv.appendChild(r)}r.textContent+=d.content}}catch(e){console.log(e)}};evtSource.onerror=function(){btn.disabled=false;btn.textContent='Send';document.getElementById('stream-status').textContent='error'}}
async function loadPeers(){let r=await fetch('/api/v1/node/peers');let d=await r.json();let el=document.getElementById('peers-list');el.innerHTML='';if(!d.peers||d.peers.length===0){el.innerHTML='<div class=item><span class=label>No peers yet</span></div>';return}
d.peers.forEach(p=>{el.innerHTML+='<div class=item><span class=label>'+p+'</span></div>'})}
setInterval(refresh,5000);refresh();loadChat();loadPeers();setInterval(loadPeers,5000)
</script></body></html>"##;
