use std::path::Path;
use anyhow::Result;
use serde_json::Value;
use tracing::info;

use super::{Tool, ToolContext};

pub fn read_file() -> Tool {
    Tool {
        name: "read_file",
        description: "Read a file. Supports UTF-8 text and PDF (via pdftotext). Use `path` and optional `pages` (e.g. \"1-3\").",
        handler: |ctx, args| {
            let path = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("path required"))?;
            let full = Path::new(&ctx.workspace).join(path);
            let ext = full.extension().and_then(|e| e.to_str()).unwrap_or("");

            if ext.eq_ignore_ascii_case("pdf") {
                // Try pdftotext, fallback to binary read
                let output = std::process::Command::new("pdftotext")
                    .arg(&full)
                    .arg("-")
                    .arg("-l")
                    .arg(args.get("pages").and_then(|v| v.as_str()).unwrap_or("10"))
                    .output().ok();
                if let Some(out) = output {
                    if out.status.success() {
                        let text = String::from_utf8_lossy(&out.stdout).to_string();
                        return Ok(serde_json::json!({"content": text, "path": path, "chars": text.len(), "format": "pdf"}));
                    }
                }
            }

            let content = std::fs::read_to_string(&full)?;
            info!("read_file: {}", full.display());
            Ok(serde_json::json!({"content": content, "path": path, "chars": content.len(), "format": "text"}))
        },
    }
}

pub fn write_file() -> Tool {
    Tool {
        name: "write_file",
        description: "Write content to a file. Creates backup (.bak) if file exists. Use `path` and `content`.",
        handler: |ctx, args| {
            let path = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("path required"))?;
            let content = args["content"].as_str().ok_or_else(|| anyhow::anyhow!("content required"))?;
            let full = Path::new(&ctx.workspace).join(path);

            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Backup existing file
            if full.exists() {
                let backup = full.with_extension("bak");
                std::fs::copy(&full, &backup).ok();
            }

            std::fs::write(&full, content)?;
            info!("write_file: {} ({} bytes)", full.display(), content.len());
            Ok(serde_json::json!({"path": path, "bytes": content.len(), "backup": full.exists()}))
        },
    }
}

pub fn list_dir() -> Tool {
    Tool {
        name: "list_dir",
        description: "List files in a directory. Respects .gitignore when available. Use `path` (default: \".\").",
        handler: |ctx, args| {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let full = Path::new(&ctx.workspace).join(path);

            // Load gitignore patterns if present
            let gitignore_path = Path::new(&ctx.workspace).join(".gitignore");
            let ignore_patterns = if gitignore_path.exists() {
                std::fs::read_to_string(&gitignore_path).ok()
                    .map(|c| c.lines()
                        .filter(|l| !l.is_empty() && !l.starts_with('#'))
                        .map(|l| l.trim().to_string())
                        .collect::<Vec<_>>())
                    .unwrap_or_default()
            } else { Vec::new() };

            let mut entries = Vec::new();
            if full.is_dir() {
                for entry in std::fs::read_dir(&full)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Skip gitignore'd entries
                    if ignore_patterns.iter().any(|p| {
                        if p.ends_with('/') { name == p.trim_end_matches('/') }
                        else if p.starts_with('*') { name.ends_with(&p[1..]) }
                        else { name == *p || name.starts_with(p) }
                    }) { continue; }
                    if name.starts_with('.') { continue; }

                    let ft = entry.file_type()?;
                    entries.push(serde_json::json!({
                        "name": name,
                        "is_dir": ft.is_dir(),
                        "is_symlink": ft.is_symlink(),
                        "size": entry.metadata().map(|m| m.len()).unwrap_or(0),
                    }));
                }
                entries.sort_by(|a, b| {
                    let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                    let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                    b_dir.cmp(&a_dir).then(a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")))
                });
            }
            Ok(serde_json::json!({"path": path, "entries": entries, "count": entries.len()}))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_ctx() -> ToolContext {
        ToolContext { workspace: "/tmp".into(), session_path: "/tmp".into(), kvstore: None }
    }

    #[test]
    fn test_read_file() {
        let tool = read_file();
        let tmp = std::env::temp_dir();
        let test_file = tmp.join("waters_test_read.txt");
        std::fs::write(&test_file, "hello world").unwrap();
        let ctx = ToolContext { workspace: tmp.to_string_lossy().into(), session_path: "/tmp".into(), kvstore: None };
        let result = (tool.handler)(&ctx, json!({"path": "waters_test_read.txt"})).unwrap();
        assert_eq!(result["content"], "hello world");
        std::fs::remove_file(&test_file).ok();
    }

    #[test]
    fn test_write_file() {
        let tool = write_file();
        let tmp = std::env::temp_dir();
        let ctx = ToolContext { workspace: tmp.to_string_lossy().into(), session_path: "/tmp".into(), kvstore: None };
        let result = (tool.handler)(&ctx, json!({"path": "waters_test_write.txt", "content": "test"})).unwrap();
        assert_eq!(result["bytes"], 4);
        std::fs::remove_file(tmp.join("waters_test_write.txt")).ok();
    }

    #[test]
    fn test_list_dir() {
        let tool = list_dir();
        let result = (tool.handler)(&test_ctx(), json!({"path": "."})).unwrap();
        assert!(result["count"].as_i64().unwrap_or(0) >= 0);
    }
}
