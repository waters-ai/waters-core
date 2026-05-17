use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

pub struct SelfDeploy {
    pub build_dir: PathBuf,
    pub binary_name: String,
    pub backup_dir: PathBuf,
}

impl SelfDeploy {
    pub fn new(build_dir: &Path, binary_name: &str) -> Self {
        SelfDeploy {
            build_dir: build_dir.to_path_buf(),
            binary_name: binary_name.to_string(),
            backup_dir: PathBuf::from("/tmp/waters-backup"),
        }
    }

    /// Полный цикл деплоя: build → backup → replace → restart
    pub fn deploy(&self) -> Result<String, String> {
        info!("SelfDeploy: starting...");

        // 1. Build
        let build = self.build_release()?;

        // 2. Backup текущего бинарника
        let backup = self.backup_current()?;

        // 3. Replace
        self.replace_binary()?;

        Ok(format!("✅ Деплой завершён\n  Build: {}\n  Backup: {}\n  Новый бинарник: {}", build, backup, self.binary_name))
    }

    fn build_release(&self) -> Result<String, String> {
        info!("SelfDeploy: building release...");
        let output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&self.build_dir)
            .output()
            .map_err(|e| format!("cargo not found: {}", e))?;

        if output.status.success() {
            let target = self.build_dir.join("target/release/waters-node");
            if target.exists() {
                let size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                Ok(format!("{} MB", size / 1_048_576))
            } else {
                Err("Build succeeded but binary not found".into())
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("SelfDeploy: build failed: {}", stderr);
            Err(format!("Build failed:\n{}", stderr.chars().take(500).collect::<String>()))
        }
    }

    fn backup_current(&self) -> Result<String, String> {
        let current = &self.binary_name;
        if !Path::new(current).exists() {
            return Ok("(нет текущего бинарника)".into());
        }
        std::fs::create_dir_all(&self.backup_dir).map_err(|e| format!("Cannot create backup dir: {}", e))?;
        let backup_name = format!("waters-node.backup.{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
        let backup_path = self.backup_dir.join(&backup_name);
        std::fs::copy(current, &backup_path).map_err(|e| format!("Backup failed: {}", e))?;
        Ok(backup_name)
    }

    fn replace_binary(&self) -> Result<(), String> {
        let source = self.build_dir.join("target/release/waters-node");
        let dest = Path::new(&self.binary_name);
        if !source.exists() {
            return Err("Release binary not found".into());
        }
        std::fs::copy(&source, dest).map_err(|e| format!("Replace failed: {}", e))?;
        info!("SelfDeploy: binary replaced: {} → {}", source.display(), dest.display());
        Ok(())
    }

    pub fn summary(&self) -> String {
        format!("🔧 SelfDeploy: build_dir={}, binary={}", self.build_dir.display(), self.binary_name)
    }
}
