/// Video recorder — DVR/NVR с детекцией движения
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    pub name: String,
    pub stream_url: String,
    pub output_dir: PathBuf,
    pub segment_secs: u32,
    pub max_segments: u32,
    pub motion_detect: bool,
    pub motion_sensitivity: u8, // 1-10
}

impl Default for RecordingConfig {
    fn default() -> Self {
        RecordingConfig {
            name: String::new(),
            stream_url: String::new(),
            output_dir: PathBuf::from("/var/waters/recordings"),
            segment_secs: 300, // 5 минут на сегмент
            max_segments: 288, // 24 часа при 5-минутных сегментах
            motion_detect: false,
            motion_sensitivity: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingClip {
    pub id: String,
    pub camera: String,
    pub start: String,
    pub end: Option<String>,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub motion: bool,
    pub tags: Vec<String>,
}

pub struct Recorder {
    recordings: Vec<RecordingConfig>,
    clips: Vec<RecordingClip>,
    output_base: PathBuf,
}

impl Recorder {
    pub fn new(output_dir: &Path) -> Self {
        Recorder {
            recordings: Vec::new(),
            clips: Vec::new(),
            output_base: output_dir.to_path_buf(),
        }
    }

    pub fn add_stream(&mut self, name: &str, stream_url: &str) {
        self.recordings.push(RecordingConfig {
            name: name.to_string(),
            stream_url: stream_url.to_string(),
            output_dir: self.output_base.join(name),
            ..Default::default()
        });
        info!("Recorder: added stream '{}' from {}", name, stream_url);
    }

    pub fn list_recordings(&self) -> &[RecordingConfig] {
        &self.recordings
    }

    pub fn list_clips(&self) -> &[RecordingClip] {
        &self.clips
    }

    /// Найти клипы по камере и тегу
    pub fn search_clips(&self, camera: &str, tag: &str) -> Vec<&RecordingClip> {
        self.clips
            .iter()
            .filter(|c| c.camera == camera && (tag.is_empty() || c.tags.contains(&tag.to_string())))
            .collect()
    }

    pub fn summary(&self) -> String {
        let total_size: u64 = self.clips.iter().map(|c| c.size_bytes).sum();
        let total_gb = total_size as f64 / 1_073_741_824.0;
        format!(
            "📼 Записи: {} потоков, {} клипов ({:.1} GB)",
            self.recordings.len(),
            self.clips.len(),
            total_gb
        )
    }
}
