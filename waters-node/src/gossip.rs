use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::channel::ChannelManager;

const MAX_PEERS: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPeer {
    pub node_id: String,
    pub node_name: String,
    pub address: String,
    pub groups: Vec<String>,
    pub token: String,
    pub arrived_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: String,
    pub node_name: String,
    pub version: String,
    pub addresses: Vec<String>,
    pub channels: Vec<String>,
    pub groups: Vec<String>,
    pub token: String,
    pub last_seen: String,
    pub uptime: u64,
}

pub struct GossipEngine {
    node_id: String,
    node_name: String,
    port: u16,
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    groups: Arc<Mutex<Vec<(String, String)>>>,
    chan_list: Arc<Mutex<Vec<String>>>,
    pub pending_peers: Arc<Mutex<Vec<PendingPeer>>>,
}

impl GossipEngine {
    pub fn new(node_id: &str, node_name: &str, port: u16) -> Self {
        let pending_peers = Arc::new(Mutex::new(Vec::new()));
        GossipEngine {
            node_id: node_id.to_string(),
            pending_peers,
            node_name: node_name.to_string(),
            port,
            peers: Arc::new(Mutex::new(HashMap::new())),
            groups: Arc::new(Mutex::new(Vec::new())),
            chan_list: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_group(&self, name: &str, token: &str) {
        self.groups.lock().await.push((name.to_string(), token.to_string()));
    }

    pub fn check_token(&self, group: &str, token: &str) -> bool {
        if let Ok(groups) = self.groups.try_lock() {
            groups.iter().any(|(g, t)| g == group && t == token)
        } else {
            false
        }
    }

    pub async fn can_add_peer(&self) -> bool {
        self.peers.lock().await.len() < MAX_PEERS
    }

    pub async fn add_channel(&self, name: &str) {
        self.chan_list.lock().await.push(name.to_string());
    }

    pub async fn get_channel_list(&self) -> Vec<String> {
        self.chan_list.lock().await.clone()
    }

    fn clone_state(&self) -> (String, String, u16, Arc<Mutex<HashMap<String, PeerInfo>>>, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<(String, String)>>>, Arc<Mutex<Vec<PendingPeer>>>) {
        (self.node_id.clone(), self.node_name.clone(), self.port, self.peers.clone(), self.chan_list.clone(), self.groups.clone(), self.pending_peers.clone())
    }

    // ─── mDNS ────────────────────────────────────────

    pub async fn start_mdns_listener(&self) -> anyhow::Result<()> {
        let (node_id, _, _, peers, _, _, _) = self.clone_state();
        let bind: SocketAddr = format!("0.0.0.0:{}", self.port + 1).parse()?;
        let socket = tokio::net::UdpSocket::bind(bind).await?;

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                if let Ok((len, _addr)) = socket.recv_from(&mut buf).await {
                    if let Ok(msg) = serde_json::from_slice::<PeerInfo>(&buf[..len]) {
                        if msg.node_id != node_id {
                            info!("mDNS: {} groups: {}", msg.node_name, msg.groups.len());
                            peers.lock().await.insert(msg.node_id.clone(), msg);
                        }
                    }
                }
            }
        });
        info!("mDNS on {}", bind);
        Ok(())
    }

    pub async fn start_mdns_broadcast(&self, interval: u64) -> anyhow::Result<()> {
        let (node_id, node_name, port, _, chan_list, groups, _) = self.clone_state();
        let bind: SocketAddr = format!("0.0.0.0:{}", port + 2).parse()?;
        let socket = tokio::net::UdpSocket::bind(bind).await?;
        socket.set_broadcast(true)?;
        let target: SocketAddr = "255.255.255.255:42070".parse()?;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(interval)).await;
                let chs = chan_list.lock().await.clone();
                let gs = groups.lock().await.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
                let announce = PeerInfo {
                    node_id: node_id.clone(),
                    node_name: node_name.clone(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    addresses: vec![format!("tcp://0.0.0.0:{}", port)],
                    channels: chs,
                    groups: gs,
                    token: String::new(),
                    last_seen: chrono::Utc::now().to_rfc3339(),
                    uptime: 0,
                };
                if let Ok(data) = serde_json::to_vec(&announce) {
                    let _ = socket.send_to(&data, target).await;
                }
            }
        });
        info!("mDNS broadcast every {}s", interval);
        Ok(())
    }

    // ─── TCP listener ────────────────────────────────

    pub async fn start_tcp_listener(&self, mgr: Arc<Mutex<ChannelManager>>) -> anyhow::Result<()> {
        let (node_id, node_name, port, peers, chan_list, groups, pending_peers) = self.clone_state();
        let bind: SocketAddr = format!("0.0.0.0:{}", port + 3).parse()?;
        let listener = TcpListener::bind(bind).await?;

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        let p = peers.clone();
                        let nid = node_id.clone();
                        let nn = node_name.clone();
                        let cl = chan_list.clone();
                        let cm = mgr.clone();
                        let gs = groups.clone();
                        let pp = pending_peers.clone();
                        tokio::spawn(async move {
                            handle_incoming(stream, addr, &nid, &nn, &p, &cl, &cm, &gs, &pp).await.ok();
                        });
                    }
                    Err(e) => warn!("TCP accept: {}", e),
                }
            }
        });
        info!("TCP sync on {}", bind);
        Ok(())
    }

    // ─── Periodic gossip ─────────────────────────────

    pub async fn start_periodic_sync(&self, mgr: Arc<Mutex<ChannelManager>>, interval: u64) {
        let (node_id, node_name, _, peers, chan_list, groups, _) = self.clone_state();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(interval)).await;

                let peer_vec = peers.lock().await.values().cloned().collect::<Vec<_>>();
                if peer_vec.is_empty() { continue; }

                let idx = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as usize) % peer_vec.len();
                let peer = &peer_vec[idx];

                let addr = peer.addresses.first()
                    .and_then(|a| a.strip_prefix("tcp://"))
                    .map(|a| a.replace("0.0.0.0", "127.0.0.1"));

                if let Some(a) = addr {
                    info!("Gossip sync with {} ({})", peer.node_name, a);
                    if let Err(e) = sync_with_peer(&a, &node_id, &node_name, &peers, &chan_list, &mgr, &groups).await {
                        warn!("Gossip failed: {}", e);
                    }
                }
            }
        });
    }

    pub async fn direct_sync(&self, addr: &str, mgr: Arc<Mutex<ChannelManager>>) -> anyhow::Result<()> {
        sync_with_peer(addr, &self.node_id, &self.node_name, &self.peers, &self.chan_list, &mgr, &self.groups).await
    }

    /// Get pending peers awaiting approval
    pub async fn pending_list(&self) -> Vec<PendingPeer> {
        self.pending_peers.lock().await.clone()
    }

    /// Approve a pending peer by index
    pub async fn approve_pending(&self, idx: usize) -> Option<PendingPeer> {
        let mut pps = self.pending_peers.lock().await;
        if idx < pps.len() {
            Some(pps.remove(idx))
        } else {
            None
        }
    }

    /// Reject a pending peer by index
    pub async fn reject_pending(&self, idx: usize) -> Option<PendingPeer> {
        let mut pps = self.pending_peers.lock().await;
        if idx < pps.len() {
            Some(pps.remove(idx))
        } else {
            None
        }
    }

    pub fn peer_count(&self) -> usize {
        self.peers.try_lock().map(|p| p.len()).unwrap_or(0)
    }

    pub async fn list_peers(&self) -> Vec<PeerInfo> {
        self.peers.lock().await.values().cloned().collect()
    }
}

