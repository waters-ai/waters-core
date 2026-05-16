use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::{info, warn};

use crate::bridge::{BridgeInfo, BridgePool, BridgeWeight};
use crate::store::KvStore;
use crate::subagent::SubAgentManager;

// ═══════════════════════════════════════════════════════
// MEDIA BRIDGE — профессиональное видео/аудио
// ═══════════════════════════════════════════════════════

/// Тип медиа-системы
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MediaSystem {
    Ndi,          // NDI (Network Device Interface)
    ObsWebSocket, // OBS Studio через WebSocket
    Rtmp,         // RTMP (YouTube, Twitch)
    WebRtc,       // WebRTC (браузеры)
    Srt,          // SRT (Secure Reliable Transport)
    Hdmi,         // HDMI прямой вывод (RPi, SDL2)
}

/// Медиа-команда для устройства
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaCommand {
    pub system: MediaSystem,
    pub action: String, // "switch_scene" | "start_stream" | "show_image" | "play_audio" | ...
    pub params: serde_json::Value,
    pub source_agent: String, // какой агент отправил
    pub timestamp: String,
}

/// Состояние медиа-устройства
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDeviceState {
    pub name: String,
    pub system: MediaSystem,
    pub connected: bool,
    pub active_scene: Option<String>,
    pub is_streaming: bool,
    pub is_recording: bool,
    pub input_sources: Vec<String>,
    pub output_resolution: String,
    pub fps: u8,
}

impl MediaDeviceState {
    pub fn summary(&self) -> String {
        format!(
            "{} [{:?}] {} | {} | {}x{}@{}fps | inputs:{}",
            self.name,
            self.system,
            if self.connected { "✅" } else { "❌" },
            self.active_scene.as_deref().unwrap_or("no scene"),
            1920,
            1080,
            self.fps,
            self.input_sources.len()
        )
    }
}

// ═══════════════════════════════════════════════════════
// VIDEO MIXER — управление видеомикшером
// ═══════════════════════════════════════════════════════

/// Пул медиа-устройств (видеомикшеры, стримеры, NDI)
pub struct MediaMixer {
    devices: Arc<Mutex<HashMap<String, MediaDeviceState>>>,
    obs_ws_url: String,
    obs_password: String,
    ndi_sources: Arc<Mutex<Vec<String>>>,
    kvstore: Arc<KvStore>,
}

impl MediaMixer {
    pub fn new(kvstore: Arc<KvStore>) -> Self {
        MediaMixer {
            devices: Arc::new(Mutex::new(HashMap::new())),
            obs_ws_url: "ws://localhost:4455".into(),
            obs_password: String::new(),
            ndi_sources: Arc::new(Mutex::new(Vec::new())),
            kvstore,
        }
    }

    /// Зарегистрировать медиа-устройство
    pub fn register_device(&self, name: &str, system: MediaSystem, fps: u8) {
        let device = MediaDeviceState {
            name: name.to_string(),
            system,
            connected: true,
            active_scene: None,
            is_streaming: false,
            is_recording: false,
            input_sources: vec![],
            output_resolution: "1920x1080".into(),
            fps,
        };
        let sys_debug = format!("{:?}", device.system);
        self.devices
            .lock()
            .unwrap()
            .insert(name.to_string(), device);
        info!("Media device registered: {} ({})", name, sys_debug);
    }

    /// Список устройств
    pub fn list_devices(&self) -> Vec<MediaDeviceState> {
        self.devices.lock().unwrap().values().cloned().collect()
    }

    /// Переключить сцену в OBS
    pub fn obs_switch_scene(&self, scene_name: &str) -> Result<()> {
        let mut devices = self.devices.lock().unwrap();
        for (_, dev) in devices.iter_mut() {
            if dev.system == MediaSystem::ObsWebSocket {
                dev.active_scene = Some(scene_name.to_string());
                info!("OBS: switched to scene '{}'", scene_name);
                // Здесь будет реальный WebSocket-вызов к OBS
            }
        }
        Ok(())
    }

    /// Отправить изображение в NDI
    pub fn ndi_send_image(&self, source_name: &str, image_b64: &str) -> Result<String> {
        let frame_id = uuid::Uuid::new_v4().to_string();
        info!(
            "NDI: sending image frame {} from '{}' ({} bytes)",
            &frame_id[..8],
            source_name,
            image_b64.len()
        );

        // Сохраняем в Redis: NDI может читать из Redis Stream
        let _ = self.kvstore.select_db(0).xadd(
            &format!("media:ndi:{}", source_name),
            &[("frame_id", &frame_id), ("data", image_b64)],
            100,
        );

        Ok(frame_id)
    }

