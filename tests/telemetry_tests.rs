use std::io::Write;
use track_overlay::telemetry::TelemetryLog;
use tempfile::NamedTempFile;

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

    let log = TelemetryLog::load_csv(file.path()).unwrap();
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
}
