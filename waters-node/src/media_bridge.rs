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

// ═══════════════════════════════════════════════════════════════
// REMOTE CAMERA MANAGEMENT — RTSP/ONVIF камеры, PTZ, стримы
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCamera {
    pub name: String,
    pub rtsp_url: String,  // rtsp://user:pass@ip:554/stream1
    pub onvif_url: String, // http://ip:5000/onvif/device_service
    pub ptz_supported: bool,
    pub location: String, // "поле 42", "ферма", "завод цех 3"
    pub status: String,   // online | offline | recording
    pub stream_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PtzDirection {
    Left,
    Right,
    Up,
    Down,
    ZoomIn,
    ZoomOut,
    Home,
    Patrol,
}

/// Управление удалёнными камерами
pub struct RemoteCameraManager {
    cameras: Arc<Mutex<Vec<RemoteCamera>>>,
    kvstore: Arc<KvStore>,
}

impl RemoteCameraManager {
    pub fn new(kvstore: Arc<KvStore>) -> Self {
        RemoteCameraManager {
            cameras: Arc::new(Mutex::new(Vec::new())),
            kvstore,
        }
    }

    pub fn add_camera(&self, name: &str, rtsp: &str, onvif: &str, location: &str) {
        let mut cams = self.cameras.lock().map_err(|e| warn!("Mutex: {}", e)).ok();
        if let Some(ref mut cams) = cams {
            cams.push(RemoteCamera {
                name: name.to_string(),
                rtsp_url: rtsp.to_string(),
                onvif_url: onvif.to_string(),
                ptz_supported: !onvif.is_empty(),
                location: location.to_string(),
                status: "online".into(),
                stream_active: false,
            });
            info!("CameraManager: added '{}' at {} ({})", name, rtsp, location);
        }
    }

    pub fn list_cameras(&self) -> Vec<RemoteCamera> {
        self.cameras.lock().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn ptz(&self, camera: &str, dir: &PtzDirection) -> Result<String> {
        let cams = self
            .cameras
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex: {}", e))?;
        let cam = cams
            .iter()
            .find(|c| c.name == camera)
            .ok_or_else(|| anyhow::anyhow!("Camera '{}' not found", camera))?;
        if !cam.ptz_supported {
            return Err(anyhow::anyhow!("Camera '{}' has no PTZ", camera));
        }
        // ONVIF PTZ — http-запрос к камере
        info!("PTZ: {} → {:?}", camera, dir);
        Ok(format!("✅ PTZ {} → {:?}", camera, dir))
    }

    /// Получить RTSP-поток для просмотра/транскодирования
    pub fn get_stream_url(&self, camera: &str) -> Option<String> {
        self.cameras
            .lock()
            .ok()?
            .iter()
            .find(|c| c.name == camera)
            .map(|c| c.rtsp_url.clone())
    }

    /// Включить стрим с удалённой камеры на ноду
    pub fn start_stream(&self, camera: &str) -> Result<String> {
        let mut cams = self
            .cameras
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex: {}", e))?;
        if let Some(cam) = cams.iter_mut().find(|c| c.name == camera) {
            cam.stream_active = true;
            let _ = self.kvstore.select_db(0).xadd(
                "media:streams:active",
                &[
                    ("camera", camera),
                    ("rtsp", &cam.rtsp_url),
                    ("status", "active"),
                ],
                100,
            );
            info!("Stream: started from camera '{}'", camera);
            Ok(format!("✅ Стрим с '{}' запущен", camera))
        } else {
            Err(anyhow::anyhow!("Camera '{}' not found", camera))
        }
    }

    pub fn stop_stream(&self, camera: &str) -> Result<String> {
        let mut cams = self
            .cameras
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex: {}", e))?;
        if let Some(cam) = cams.iter_mut().find(|c| c.name == camera) {
            cam.stream_active = false;
            Ok(format!("⏹ Стрим с '{}' остановлен", camera))
        } else {
            Err(anyhow::anyhow!("Camera '{}' not found", camera))
        }
    }

    pub fn summary(&self) -> String {
        let cams = match self.cameras.lock() {
            Ok(c) => c.clone(),
            Err(_) => vec![],
        };
        let mut out = format!("📹 Удалённые камеры ({}):\n", cams.len());
        for c in cams.iter() {
            let icon = if c.stream_active { "🔴" } else { "📷" };
            out.push_str(&format!(
                "  {} {} [{}] {} — {} | PTZ:{}\n",
                icon, c.name, c.status, c.location, c.rtsp_url, c.ptz_supported
            ));
        }
        out
    }
}

// ═══════════════════════════════════════════════════════════════
// DIRECTOR CONSOLE — режиcсёрский пульт для видео-продакшна
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorScene {
    pub name: String,
    pub sources: Vec<String>,
    pub active_source: String,
    pub transition: String, // cut | fade | wipe
    pub duration_secs: u32,
}

pub struct DirectorConsole {
    scenes: Arc<Mutex<Vec<DirectorScene>>>,
    kvstore: Arc<KvStore>,
}

impl DirectorConsole {
    pub fn new(kvstore: Arc<KvStore>) -> Self {
        let mut scenes = Vec::new();
        scenes.push(DirectorScene {
            name: "Основной".into(),
            sources: vec![],
            active_source: String::new(),
            transition: "cut".into(),
            duration_secs: 0,
        });
        DirectorConsole {
            scenes: Arc::new(Mutex::new(scenes)),
            kvstore,
        }
    }

    /// Переключить источник в сцене (режиcсёрский пульт)
    pub fn switch_source(&self, scene: &str, source: &str) -> Result<String> {
        let mut sc = self
            .scenes
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex: {}", e))?;
        if let Some(s) = sc.iter_mut().find(|s| s.name == scene) {
            s.active_source = source.to_string();
            let _ = self.kvstore.select_db(0).xadd(
                "media:director:switches",
                &[
                    ("scene", scene),
                    ("source", source),
                    ("ts", &chrono::Utc::now().to_rfc3339()),
                ],
                1000,
            );
            info!(
                "Director: switched scene '{}' to source '{}'",
                scene, source
            );
            Ok(format!("🎬 Сцена '{}' → {}", scene, source))
        } else {
            Err(anyhow::anyhow!("Scene '{}' not found", scene))
        }
    }

    /// Добавить источник в сцену (камера, NDI, видеофайл)
    pub fn add_source(&self, scene: &str, source: &str) -> Result<String> {
        let mut sc = self
            .scenes
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex: {}", e))?;
        if let Some(s) = sc.iter_mut().find(|s| s.name == scene) {
            if !s.sources.contains(&source.to_string()) {
                s.sources.push(source.to_string());
                info!("Director: added source '{}' to scene '{}'", source, scene);
            }
            Ok(format!(
                "✅ Источник '{}' добавлен в сцену '{}'",
                source, scene
            ))
        } else {
            // Создаём новую сцену
            sc.push(DirectorScene {
                name: scene.to_string(),
                sources: vec![source.to_string()],
                active_source: source.to_string(),
                transition: "cut".into(),
                duration_secs: 0,
            });
            Ok(format!(
                "✅ Сцена '{}' создана с источником '{}'",
                scene, source
            ))
        }
    }

    /// Создать репортаж из источника на удалённом объекте
    pub fn create_report(
        &self,
        location: &str,
        source: &str,
        duration_secs: u32,
    ) -> Result<String> {
        let report_id = uuid::Uuid::new_v4().to_string();
        let _ = self.kvstore.select_db(0).xadd(
            "media:reports",
            &[
                ("id", &report_id),
                ("location", location),
                ("source", source),
                ("duration", &duration_secs.to_string()),
                ("ts", &chrono::Utc::now().to_rfc3339()),
            ],
            1000,
        );
        info!(
            "Report: created from {} ({}, {}s) — id:{}",
            location,
            source,
            duration_secs,
            &report_id[..8]
        );
        Ok(format!(
            "📡 Репортаж из '{}' создан (id:{})",
            location,
            &report_id[..8]
        ))
    }

    pub fn summary(&self) -> String {
        let sc = self.scenes.lock().map(|s| s.clone()).unwrap_or_default();
        let mut out = format!("🎬 Режиcсёрский пульт ({} сцен):\n", sc.len());
        for s in &sc {
            out.push_str(&format!(
                "  🎥 {}: [{}] активный: {}\n",
                s.name,
                s.sources.join(", "),
                s.active_source
            ));
        }
        out
    }
}

// ═══════════════════════════════════════════════════════════════
// VIDEO OUTPUT — вывод видео на мониторы, телевизоры, приборы
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisplayDevice {
    Hdmi,      // прямой HDMI-выход (Raspberry Pi, SDL2)
    NdiOutput, // NDI-приёмник (видео по сети)
    WebRtc,    // браузерный просмотр
    SmartTv,   // Smart TV (DLNA, Chromecast, AirPlay)
    Projector, // проектор (HDMI/SDI)
    Monitor,   // монитор (DisplayPort/HDMI)
    LedWall,   // светодиодный экран (LED video wall)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoOutput {
    pub name: String,
    pub device: DisplayDevice,
    pub current_source: Option<String>,
    pub resolution: String,
    pub is_active: bool,
    pub location: String, // "зал заседаний", "цех 3", "дом гостиная"
}

pub struct DisplayManager {
    outputs: Arc<Mutex<Vec<VideoOutput>>>,
    kvstore: Arc<KvStore>,
}

impl DisplayManager {
    pub fn new(kvstore: Arc<KvStore>) -> Self {
        let mut outputs = Vec::new();
        outputs.push(VideoOutput {
            name: "Главный монитор".into(),
            device: DisplayDevice::Hdmi,
            current_source: None,
            resolution: "1920x1080".into(),
            is_active: false,
            location: "Студия".into(),
        });
        outputs.push(VideoOutput {
            name: "Телевизор зал".into(),
            device: DisplayDevice::SmartTv,
            current_source: None,
            resolution: "3840x2160".into(),
            is_active: false,
            location: "Дом".into(),
        });
        outputs.push(VideoOutput {
            name: "NDI-приёмник".into(),
            device: DisplayDevice::NdiOutput,
            current_source: None,
            resolution: "1920x1080".into(),
            is_active: false,
            location: "Сеть".into(),
        });
        DisplayManager {
            outputs: Arc::new(Mutex::new(outputs)),
            kvstore,
        }
    }

    /// Назначить источник на устройство вывода
    pub fn route(&self, source: &str, output: &str) -> Result<String> {
        let mut outs = self
            .outputs
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex: {}", e))?;
        if let Some(o) = outs.iter_mut().find(|o| o.name == output) {
            o.current_source = Some(source.to_string());
            o.is_active = true;
            let _ = self.kvstore.select_db(0).xadd(
                "media:display:routes",
                &[
                    ("source", source),
                    ("output", output),
                    ("ts", &chrono::Utc::now().to_rfc3339()),
                ],
                100,
            );
            info!("Display: routed '{}' → {}", source, output);
            Ok(format!("📺 {} → {} (активно)", source, output))
        } else if output == "*" {
            // На все устройства
            for o in outs.iter_mut() {
                o.current_source = Some(source.to_string());
                o.is_active = true;
            }
            info!("Display: routed '{}' → ALL outputs", source);
            Ok(format!("📺 {} → ВСЕ выходы", source))
        } else {
            Err(anyhow::anyhow!(
                "Output '{}' not found. Доступны: {}",
                output,
                outs.iter()
                    .map(|o| o.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }

    /// Отключить устройство вывода
    pub fn disconnect(&self, output: &str) -> Result<String> {
        let mut outs = self
            .outputs
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex: {}", e))?;
        if let Some(o) = outs.iter_mut().find(|o| o.name == output) {
            o.is_active = false;
            o.current_source = None;
            Ok(format!("⏹ {} отключён", output))
        } else {
            Err(anyhow::anyhow!("Output '{}' not found", output))
        }
    }

    pub fn list(&self) -> Vec<VideoOutput> {
        self.outputs.lock().map(|o| o.clone()).unwrap_or_default()
    }

    pub fn summary(&self) -> String {
        let outs = self.list();
        let mut out = format!("📺 Видеовыходы ({}):\n", outs.len());
        for o in &outs {
            let icon = match o.device {
                DisplayDevice::Hdmi => "🖥",
                DisplayDevice::NdiOutput => "🌐",
                DisplayDevice::WebRtc => "🌍",
                DisplayDevice::SmartTv => "📺",
                DisplayDevice::Projector => "🔦",
                DisplayDevice::Monitor => "🖥",
                DisplayDevice::LedWall => "✨",
            };
            let src = o.current_source.as_deref().unwrap_or("—");
            out.push_str(&format!(
                "  {} {} — {} [{}] {}\n",
                icon,
                o.name,
                if o.is_active { "🔴" } else { "⏹" },
                o.resolution,
                src
            ));
        }
        out
    }
}
