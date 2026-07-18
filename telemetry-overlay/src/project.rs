use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OverlayKind {
    SpeedReadout,
    GForceMeter,
    LapTimer,
    // Add more types as needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayElement {
    pub kind: OverlayKind,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncState {
    pub offset_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub video_path: PathBuf,
    pub telemetry_path: PathBuf,
    pub sync: SyncState,
    pub elements: Vec<OverlayElement>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            video_path: PathBuf::new(),
            telemetry_path: PathBuf::new(),
            sync: SyncState::default(),
            elements: vec![
                OverlayElement {
                    kind: OverlayKind::SpeedReadout,
                    x: 0.1,
                    y: 0.8,
                    scale: 1.0,
                },
                OverlayElement {
                    kind: OverlayKind::GForceMeter,
                    x: 0.5,
                    y: 0.8,
                    scale: 1.0,
                },
                OverlayElement {
                    kind: OverlayKind::LapTimer,
                    x: 0.8,
                    y: 0.1,
                    scale: 1.0,
                },
            ],
        }
    }
}

impl ProjectConfig {
    pub fn save<P: AsRef<std::path::Path>>(&self, path: P) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&json)?;
        Ok(config)
    }
}
