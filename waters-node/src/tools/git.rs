use anyhow::Result;
use serde_json::Value;
use tracing::info;

use super::{Tool, ToolContext};

pub fn git_status() -> Tool {
    Tool {
        name: "git_status",
        description: "Show git status. Requires git in PATH.",
        handler: |ctx, _args| {
            let output = std::process::Command::new("git")
                .args(["status", "--short"])
                .current_dir(&ctx.workspace)
                .output()?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(serde_json::json!({"status": stdout, "is_repo": !stdout.is_empty() || output.status.success()}))
        },
    }
}

pub fn git_diff() -> Tool {
    Tool {
        name: "git_diff",
        description: "Show git diff. Use `staged` (bool) for --cached.",
        handler: |ctx, args| {
            let staged = args.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
            let mut cmd = std::process::Command::new("git");
            cmd.arg("diff").current_dir(&ctx.workspace);
            if staged { cmd.arg("--cached"); }
            let output = cmd.output()?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(serde_json::json!({"diff": stdout, "has_changes": !stdout.is_empty()}))
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
    fn test_git_status_not_repo() {
        let tool = git_status();
        let result = (tool.handler)(&test_ctx(), json!({})).unwrap();
        assert_eq!(result["is_repo"], false);
    }
}
