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
        .arg("0:v")
        .arg("-map")
        .arg("0:a")
        .arg("-map")
        .arg("0:d")
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
