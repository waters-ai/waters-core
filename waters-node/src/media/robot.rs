/// Robot bridge — управление роботами через чат
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RobotType {
    Vacuum,    // пылесос
    Lawnmower, // газонокосилка
    Drone,     // дрон
    Arm,       // манипулятор
    Delivery,  // доставщик
    Companion, // компаньон (социальный робот)
    Cleaner,   // мойщик окон/фасадов
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotConfig {
    pub name: String,
    pub robot_type: RobotType,
    pub address: String,
    pub api_key: String,
    pub max_speed: f64,
    pub battery_warn: u8,
}

pub struct RobotFleet {
    robots: Vec<RobotConfig>,
    chat_enabled: bool,
}

impl RobotFleet {
    pub fn new() -> Self {
        RobotFleet {
            robots: Vec::new(),
            chat_enabled: true,
        }
    }

    pub fn add_robot(&mut self, config: RobotConfig) {
        info!("Robot: added '{}' ({:?})", config.name, config.robot_type);
        self.robots.push(config);
    }

    /// Отправить команду роботу через чат
    pub fn chat_command(&self, robot_name: &str, command: &str) -> Result<String> {
        let robot = self
            .robots
            .iter()
            .find(|r| r.name == robot_name || robot_name == "all")
            .ok_or_else(|| anyhow::anyhow!("Robot '{}' not found", robot_name))?;

        let cmd = command.to_lowercase();
        let response = match cmd.as_str() {
            c if c.contains("вперёд") || c.contains("вперед") || c.contains("forward") =>
            {
                format!("✅ {} движется вперёд", robot.name)
            }
            c if c.contains("назад") || c.contains("back") => {
                format!("✅ {} движется назад", robot.name)
            }
            c if c.contains("стоп") || c.contains("стоп") || c.contains("stop") => {
                format!("✅ {} остановлен", robot.name)
            }
            c if c.contains("дом") || c.contains("база") || c.contains("home") => {
                format!("✅ {} возвращается на базу", robot.name)
            }
            c if c.contains("статус") || c.contains("status") => {
                format!("🤖 {}: онлайн, заряд 85%, позиция 54.32,48.22", robot.name)
            }
            c if c.contains("карта") || c.contains("map") => {
                format!("🗺 {}: карта помещения загружена", robot.name)
            }
            c if c.contains("сними") || c.contains("фото") || c.contains("photo") => {
                format!("📸 {}: фото сделано", robot.name)
            }
            _ => format!("❌ Команда '{}' не распознана для {}", command, robot.name),
        };
        info!("Robot: {} → {}", robot_name, command);
        Ok(response)
    }

    /// Чат роботов — robots могут общаться между собой
    pub fn robot_chat(&self, from: &str, to: &str, message: &str) -> Result<String> {
        if !self.chat_enabled {
            return Err(anyhow::anyhow!("Robot chat disabled"));
        }
        info!("🤖 Robot chat: {} → {}: {}", from, to, message);
        Ok(format!("📡 {} → {}: {}", from, to, message))
    }

    pub fn summary(&self) -> String {
        let mut out = format!("🤖 Роботы ({}):\n", self.robots.len());
        for r in &self.robots {
            let icon = match r.robot_type {
                RobotType::Vacuum => "🧹",
                RobotType::Lawnmower => "🌿",
                RobotType::Drone => "🚁",
                RobotType::Arm => "🦾",
                RobotType::Delivery => "📦",
                RobotType::Companion => "🤗",
                RobotType::Cleaner => "🧼",
            };
            out.push_str(&format!("  {} {} — {:?}\n", icon, r.name, r.robot_type));
        }
        out
    }
}
