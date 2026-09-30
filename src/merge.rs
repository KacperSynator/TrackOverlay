use crate::error::MergeError;
use std::io::Write;
use std::path::{Path, PathBuf};

fn get_metadata_value(video: &Path, key: &str) -> Result<Option<String>, MergeError> {
    let output = crate::ff_commands::ffprobe_get_format_tags(video, key)?;
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if value.is_empty() { None } else { Some(value) })
}

pub fn merge_videos(video1: &Path, video2: &Path) -> Result<PathBuf, MergeError> {
    let video1_lossy = video1.to_string_lossy();
    let video2_lossy = video2.to_string_lossy();

    // Prevent path injection vulnerabilities via newline characters
    // since the FFmpeg concat demuxer reads line by line.
    if video1_lossy.contains('\n') || video1_lossy.contains('\r') {
        return Err(MergeError::InvalidPath(
            "video1 path contains newline characters".to_string(),
        ));
    }
    if video2_lossy.contains('\n') || video2_lossy.contains('\r') {
        return Err(MergeError::InvalidPath(
            "video2 path contains newline characters".to_string(),
        ));
    }

    let mut concat_file = tempfile::Builder::new()
        .prefix("trackoverlay_concat_")
        .suffix(".txt")
        .tempfile()?;

    // FFmpeg concat demuxer requires paths to be escaped or wrapped in quotes.
    // Importantly, FFmpeg parses backslashes as escape characters even inside quotes.
    // So we replace backslashes with forward slashes for cross-platform safety.
    let path1_str = video1_lossy.replace('\\', "/").replace('\'', "'\\''");
    let path2_str = video2_lossy.replace('\\', "/").replace('\'', "'\\''");

    writeln!(concat_file, "file '{}'", path1_str)?;
    writeln!(concat_file, "file '{}'", path2_str)?;
    concat_file.flush()?;

    let concat_path = concat_file.into_temp_path();
    // Prevent the temp file from being deleted immediately
    let concat_path_buf = concat_path.to_path_buf();
    concat_path.keep()?;

    // Create the tempfile but immediately close the file handle,
    // saving only the path. This prevents file locking errors on Windows
    // when ffmpeg tries to open and write to it.
    let output_file = tempfile::Builder::new()
        .prefix("trackoverlay_merged_")
        .suffix(".mp4")
        .tempfile()?;

    let output_path = output_file.into_temp_path();
    let output_path_buf = output_path.to_path_buf();
    output_path.keep()?;

    // Fetch critical metadata tags from the first video to inject manually.
    // The concat demuxer does not carry over global or stream tags to the virtual container.
    let creation_time = get_metadata_value(video1, "creation_time").unwrap_or(None);
    let firmware = get_metadata_value(video1, "firmware").unwrap_or(None);

    // Execute FFmpeg
    let status = crate::ff_commands::ffmpeg_merge_videos(
        &concat_path_buf,
        &output_path_buf,
        creation_time.as_deref(),
        firmware.as_deref(),
    )?;

    // Cleanup concat file
    let _ = std::fs::remove_file(concat_path_buf);

    if !status.success() {
        // If it fails, try to cleanup output file
        let _ = std::fs::remove_file(&output_path_buf);
        return Err(MergeError::CommandFailed(status));
    }

    Ok(output_path_buf)
}
