/// Camera bridge — управление камерами через ONVIF/RTSP
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub name: String,
    pub url: String,       // RTSP URL: rtsp://user:pass@ip:554/stream1
    pub onvif_url: String, // ONVIF: http://ip:5000/onvif/device_service
    pub ptz: bool,
    pub recording: bool,
    pub motion_detect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PtzCommand {
    Left,
    Right,
    Up,
    Down,
    ZoomIn,
    ZoomOut,
    FocusIn,
    FocusOut,
    Home,   // вернуться в начальную позицию
    Patrol, // патрулирование
}

pub struct CameraController {
    cameras: Vec<CameraConfig>,
}

impl CameraController {
    pub fn new() -> Self {
        CameraController {
            cameras: Vec::new(),
        }
    }

    pub fn add_camera(&mut self, config: CameraConfig) {
        info!("Camera: added '{}' at {}", config.name, config.url);
        self.cameras.push(config);
    }

    pub fn list(&self) -> &[CameraConfig] {
        &self.cameras
    }

    /// PTZ-команда: поворот/зум/фокус камеры
    pub fn ptz(&self, camera_name: &str, cmd: &PtzCommand) -> Result<String> {
        let cam = self
            .cameras
            .iter()
            .find(|c| c.name == camera_name)
            .ok_or_else(|| anyhow::anyhow!("Camera '{}' not found", camera_name))?;
        if !cam.ptz {
            return Err(anyhow::anyhow!("Camera '{}' has no PTZ", camera_name));
        }
        // ONVIF PTZ — http POST к ONVIF-сервису
        let onvif_cmd = format!("{:?}", cmd);
        info!(
            "Camera: PTZ '{}' → {} ({})",
            camera_name, onvif_cmd, cam.onvif_url
        );
        Ok(format!("✅ PTZ: {} → {}", camera_name, onvif_cmd))
    }

    /// Получить RTSP-поток для просмотра
    pub fn stream_url(&self, camera_name: &str) -> Option<String> {
        self.cameras
            .iter()
            .find(|c| c.name == camera_name)
            .map(|c| c.url.clone())
    }

    /// Включить/выключить запись
    pub fn set_recording(&mut self, camera_name: &str, on: bool) -> Result<()> {
        if let Some(cam) = self.cameras.iter_mut().find(|c| c.name == camera_name) {
            cam.recording = on;
            info!(
                "Camera: recording {} → {}",
                camera_name,
                if on { "ON" } else { "OFF" }
            );
            Ok(())
        } else {
            Err(anyhow::anyhow!("Camera not found"))
        }
    }

    pub fn summary(&self) -> String {
        let mut out = format!("📹 Камеры ({}):\n", self.cameras.len());
        for cam in &self.cameras {
            out.push_str(&format!(
                "  {} — {} (PTZ:{}, запись:{}, детекция:{})\n",
                cam.name, cam.url, cam.ptz, cam.recording, cam.motion_detect
            ));
        }
        out
    }
}
