use crate::telemetry::TelemetryLog;

/// Auto-correlates two GPS traces to find the time offset.
/// Returns the offset_ms that should be added to the video playhead
/// to match the telemetry.
pub fn auto_correlate_gps(gopro_gps: &[(i64, f64, f64)], telemetry: &TelemetryLog) -> Option<i64> {
    if gopro_gps.is_empty() || telemetry.samples.is_empty() {
        return None;
    }

    // Convert both into vectors of distances from start to simplify correlation
    let mut gopro_speeds = Vec::new();
    for i in 1..gopro_gps.len() {
        let (t1, lat1, lon1) = gopro_gps[i-1];
        let (t2, lat2, lon2) = gopro_gps[i];

        let dist = haversine(lat1, lon1, lat2, lon2);
        let dt = (t2 - t1) as f64 / 1000.0;
        if dt > 0.0 {
            gopro_speeds.push((t2, dist / dt)); // m/s roughly
        }
    }

    let mut telem_speeds = Vec::new();
    for s in &telemetry.samples {
        telem_speeds.push((s.time_ms, s.speed_kph as f64 / 3.6));
    }

    if gopro_speeds.is_empty() || telem_speeds.is_empty() {
        return None;
    }

    // Very naive cross-correlation of speeds
    // We slide the GoPro trace across the telemetry trace
    let mut best_offset = 0;
    let mut max_corr = -1.0;

    let telem_start = telem_speeds.first().unwrap().0;
    let telem_end = telem_speeds.last().unwrap().0;

    // We try offsets from -10000ms to 10000ms
    for offset_ms in (-10000..=10000).step_by(100) {
        let mut corr = 0.0;
        let mut count = 0;

        for &(gt, gs) in &gopro_speeds {
            let t_target = gt + offset_ms;
            if t_target >= telem_start && t_target <= telem_end {
                // Find nearest telemetry point (naive linear search for simplicity, can be binary)
                let nearest = telem_speeds.binary_search_by_key(&t_target, |&(t, _)| t);
                let idx = nearest.unwrap_or_else(|e| e);
                if idx < telem_speeds.len() {
                    let ts = telem_speeds[idx].1;
                    // Product of speeds (cross-correlation)
                    corr += gs * ts;
                    count += 1;
                }
            }
        }

        if count > 0 {
            corr /= count as f64;
            if corr > max_corr {
                max_corr = corr;
                best_offset = offset_ms;
            }
        }
    }

    Some(best_offset)
}

fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371000.0; // Earth radius in meters
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2) +
            lat1.to_radians().cos() * lat2.to_radians().cos() *
            (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}
