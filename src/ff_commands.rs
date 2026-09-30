use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Output};

pub fn ffprobe_get_video_rotation(path_str: &str) -> io::Result<Output> {
    Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_streams")
        .arg("-of")
        .arg("json")
        .arg("-i")
        .arg(path_str)
        .output()
}

pub fn ffprobe_get_metadata_entries(path_str: &str, entries: &str) -> io::Result<Output> {
    Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg(entries)
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg("-i")
        .arg(path_str)
        .output()
}

pub fn ffprobe_get_format_tags(path: &Path, key: &str) -> io::Result<Output> {
    Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-show_entries")
        .arg(format!("format_tags={}", key))
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg("-i")
        .arg(path)
        .output()
}

pub fn ffmpeg_merge_videos(
    concat_path: &Path,
    output_path: &Path,
    creation_time: Option<&str>,
    firmware: Option<&str>,
) -> io::Result<ExitStatus> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(concat_path)
        .arg("-c")
        .arg("copy")
        .arg("-map")
        .arg("0:v?")
        .arg("-map")
        .arg("0:a?")
        .arg("-map")
        .arg("0:d?")
        .arg("-movflags")
        .arg("use_metadata_tags")
        .arg("-copy_unknown")
        .arg("-ignore_unknown");

    if let Some(ct) = creation_time {
        cmd.arg("-metadata").arg(format!("creation_time={}", ct));
    }
    if let Some(fw) = firmware {
        cmd.arg("-metadata").arg(format!("firmware={}", fw));
    }

    cmd.arg(output_path).status()
}

pub fn ffprobe_find_telemetry_stream(video_path: &str) -> io::Result<Output> {
    Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("d")
        .arg("-show_entries")
        .arg("stream=index,codec_tag_string")
        .arg("-of")
        .arg("csv=p=0")
        .arg("-i")
        .arg(video_path)
        .output()
}

pub fn ffmpeg_dump_telemetry(
    video_path: &str,
    stream_idx: &str,
    output_path: &Path,
) -> io::Result<ExitStatus> {
    Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg(format!("0:{}", stream_idx))
        .arg("-c")
        .arg("copy")
        .arg("-f")
        .arg("data")
        .arg(output_path)
        .status()
}

pub fn ffmpeg_mux_exported_video(
    video_path: &str,
    temp_path: &Path,
    output_path: &Path,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
) -> io::Result<ExitStatus> {
    let mut cmd = Command::new("ffmpeg");

    cmd.arg("-y").arg("-i").arg(temp_path);

    #[allow(clippy::collapsible_if)]
    if let Some(start) = start_ms {
        if start > 0 {
            cmd.arg("-ss");
            cmd.arg(format!("{:.3}", start as f64 / 1000.0));
        }
    }

    if let Some(end) = end_ms {
        let start_ms = start_ms.unwrap_or(0).max(0);
        if end >= 0 && end > start_ms {
            cmd.arg("-t");
            cmd.arg(format!("{:.3}", (end - start_ms) as f64 / 1000.0));
        }
    }

    cmd.arg("-i")
        .arg(video_path)
        .arg("-c:v")
        .arg("copy")
        .arg("-c:a")
        .arg("copy")
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("1:a:0?")
        .arg(output_path);

    cmd.status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::NamedTempFile;

    fn create_dummy_video_with_metadata(key: &str, value: &str) -> NamedTempFile {
        let temp_file = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .expect("Failed to create temp file");

        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=black:s=10x10:d=1")
            .arg("-metadata")
            .arg(format!("{}={}", key, value))
            .arg(temp_file.path())
            .status()
            .expect("Failed to run ffmpeg to create dummy video");

        assert!(
            status.success(),
            "ffmpeg command failed to create dummy video"
        );
        temp_file
    }

    #[test]
    fn test_ffprobe_get_format_tags_success() {
        let expected_time = "2023-01-01T12:00:00.000000Z";
        let dummy_video = create_dummy_video_with_metadata("creation_time", expected_time);

        let output = ffprobe_get_format_tags(dummy_video.path(), "creation_time")
            .expect("failed to execute ffprobe_get_format_tags");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), expected_time);
    }

    #[test]
    fn test_ffprobe_get_format_tags_missing_key() {
        let dummy_video =
            create_dummy_video_with_metadata("creation_time", "2023-01-01T12:00:00.000000Z");

        let output = ffprobe_get_format_tags(dummy_video.path(), "non_existent_key")
            .expect("failed to execute ffprobe_get_format_tags");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "");
    }

    #[test]
    fn test_ffprobe_get_format_tags_file_not_found() {
        let non_existent_path = Path::new("does_not_exist_xyz.mp4");

        let output = ffprobe_get_format_tags(non_existent_path, "creation_time")
            .expect("failed to execute ffprobe_get_format_tags");

        assert!(!output.status.success());
    }

    #[test]
    fn test_ffprobe_find_telemetry_stream_success() {
        let temp_timecode = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .expect("Failed to create temp timecode video file");

        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=black:s=10x10:d=1")
            .arg("-c:v")
            .arg("libx264")
            .arg("-timecode")
            .arg("00:00:00:00")
            .arg(temp_timecode.path())
            .status()
            .expect("Failed to create video with timecode data stream");
        assert!(status.success(), "ffmpeg failed to create timecode stream");

        let output =
            ffprobe_find_telemetry_stream(temp_timecode.path().to_str().expect("valid utf8 path"))
                .expect("failed to execute ffprobe_find_telemetry_stream");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("tmcd"));
    }

    #[test]
    fn test_ffmpeg_dump_telemetry_success() {
        let temp_timecode = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .expect("Failed to create temp timecode video file");

        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=black:s=10x10:d=1")
            .arg("-c:v")
            .arg("libx264")
            .arg("-timecode")
            .arg("00:00:00:00")
            .arg(temp_timecode.path())
            .status()
            .expect("Failed to create video with timecode data stream");
        assert!(status.success());

        let dump_out = tempfile::Builder::new()
            .suffix(".bin")
            .tempfile()
            .expect("Failed to create temp dump bin");

        let exit_status = ffmpeg_dump_telemetry(
            temp_timecode.path().to_str().expect("valid utf8 path"),
            "d:0",
            dump_out.path(),
        )
        .expect("failed to dump telemetry");

        assert!(exit_status.success());
        let meta = std::fs::metadata(dump_out.path()).unwrap();
        assert!(meta.len() > 0);
    }
}
