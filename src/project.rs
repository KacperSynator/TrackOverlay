use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OverlayKind {
    SpeedReadout,
    GForceMeter,
    LapTimer,
    TrackMap,
    ThrottleBar,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverlayElement {
    pub kind: OverlayKind,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncMode {
    Manual,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncState {
    pub offset_ms: i64,
    pub mode: SyncMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    pub video_path: PathBuf,
    pub telemetry_path: PathBuf,
    pub sync: SyncState,
    pub elements: Vec<OverlayElement>,
    pub flip_vertical: bool,
    pub flip_horizontal: bool,
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
