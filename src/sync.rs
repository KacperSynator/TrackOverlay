use crate::telemetry::TelemetryLog;
use log::{info, warn, debug};

/// Auto-correlates two GPS traces to find the time offset.
/// Returns the offset_ms that should be added to the video playhead
/// to match the telemetry.
pub fn auto_correlate_gps(gopro_gps: &[(i64, f64, f64)], telemetry: &TelemetryLog) -> Option<i64> {
    if gopro_gps.is_empty() || telemetry.samples.is_empty() {
        warn!("Cannot auto-correlate: missing GPS or Telemetry data");
        return None;
    }

    info!("Preparing Gopro GPS data for correlation ({} points)", gopro_gps.len());

    // Primary Strategy: Lap Line Crossings
    // 1. Get telemetry lap crossings
    let telem_laps = telemetry.extract_laps();

    // We need at least one lap crossing in the telemetry
    if !telem_laps.is_empty() {
        info!("Found {} lap crossings in telemetry. Attempting Start/Finish detection...", telem_laps.len());

        // Let's find the GPS coordinate of the Start/Finish line from the first lap crossing
        let first_lap_ms = telem_laps[0].1;
        if let Some(sf_sample) = telemetry.sample_at(first_lap_ms) {
            let sf_lat = sf_sample.lat;
            let sf_lon = sf_sample.lon;

            // 2. Find times in the GoPro trace where the car passes exactly this S/F line
            let mut gopro_crossings = Vec::new();
            let mut in_zone = false;
            let mut local_min_dist = f64::MAX;
            let mut local_min_time = 0;

            for &(t, lat, lon) in gopro_gps {
                let dist = haversine(sf_lat, sf_lon, lat, lon);

                // If we get within 30 meters, we are crossing the line
                if dist < 10.0 {
                    in_zone = true;
                    if dist < local_min_dist {
                        local_min_dist = dist;
                        local_min_time = t;
                    }
                } else if in_zone {
                    // We left the 30m radius, record the closest moment as the crossing
                    gopro_crossings.push(local_min_time);
                    in_zone = false;
                    local_min_dist = f64::MAX;
                }
            }
            // Catch if we end exactly on the line
            if in_zone {
                gopro_crossings.push(local_min_time);
            }

            info!("Detected {} Start/Finish crossings in GoPro GPS track.", gopro_crossings.len());

            // 3. Find the offset that matches the highest number of laps between the two arrays
            if !gopro_crossings.is_empty() {
                let mut best_offset = 0;
                let mut max_matches = 0;
                let tolerance_ms = 3000; // 3 seconds tolerance for GPS polling inaccuracies

                for &g_time in &gopro_crossings {
                    for &(_lap_num, t_time) in &telem_laps {
                        let potential_offset = t_time - g_time;

                        // Count how many GoPro crossings align with telemetry crossings using this offset
                        let mut matches = 0;
                        for &g_check in &gopro_crossings {
                            let mapped_time = g_check + potential_offset;
                            if telem_laps.iter().any(|&(_, t)| (t - mapped_time).abs() <= tolerance_ms) {
                                matches += 1;
                            }
                        }

                        if matches > max_matches {
                            max_matches = matches;
                            best_offset = potential_offset;
                        }
                    }
                }

                if max_matches > 0 {
                    info!("Lap-based correlation successful! Found {} matching laps with offset {} ms", max_matches, best_offset);
                    return Some(best_offset);
                } else {
                    warn!("Lap-based correlation found crossings but couldn't match cadence. Falling back to distance least-squares...");
                }
            } else {
                warn!("No Start/Finish crossings detected in GoPro track. Falling back to distance least-squares...");
            }
        }
    } else {
        info!("No lap data found in telemetry. Falling back to distance least-squares...");
    }

    // Fallback Strategy: Distance least-squares matching
    auto_correlate_gps_fallback(gopro_gps, telemetry)
}

fn auto_correlate_gps_fallback(gopro_gps: &[(i64, f64, f64)], telemetry: &TelemetryLog) -> Option<i64> {
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

    debug!("Searching for offset via fallback...");

    let mut best_offset = 0;
    let mut min_error = f64::MAX;

    let telem_start = telem_dist.first().unwrap().0;
    let telem_end = telem_dist.last().unwrap().0;

    // We try offsets from -120000ms to 120000ms (2 minutes)
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

        if count > gopro_dist.len() / 4 { // Need at least 25% overlap
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
        info!("Auto-sync fallback found best offset {} ms with error {}", best_offset, min_error);
        Some(best_offset)
    }
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