// ─── Incoming handler ───────────────────────────────

async fn handle_incoming(
    mut stream: TcpStream, addr: SocketAddr,
    node_id: &str, node_name: &str,
    peers: &Arc<Mutex<HashMap<String, PeerInfo>>>,
    _chan_list: &Arc<Mutex<Vec<String>>>,
    mgr: &Arc<Mutex<ChannelManager>>,
    groups: &Arc<Mutex<Vec<(String, String)>>>,
    pending_peers: &Arc<Mutex<Vec<PendingPeer>>>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
            let ev = msg["event"].as_str().unwrap_or("").to_string();
            match ev.as_str() {
                "handshake" => {
                    let pid = msg["node_id"].as_str().unwrap_or("?").to_string();
                    let pname = msg["node_name"].as_str().unwrap_or("?").to_string();
                    let their_groups: Vec<String> = msg["groups"].as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    let their_token = msg["token"].as_str().unwrap_or("").to_string();

                    // Chat approval: if groups exist, require user approval
                    {
                        let gs = groups.lock().await;
                        if !gs.is_empty() {
                            let pending = PendingPeer {
                                node_id: pid.clone(),
                                node_name: pname.clone(),
                                address: addr.to_string(),
                                groups: their_groups.clone(),
                                token: their_token.clone(),
                                arrived_at: chrono::Utc::now().to_rfc3339(),
                            };
                            pending_peers.lock().await.push(pending);

                            info!("Approval needed: {} from {} wants to join", pname, addr);
                            let wait = serde_json::json!({"event": "awaiting_approval", "node_id": pid, "reason": "admin_approval_required"});
                            let data = serde_json::to_vec(&wait)?;
                            writer.write_all(&data).await?;
                            continue;
                        }
                    }

                    // Check max peers
                    let current_peers = peers.lock().await.len();
                    if current_peers >= MAX_PEERS {
                        warn!("Max peers ({}) reached, rejecting {}", MAX_PEERS, pname);
                        let deny = serde_json::json!({"event": "access_denied", "reason": "max_peers"});
                        let data = serde_json::to_vec(&deny)?;
                        writer.write_all(&data).await?;
                        continue;
                    }

                    info!("Handshake from {} ({}) groups: {:?}", pname, addr, their_groups);

                    let peer_info = PeerInfo {
                        node_id: pid.clone(),
                        node_name: pname.clone(),
                        version: "0.2.0".into(),
                        addresses: vec![addr.to_string()],
                        channels: vec![],
                        groups: their_groups,
                        token: their_token,
                        last_seen: chrono::Utc::now().to_rfc3339(),
                        uptime: 0,
                    };
                    peers.lock().await.insert(pid.clone(), peer_info);

                    let cm_guard = mgr.lock().await;
                    let all_channels = cm_guard.list();
                    let mut seqs = HashMap::new();
                    for ch in &all_channels {
                        seqs.insert(ch.name.clone(), ch.message_count);
                    }
                    drop(cm_guard);

                    let my_groups = groups.lock().await.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
                    let my_tokens = groups.lock().await.first().map(|(_, t)| t.clone()).unwrap_or_default();

                    let response = serde_json::json!({
                        "event": "handshake_ack",
                        "node_id": node_id,
                        "node_name": node_name,
                        "groups": my_groups,
                        "token": my_tokens,
                        "channels": all_channels.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
                        "last_seq": seqs,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    let data = serde_json::to_vec(&response)?;
                    writer.write_all(&data).await?;
                }

                "sync_request" => {
                    let rid = msg["request_id"].as_str().unwrap_or("0");
                    let ch_name = msg["channel"].as_str().unwrap_or("");
                    let since = msg["since_seq"].as_u64().unwrap_or(0);

                    let cm_guard = mgr.lock().await;
                    let msgs = if ch_name.is_empty() {
                        vec![]
                    } else {
                        cm_guard.read(ch_name, since).await
                    };
                    drop(cm_guard);

                    let response = serde_json::json!({
                        "event": "sync_response",
                        "node_id": node_id,
                        "request_id": rid,
                        "channel": ch_name,
                        "messages": msgs,
                    });
                    let data = serde_json::to_vec(&response)?;
                    writer.write_all(&data).await?;
                }

                _ => warn!("Unknown event: {}", ev),
            }
        }
    }
    Ok(())
}

