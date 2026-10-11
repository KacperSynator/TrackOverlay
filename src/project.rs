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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SyncMode {
    #[default]
    Manual,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncState {
    pub offset_ms: i64,
    pub mode: SyncMode,
    pub max_auto_sync_offset_ms: i64,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            offset_ms: 0,
            mode: SyncMode::Manual,
            max_auto_sync_offset_ms: 300000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    pub video_path: PathBuf,
    pub telemetry_path: PathBuf,
    pub sync: SyncState,
    pub elements: Vec<OverlayElement>,
    pub flip_vertical: bool,
    pub flip_horizontal: bool,
    #[serde(default)]
    pub export_start_ms: Option<i64>,
    #[serde(default)]
    pub export_end_ms: Option<i64>,
    #[serde(default)]
    pub speed_source: SpeedSource,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            video_path: PathBuf::new(),
            telemetry_path: PathBuf::new(),
            sync: SyncState::default(),
            elements: vec![],
            flip_vertical: false,
            flip_horizontal: false,
            export_start_ms: None,
            export_end_ms: None,
            speed_source: SpeedSource::Auto,
        }
    }
}

impl ProjectConfig {
    pub fn save<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), crate::error::ConfigError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load<P: AsRef<std::path::Path>>(path: P) -> Result<Self, crate::error::ConfigError> {
        let json = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&json)?;
        Ok(config)
    }
}
