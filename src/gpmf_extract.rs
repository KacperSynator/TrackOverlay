use anyhow::{Result, anyhow};
use std::process::Command;

/// Attempts to extract the GPS5 data track from a GoPro MP4 via ffmpeg
/// into a sequence of roughly {time_ms, lat, lon}.
/// We use ffprobe to find the telemetry stream, then ffmpeg to dump it.
pub fn extract_gopro_gps(video_path: &str) -> Result<Vec<(i64, f64, f64)>> {
    // 1. Find telemetry stream
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "d",
            "-show_entries",
            "stream=index,codec_tag_string",
            "-of",
            "csv=p=0",
            video_path,
        ])
        .output()?;

    let output_str = String::from_utf8_lossy(&probe.stdout);

    // Format might be "3,gpmd" or similar
    let mut gpmd_stream_idx = None;
    for line in output_str.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() == 2 && parts[1].trim() == "gpmd" {
            gpmd_stream_idx = Some(parts[0].trim().to_string());
            break;
        }
    }

    let stream_idx = gpmd_stream_idx.ok_or_else(|| anyhow!("No GPMD stream found in MP4"))?;

    // 2. Dump stream data using ffmpeg to a temporary file
    let temp_gpmf = tempfile::NamedTempFile::new()?;
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            video_path,
            "-map",
            &format!("0:{}", stream_idx),
            "-c",
            "copy",
            "-f",
            "data",
            temp_gpmf.path().to_str().unwrap(),
        ])
        .status()?;

    if !status.success() {
        return Err(anyhow!("Failed to extract GPMD data track"));
    }

    // 3. Fallback: raw byte scan for GPS5
    // GoPro GPS5 block: 'GPS5' (4 bytes), type (1 char), size (1 byte), count (2 bytes), data...
    let raw_data = std::fs::read(temp_gpmf.path())?;

    let mut gps_points = Vec::new();
    let mut i = 0;
    let mut current_time_ms = 0; // we'd need time info, but we'll approximate based on sample idx
    while i < raw_data.len() - 8 {
        if &raw_data[i..i + 4] == b"GPS5" {
            let item_size = raw_data[i + 5] as usize;
            let item_count = u16::from_be_bytes([raw_data[i + 6], raw_data[i + 7]]) as usize;

            let data_start = i + 8;
            let data_len = item_size * item_count;

            if data_start + data_len <= raw_data.len() && item_size >= 16 {
                // Parse int32 values: lat, lon, alt, speed, speed3d
                for c in 0..item_count {
                    let off = data_start + (c * item_size);
                    let lat = i32::from_be_bytes([
                        raw_data[off],
                        raw_data[off + 1],
                        raw_data[off + 2],
                        raw_data[off + 3],
                    ]) as f64
                        / 10000000.0;
                    let lon = i32::from_be_bytes([
                        raw_data[off + 4],
                        raw_data[off + 5],
                        raw_data[off + 6],
                        raw_data[off + 7],
                    ]) as f64
                        / 10000000.0;

                    if lat != 0.0 && lon != 0.0 {
                        gps_points.push((current_time_ms, lat, lon));
                        current_time_ms += 50; // Assume 18Hz or ~50ms per sample
                    }
                }
            }
            i += 8 + data_len;
            // Pad to 4 bytes
            if i % 4 != 0 {
                i += 4 - (i % 4);
            }
        } else {
            i += 1;
        }
    }

    Ok(gps_points)
}
