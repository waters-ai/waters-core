use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use tokio::process::Command;
use tracing::{info, warn};

pub struct McpClient {
    servers: HashMap<String, McpServerHandle>,
}

struct McpServerHandle {
    command: String,
    args: Vec<String>,
}

impl McpClient {
    pub fn new() -> Self {
        McpClient {
            servers: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, _transport: &str, command: &str, args: &[String]) {
        self.servers.insert(
            name.to_string(),
            McpServerHandle {
                command: command.to_string(),
                args: args.to_vec(),
            },
        );
        info!("MCP server registered: {} ({})", name, _transport);
    }

    pub async fn call_tool(&self, server: &str, tool: &str, args: &Value) -> Result<Value> {
        let handle = self
            .servers
            .get(server)
            .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found", server))?;

        let input = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": tool,
                "arguments": args
            },
            "id": 1
        });

        let serialized = serde_json::to_string(&input)?;

        let output = Command::new(&handle.command)
            .args(&handle.args)
            .arg("--mcp")
            .arg("-")
            .arg(&serialized)
            .output()
            .await?;

        if output.status.success() {
            let result: Value = serde_json::from_slice(&output.stdout)?;
            Ok(result)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("MCP call failed: {stderr} (server: {server}, tool: {tool})");
            Err(anyhow::anyhow!("MCP call failed: {stderr}"))
        }
    }

    pub async fn healthcheck(&self) -> Vec<(String, bool)> {
        let mut results = Vec::new();
        for (name, handle) in &self.servers {
            let alive = Command::new(&handle.command)
                .args(&handle.args)
                .arg("--health")
                .output()
                .await
                .is_ok();
            results.push((name.clone(), alive));
            if alive {
                info!("MCP server {name} is healthy");
            } else {
                warn!("MCP server {name} is unhealthy");
            }
        }
        results
    }
}
