use std::path::Path;
use std::process::Command;
use track_overlay::error::MergeError;
use track_overlay::merge::merge_videos;

fn create_mock_video(path: &Path) {
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("testsrc=duration=1:size=320x240:rate=30")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("anullsrc=r=44100:cl=stereo")
        .arg("-c:v")
        .arg("libx264")
        .arg("-c:a")
        .arg("aac")
        .arg("-shortest")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg(path.to_str().unwrap())
        .status()
        .expect("Failed to run ffmpeg to create test video");

    assert!(status.success());
}

#[test]
fn test_merge_videos_success() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let video1_path = temp_dir.path().join("vid1.mp4");
    let video2_path = temp_dir.path().join("vid2.mp4");

    create_mock_video(&video1_path);
    create_mock_video(&video2_path);

    let result = merge_videos(&video1_path, &video2_path);

    assert!(result.is_ok(), "Merge should succeed for valid videos");
    let merged_path = result.unwrap();

    assert!(merged_path.exists(), "Merged output file should exist");

    // Attempt to parse output using ffprobe to ensure it's a valid video
    let status = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(merged_path.to_str().unwrap())
        .status()
        .expect("Failed to run ffprobe on merged video");

    assert!(status.success(), "Merged output should be a valid video");

    // Clean up
    let _ = std::fs::remove_file(merged_path);
}

#[test]
fn test_merge_videos_non_existent_files() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let video1_path = temp_dir.path().join("non_existent1.mp4");
    let video2_path = temp_dir.path().join("non_existent2.mp4");

    let result = merge_videos(&video1_path, &video2_path);

    assert!(result.is_err(), "Merge should fail for non-existent videos");

    match result.unwrap_err() {
        MergeError::CommandFailed(_) => {
            // Expected
        }
        e => {
            panic!("Expected MergeError::CommandFailed, got {:?}", e);
        }
    }
}

#[test]
fn test_merge_videos_invalid_path_injection() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let video1_path = temp_dir.path().join("vid1.mp4'\nfile '/tmp");
    let video2_path = temp_dir.path().join("vid2.mp4");

    let results = [
        merge_videos(&video1_path, &video2_path),
        merge_videos(&video2_path, &video1_path),
    ];

    results.iter().for_each(|result| {
        assert!(
            result.is_err(),
            "Merge should fail when any path contains newlines"
        );

        match result.as_ref().unwrap_err() {
            MergeError::InvalidPath(msg) => {
                assert!(msg.contains("newline"), "Expected newline validation error");
            }
            e => {
                panic!("Expected MergeError::InvalidPath, got {:?}", e);
            }
        }
    });
}
