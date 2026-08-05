use track_overlay::overlay::advanced_lap_timer::AdvancedLapTimer;
use track_overlay::overlay::OverlayImpl;
use track_overlay::project::{OverlayElement, OverlayKind};
use track_overlay::telemetry::{LapStat, TelemetrySample, TelemetryState};
use eframe::egui;
use tiny_skia::PixmapMut;

fn create_test_state() -> TelemetryState {
    let sample = TelemetrySample {
        time_ms: 1000,
        speed_kph: 120.5,
        lat: 10.0,
        lon: 20.0,
        accel_lat_g: 1.5,
        accel_lon_g: -0.5,
        lap_number: Some(2),
        lap_time_ms: Some(150500),
        throttle_pct: 75.0,
        session_distance_m: 1000.0,
        lap_distance_m: 500.0,
    };

    let best_lap = LapStat {
        lap_number: 1,
        start_time_ms: 0,
        end_time_ms: 90000,
        duration_ms: 90000,
        total_distance_m: 2000.0,
    };

    let previous_laps = vec![best_lap.clone()];

    TelemetryState {
        current_sample: Some(sample),
        previous_laps,
        best_lap: Some(best_lap),
        projection_ms: Some(-1500),
    }
}

#[test]
fn test_advanced_lap_timer_render_skia() {
    let timer = AdvancedLapTimer;
    let mut data = vec![0; 800 * 600 * 4];
    let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

    let el = OverlayElement {
        enabled: true,
        kind: OverlayKind::AdvancedLapTimer,
        x: 0.5,
        y: 0.5,
        scale: 1.0,
    };

    let state = create_test_state();

    // With font
    timer.render_skia(&mut pixmap, &el, &state, None);

    // Ensure it doesn't panic on missing fields
    let empty_state = TelemetryState {
        current_sample: None,
        previous_laps: vec![],
        best_lap: None,
        projection_ms: None,
    };
    timer.render_skia(&mut pixmap, &el, &empty_state, None);
}

#[test]
fn test_advanced_lap_timer_render_ui() {
    let timer = AdvancedLapTimer;

    let el = OverlayElement {
        enabled: true,
        kind: OverlayKind::AdvancedLapTimer,
        x: 0.5,
        y: 0.5,
        scale: 1.0,
    };

    let state = create_test_state();
    let empty_state = TelemetryState {
        current_sample: None,
        previous_laps: vec![],
        best_lap: None,
        projection_ms: None,
    };

    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show_inside(ctx, |ui| {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
            timer.render_ui(ui, rect, &el, &state, None);
            timer.render_ui(ui, rect, &el, &empty_state, None);
        });
    });
}
