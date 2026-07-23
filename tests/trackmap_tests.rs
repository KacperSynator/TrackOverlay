use track_overlay::telemetry::{TelemetryLog, TelemetrySample};
use track_overlay::trackmap::TrackMap;

#[test]
fn test_trackmap_projection() {
    // Square around equator (0,0)
    let samples = vec![
        TelemetrySample {
            time_ms: 0,
            speed_kph: 0.0,
            lat: 0.0,
            lon: 0.0,
            accel_lat_g: 0.0,
            accel_lon_g: 0.0,
            lap_number: Some(1),
            lap_time_ms: Some(0),
            throttle_pct: 0.0,
        },
        TelemetrySample {
            time_ms: 1000,
            speed_kph: 0.0,
            lat: 1.0,
            lon: 0.0,
            accel_lat_g: 0.0,
            accel_lon_g: 0.0,
            lap_number: Some(1),
            lap_time_ms: Some(1000),
            throttle_pct: 0.0,
        },
        TelemetrySample {
            time_ms: 2000,
            speed_kph: 0.0,
            lat: 1.0,
            lon: 1.0,
            accel_lat_g: 0.0,
            accel_lon_g: 0.0,
            lap_number: Some(1),
            lap_time_ms: Some(2000),
            throttle_pct: 0.0,
        },
        TelemetrySample {
            time_ms: 3000,
            speed_kph: 0.0,
            lat: 0.0,
            lon: 1.0,
            accel_lat_g: 0.0,
            accel_lon_g: 0.0,
            lap_number: Some(1),
            lap_time_ms: Some(3000),
            throttle_pct: 0.0,
        },
        TelemetrySample {
            time_ms: 4000,
            speed_kph: 0.0,
            lat: 0.0,
            lon: 0.0,
            accel_lat_g: 0.0,
            accel_lon_g: 0.0,
            lap_number: Some(2),
            lap_time_ms: Some(0),
            throttle_pct: 0.0,
        },
    ];

    let log = TelemetryLog {
        samples,
        start_time_utc: None,
    };
    let laps = log.extract_laps();

    // We need at least 10 samples to bypass the early exit safeguard
    let mut log_padded = log.clone();
    for _i in 0..10 {
        log_padded.samples.push(log.samples[0].clone());
    }

    let track = TrackMap::from_telemetry(&log_padded, &laps).unwrap();

    // Test that the interpolated points lie within the bounding box
    let p0 = track.point_at_time(0).unwrap();
    let p1 = track.point_at_time(2000).unwrap();

    assert!(p0.0 >= 0.0 && p0.0 <= 1.0);
    assert!(p0.1 >= 0.0 && p0.1 <= 1.0);
    assert!(p1.0 >= 0.0 && p1.0 <= 1.0);
    assert!(p1.1 >= 0.0 && p1.1 <= 1.0);
}
