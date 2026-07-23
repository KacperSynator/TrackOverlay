use std::path::PathBuf;
use track_overlay::project::{OverlayElement, OverlayKind, ProjectConfig, SyncMode, SyncState};
use tempfile::NamedTempFile;

#[test]
fn test_default_config_serialization() {
    let config = ProjectConfig::default();

    // Test serialization
    let json = serde_json::to_string(&config).expect("Failed to serialize default config");

    // Test deserialization
    let loaded_config: ProjectConfig = serde_json::from_str(&json).expect("Failed to deserialize default config");

    // Verify equality
    assert_eq!(config, loaded_config);
}

#[test]
fn test_custom_config_serialization() {
    let config = ProjectConfig {
        video_path: PathBuf::from("/path/to/video.mp4"),
        telemetry_path: PathBuf::from("/path/to/telemetry.csv"),
        sync: SyncState {
            offset_ms: 1500,
            mode: SyncMode::Auto,
        },
        flip_vertical: true,
        flip_horizontal: false,
        elements: vec![
            OverlayElement {
                kind: OverlayKind::SpeedReadout,
                x: 0.25,
                y: 0.75,
                scale: 1.5,
            },
            OverlayElement {
                kind: OverlayKind::LapTimer,
                x: 0.9,
                y: 0.9,
                scale: 0.8,
            },
        ],
    };

    // Test serialization
    let json = serde_json::to_string(&config).expect("Failed to serialize custom config");

    // Test deserialization
    let loaded_config: ProjectConfig = serde_json::from_str(&json).expect("Failed to deserialize custom config");

    // Verify equality
    assert_eq!(config, loaded_config);
}

#[test]
fn test_config_save_and_load() {
    let config = ProjectConfig {
        video_path: PathBuf::from("/another/path/video.mp4"),
        telemetry_path: PathBuf::from("/another/path/telemetry.csv"),
        sync: SyncState {
            offset_ms: -500,
            mode: SyncMode::Manual,
        },
        flip_vertical: false,
        flip_horizontal: true,
        elements: vec![
            OverlayElement {
                kind: OverlayKind::TrackMap,
                x: 0.1,
                y: 0.1,
                scale: 2.0,
            },
        ],
    };

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");

    // Save to temp file
    config.save(temp_file.path()).expect("Failed to save config");

    // Load from temp file
    let loaded_config = ProjectConfig::load(temp_file.path()).expect("Failed to load config");

    // Verify equality
    assert_eq!(config, loaded_config);
}
