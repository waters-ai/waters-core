use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpStatus {
    Starting,
    Ready,
    Failed(String),
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub server_name: String,
    pub tool_name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
}

pub struct McpServerProcess {
    stdin: ChildStdin,
    _child: Child,
    started_at: Instant,
}

impl std::fmt::Debug for McpServerProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServerProcess").finish()
    }
}

#[derive(Debug)]
pub struct McpClient {
    servers: HashMap<String, McpServerHandle>,
    processes: Arc<Mutex<HashMap<String, McpServerProcess>>>,
    next_id: Arc<AtomicU64>,
}

#[derive(Debug)]
struct McpServerHandle {
    command: String,
    args: Vec<String>,
    status: McpStatus,
    tools: Vec<McpToolInfo>,
    last_healthcheck: Instant,
}

impl McpClient {
    pub fn new() -> Self {
        McpClient {
            servers: HashMap::new(),
            processes: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn register(&mut self, name: &str, _transport: &str, command: &str, args: &[String]) {
        self.servers.insert(
            name.to_string(),
            McpServerHandle {
                command: command.to_string(),
                args: args.to_vec(),
                status: McpStatus::Starting,
                tools: Vec::new(),
                last_healthcheck: Instant::now(),
            },
        );
        info!("MCP server registered: {} ({})", name, _transport);
    }

    pub fn tool_discovery(&mut self) -> Vec<McpToolInfo> {
        let mut all_tools = Vec::new();
        let server_names: Vec<String> = self.servers.keys().cloned().collect();

        for name in &server_names {
            if let Some(handle) = self.servers.get_mut(name) {
                let result = Self::request(
                    &handle.command,
                    &handle.args,
                    serde_json::json!({
                        "jsonrpc": "2.0", "method": "tools/list", "params": {}, "id": 1
                    }),
                );

                match result {
                    Ok(resp) => {
                        let tools = resp["result"]["tools"]
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .map(|t| McpToolInfo {
                                        server_name: name.clone(),
                                        tool_name: t["name"].as_str().unwrap_or("?").to_string(),
                                        description: t["description"].as_str().map(String::from),
                                        input_schema: t.get("inputSchema").cloned(),
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();

                        handle.tools = tools.clone();
                        handle.status = McpStatus::Ready;
                        info!("MCP {}: discovered {} tools", name, tools.len());
                        all_tools.extend(tools);
                    }
                    Err(e) => {
                        handle.status = McpStatus::Failed(format!("discovery: {}", e));
                        warn!("MCP {} tool discovery failed: {}", name, e);
                    }
                }
            }
        }
        all_tools
    }

    pub fn call_tool(&self, server: &str, tool: &str, args: &Value) -> Result<Value> {
        let handle = self
            .servers
            .get(server)
            .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found", server))?;

        if handle.status != McpStatus::Ready {
            return Err(anyhow::anyhow!(
                "MCP server '{}' is not ready (status: {:?})",
                server,
                handle.status
            ));
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let input = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": { "name": tool, "arguments": args },
            "id": id,
        });

        let result = Self::request(&handle.command, &handle.args, input)?;

        if let Some(err) = result.get("error") {
            warn!(
                "MCP call error: {} (server: {}, tool: {})",
                err, server, tool
            );
            return Err(anyhow::anyhow!("MCP error: {:?}", err));
        }

        Ok(result["result"].clone())
    }

    fn request(command: &str, args: &[String], request: Value) -> Result<Value> {
        let serialized = serde_json::to_string(&request)?;
        let output = Command::new(command)
            .args(args)
            .arg("--mcp")
            .arg("-")
            .arg(&serialized)
            .output()?;

        if output.status.success() {
            let result: Value = serde_json::from_slice(&output.stdout)?;
            Ok(result)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("MCP request failed: {}", stderr))
        }
    }

    pub fn healthcheck(&self) -> Vec<(String, McpStatus)> {
        let mut results = Vec::new();
        for (name, handle) in &self.servers {
            let alive = Command::new(&handle.command)
                .args(&handle.args)
                .arg("--health")
                .arg("-")
                .output()
                .is_ok();
            results.push((
                name.clone(),
                if alive {
                    McpStatus::Ready
                } else {
                    McpStatus::Failed("healthcheck failed".into())
                },
            ));
        }
        results
    }

    pub fn get_tools(&self, server: &str) -> Vec<McpToolInfo> {
        self.servers
            .get(server)
            .map(|h| h.tools.clone())
            .unwrap_or_default()
    }

    pub fn list_servers(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }

    pub fn get_status(&self, server: &str) -> Option<McpStatus> {
        self.servers.get(server).map(|h| h.status.clone())
    }
}

#[cfg(test)]
#[test]
fn test_mcp_security_blocked_patterns() {
    use crate::mcp::McpSecurity;
    let sec = McpSecurity::new();
    assert!(sec
        .validate_tool_args("exec_shell", &serde_json::json!({"cmd": "rm -rf /"}))
        .is_err());
    assert!(sec
        .validate_tool_args("exec_shell", &serde_json::json!({"cmd": "ls -la"}))
        .is_ok());
}

#[test]
fn test_mcp_security_system_paths() {
    use crate::mcp::McpSecurity;
    let sec = McpSecurity::new();
    assert!(sec
        .validate_tool_args("write_file", &serde_json::json!({"path": "/etc/passwd"}))
        .is_err());
    assert!(sec
        .validate_tool_args("write_file", &serde_json::json!({"path": "/tmp/test.txt"}))
        .is_ok());
}

#[test]
fn test_mcp_security_sanitize() {
    use crate::mcp::McpSecurity;
    let sec = McpSecurity::new();
    let log = r#"{"api_key": "sk-secret123", "data": "ok"}"#;
    let clean = sec.sanitize_for_log(log);
    assert!(!clean.contains("sk-secret123"));
    assert!(clean.contains("***"));
}
mod tests {
    use super::*;

    #[test]
    fn test_mcp_new() {
        let client = McpClient::new();
        assert!(client.list_servers().is_empty());
    }

    #[test]
    fn test_mcp_register() {
        let mut client = McpClient::new();
        client.register("test", "stdio", "echo", &[]);
        assert_eq!(client.list_servers().len(), 1);
    }
}

/// ---------- MCP Security — защита от утечек и инъекций ----------

pub struct McpSecurity {
    /// Паттерны, которые нужно заблокировать в tool args
    blocked_patterns: Vec<String>,
    /// Поля, которые нужно маскировать в логах
    secret_fields: Vec<String>,
}

impl McpSecurity {
    pub fn new() -> Self {
        McpSecurity {
            blocked_patterns: vec![
                "rm -rf".into(),
                "sudo".into(),
                "DROP TABLE".into(),
                "DELETE FROM".into(),
                "exec(".into(),
                "eval(".into(),
                "os.system".into(),
                "subprocess".into(),
            ],
            secret_fields: vec![
                "api_key".into(),
                "password".into(),
                "token".into(),
                "secret".into(),
                "auth".into(),
                "key".into(),
                "passwd".into(),
            ],
        }
    }

    /// Проверить tool args на опасные паттерны
    pub fn validate_tool_args(&self, tool: &str, args: &serde_json::Value) -> Result<(), String> {
        let args_str = serde_json::to_string(args)
            .unwrap_or_default()
            .to_lowercase();
        for pattern in &self.blocked_patterns {
            if args_str.contains(&pattern.to_lowercase()) {
                return Err(format!(
                    "Blocked dangerous pattern in tool '{}': {}",
                    tool, pattern
                ));
            }
        }
        // Блокируем запись в системные файлы через любые tools
        if tool == "write_file" || tool == "exec_shell" || tool == "file_write" {
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                let dangerous = [
                    "/etc/",
                    "/usr/",
                    "/boot/",
                    "/var/",
                    "/sys/",
                    ".ssh/",
                    ".git/config",
                    "authorized_keys",
                ];
                for d in &dangerous {
                    if path.contains(d) {
                        return Err(format!("Blocked write to system path: {}", d));
                    }
                }
            }
        }
        Ok(())
    }

    /// Замаскировать секреты в логах
    pub fn sanitize_for_log(&self, data: &str) -> String {
        let mut result = data.to_string();
        for field in &self.secret_fields {
            // Маскируем "field": "value" → "field": "***"
            let pattern = format!("\"{}\": \"", field);
            let mut search_start = 0;
            while let Some(start) = result[search_start..].find(&pattern) {
                let abs_start = search_start + start;
                let value_start = abs_start + pattern.len();
                // Находим закрывающую кавычку
                if let Some(end) = result[value_start..].find('\"') {
                    let abs_end = value_start + end;
                    if abs_end > value_start {
                        result.replace_range(value_start..abs_end, "***");
                        search_start = abs_end + 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        result
    }

    pub fn summary(&self) -> String {
        format!(
            "🔒 MCP Security: {} blocked patterns, {} secret fields masked",
            self.blocked_patterns.len(),
            self.secret_fields.len()
        )
    }
}
