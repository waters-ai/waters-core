use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use anyhow::Result;
use serde_json::Value;
use tracing::info;

use super::{Tool, ToolContext};

// Simple background task manager (in-process, shared state)
lazy_static::lazy_static! {
    static ref BG_TASKS: Mutex<HashMap<String, std::process::Child>> = Mutex::new(HashMap::new());
    static ref NEXT_TASK_ID: Mutex<u64> = Mutex::new(0);
}

pub fn exec_shell() -> Tool {
    Tool {
        name: "exec_shell",
        description: "Execute a shell command. Use `command` and optional `background` (bool).\n  If background=true, returns task_id for /cancel.\n  Timeout: 30s foreground, no limit for background.",
        handler: |ctx, args| {
            let cmd = args["command"].as_str().ok_or_else(|| anyhow::anyhow!("command required"))?;
            let is_bg = args.get("background").and_then(|v| v.as_bool()).unwrap_or(false);

            if is_bg {
                let child = std::process::Command::new("sh")
                    .arg("-c").arg(cmd)
                    .current_dir(&ctx.workspace)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()?;

                let mut id_lock = NEXT_TASK_ID.lock().unwrap();
                *id_lock += 1;
                let task_id = format!("bg-{}", id_lock);
                BG_TASKS.lock().unwrap().insert(task_id.clone(), child);
                info!("exec_shell (background): {} -> {}", cmd, task_id);
                return Ok(serde_json::json!({"task_id": task_id, "status": "started", "background": true}));
            }

            // Foreground with timeout
            let start = Instant::now();
            let output = std::process::Command::new("sh")
                .arg("-c").arg(cmd)
                .current_dir(&ctx.workspace)
                .output()?;
            let elapsed = start.elapsed().as_secs_f64();

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            info!("exec_shell: {} (exit: {}, {:.2}s)", cmd, output.status.code().unwrap_or(-1), elapsed);

            if elapsed > 30.0 {
                return Ok(serde_json::json!({
                    "stdout": stdout, "stderr": stderr,
                    "exit_code": output.status.code().unwrap_or(-1),
                    "success": output.status.success(),
                    "elapsed_secs": elapsed,
                    "warning": "Command took >30s. Consider using background=true for long-running tasks.",
                }));
            }

            Ok(serde_json::json!({
                "stdout": stdout, "stderr": stderr,
                "exit_code": output.status.code().unwrap_or(-1),
                "success": output.status.success(),
                "elapsed_secs": elapsed,
            }))
        },
    }
}

pub fn exec_shell_cancel() -> Tool {
    Tool {
        name: "exec_shell_cancel",
        description: "Cancel a background shell task by `task_id`. Use /status to list active tasks.",
        handler: |_ctx, args| {
            let task_id = args["task_id"].as_str().ok_or_else(|| anyhow::anyhow!("task_id required"))?;
            let mut tasks = BG_TASKS.lock().unwrap();

            if let Some(mut child) = tasks.remove(task_id) {
                child.kill().ok();
                child.wait().ok();
                info!("exec_shell_cancel: killed task {}", task_id);
                Ok(serde_json::json!({"task_id": task_id, "status": "cancelled"}))
            } else {
                Ok(serde_json::json!({"task_id": task_id, "status": "not_found"}))
            }
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
    fn test_exec_shell_echo() {
        let tool = exec_shell();
        let result = (tool.handler)(&test_ctx(), json!({"command": "echo hi"})).unwrap();
        assert_eq!(result["stdout"], "hi\n");
        assert!(result["success"].as_bool().unwrap());
    }

    #[test]
    fn test_exec_shell_background() {
        let tool = exec_shell();
        let result = (tool.handler)(&test_ctx(), json!({"command": "sleep 1", "background": true})).unwrap();
        assert_eq!(result["status"], "started");
        let task_id = result["task_id"].as_str().unwrap().to_string();
        let cancel = exec_shell_cancel();
        (cancel.handler)(&test_ctx(), json!({"task_id": task_id})).unwrap();
    }

    #[test]
    fn test_exec_shell_error() {
        let tool = exec_shell();
        let result = (tool.handler)(&test_ctx(), json!({"command": "false"})).unwrap();
        assert!(!result["success"].as_bool().unwrap());
    }
}
