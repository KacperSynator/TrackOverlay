use std::process::Command;
use track_overlay::video::{VideoPlayer, parse_video_rotation_from_json};

#[test]
fn test_parse_video_rotation_from_json_display_matrix_float() {
    let json_str = r#"{
        "streams": [
            {
                "side_data_list": [
                    {
                        "side_data_type": "Display Matrix",
                        "rotation": 90.0
                    }
                ]
            }
        ]
    }"#;
    assert_eq!(parse_video_rotation_from_json(json_str), Some(90.0));
}

#[test]
fn test_parse_video_rotation_from_json_display_matrix_int() {
    let json_str = r#"{
        "streams": [
            {
                "side_data_list": [
                    {
                        "side_data_type": "Display Matrix",
                        "rotation": -90
                    }
                ]
            }
        ]
    }"#;
    assert_eq!(parse_video_rotation_from_json(json_str), Some(-90.0));
}

#[test]
fn test_parse_video_rotation_from_json_tags_rotate() {
    let json_str = r#"{
        "streams": [
            {
                "tags": {
                    "rotate": "180"
                }
            }
        ]
    }"#;
    assert_eq!(parse_video_rotation_from_json(json_str), Some(180.0));
}

#[test]
fn test_parse_video_rotation_from_json_tags_rotation() {
    let json_str = r#"{
        "streams": [
            {
                "tags": {
                    "rotation": "270"
                }
            }
        ]
    }"#;
    assert_eq!(parse_video_rotation_from_json(json_str), Some(270.0));
}

#[test]
fn test_parse_video_rotation_from_json_invalid_json() {
    let json_str = r#"{ "streams": [ { "tags": { "rotate": "180" "#;
    assert_eq!(parse_video_rotation_from_json(json_str), None);
}

#[test]
fn test_parse_video_rotation_from_json_missing_streams() {
    let json_str = r#"{
        "format": {
            "tags": {
                "rotate": "180"
            }
        }
    }"#;
    assert_eq!(parse_video_rotation_from_json(json_str), None);
}

#[test]
fn test_parse_video_rotation_from_json_empty_streams() {
    let json_str = r#"{
        "streams": []
    }"#;
    assert_eq!(parse_video_rotation_from_json(json_str), None);
}

#[test]
fn test_parse_video_rotation_from_json_no_rotation_data() {
    let json_str = r#"{
        "streams": [
            {
                "codec_type": "video",
                "width": 1920,
                "height": 1080
            }
        ]
    }"#;
    assert_eq!(parse_video_rotation_from_json(json_str), None);
}

#[test]
fn test_video_decode() {
    let test_vid_path = "/tmp/test_video.mp4";
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=2:size=320x240:rate=30",
            "-pix_fmt",
            "yuv420p",
            "-c:v",
            "libx264",
            test_vid_path,
        ])
        .status()
        .expect("Failed to run ffmpeg to create test video");

    assert!(status.success());

    // Creating a mock context for the test
    let mut player = VideoPlayer::new(test_vid_path, || {})
        .expect("Failed to initialize VideoPlayer for test video");

    let _ = player.seek(100);
    std::thread::sleep(std::time::Duration::from_millis(1500)); // give bg thread time to decode

    let sample = player.get_frame();
    assert!(sample.is_some(), "Should have decoded a frame");

    let frame = sample.expect("Failed to unwrap decoded frame");
    assert_eq!(frame.width, 320);
    assert_eq!(frame.height, 240);
    assert_eq!(frame.data.len(), 320 * 240 * 4); // Tightly packed RGBA
}
