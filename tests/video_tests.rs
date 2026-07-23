use eframe::egui;
use std::process::Command;
use track_overlay::video::VideoPlayer;

#[test]
fn test_video_decode() {
    let test_vid_path = "/tmp/test_video.mp4";
    let status = Command::new("ffmpeg")
        .args(&[
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
    let ctx = egui::Context::default();
    let mut player = VideoPlayer::new(test_vid_path, ctx).unwrap();

    let _ = player.seek(100);
    std::thread::sleep(std::time::Duration::from_millis(1500)); // give bg thread time to decode

    let sample = player.get_frame();
    assert!(sample.is_some(), "Should have decoded a frame");

    let frame = sample.unwrap();
    assert_eq!(frame.width, 320);
    assert_eq!(frame.height, 240);
    assert_eq!(frame.data.len(), 320 * 240 * 4); // Tightly packed RGBA
}
