use crate::telemetry::TelemetryLog;
use log::{info, warn};

/// Auto-correlates two GPS traces to find the time offset.
/// Returns the offset_ms that should be added to the video playhead
/// to match the telemetry.
pub fn auto_correlate_gps(gopro_gps: &[(i64, f64, f64)], telemetry: &TelemetryLog) -> Option<i64> {
    if gopro_gps.is_empty() || telemetry.samples.is_empty() {
        warn!("Cannot auto-correlate: missing GPS or Telemetry data");
        return None;
    }

    info!(
        "Preparing Gopro GPS data for correlation ({} points)",
        gopro_gps.len()
    );

    // Instead of speeds, we'll try correlating relative distance from origin, which is less noisy.
    let mut gopro_dist = Vec::new();
    let (t0, lat0, lon0) = gopro_gps[0];
    for &(t, lat, lon) in gopro_gps {
        gopro_dist.push((t - t0, haversine(lat0, lon0, lat, lon)));
    }

    let mut telem_dist = Vec::new();
    let sample0 = &telemetry.samples[0];
    for s in &telemetry.samples {
        telem_dist.push((s.time_ms, haversine(sample0.lat, sample0.lon, s.lat, s.lon)));
    }

    info!("Searching for offset...");

    let mut best_offset = 0;
    let mut min_error = f64::MAX;

    let telem_start = telem_dist.first().unwrap().0;
    let telem_end = telem_dist.last().unwrap().0;

    // We try offsets from -120000ms to 120000ms (2 minutes)
    // Finding minimum least-squares error of distance.
    for offset_ms in (-120000..=120000).step_by(100) {
        let mut error = 0.0;
        let mut count = 0;

        for &(gt, gd) in &gopro_dist {
            let t_target = gt + offset_ms;
            if t_target >= telem_start && t_target <= telem_end {
                let nearest = telem_dist.binary_search_by_key(&t_target, |&(t, _)| t);
                let idx = nearest.unwrap_or_else(|e| e);
                if idx < telem_dist.len() {
                    let td = telem_dist[idx].1;
                    error += (gd - td).powi(2);
                    count += 1;
                }
            }
        }

        if count > gopro_dist.len() / 4 {
            // Need at least 25% overlap
            error /= count as f64;
            if error < min_error {
                min_error = error;
                best_offset = offset_ms;
            }
        }
    }

    if min_error == f64::MAX {
        warn!("Auto-sync failed to find adequate overlap between GPS and telemetry paths.");
        None
    } else {
        info!(
            "Auto-sync found best offset {} ms with error {}",
            best_offset, min_error
        );
        Some(best_offset)
    }
}

fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371000.0; // Earth radius in meters
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}
