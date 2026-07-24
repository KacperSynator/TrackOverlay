use track_overlay::sync::auto_correlate_gps;
use track_overlay::telemetry::{TelemetryLog, TelemetrySample};

fn create_sample(time_ms: i64, lat: f64, lon: f64, lap_number: Option<u32>) -> TelemetrySample {
    TelemetrySample {
        time_ms,
        speed_kph: 0.0,
        lat,
        lon,
        accel_lat_g: 0.0,
        accel_lon_g: 0.0,
        lap_number,
        lap_time_ms: None,
        throttle_pct: 0.0,
    }
}

#[test]
fn test_auto_correlate_empty_data() {
    let empty_gopro: Vec<(i64, f64, f64)> = vec![];
    let empty_telemetry = TelemetryLog {
        samples: vec![],
        start_time_utc: None,
    };
    assert_eq!(auto_correlate_gps(&empty_gopro, &empty_telemetry), None);
}

fn generate_circular_track(
    center_lat: f64,
    center_lon: f64,
    radius_deg: f64,
    num_points: usize,
    start_time: i64,
    time_step: i64,
) -> Vec<(i64, f64, f64)> {
    let mut points = Vec::new();
    for i in 0..num_points {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (num_points as f64);
        let lat = center_lat + radius_deg * angle.sin();
        let lon = center_lon + radius_deg * angle.cos();
        points.push((start_time + (i as i64) * time_step, lat, lon));
    }
    points
}

#[test]
fn test_auto_correlate_lap_based() {
    let center_lat = 53.0;
    let center_lon = 18.0;
    let radius = 0.001;
    let mut telem_samples = Vec::new();
    let mut time = 0;
    for lap in 1..=3 {
        let points = generate_circular_track(center_lat, center_lon, radius, 100, time, 100);
        for p in points {
            let lap_num = if p.0 == time { Some(lap) } else { None };
            telem_samples.push(create_sample(p.0, p.1, p.2, lap_num));
        }
        time += 10000;
    }
    let telemetry_data = TelemetryLog {
        samples: telem_samples,
        start_time_utc: None,
    };
    let gopro_offset = -5000;
    let mut gopro_data = Vec::new();
    let mut g_time = -gopro_offset;
    for _lap in 1..=3 {
        let points = generate_circular_track(center_lat, center_lon, radius, 100, g_time, 100);
        for p in points {
            gopro_data.push((p.0, p.1, p.2));
        }
        g_time += 10000;
    }
    let offset = auto_correlate_gps(&gopro_data, &telemetry_data);
    let found_offset = offset.unwrap();
    assert!((found_offset - gopro_offset).abs() <= 500);
}

#[test]
fn test_auto_correlate_distance_fallback() {
    let expected_offset_zero = 0;
    let mut telem_samples = Vec::new();
    let mut current_lat = 53.0;
    for t in (0..20000).step_by(100) {
        let velocity = if t < 10000 {
            t as f64 / 10000.0
        } else {
            (20000 - t) as f64 / 10000.0
        };
        current_lat += velocity * 0.00001;
        telem_samples.push(create_sample(t, current_lat, 18.0, None));
    }
    let telemetry_data = TelemetryLog {
        samples: telem_samples,
        start_time_utc: None,
    };
    let mut gopro_data_zero = Vec::new();
    for g_time in (0..20000).step_by(100) {
        let sample = telemetry_data
            .samples
            .iter()
            .find(|s| s.time_ms == g_time)
            .unwrap();
        gopro_data_zero.push((g_time, sample.lat, sample.lon));
    }
    let offset = auto_correlate_gps(&gopro_data_zero, &telemetry_data);
    assert!(offset.is_some());
    assert!((offset.unwrap() - expected_offset_zero).abs() <= 100);
}

#[test]
fn test_auto_correlate_failure() {
    // Generate tracks that are entirely mismatched and very short, such that overlap threshold (< 25%) fails

    let mut telem_samples = Vec::new();
    let mut current_lat = 53.0;
    for t in (0..1000).step_by(100) {
        current_lat += 0.00001;
        telem_samples.push(create_sample(t, current_lat, 18.0, None));
    }
    let telemetry_data = TelemetryLog {
        samples: telem_samples,
        start_time_utc: None,
    };

    // Make gopro data very long but physically un-correlatable, actually if we make it such that it doesn't even
    // overlap in time it will fail. The fallback requires at least 25% overlap in count.
    // So if gopro_dist length is 200 (20 seconds), 25% is 50. But telemetry is only 10 samples (1 second).
    // So overlap count can be at most 10.
    // 10 is NOT > 200 / 4 (50).
    // Thus it will fail gracefully.

    let mut gopro_data = Vec::new();
    let mut g_lat = -30.0;
    for g_time in (0..20000).step_by(100) {
        g_lat -= 0.0001;
        gopro_data.push((g_time, g_lat, 100.0));
    }

    let offset = auto_correlate_gps(&gopro_data, &telemetry_data);
    assert_eq!(
        offset, None,
        "Expected correlation to fail for completely unmatched tracks"
    );
}
