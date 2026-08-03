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

    let stream_idx = parse_ffprobe_output(&output_str)
        .ok_or_else(|| anyhow!("No GPMD stream found in MP4"))?;

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
    let raw_data = std::fs::read(temp_gpmf.path())?;

    Ok(parse_gps5_data(&raw_data))
}

fn parse_ffprobe_output(output_str: &str) -> Option<String> {
    // Format might be "3,gpmd" or similar
    for line in output_str.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() == 2 && parts[1].trim() == "gpmd" {
            return Some(parts[0].trim().to_string());
        }
    }
    None
}

// GoPro GPS5 block: 'GPS5' (4 bytes), type (1 char), size (1 byte), count (2 bytes), data...
fn parse_gps5_data(raw_data: &[u8]) -> Vec<(i64, f64, f64)> {
    let mut gps_points = Vec::new();
    let mut i = 0;
    let mut current_time_ms = 0; // we'd need time info, but we'll approximate based on sample idx

    // Using i + 8 <= raw_data.len() instead of i < raw_data.len() - 8 to avoid underflow
    while i + 8 <= raw_data.len() {
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

    gps_points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ffprobe_output_found() {
        let output = "0,video\n1,audio\n2,gpmd\n3,subtitles\n";
        assert_eq!(parse_ffprobe_output(output), Some("2".to_string()));
    }

    #[test]
    fn test_parse_ffprobe_output_not_found() {
        let output = "0,video\n1,audio\n2,data\n";
        assert_eq!(parse_ffprobe_output(output), None);
    }

    #[test]
    fn test_parse_ffprobe_output_empty() {
        assert_eq!(parse_ffprobe_output(""), None);
    }

    #[test]
    fn test_parse_gps5_data_empty() {
        let data: Vec<u8> = vec![];
        let points = parse_gps5_data(&data);
        assert!(points.is_empty());
    }

    #[test]
    fn test_parse_gps5_data_valid() {
        // Construct a valid GPMF GPS5 block
        let mut data = Vec::new();
        data.extend_from_slice(b"GPS5");
        data.push(b'l'); // type (not actually checked by the parser, but required for alignment)
        data.push(16); // item_size (must be >= 16)
        data.extend_from_slice(&2u16.to_be_bytes()); // item_count (2 items)

        // Item 1: lat=37.7749 (377749000), lon=-122.4194 (-1224194000)
        data.extend_from_slice(&377749000i32.to_be_bytes());
        data.extend_from_slice(&(-1224194000i32).to_be_bytes());
        data.extend_from_slice(&0i32.to_be_bytes()); // alt
        data.extend_from_slice(&0i32.to_be_bytes()); // speed

        // Item 2: lat=37.7750 (377750000), lon=-122.4195 (-1224195000)
        data.extend_from_slice(&377750000i32.to_be_bytes());
        data.extend_from_slice(&(-1224195000i32).to_be_bytes());
        data.extend_from_slice(&0i32.to_be_bytes()); // alt
        data.extend_from_slice(&0i32.to_be_bytes()); // speed

        let points = parse_gps5_data(&data);
        assert_eq!(points.len(), 2);

        // Check point 1
        assert_eq!(points[0].0, 0); // initial time
        assert!((points[0].1 - 37.7749).abs() < f64::EPSILON);
        assert!((points[0].2 - -122.4194).abs() < f64::EPSILON);

        // Check point 2
        assert_eq!(points[1].0, 50); // time advanced by 50ms
        assert!((points[1].1 - 37.7750).abs() < f64::EPSILON);
        assert!((points[1].2 - -122.4195).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_gps5_data_ignores_zeros() {
        // Construct a valid GPMF GPS5 block but with zero coordinates
        let mut data = Vec::new();
        data.extend_from_slice(b"GPS5");
        data.push(b'l');
        data.push(16); // item_size
        data.extend_from_slice(&1u16.to_be_bytes()); // item_count (1 item)

        // Item 1: lat=0, lon=0
        data.extend_from_slice(&0i32.to_be_bytes());
        data.extend_from_slice(&0i32.to_be_bytes());
        data.extend_from_slice(&0i32.to_be_bytes()); // alt
        data.extend_from_slice(&0i32.to_be_bytes()); // speed

        let points = parse_gps5_data(&data);
        assert!(points.is_empty(), "Points with lat=0, lon=0 should be ignored");
    }

    #[test]
    fn test_parse_gps5_data_ignores_small_item_size() {
        // Construct a GPMF GPS5 block but item_size is < 16
        let mut data = Vec::new();
        data.extend_from_slice(b"GPS5");
        data.push(b'l');
        data.push(15); // item_size (too small)
        data.extend_from_slice(&1u16.to_be_bytes()); // item_count (1 item)

        data.extend_from_slice(&vec![0; 15]); // Pad with 15 bytes

        let points = parse_gps5_data(&data);
        assert!(points.is_empty(), "Should ignore blocks with item_size < 16");
    }

    #[test]
    fn test_parse_gps5_data_incomplete_block() {
        // Block declares length longer than actual data
        let mut data = Vec::new();
        data.extend_from_slice(b"GPS5");
        data.push(b'l');
        data.push(16); // item_size
        data.extend_from_slice(&10u16.to_be_bytes()); // item_count (10 items = 160 bytes)

        // But we only provide 1 item's worth of data
        data.extend_from_slice(&377749000i32.to_be_bytes());
        data.extend_from_slice(&(-1224194000i32).to_be_bytes());
        data.extend_from_slice(&0i32.to_be_bytes()); // alt
        data.extend_from_slice(&0i32.to_be_bytes()); // speed

        let points = parse_gps5_data(&data);
        assert!(points.is_empty(), "Should ignore incomplete blocks where data_start + data_len > raw_data.len()");
    }
}
