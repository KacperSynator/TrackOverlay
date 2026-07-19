use eframe::egui;
use crate::project::{OverlayElement, OverlayKind};
use crate::telemetry::TelemetrySample;

pub fn render_overlay(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    elements: &mut [OverlayElement],
    sample: Option<&TelemetrySample>,
    _is_dragging: bool,
) {
    let painter = ui.painter_at(rect);

    // Draw elements
    for el in elements.iter_mut() {
        let center = egui::pos2(
            rect.left() + el.x * rect.width(),
            rect.top() + el.y * rect.height()
        );

        match el.kind {
            OverlayKind::SpeedReadout => {
                let speed = sample.map_or(0.0, |s| s.speed_kph);
                let text = format!("{:.0} km/h", speed);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(32.0 * el.scale),
                    egui::Color32::WHITE,
                );
            }
            OverlayKind::GForceMeter => {
                let lat_g = sample.map_or(0.0, |s| s.accel_lat_g);
                let lon_g = sample.map_or(0.0, |s| s.accel_lon_g);

                let radius = 40.0 * el.scale;
                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                );

                // Dot representing g-force
                let dot_pos = center + egui::vec2(lat_g * radius, -lon_g * radius);
                painter.circle_filled(dot_pos, 5.0 * el.scale, egui::Color32::RED);
            }
            OverlayKind::LapTimer => {
                let time_ms = sample.and_then(|s| s.lap_time_ms).unwrap_or(0);
                let seconds = time_ms as f64 / 1000.0;
                let mins = (seconds / 60.0).floor() as i32;
                let secs = seconds % 60.0;
                let text = format!("{:02}:{:05.2}", mins, secs);

                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(24.0 * el.scale),
                    egui::Color32::YELLOW,
                );
            }
        }
    }
}
