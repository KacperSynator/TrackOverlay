use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn merge_videos(video1: &Path, video2: &Path) -> Result<PathBuf> {
    let concat_file = tempfile::Builder::new()
        .prefix("trackoverlay_concat_")
        .suffix(".txt")
        .tempfile()?;

    let concat_path = concat_file.into_temp_path();
    // Prevent the temp file from being deleted immediately
    let concat_path_buf = concat_path.to_path_buf();
    concat_path.keep()?;

    let mut file = File::create(&concat_path_buf)?;

    // FFmpeg concat demuxer requires paths to be escaped or wrapped in quotes.
    // Importantly, FFmpeg parses backslashes as escape characters even inside quotes.
    // So we replace backslashes with forward slashes for cross-platform safety.
    let path1_str = video1
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    let path2_str = video2
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''");

    writeln!(file, "file '{}'", path1_str)?;
    writeln!(file, "file '{}'", path2_str)?;
    file.flush()?;

    let output_file = tempfile::Builder::new()
        .prefix("trackoverlay_merged_")
        .suffix(".mp4")
        .tempfile()?;

    let output_path = output_file.into_temp_path();
    let output_path_buf = output_path.to_path_buf();
    output_path.keep()?;

    // Execute FFmpeg
    // For GoPro files, stream 0 is video, stream 1 is audio, stream 2 is timecode, stream 3 is GPMF telemetry.
    // The concat demuxer can sometimes fail (Exit Code 234) if it encounters inconsistent data stream durations.
    // Using `-copy_unknown` helps preserve streams like GPMF telemetry.
    let status = Command::new("ffmpeg")
        .arg("-y") // Overwrite output if exists
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&concat_path_buf)
        .arg("-c")
        .arg("copy")
        .arg("-map")
        .arg("0") // Map all streams
        .arg("-copy_unknown") // Allow unknown streams (like GPMF) to be copied without failure
        .arg("-ignore_unknown") // Ignore unknown stream failures
        .arg(&output_path_buf)
        .status()
        .context("Failed to run ffmpeg command for concatenation")?;

    // Cleanup concat file
    let _ = std::fs::remove_file(concat_path_buf);

    if !status.success() {
        // If it fails, try to cleanup output file
        let _ = std::fs::remove_file(&output_path_buf);
        anyhow::bail!("ffmpeg concat failed with status: {}", status);
    }

    Ok(output_path_buf)
}
