use anyhow::Result;
use serde_json::Value;
use std::path::Path;
use tracing::info;

// ─── Helpers for web_search ────────────────────────

fn urlencoding(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        _ => format!("%{:02X}", c as u8),
    }).collect()
}

fn extract_href(line: &str) -> Option<String> {
    let start = line.find("href=\"")?;
    let rest = &line[start + 6..];
    let end = rest.find('"')?;
    let href = &rest[..end];
    if href.starts_with("http") { Some(href.to_string()) } else { None }
}

fn extract_title(line: &str) -> Option<String> {
    let start = line.find(">")?;
    let rest = &line[start + 1..];
    let end = rest.find('<')?;
    let title = rest[..end].trim();
    if title.is_empty() { None } else { Some(title.to_string()) }
}

fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    // Collapse whitespace
    let mut out = String::new();
    let mut prev_space = false;
    for c in result.chars() {
        if c.is_whitespace() {
            if !prev_space { out.push(' '); }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().chars().take(5000).collect()
}

pub struct ToolRegistry {
    tools: Vec<Tool>,
}

pub struct Tool {
    pub name: String,
    pub description: String,
    pub handler: fn(&ToolContext, Value) -> Result<Value>,
}

pub struct ToolContext {
    pub workspace: String,
    pub session_path: String,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut reg = ToolRegistry { tools: Vec::new() };
        reg.register(Tool {
            name: "read_file".into(),
            description: "Read a file from the workspace".into(),
            handler: |ctx, args| {
                let path = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("path required"))?;
                let full = Path::new(&ctx.workspace).join(path);
                let content = std::fs::read_to_string(&full)?;
                info!("read_file: {}", full.display());
                Ok(serde_json::json!({ "content": content, "path": path, "chars": content.len() }))
            },
        });
        reg.register(Tool {
            name: "write_file".into(),
            description: "Write content to a file".into(),
            handler: |ctx, args| {
                let path = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("path required"))?;
                let content = args["content"].as_str().ok_or_else(|| anyhow::anyhow!("content required"))?;
                let full = Path::new(&ctx.workspace).join(path);
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&full, content)?;
                info!("write_file: {} ({} bytes)", full.display(), content.len());
                Ok(serde_json::json!({ "path": path, "bytes": content.len() }))
            },
        });
        reg.register(Tool {
            name: "grep_files".into(),
            description: "Search for a pattern in files".into(),
            handler: |ctx, args| {
                let pattern = args["pattern"].as_str().ok_or_else(|| anyhow::anyhow!("pattern required"))?;
                let include = args.get("include").and_then(|v| v.as_str()).unwrap_or("*");
                let re = regex::Regex::new(pattern)?;
                let mut results = Vec::new();
                let glob_pattern = format!("{}/**/{}", ctx.workspace, include);
                for entry in glob::glob(&glob_pattern).map_err(|e| anyhow::anyhow!("glob: {}", e))? {
                    let entry = entry?;
                    if entry.is_file() {
                        if let Ok(content) = std::fs::read_to_string(&entry) {
                            for (i, line) in content.lines().enumerate() {
                                if re.is_match(line) {
                                    let rel = entry.strip_prefix(&ctx.workspace)
                                        .unwrap_or(&entry).to_string_lossy();
                                    results.push(serde_json::json!({
                                        "file": rel, "line": i + 1, "match": line
                                    }));
                                }
                            }
                        }
                    }
                }
                Ok(serde_json::json!({ "matches": results, "count": results.len() }))
            },
        });
        reg.register(Tool {
            name: "exec_shell".into(),
            description: "Execute a shell command".into(),
            handler: |ctx, args| {
                let cmd = args["command"].as_str().ok_or_else(|| anyhow::anyhow!("command required"))?;
                let output = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .current_dir(&ctx.workspace)
                    .output()?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                info!("exec_shell: {} (exit: {})", cmd, output.status.code().unwrap_or(-1));
                Ok(serde_json::json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": output.status.code().unwrap_or(-1),
                    "success": output.status.success(),
                }))
            },
        });
        reg.register(Tool {
            name: "list_dir".into(),
            description: "List files in a directory".into(),
            handler: |ctx, args| {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let full = Path::new(&ctx.workspace).join(path);
                let mut entries = Vec::new();
                if full.is_dir() {
                    for entry in std::fs::read_dir(&full)? {
                        let entry = entry?;
                        entries.push(serde_json::json!({
                            "name": entry.file_name().to_string_lossy(),
                            "is_dir": entry.file_type()?.is_dir(),
                        }));
                    }
                }
                Ok(serde_json::json!({ "path": path, "entries": entries }))
            },
        });
        reg.register(Tool {
            name: "web_search".into(),
            description: "Search the web. Returns URL + title + snippet. Region: ru/us/cn".into(),
            handler: |_ctx, args| {
                let query = args["query"].as_str().ok_or_else(|| anyhow::anyhow!("query required"))?;
                let region = args.get("region").and_then(|v| v.as_str()).unwrap_or("us");
                info!("web_search: '{}' region={}", &query[..query.len().min(60)], region);

                // Use DuckDuckGo (free, no key)
                let url = format!("https://duckduckgo.com/html/?q={}", urlencoding(query));
                let client = reqwest::blocking::Client::new();
                let resp = client.get(&url)
                    .header("User-Agent", "Mozilla/5.0 (compatible; waters-node)")
                    .send()?;
                let html = resp.text()?;

                // Simple parsing of DDG results
                let mut results = Vec::new();
                for line in html.lines() {
                    if line.contains("class=\"result__a\"") {
                        if let Some(href) = extract_href(line) {
                            if let Some(title) = extract_title(line) {
                                results.push(serde_json::json!({
                                    "url": href, "title": title,
                                    "region": region,
                                }));
                                if results.len() >= 8 { break; }
                            }
                        }
                    }
                }

                Ok(serde_json::json!({
                    "query": query, "region": region,
                    "results": results, "count": results.len()
                }))
            },
        });
        reg.register(Tool {
            name: "fetch_url".into(),
            description: "Fetch a URL and return its text content".into(),
            handler: |_ctx, args| {
                let url = args["url"].as_str().ok_or_else(|| anyhow::anyhow!("url required"))?;
                info!("fetch_url: {}", &url[..url.len().min(80)]);
                let client = reqwest::blocking::Client::new();
                let resp = client.get(url)
                    .header("User-Agent", "Mozilla/5.0 (compatible; waters-node)")
                    .send()?;
                let content = resp.text()?;
                let stripped = strip_html(&content);
                Ok(serde_json::json!({
                    "url": url, "content": stripped, "chars": stripped.len()
                }))
            },
        });
        reg.register(Tool {
            name: "git_status".into(),
            description: "Show git status (requires git in PATH)".into(),
            handler: |ctx, _args| {
                let output = std::process::Command::new("git")
                    .args(["status", "--short"])
                    .current_dir(&ctx.workspace)
                    .output()?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                Ok(serde_json::json!({
                    "status": stdout,
                    "is_repo": !stdout.is_empty() || output.status.success(),
                }))
            },
        });
        reg.register(Tool {
            name: "git_diff".into(),
            description: "Show git diff (requires git in PATH)".into(),
            handler: |ctx, args| {
                let staged = args.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut cmd = std::process::Command::new("git");
                cmd.arg("diff");
                if staged { cmd.arg("--cached"); }
                cmd.current_dir(&ctx.workspace);
                let output = cmd.output()?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                Ok(serde_json::json!({
                    "diff": stdout, "has_changes": !stdout.is_empty()
                }))
            },
        });
        reg
    }
    }

    pub fn register(&mut self, tool: Tool) {
        info!("Tool registered: {}", tool.name);
        self.tools.push(tool);
    }

    pub fn call(&self, name: &str, ctx: &ToolContext, args: Value) -> Result<Value> {
        self.tools.iter()
            .find(|t| t.name == name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))
            .and_then(|t| (t.handler)(ctx, args))
    }

    pub fn list(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name.clone()).collect()
    }
}
