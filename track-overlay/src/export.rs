use anyhow::Result;
use std::process::Command;
use std::path::Path;
use crate::project::ProjectConfig;
use crate::telemetry::TelemetryLog;

pub fn export_video(
    config: &ProjectConfig,
    _telemetry: &TelemetryLog,
    output_path: &Path,
) -> Result<()> {
    println!("Starting export for {:?}", config.video_path);

    // Stub: create a dummy ffmpeg command just to ensure the pipeline path works.
    let video_path = config.video_path.to_str().unwrap_or("");
    if video_path.is_empty() {
        // Just create a dummy colorbars video to verify output works without a real input video
        let status = Command::new("ffmpeg")
            .args(&[
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=1:size=320x240:rate=30",
                "-vf", "drawtext=text='Telemetry Overlay':x=10:y=10:fontsize=24:fontcolor=white",
                output_path.to_str().unwrap_or("output.mp4"),
            ])
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("FFmpeg export failed with status {}", status));
        }
        return Ok(());
    }

    let status = Command::new("ffmpeg")
        .args(&[
            "-y",
            "-i", video_path,
            "-vf", "drawtext=text='Telemetry Overlay':x=10:y=10:fontsize=24:fontcolor=white",
            "-c:a", "copy",
            output_path.to_str().unwrap_or("output.mp4"),
        ])
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("FFmpeg export failed with status {}", status));
    }

    Ok(())
}
