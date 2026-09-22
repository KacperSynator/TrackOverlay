use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OverlayKind {
    SpeedReadout,
    GForceMeter,
    LapTimer,
    AdvancedLapTimer,
    TrackMap,
    ThrottleBar,
    RpmOverlay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverlayElement {
    pub enabled: bool,
    pub kind: OverlayKind,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SpeedSource {
    #[default]
    Auto,
    Gps,
    Obd,
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
    pub max_auto_sync_offset_ms: i64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    pub video_path: PathBuf,
    pub telemetry_path: PathBuf,
    pub sync: SyncState,
    pub elements: Vec<OverlayElement>,
    pub flip_vertical: bool,
    pub flip_horizontal: bool,
    #[serde(default = "default_true")]
    pub use_hardware_acceleration: bool,
    #[serde(default)]
    pub export_start_ms: Option<i64>,
    #[serde(default)]
    pub export_end_ms: Option<i64>,
    #[serde(default)]
    pub speed_source: SpeedSource,
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