    /// Запустить стрим на RTMP-платформу
    pub fn start_rtmp_stream(&self, platform: &str, url: &str, key: &str) -> Result<()> {
        info!("RTMP: starting stream to {} at {}", platform, url);
        let mut devices = self.devices.lock().unwrap();
        for (_, dev) in devices.iter_mut() {
            if dev.system == MediaSystem::Rtmp {
                dev.is_streaming = true;
            }
        }
        Ok(())
    }

    /// Остановить стрим
    pub fn stop_stream(&self, platform: &str) -> Result<()> {
        info!("RTMP: stopping stream to {}", platform);
        let mut devices = self.devices.lock().unwrap();
        for (_, dev) in devices.iter_mut() {
            if dev.system == MediaSystem::Rtmp {
                dev.is_streaming = false;
            }
        }
        Ok(())
    }

    /// Получить сводку для LLM
    pub fn summary_for_llm(&self) -> String {
        let devices = self.devices.lock().unwrap();
        if devices.is_empty() {
            return "Нет подключённых медиа-устройств.".to_string();
        }
        let mut out = "🎬 Медиа-устройства:\n".to_string();
        for d in devices.values() {
            out.push_str(&format!("  {}\n", d.summary()));
        }
        out
    }

    /// Обработать Finding от агента — отправить на медиа-устройства
    pub fn process_finding(
        &self,
        finding_type: &str,
        data: &serde_json::Value,
        source_agent: &str,
    ) -> Result<()> {
        match finding_type {
            "image" | "photo" => {
                // Автоматически показываем на NDI
                if let Some(base64) = data["base64"].as_str() {
                    self.ndi_send_image(source_agent, base64)?;
                }
                // Показываем на HDMI если есть
                let _ = self.kvstore.select_db(0).xadd(
                    "media:display:latest",
                    &[
                        ("type", "image"),
                        ("source", source_agent),
                        ("data", &data.to_string()),
                    ],
                    10,
                );
            }
            "audio" | "music" => {
                let _ = self.kvstore.select_db(0).xadd(
                    "media:audio:playlist",
                    &[
                        ("type", "audio"),
                        ("source", source_agent),
                        ("data", &data.to_string()),
                    ],
                    50,
                );
            }
            "video" | "stream" => {
                if let Some(url) = data["url"].as_str() {
                    info!("Media: playing video {} from agent {}", url, source_agent);
                }
            }
            "scene" => {
                if let Some(scene) = data["scene"].as_str() {
                    self.obs_switch_scene(scene)?;
                }
            }
            "command" => {
                let action = data["action"].as_str().unwrap_or("");
                match action {
                    "start_stream" => {
                        let platform = data["platform"].as_str().unwrap_or("youtube");
                        let url = data["url"].as_str().unwrap_or("");
                        let key = data["key"].as_str().unwrap_or("");
                        self.start_rtmp_stream(platform, url, key)?;
                    }
                    "stop_stream" => {
                        self.stop_stream(data["platform"].as_str().unwrap_or("youtube"))?;
                    }
                    _ => warn!("Unknown media command: {}", action),
                }
            }
            _ => {}
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════
// STUDIO FEEDBACK — захват видео/кадров для LLM
// ═══════════════════════════════════════════════════════

/// Захват кадра из продакшн-пайплайна для просмотра LLM
pub struct StudioFeedback {
    kvstore: Arc<KvStore>,
}

impl StudioFeedback {
    pub fn new(kvstore: Arc<KvStore>) -> Self {
        StudioFeedback { kvstore }
    }

    /// Сохранить захваченный кадр из студии/микшера
    pub fn capture_frame(&self, source: &str, image_b64: &str) -> Result<String> {
        let frame_id = uuid::Uuid::new_v4().to_string();
        let _ = self.kvstore.select_db(0).xadd(
            &format!("studio:capture:{}", source),
            &[("frame_id", &frame_id), ("image_b64", image_b64)],
            50,
        );
        info!("Studio capture from {}: frame {}", source, &frame_id[..8]);
        Ok(frame_id)
    }

    /// Последний захваченный кадр для LLM
    pub fn latest_frames(&self, source: &str, count: usize) -> Vec<String> {
        let key = format!("studio:capture:{}", source);
        self.kvstore
            .select_db(0)
            .list_range(&key, 0, count as isize)
            .unwrap_or_default()
    }
}

// ═══════════════════════════════════════════════════════
// ИНИЦИАЛИЗАЦИЯ МЕДИА-БРИДЖЕЙ
// ═══════════════════════════════════════════════════════

/// Настроить медиа-устройства по bridgs.json
pub fn setup_media_bridges(
    config: &serde_json::Value,
    pool: &mut BridgePool,
    kvstore: Arc<KvStore>,
) -> MediaMixer {
    let mixer = MediaMixer::new(kvstore.clone());

    // Регистрируем медиа-устройства из конфига
    if let Some(media) = config.get("media") {
        // NDI источники
        if let Some(ndi) = media.get("ndi") {
            if let Some(sources) = ndi.as_array() {
                for src in sources {
                    let name = src["name"].as_str().unwrap_or("ndi-source");
                    let fps = src["fps"].as_u64().unwrap_or(30) as u8;
                    mixer.register_device(name, MediaSystem::Ndi, fps);
                    pool.register(
                        &format!("media-ndi-{}", name),
                        Box::new(MediaBridgeProvider {
                            name: format!("ndi-{}", name),
                            system: MediaSystem::Ndi,
                            mixer: mixer.devices.clone(),
                        }),
                        BridgeInfo::new(&format!("ndi-{}", name), BridgeWeight::Heavy, 5, 50000),
                    );
                }
            }
        }

        // OBS
        if let Some(obs) = media.get("obs") {
            if let Some(url) = obs["url"].as_str() {
                if let Some(password) = obs["password"].as_str() {
                    let name = obs["name"].as_str().unwrap_or("obs-main");
                    mixer.register_device(name, MediaSystem::ObsWebSocket, 30);
                    pool.register(
                        "media-obs",
                        Box::new(MediaBridgeProvider {
                            name: "obs".into(),
                            system: MediaSystem::ObsWebSocket,
                            mixer: mixer.devices.clone(),
                        }),
                        BridgeInfo::new("media-obs", BridgeWeight::Heavy, 4, 100000),
                    );
                    info!(
                        "OBS bridge configured: {} (pass: {})",
                        url,
                        if password.is_empty() { "none" } else { "set" }
                    );
                }
            }
        }

        // RTMP
        if let Some(rtmp) = media.get("rtmp") {
            if let Some(platforms) = rtmp.as_array() {
                for plat in platforms {
                    let name = plat["name"].as_str().unwrap_or("rtmp");
                    let url = plat["url"].as_str().unwrap_or("");
                    mixer.register_device(name, MediaSystem::Rtmp, 30);
                    pool.register(
                        &format!("media-rtmp-{}", name),
                        Box::new(MediaBridgeProvider {
                            name: format!("rtmp-{}", name),
                            system: MediaSystem::Rtmp,
                            mixer: mixer.devices.clone(),
                        }),
                        BridgeInfo::new(&format!("rtmp-{}", name), BridgeWeight::Heavy, 4, 80000),
                    );
                    info!("RTMP bridge: {} → {}", name, url);
                }
            }
        }
    }

    info!("Media bridge ready: {} devices", mixer.list_devices().len());
    mixer
}

// ═══════════════════════════════════════════════════════
// MEDIA BRIDGE PROVIDER (реализует BridgeProvider)
// ═══════════════════════════════════════════════════════

use crate::bridge::BridgeProvider;
use std::fmt;

#[derive(Debug)]
pub struct MediaBridgeProvider {
    name: String,
    system: MediaSystem,
    mixer: Arc<Mutex<HashMap<String, MediaDeviceState>>>,
}

impl BridgeProvider for MediaBridgeProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn call(&self, input: &str) -> Result<String> {
        let cmd: MediaCommand = serde_json::from_str(input)?;

        let result = match cmd.system {
            MediaSystem::ObsWebSocket => match cmd.action.as_str() {
                "switch_scene" => {
                    let scene = cmd.params["scene"].as_str().unwrap_or("default");
                    let mut devices = self.mixer.lock().unwrap();
                    for (_, dev) in devices.iter_mut() {
                        if dev.system == MediaSystem::ObsWebSocket {
                            dev.active_scene = Some(scene.to_string());
                        }
                    }
                    format!("OBS: switched to scene '{}'", scene)
                }
                "start_stream" => "OBS: streaming started".into(),
                "stop_stream" => "OBS: streaming stopped".into(),
                "start_recording" => "OBS: recording started".into(),
                "stop_recording" => "OBS: recording stopped".into(),
                "take_screenshot" => {
                    // Возвращаем последний кадр из Redis
                    "screenshot requested".into()
                }
                _ => format!("Unknown OBS action: {}", cmd.action),
            },
            MediaSystem::Ndi => match cmd.action.as_str() {
                "send_frame" => "NDI: frame sent".into(),
                "list_sources" => {
                    let devices = self.mixer.lock().unwrap();
                    let names: Vec<String> = devices.keys().cloned().collect();
                    names.join(", ")
                }
                _ => format!("Unknown NDI action: {}", cmd.action),
            },
            MediaSystem::Rtmp => match cmd.action.as_str() {
                "start" => format!("RTMP: streaming to {}", cmd.params["url"]),
                "stop" => "RTMP: stopped".into(),
                _ => format!("Unknown RTMP action: {}", cmd.action),
            },
            _ => format!("Unsupported media system: {:?}", cmd.system),
        };

        Ok(result)
    }

    fn call_json(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
        let result = self.call(&serde_json::to_string(input)?)?;
        Ok(serde_json::json!({"response": result}))
    }
}
