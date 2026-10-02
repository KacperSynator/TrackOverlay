use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use track_overlay::export::export_video;
use track_overlay::project::{ProjectConfig, SpeedSource, SyncMode, SyncState};
use track_overlay::telemetry::TelemetryLog;

fn create_mock_video(path: &Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=320x240:rate=30",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=stereo",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
            "-pix_fmt",
            "yuv420p",
            path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run ffmpeg to create test video");

    assert!(status.success());
}

fn create_mock_audio(path: &Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=stereo:d=1",
            "-c:a",
            "aac",
            path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run ffmpeg to create test audio file");

    assert!(status.success());
}

fn create_mock_telemetry(path: &Path) {
    let csv_content = r#"# Dummy Header
"Time","UTC Time","Lap","Predicted Lap Time","Predicted vs Best Lap","GPS_Update","GPS_Delay","Latitude","Longitude","Altitude (m)","Altitude (ft)","Speed (Km/h)","Heading","Accuracy (m)","Accel X","Accel Y","Accel Z","Brake (calculated)","Barometric Pressure (kPa)","Pressure Altitude (m)","OBD_Update","Engine Speed (RPM) *OBD","Vehicle Speed (km/h) *OBD","Throttle Position (%) *OBD","Engine Coolant Temp (C) *OBD","Intake Air Temp (C) *OBD","Intake Manifold Pressure (kPa) *OBD"
0.000,1000.000,0,0,0,1,0.0,50.0,20.0,0,0,10.0,0,0,0,0,0,0,100.0,0,1,1000,10,10,90,30,100
1.000,1001.000,0,0,0,1,0.0,50.0,20.0,0,0,20.0,0,0,0,0,0,0,100.0,0,1,1000,10,10,90,30,100
"#;
    let mut file = std::fs::File::create(path).expect("Failed to create temporary file for test");
    file.write_all(csv_content.as_bytes())
        .expect("Failed to write CSV file");
}

#[test]
fn test_export_video_success() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let video_path = temp_dir.path().join("vid.mp4");
    let telemetry_path = temp_dir.path().join("telemetry.csv");
    let output_path = temp_dir.path().join("output.mp4");

    create_mock_video(&video_path);
    create_mock_telemetry(&telemetry_path);

    let config = ProjectConfig {
        video_path: video_path.clone(),
        telemetry_path: telemetry_path.clone(),
        sync: SyncState {
            offset_ms: 0,
            mode: SyncMode::Manual,
            max_auto_sync_offset_ms: 300000,
        },
        flip_vertical: false,
        flip_horizontal: false,
        elements: vec![],
        export_start_ms: None,
        export_end_ms: None,
        speed_source: SpeedSource::Auto,
        interpolation_mode: track_overlay::project::InterpolationMode::Linear,
        interpolation_points: 0,
    };

    let telemetry = TelemetryLog::load_csv(&telemetry_path, config.speed_source.clone())
        .expect("Failed to load telemetry");

    let result = export_video(&config, &telemetry, &output_path, None);
    assert!(result.is_ok(), "Export should succeed");

    assert!(output_path.exists(), "Output video file should exist");

    // Attempt to parse output using ffprobe to ensure it's a valid video
    let status = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(output_path.to_str().unwrap())
        .status()
        .expect("Failed to run ffprobe on merged video");

    assert!(status.success(), "Merged output should be a valid video");
}

#[test]
fn test_export_video_no_video_path() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let video_path = temp_dir.path().join("vid.mp4");
    let telemetry_path = temp_dir.path().join("telemetry.csv");
    let output_path = temp_dir.path().join("output.mp4");

    create_mock_video(&video_path);
    create_mock_telemetry(&telemetry_path);

    let config = ProjectConfig {
        video_path: PathBuf::new(), // Empty path
        telemetry_path: telemetry_path.clone(),
        sync: SyncState {
            offset_ms: 0,
            mode: SyncMode::Manual,
            max_auto_sync_offset_ms: 300000,
        },
        flip_vertical: false,
        flip_horizontal: false,
        elements: vec![],
        export_start_ms: None,
        export_end_ms: None,
        speed_source: SpeedSource::Auto,
        interpolation_mode: track_overlay::project::InterpolationMode::Linear,
        interpolation_points: 0,
    };

    let telemetry = TelemetryLog::load_csv(&telemetry_path, config.speed_source.clone())
        .expect("Failed to load telemetry");

    let result = export_video(&config, &telemetry, &output_path, None);
    assert!(
        result.is_err(),
        "Export should fail due to empty video path"
    );

    match result.unwrap_err() {
        track_overlay::error::ExportError::NoVideoPath => {
            // Expected
        }
        e => panic!("Expected NoVideoPath error, got: {:?}", e),
    }
}

#[test]
fn test_export_video_missing_file() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let video_path = temp_dir.path().join("non_existent_vid.mp4");
    let telemetry_path = temp_dir.path().join("telemetry.csv");
    let output_path = temp_dir.path().join("output.mp4");

    create_mock_telemetry(&telemetry_path);

    let config = ProjectConfig {
        video_path: video_path.clone(),
        telemetry_path: telemetry_path.clone(),
        sync: SyncState {
            offset_ms: 0,
            mode: SyncMode::Manual,
            max_auto_sync_offset_ms: 300000,
        },
        flip_vertical: false,
        flip_horizontal: false,
        elements: vec![],
        export_start_ms: None,
        export_end_ms: None,
        speed_source: SpeedSource::Auto,
        interpolation_mode: track_overlay::project::InterpolationMode::Linear,
        interpolation_points: 0,
    };

    let telemetry = TelemetryLog::load_csv(&telemetry_path, config.speed_source.clone())
        .expect("Failed to load telemetry");

    let result = export_video(&config, &telemetry, &output_path, None);
    assert!(
        result.is_err(),
        "Export should fail due to missing video file"
    );

    match result.unwrap_err() {
        track_overlay::error::ExportError::Ffmpeg(_) => {
            // Expected
        }
        e => panic!("Expected Ffmpeg error, got: {:?}", e),
    }
}

#[test]
fn test_export_video_no_video_stream() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let audio_path = temp_dir.path().join("audio.mp4");
    let telemetry_path = temp_dir.path().join("telemetry.csv");
    let output_path = temp_dir.path().join("output.mp4");

    create_mock_audio(&audio_path);
    create_mock_telemetry(&telemetry_path);

    let config = ProjectConfig {
        video_path: audio_path.clone(),
        telemetry_path: telemetry_path.clone(),
        sync: SyncState {
            offset_ms: 0,
            mode: SyncMode::Manual,
            max_auto_sync_offset_ms: 300000,
        },
        flip_vertical: false,
        flip_horizontal: false,
        elements: vec![],
        export_start_ms: None,
        export_end_ms: None,
        speed_source: SpeedSource::Auto,
        interpolation_mode: track_overlay::project::InterpolationMode::Linear,
        interpolation_points: 0,
    };

    let telemetry = TelemetryLog::load_csv(&telemetry_path, config.speed_source.clone())
        .expect("Failed to load telemetry");

    let result = export_video(&config, &telemetry, &output_path, None);
    assert!(
        result.is_err(),
        "Export should fail because input has no video stream"
    );

    match result.unwrap_err() {
        track_overlay::error::ExportError::NoVideoStream => {
            // Expected
        }
        e => panic!("Expected NoVideoStream error, got: {:?}", e),
    }
}
