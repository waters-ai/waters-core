/// Smart Home bridge — управление умным домом через MQTT/Zigbee
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    Light,      // свет
    Switch,     // выключатель
    Socket,     // розетка
    Thermostat, // термостат
    Sensor,     // датчик (температура, влажность, движение)
    Lock,       // замок
    Camera,     // камера
    Curtain,    // шторы
    Gate,       // ворота
    Irrigation, // полив
    Robot,      // робот (пылесос, газонокосилка)
    Climate,    // климат (кондиционер, обогреватель)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDevice {
    pub name: String,
    pub device_type: DeviceType,
    pub room: String,
    pub mqtt_topic: String,
    pub state: String,
    pub value: f64,
}

pub struct SmartHome {
    devices: Vec<SmartDevice>,
    voice_enabled: bool,
}

impl SmartHome {
    pub fn new() -> Self {
        SmartHome {
            devices: Vec::new(),
            voice_enabled: true,
        }
    }

    pub fn add_device(&mut self, name: &str, device_type: DeviceType, room: &str, topic: &str) {
        self.devices.push(SmartDevice {
            name: name.to_string(),
            device_type,
            room: room.to_string(),
            mqtt_topic: topic.to_string(),
            state: "off".into(),
            value: 0.0,
        });
        info!(
            "SmartHome: added '{}' ({:?}) in {}",
            name, device_type, room
        );
    }

    /// Голосовая команда: "включи свет в кухне", "температура 22 градуса"
    pub fn voice_command(&mut self, command: &str) -> Result<String> {
        let cmd = command.to_lowercase();
        let mut found = false;
        let mut response = String::new();

        for device in &mut self.devices {
            if cmd.contains(&device.name.to_lowercase())
                || cmd.contains(&device.room.to_lowercase())
            {
                if cmd.contains("включ") || cmd.contains("on") {
                    device.state = "on".into();
                    response.push_str(&format!("✅ {} включен\n", device.name));
                } else if cmd.contains("выключ") || cmd.contains("off") {
                    device.state = "off".into();
                    response.push_str(&format!("✅ {} выключен\n", device.name));
                } else if cmd.contains("температур") {
                    device.state = "on".into();
                    response.push_str(&format!("✅ {} установлен\n", device.name));
                }
                found = true;
            }
        }

        if !found {
            // Поиск по типу устройства
            for device in &mut self.devices {
                match device.device_type {
                    DeviceType::Light if cmd.contains("свет") || cmd.contains("ламп") => {
                        device.state = if cmd.contains("включ") || cmd.contains("on") {
                            "on".into()
                        } else {
                            "off".into()
                        };
                        response.push_str(&format!("✅ {} {}\n", device.name, device.state));
                    }
                    DeviceType::Thermostat
                        if cmd.contains("температур") || cmd.contains("градус") =>
                    {
                        device.state = "on".into();
                        response.push_str(&format!("✅ {} установлен\n", device.name));
                    }
                    DeviceType::Gate if cmd.contains("ворота") || cmd.contains("гараж") =>
                    {
                        device.state = if cmd.contains("открой") || cmd.contains("open") {
                            "open".into()
                        } else {
                            "closed".into()
                        };
                        response.push_str(&format!("✅ {} {}\n", device.name, device.state));
                    }
                    DeviceType::Irrigation if cmd.contains("полив") => {
                        device.state = if cmd.contains("включ") || cmd.contains("on") {
                            "on".into()
                        } else {
                            "off".into()
                        };
                        response.push_str(&format!("✅ {} {}\n", device.name, device.state));
                    }
                    _ => {}
                }
            }
        }

        if response.is_empty() {
            Ok("❌ Команда не распознана".into())
        } else {
            Ok(response.trim().to_string())
        }
    }

    pub fn list_by_room(&self, room: &str) -> Vec<&SmartDevice> {
        self.devices.iter().filter(|d| d.room == room).collect()
    }

    pub fn summary(&self) -> String {
        let mut rooms: Vec<String> = self.devices.iter().map(|d| d.room.clone()).collect();
        rooms.sort();
        rooms.dedup();
        let mut out = format!(
            "🏠 Умный дом: {} устройств, {} комнат\n",
            self.devices.len(),
            rooms.len()
        );
        for room in rooms {
            let devices: Vec<&SmartDevice> =
                self.devices.iter().filter(|d| d.room == room).collect();
            out.push_str(&format!("  🚪 {} ({}):\n", room, devices.len()));
            for d in &devices {
                let icon = match d.device_type {
                    DeviceType::Light => "💡",
                    DeviceType::Switch => "🔘",
                    DeviceType::Socket => "🔌",
                    DeviceType::Thermostat => "🌡",
                    DeviceType::Sensor => "📡",
                    DeviceType::Lock => "🔒",
                    DeviceType::Camera => "📹",
                    DeviceType::Curtain => "🪟",
                    DeviceType::Gate => "🚪",
                    DeviceType::Irrigation => "💧",
                    DeviceType::Robot => "🤖",
                    DeviceType::Climate => "❄",
                };
                out.push_str(&format!("    {} {} — {}\n", icon, d.name, d.state));
            }
        }
        out
    }
}
