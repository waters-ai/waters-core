pub mod camera;
pub mod recorder;
pub mod robot;
pub mod smarthome;

use anyhow::Result;
use tracing::info;

/// Video Engineer — общий контроллер видео-инженера
pub struct VideoEngineer {
    pub cameras: camera::CameraController,
    pub recorder: recorder::Recorder,
    pub smart_home: smarthome::SmartHome,
    pub robots: robot::RobotFleet,
}

impl VideoEngineer {
    pub fn new() -> Self {
        info!("VideoEngineer: инициализация видео-инженера");
        VideoEngineer {
            cameras: camera::CameraController::new(),
            recorder: recorder::Recorder::new(&std::path::PathBuf::from("/var/waters/recordings")),
            smart_home: smarthome::SmartHome::new(),
            robots: robot::RobotFleet::new(),
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}",
            self.cameras.summary(),
            self.recorder.summary(),
            self.smart_home.summary(),
            self.robots.summary()
        )
    }
}
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref ENGINEER: Mutex<Option<VideoEngineer>> = Mutex::new(None);
}

pub fn init_engineer() {
    let mut eng = ENGINEER.lock().unwrap();
    *eng = Some(VideoEngineer::new());
}

pub fn with_engineer<F, R>(f: F) -> R
where
    F: FnOnce(&mut VideoEngineer) -> R,
{
    let mut eng = ENGINEER.lock().unwrap();
    f(eng.as_mut().expect("VideoEngineer not initialized"))
}
