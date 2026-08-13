use criterion::{Criterion, criterion_group, criterion_main};
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
        engine_speed_rpm: 0.0,
        session_distance_m: 0.0,
        lap_distance_m: 0.0,
    }
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

/// 100-lap synthetic circular track, used to stress-test many laps at once.
fn generate_lap_data(num_laps: u32) -> (Vec<(i64, f64, f64)>, TelemetryLog) {
    let center_lat = 53.0;
    let center_lon = 18.0;
    let radius = 0.001;

    let mut telem_samples = Vec::new();
    let mut time = 0;
    for lap in 1..=num_laps {
        let points = generate_circular_track(center_lat, center_lon, radius, 100, time, 100);
        for p in points {
            let lap_num = if p.0 == time { Some(lap) } else { None };
            telem_samples.push(create_sample(p.0, p.1, p.2, lap_num));
        }
        time += 10000; // 100 points * 100ms
    }
    let telemetry_data = TelemetryLog {
        samples: telem_samples,
        start_time_utc: None,
        parsed_speed_source: track_overlay::project::SpeedSource::Auto,
    };

    let gopro_offset = -5000;
    let mut gopro_data = Vec::new();
    let mut g_time = -gopro_offset;
    for _lap in 1..=num_laps {
        let points = generate_circular_track(center_lat, center_lon, radius, 100, g_time, 100);
        for p in points {
            gopro_data.push((p.0, p.1, p.2));
        }
        g_time += 10000;
    }

    (gopro_data, telemetry_data)
}

/// 12 minutes of a Lissajous-curve track at 10Hz, with a fixed GoPro offset,
/// and `lap_number: None` throughout to exercise the fallback path.
fn generate_lissajous_data() -> (Vec<(i64, f64, f64)>, TelemetryLog) {
    let duration_ms = 12 * 60 * 1000;
    let hz = 10;
    let dt = 1000 / hz;

    let mut samples = Vec::new();
    for time_ms in (0..duration_ms).step_by(dt as usize) {
        let t = time_ms as f64 / 1000.0;
        let lat = 45.0 + (t / 100.0).sin() * 0.01;
        let lon = 10.0 + (t / 50.0).cos() * 0.01;

        samples.push(TelemetrySample {
            time_ms,
            speed_kph: 100.0,
            lat,
            lon,
            accel_lat_g: 0.0,
            accel_lon_g: 0.0,
            lap_number: None, // Ensure fallback is triggered
            lap_time_ms: None,
            throttle_pct: 100.0,
            engine_speed_rpm: 0.0,
            session_distance_m: 0.0,
            lap_distance_m: 0.0,
        });
    }

    let telemetry = TelemetryLog {
        samples,
        start_time_utc: None,
        parsed_speed_source: track_overlay::project::SpeedSource::Auto,
    };

    let target_offset = 54300; // 54.3 seconds
    let mut gopro_gps = Vec::new();
    for time_ms in (0..duration_ms).step_by(dt as usize) {
        let t = (time_ms + target_offset) as f64 / 1000.0;
        let lat = 45.0 + (t / 100.0).sin() * 0.01;
        let lon = 10.0 + (t / 50.0).cos() * 0.01;
        gopro_gps.push((time_ms, lat, lon));
    }

    (gopro_gps, telemetry)
}

fn bench_sync(c: &mut Criterion) {
    let max_auto_sync_offset_ms = 300000;

    let (gopro_data, telemetry_data) = generate_lap_data(100);
    c.bench_function("auto_correlate_gps_100_laps", |b| {
        b.iter(|| auto_correlate_gps(&gopro_data, &telemetry_data, max_auto_sync_offset_ms))
    });

    let (gopro_gps, telemetry) = generate_lissajous_data();
    c.bench_function("auto_correlate_gps_lissajous_12min", |b| {
        b.iter(|| auto_correlate_gps(&gopro_gps, &telemetry, max_auto_sync_offset_ms))
    });
}

criterion_group!(benches, bench_sync);
criterion_main!(benches);