// ─── Outgoing sync ──────────────────────────────────

async fn sync_with_peer(
    addr: &str,
    node_id: &str,
    node_name: &str,
    peers: &Arc<Mutex<HashMap<String, PeerInfo>>>,
    chan_list: &Arc<Mutex<Vec<String>>>,
    mgr: &Arc<Mutex<ChannelManager>>,
    groups: &Arc<Mutex<Vec<(String, String)>>>,
) -> anyhow::Result<()> {
    let stream = TcpStream::connect(addr).await?;
    let (reader, mut writer) = tokio::io::split(stream);

    // 1. Handshake with groups + token
    let chs = chan_list.lock().await.clone();
    let gs = groups.lock().await.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    let tk = groups.lock().await.first().map(|(_, t)| t.clone()).unwrap_or_default();
    let h = serde_json::json!({
        "event": "handshake",
        "node_id": node_id,
        "node_name": node_name,
        "groups": gs,
        "token": tk,
        "channels": chs,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    writer.write_all(&serde_json::to_vec(&h)?).await?;

    // 2. Read handshake_ack
    let mut lines = BufReader::new(reader).lines();
    let mut remote_channels: Vec<String> = Vec::new();
    let mut remote_seqs: HashMap<String, u64> = HashMap::new();
    let mut remote_name = String::new();

    while let Some(line) = lines.next_line().await? {
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
            let ev = msg["event"].as_str().unwrap_or("").to_string();

            if ev == "access_denied" {
                let reason = msg["reason"].as_str().unwrap_or("unknown");
                warn!("Access denied: {} (connecting to {})", reason, addr);
                return Err(anyhow::anyhow!("Access denied: {}", reason));
            }

            if ev == "handshake_ack" {
                remote_name = msg["node_name"].as_str().unwrap_or("?").to_string();
                if let Some(arr) = msg["channels"].as_array() {
                    for v in arr {
                        if let Some(s) = v.as_str() {
                            remote_channels.push(s.to_string());
                        }
                    }
                }
                if let Some(obj) = msg["last_seq"].as_object() {
                    for (k, v) in obj {
                        if let Some(n) = v.as_u64() {
                            remote_seqs.insert(k.clone(), n);
                        }
                    }
                }

                let rn = remote_name.clone();
                let rid = msg["node_id"].as_str().unwrap_or("?").to_string();
                info!("Handshake done with {} ({}), {} channels", rn, addr, remote_channels.len());

                peers.lock().await.insert(rid.clone(), PeerInfo {
                    node_id: rid,
                    node_name: rn,
                    version: "".into(),
                    addresses: vec![addr.to_string()],
                    channels: remote_channels.clone(),
                    groups: vec![],
                    token: String::new(),
                    last_seen: chrono::Utc::now().to_rfc3339(),
                    uptime: 0,
                });
                break;
            }
        }
    }

    // 3. Sync channels
    for ch_name in &remote_channels {
        let local_count = {
            let cm_guard = mgr.lock().await;
            cm_guard.get_message_count(ch_name).unwrap_or(0)
        };
        let remote_count = remote_seqs.get(ch_name).copied().unwrap_or(0);
        if remote_count > local_count {
            let req = serde_json::json!({
                "event": "sync_request", "channel": ch_name,
                "since_seq": local_count,
                "request_id": uuid::Uuid::new_v4().to_string(),
                "node_id": node_id,
            });
            let data = serde_json::to_vec(&req)?;
            writer.write_all(&data).await?;
        }
    }

    info!("Sync with {} ({}) OK", remote_name, addr);
    Ok(())
}
