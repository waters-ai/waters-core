use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, warn};

use crate::skill::SkillRegistry;
use crate::subagent::SubAgentManager;
use crate::store::KvStore;

/// MCP-сервер, который выставляет агентов WATERS как MCP-инструменты.
/// Позволяет внешним клиентам (Claude Code, Cursor, TUI) видеть и вызывать
/// наших агентов через стандартный MCP протокол (JSON-RPC 2.0).
pub struct McpServer {
    pub port: u16,
    kvstore: Arc<KvStore>,
    skills: Arc<SkillRegistry>,
    subagents: Arc<SubAgentManager>,
    next_id: AtomicU64,
}

impl McpServer {
    pub fn new(
        port: u16,
        kvstore: Arc<KvStore>,
        skills: Arc<SkillRegistry>,
        subagents: Arc<SubAgentManager>,
    ) -> Self {
        McpServer {
            port,
            kvstore,
            skills,
            subagents,
            next_id: AtomicU64::new(1),
        }
    }

    pub async fn serve(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("MCP-сервер агентов на tcp://{}", addr);

        loop {
            let (socket, _) = listener.accept().await?;
            let handler = McpHandler {
                kvstore: self.kvstore.clone(),
                skills: self.skills.clone(),
                subagents: self.subagents.clone(),
                next_id: AtomicU64::new(1),
            };
            tokio::spawn(async move {
                let _ = handler.handle_conn(socket).await;
            });
        }
    }
}

struct McpHandler {
    kvstore: Arc<KvStore>,
    skills: Arc<SkillRegistry>,
    subagents: Arc<SubAgentManager>,
    next_id: AtomicU64,
}

impl McpHandler {
    async fn handle_conn(&self, stream: TcpStream) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        let mut buffer = String::new();

        while let Some(line) = lines.next_line().await? {
            if line.is_empty() && !buffer.is_empty() {
                // Пустая строка = конец JSON-RPC запроса
                if let Some(response) = self.handle_request(&buffer).await {
                    let resp = format!("{}\n\n", response);
                    writer.write_all(resp.as_bytes()).await?;
                    writer.flush().await?;
                }
                buffer.clear();
            } else {
                buffer.push_str(&line);
            }
        }
        Ok(())
    }

    async fn handle_request(&self, json: &str) -> Option<String> {
        let req: Value = serde_json::from_str(json).ok()?;
        let method = req["method"].as_str()?;
        let id = req["id"].clone();

        let result = match method {
            "initialize" => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        },
                        "resources": {
                            "subscribe": false
                        }
                    },
                    "serverInfo": {
                        "name": "waters-node",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            })),

            "tools/list" => {
                let skills = self.skills.list();
                let tools: Vec<Value> = skills.iter().map(|s| {
                    serde_json::json!({
                        "name": s.manifest.name,
                        "description": s.manifest.description,
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task": {
                                    "type": "string",
                                    "description": "Задача для агента"
                                },
                                "confidence": {
                                    "type": "number",
                                    "description": "Минимальная уверенность (0.0-1.0)"
                                }
                            },
                            "required": ["task"]
                        }
                    })
                }).collect();

                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tools }
                }))
            }

            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("").to_string();
                let args = &req["params"]["arguments"];
                let task = args["task"].as_str().unwrap_or("default task");

                // Ищем скилл в реестре
                let skill = self.skills.get(&name);
                if skill.is_none() {
                    return Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32602,
                            "message": format!("Agent '{}' not found", name)
                        }
                    }).to_string());
                }

                let skill = match skill {
                    Some(s) => s,
                    None => {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "error": {"code": -32602, "message": "Agent not found"}
                        }).to_string());
                    }
                };
                let role = &skill.manifest.role;
                let llm = &skill.manifest.llm.preferred;
                let node_id = "mcp-server";

                // Открываем агента
                match self.subagents.agent_open(role, &name, llm, 0, node_id, None, false).await {
                    Ok(agent_id) => {
                        // Назначаем задачу
                        // Assign is best-effort for MCP calls
                        let _ = self.subagents.agent_assign(&agent_id, task, 0).await;

                        // Создаём finding-результат
                        let finding_data = serde_json::json!({
                            "agent_name": name,
                            "task": task,
                            "skill_prompt": skill.prompt.chars().take(200).collect::<String>(),
                        });

                        let _ = self.subagents.agent_complete_with_finding(
                            &agent_id, "mcp_result", 0.8,
                            finding_data, &name, node_id, 0,
                        );

                        // Читаем результат
                        let result = self.subagents.agent_eval(&agent_id, 0)
                            .map(|r| serde_json::json!({
                                "content": [{
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&r).unwrap_or_default()
                                }]
                            }))
                            .unwrap_or_else(|e| serde_json::json!({
                                "content": [{"type": "text", "text": format!("Error: {}", e)}],
                                "isError": true
                            }));

                        Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        }))
                    }
                    Err(e) => {
                        Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32000,
                                "message": format!("Agent error: {}", e)
                            }
                        }))
                    }
                }
            }

            "resources/list" => {
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "resources": [{
                            "uri": "waters://agents",
                            "name": "WATERS Agents",
                            "description": "Available agents on this node"
                        }, {
                            "uri": "waters://findings",
                            "name": "WATERS Findings",
                            "description": "Recent findings from agents"
                        }]
                    }
                }))
            }

            "resources/read" => {
                let uri = req["params"]["uri"].as_str().unwrap_or("");
                let content = match uri {
                    "waters://agents" => {
                        let skills = self.skills.list();
                        let text: String = skills.iter()
                            .map(|s| format!("- {}: {} (role: {}, llm: {})",
                                s.manifest.name, s.manifest.description,
                                s.manifest.role, s.manifest.llm.preferred))
                            .collect::<Vec<_>>()
                            .join("\n");
                        text
                    }
                    "waters://findings" => {
                        format!("Active agents: {}",
                            self.subagents.list_active(0).unwrap_or_default().len())
                    }
                    _ => format!("Unknown resource: {}", uri),
                };
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "contents": [{
                            "uri": uri,
                            "mimeType": "text/plain",
                            "text": content
                        }]
                    }
                }))
            }

            "notifications/initialized" => {
                None // No response needed
            }

            _ => {
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method '{}' not found", method)
                    }
                }))
            }
        };

        result.map(|r| r.to_string())
    }
}
