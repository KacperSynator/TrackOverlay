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

    {
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
    }

    let output_file = tempfile::Builder::new()
        .prefix("trackoverlay_merged_")
        .suffix(".mp4")
        .tempfile()?;

    // Keep the file handle alive while ffmpeg runs, but get the path
    let output_path_buf = output_file.path().to_path_buf();

    // Execute FFmpeg
    // GoPro MP4 files often contain obscure/unknown streams (like timecode or other metadata).
    // Using a generic `-map 0` forces FFmpeg to copy these unknown streams into the new MP4,
    // which causes the MP4 muxer to fail with "Could not find tag for codec none in stream...".
    // To safely preserve GPMF telemetry and skip the broken ones, we explicitly map Video (v),
    // Audio (a), and Data (d).
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
        .arg("0:v") // Map video streams
        .arg("-map")
        .arg("0:a") // Map audio streams
        .arg("-map")
        .arg("0:d") // Map data streams (preserves GPMF telemetry)
        .arg("-map_metadata")
        .arg("0") // Copy global metadata from the first file (preserves creation_time)
        .arg("-movflags")
        .arg("use_metadata_tags") // Write metadata tags into the MP4 container
        .arg("-copy_unknown") // Allow unknown streams (like GPMF) to be copied without failure
        .arg("-ignore_unknown") // Ignore unknown stream failures
        .arg(&output_path_buf)
        .status()
        .context("Failed to run ffmpeg command for concatenation")?;

    // Cleanup concat file
    let _ = std::fs::remove_file(concat_path_buf);

    if !status.success() {
        // Output file is dropped and deleted automatically here since we didn't call keep()
        anyhow::bail!("ffmpeg concat failed with status: {}", status);
    }

    // Now that it succeeded, persist the output file
    let (_, output_path) = output_file.keep()?;

    Ok(output_path)
}
