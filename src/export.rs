use crate::project::ProjectConfig;
use crate::telemetry::TelemetryLog;
use anyhow::Result;
use std::path::Path;
use std::process::Command;

pub fn export_video(
    config: &ProjectConfig,
    _telemetry: &TelemetryLog,
    output_path: &Path,
) -> Result<()> {
    println!("Starting export for {:?}", config.video_path);

    let video_path = config.video_path.to_str().unwrap_or("");
    if video_path.is_empty() {
        return Err(anyhow::anyhow!("No video path specified for export"));
    }

    let mut filtergraph = String::from(
        "drawtext=text='Telemetry Overlay (v1 Export Stub)':x=10:y=10:fontsize=48:fontcolor=white:box=1:boxcolor=black@0.5",
    );

    if config.flip_vertical {
        filtergraph = format!("vflip,{}", filtergraph);
    }

    if config.flip_horizontal {
        filtergraph = format!("hflip,{}", filtergraph);
    }

    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            video_path,
            "-vf",
            &filtergraph,
            "-c:a",
            "copy",
            output_path.to_str().unwrap_or("output.mp4"),
        ])
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "FFmpeg export failed with status {}",
            status
        ));
    }

    Ok(())
}
