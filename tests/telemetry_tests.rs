use std::io::Write;
use tempfile::NamedTempFile;
use track_overlay::telemetry::TelemetryLog;

#[test]
fn test_telemetry_parsing_and_interpolation() {
    let mut file = NamedTempFile::new().unwrap();
    let csv_content = r#"# RaceRender Data: TrackAddict 4.8.2 on Android 14 [samsung SM-S928B] (Mode: 0)
# Vehicle: Hyundai
# Vehicle Tune: OSID's: OSMQKM_O_T0B, CVN's: 5081166E
# End Point: 53.021976, 18.549740  @ -1.00 deg
# GPS: Android; Mode: Android
# OBD Mode: BT (OBDLink MX+ 64534); ID: "ELM327 v1.4b"
# OBD Settings: AP1;AF1;RPR0
# User Settings: SL1;U1;AS1;LT0/1;EC0;VC-1;VQ3;VS0;VSOIS0;VIF1
# Device Free Space: 374544 MB
"Time","UTC Time","Lap","Predicted Lap Time","Predicted vs Best Lap","GPS_Update","GPS_Delay","Latitude","Longitude","Altitude (m)","Altitude (ft)","Speed (Km/h)","Heading","Accuracy (m)","Accel X","Accel Y","Accel Z","Brake (calculated)","Barometric Pressure (kPa)","Pressure Altitude (m)","OBD_Update","Engine Speed (RPM) *OBD","Vehicle Speed (km/h) *OBD","Throttle Position (%) *OBD","Engine Coolant Temp (C) *OBD","Intake Air Temp (C) *OBD","Intake Manifold Pressure (kPa) *OBD"
0.000,1727597624.000,0,0,0,1,0.000,53.0229789,18.5481845,76.3,250,26.5,116.9,3.8,-0.30,0.30,-0.17,0,101.91,-48.7,1,4162.000,36.000,27.059,90.000,34.000,78.000
0.100,1727597624.100,0,0,0,0,0.000,53.0229789,18.5481845,76.3,250,27.5,116.9,3.8,-0.32,0.30,-0.20,0,101.91,-48.7,0,4162.000,36.000,27.059,90.000,34.000,78.000
"#;
    file.write_all(csv_content.as_bytes()).unwrap();

    let log =
        TelemetryLog::load_csv(file.path(), track_overlay::project::SpeedSource::Auto).unwrap();
    assert_eq!(log.samples.len(), 2);

    let s1 = &log.samples[0];
    assert_eq!(s1.time_ms, 0);
    assert_eq!(s1.speed_kph, 26.5);
    assert_eq!(s1.lap_number, Some(0));
    assert_eq!(s1.lap_time_ms, Some(0));

    let s2 = &log.samples[1];
    assert_eq!(s2.time_ms, 100);
    assert_eq!(s2.speed_kph, 27.5);
    assert_eq!(s2.lap_time_ms, Some(100));

    // Test interpolation
    let interp = log.sample_at(50).unwrap();
    assert_eq!(interp.time_ms, 50);
    assert_eq!(interp.speed_kph, 27.0);
    assert_eq!(interp.lap_time_ms, Some(50));

    // Test out of bounds (before start)
    let early = log.sample_at(-1000).unwrap();
    assert_eq!(early.time_ms, 0);
    assert_eq!(early.speed_kph, 26.5);
    assert_eq!(early.lap_time_ms, Some(0));

    // Test out of bounds (after end)
    let late = log.sample_at(10000).unwrap();
    assert_eq!(late.time_ms, 100);
    assert_eq!(late.speed_kph, 27.5);
    assert_eq!(late.lap_time_ms, Some(100));
}

#[test]
fn test_telemetry_view_truncation_and_laps() {
    use track_overlay::telemetry::TelemetryView;

    let mut file = NamedTempFile::new().unwrap();
    let csv_content = r#"# Dummy Header
"Time","UTC Time","Lap","Predicted Lap Time","Predicted vs Best Lap","GPS_Update","GPS_Delay","Latitude","Longitude","Altitude (m)","Altitude (ft)","Speed (Km/h)","Heading","Accuracy (m)","Accel X","Accel Y","Accel Z","Brake (calculated)","Barometric Pressure (kPa)","Pressure Altitude (m)","OBD_Update","Engine Speed (RPM) *OBD","Vehicle Speed (km/h) *OBD","Throttle Position (%) *OBD","Engine Coolant Temp (C) *OBD","Intake Air Temp (C) *OBD","Intake Manifold Pressure (kPa) *OBD"
0.000,1000.000,0,0,0,1,0.0,50.0,20.0,0,0,10.0,0,0,0,0,0,0,100.0,0,1,1000,10,10,90,30,100
1.000,1001.000,0,0,0,1,0.0,50.0,20.0,0,0,20.0,0,0,0,0,0,0,100.0,0,1,1000,10,10,90,30,100
2.000,1002.000,1,0,0,1,0.0,50.0,20.0,0,0,30.0,0,0,0,0,0,0,100.0,0,1,1000,10,10,90,30,100
3.000,1003.000,1,0,0,1,0.0,50.0,20.0,0,0,40.0,0,0,0,0,0,0,100.0,0,1,1000,10,10,90,30,100
4.000,1004.000,2,0,0,1,0.0,50.0,20.0,0,0,50.0,0,0,0,0,0,0,100.0,0,1,1000,10,10,90,30,100
5.000,1005.000,2,0,0,1,0.0,50.0,20.0,0,0,60.0,0,0,0,0,0,0,100.0,0,1,1000,10,10,90,30,100
"#;
    file.write_all(csv_content.as_bytes()).unwrap();
    let log =
        TelemetryLog::load_csv(file.path(), track_overlay::project::SpeedSource::Auto).unwrap();

    assert_eq!(log.samples.len(), 6);
    assert_eq!(log.samples[2].time_ms, 2000); // Lap 1 starts

    // 1. Unbounded View
    let view_all = TelemetryView::new(&log, None, None, 0);
    assert_eq!(view_all.samples.len(), 6);
    let laps_all = view_all.extract_laps();
    assert_eq!(laps_all.len(), 3);
    assert_eq!(laps_all[0].0, 0); // Lap 0
    assert_eq!(laps_all[1].0, 1); // Lap 1
    assert_eq!(laps_all[2].0, 2); // Lap 2

    // 2. Truncate Start
    // start_ms = 2500 -> start_idx = 3 (time_ms=3000)
    // The first sample is Lap 1. `lap_offset` = 1 - 1 = 0.
    let view_cut_start = TelemetryView::new(&log, Some(2500), None, 0);
    assert_eq!(view_cut_start.samples.len(), 3);
    assert_eq!(view_cut_start.samples[0].time_ms, 3000);
    let laps_cut = view_cut_start.extract_laps();
    assert_eq!(laps_cut[0].0, 1);
    assert_eq!(laps_cut[1].0, 2);

    // 3. Sync Offset interaction
    // sync_offset = 1000, start_ms = 1500
    // s.time_ms=2000 -> sync_time=1000
    // s.time_ms=3000 -> sync_time=2000
    // start_idx = 3 (time_ms=3000)
    let view_offset = TelemetryView::new(&log, Some(1500), None, 1000);
    assert_eq!(view_offset.samples.len(), 3);
    assert_eq!(view_offset.samples[0].time_ms, 3000);

    // 4. Test State Best Lap
    let view_best = TelemetryView::new(&log, Some(1500), None, 0);
    let _state = view_best.get_state(6000);
}
