use std::process::Command;
use telemetry_overlay::video::VideoPlayer;
use std::thread::sleep;
use std::time::Duration;

#[test]
fn test_video_decode() {
    // Generate a test video using ffmpeg
    let test_vid_path = "/tmp/test_video.mp4";
    let status = Command::new("ffmpeg")
        .args(&[
            "-y", "-f", "lavfi", "-i", "testsrc=duration=2:size=320x240:rate=30",
            "-pix_fmt", "yuv420p", "-c:v", "libx264", test_vid_path,
        ])
        .status()
        .expect("Failed to run ffmpeg to create test video");

    assert!(status.success());

    // Initialize our player
    let mut player = VideoPlayer::new(test_vid_path).unwrap();
    player.play().unwrap();

    // Give it a moment to start playing and decode a frame
    sleep(Duration::from_millis(500));

    let sample = player.get_frame().unwrap();
    assert!(sample.is_some(), "Should have decoded a frame");

    assert_eq!(player.width(), 320);
    assert_eq!(player.height(), 240);

    player.pause().unwrap();
}
