pub mod file;
pub mod search;
pub mod shell;
pub mod git;

use std::sync::Arc;
use anyhow::Result;
use serde_json::Value;
use tracing::info;

use crate::store::KvStore;

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: fn(&ToolContext, Value) -> Result<Value>,
}

pub struct ToolContext {
    pub workspace: String,
    pub session_path: String,
    pub kvstore: Option<Arc<KvStore>>,
}

pub struct ToolRegistry {
    tools: Vec<Tool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut reg = ToolRegistry { tools: Vec::new() };
        reg.add(file::read_file());
        reg.add(file::write_file());
        reg.add(file::list_dir());
        reg.add(search::grep_files());
        reg.add(search::web_search());
        reg.add(search::fetch_url());
        reg.add(shell::exec_shell());
        reg.add(shell::exec_shell_cancel());
        reg.add(git::git_status());
        reg.add(git::git_diff());
        reg
    }

    pub fn add(&mut self, tool: Tool) {
        info!("Tool registered: {}", tool.name);
        self.tools.push(tool);
    }

    pub fn call(&self, name: &str, ctx: &ToolContext, args: Value) -> Result<Value> {
        self.tools.iter()
            .find(|t| t.name == name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))
            .and_then(|t| (t.handler)(ctx, args))
    }

    pub fn list(&self) -> Vec<&'static str> {
        self.tools.iter().map(|t| t.name).collect()
    }
}
