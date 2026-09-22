use std::path::PathBuf;
use tempfile::NamedTempFile;
use track_overlay::project::{OverlayElement, OverlayKind, ProjectConfig, SyncMode, SyncState};

#[test]
fn test_custom_config_serialization() {
    let config = ProjectConfig {
        video_path: PathBuf::from("/path/to/video.mp4"),
        telemetry_path: PathBuf::from("/path/to/telemetry.csv"),
        sync: SyncState {
            offset_ms: 1500,
            mode: SyncMode::Auto,
            max_auto_sync_offset_ms: 300000,
        },
        flip_vertical: true,
        flip_horizontal: false,
        use_hardware_acceleration: true,
        elements: vec![
            OverlayElement {
                enabled: true,
                kind: OverlayKind::SpeedReadout,
                x: 0.25,
                y: 0.75,
                scale: 1.5,
                options: None,
            },
            OverlayElement {
                enabled: true,
                kind: OverlayKind::LapTimer,
                x: 0.9,
                y: 0.9,
                scale: 0.8,
                options: None,
            },
        ],
        export_start_ms: Some(100),
        export_end_ms: None,
        speed_source: track_overlay::project::SpeedSource::Auto,
    };

    // Test serialization
    let json = serde_json::to_string(&config).expect("Failed to serialize custom config");

    // Test deserialization
    let loaded_config: ProjectConfig =
        serde_json::from_str(&json).expect("Failed to deserialize custom config");

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
            max_auto_sync_offset_ms: 300000,
        },
        flip_vertical: false,
        flip_horizontal: true,
        use_hardware_acceleration: true,
        elements: vec![OverlayElement {
            enabled: true,
            kind: OverlayKind::TrackMap,
            x: 0.1,
            y: 0.1,
            scale: 2.0,
            options: None,
        }],
        export_start_ms: None,
        export_end_ms: Some(5000),
        speed_source: track_overlay::project::SpeedSource::Auto,
    };

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");

    // Save to temp file
    config
        .save(temp_file.path())
        .expect("Failed to save config");

    // Load from temp file
    let loaded_config = ProjectConfig::load(temp_file.path()).expect("Failed to load config");

    // Verify equality
    assert_eq!(config, loaded_config);
}
