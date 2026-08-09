use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::TelemetryState;
use crate::trackmap::TrackMap;
use eframe::egui;
use tiny_skia::{Paint, PathBuilder, PixmapMut, Stroke, Transform};

pub struct GForceMeter;

impl OverlayImpl for GForceMeter {
    fn render_ui(
        &self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        el: &OverlayElement,
        state: &TelemetryState,
        _trackmap: Option<&TrackMap>,
    ) {
        let painter = ui.painter_at(rect);
        let center = egui::pos2(
            rect.left() + el.x * rect.width(),
            rect.top() + el.y * rect.height(),
        );

        let radius = 40.0 * el.scale;
        painter.circle_stroke(
            center,
            radius,
            egui::Stroke::new(2.0 * el.scale, egui::Color32::WHITE),
        );

        let (dx, dy) = common::get_gforce_dot(state.current_sample.as_ref(), radius);
        let dot_pos = center + egui::vec2(dx, dy);
        painter.circle_filled(dot_pos, 5.0 * el.scale, egui::Color32::RED);
    }

    fn render_skia(
        &self,
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        state: &TelemetryState,
        _trackmap: Option<&TrackMap>,
        _font_opt: Option<&rusttype::Font>,
    ) {
        let width = pixmap.width() as f32;
        let height = pixmap.height() as f32;
        let res_scale = height / 720.0;
        let center_x = el.x * width;
        let center_y = el.y * height;

        let radius = 40.0 * el.scale * res_scale;

        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, 255);
        paint.anti_alias = true;

        let stroke = Stroke {
            width: 2.0 * el.scale * res_scale,
            ..Default::default()
        };

        if let Some(path) = PathBuilder::from_circle(center_x, center_y, radius) {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }

        let (dx, dy) = common::get_gforce_dot(state.current_sample.as_ref(), radius);
        let mut paint_red = Paint::default();
        paint_red.set_color_rgba8(255, 0, 0, 255);
        paint_red.anti_alias = true;

        if let Some(path) =
            PathBuilder::from_circle(center_x + dx, center_y + dy, 5.0 * el.scale * res_scale)
        {
            pixmap.fill_path(
                &path,
                &paint_red,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    pub fn create_test_element() -> OverlayElement {
        OverlayElement {
            enabled: true,
            kind: crate::project::OverlayKind::GForceMeter,
            x: 0.5,
            y: 0.5,
            scale: 1.0,
            options: None,
        }
    }

    pub fn create_test_sample() -> crate::telemetry::TelemetrySample {
        crate::telemetry::TelemetrySample {
            time_ms: 1000,
            speed_kph: 120.5,
            lat: 10.0,
            lon: 20.0,
            accel_lat_g: 1.5,
            accel_lon_g: -0.5,
            lap_number: Some(2),
            lap_time_ms: Some(150500),
            throttle_pct: 75.0,
            engine_speed_rpm: 6200.0,
            session_distance_m: 0.0,
            lap_distance_m: 0.0,
        }
    }

    #[test]
    fn test_gforce_meter_render_skia() {
        let el = create_test_element();
        let mut data = vec![0; 800 * 600 * 4];
        let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

        let meter = GForceMeter;
        meter.render_skia(
            &mut pixmap,
            &el,
            &crate::telemetry::TelemetryState {
                current_sample: None,
                previous_laps: vec![],
                best_lap: None,
                projection_ms: None,
            },
            None,
            None,
        );

        let sample = create_test_sample();
        meter.render_skia(
            &mut pixmap,
            &el,
            &crate::telemetry::TelemetryState {
                current_sample: Some(sample.clone()),
                previous_laps: vec![],
                best_lap: None,
                projection_ms: None,
            },
            None,
            None,
        );
    }

    #[test]
    fn test_gforce_meter_render_ui() {
        let el = create_test_element();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show_inside(ctx, |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

                let meter = GForceMeter;
                meter.render_ui(
                    ui,
                    rect,
                    &el,
                    &crate::telemetry::TelemetryState {
                        current_sample: None,
                        previous_laps: vec![],
                        best_lap: None,
                        projection_ms: None,
                    },
                    None,
                );

                let sample = create_test_sample();
                meter.render_ui(
                    ui,
                    rect,
                    &el,
                    &crate::telemetry::TelemetryState {
                        current_sample: Some(sample.clone()),
                        previous_laps: vec![],
                        best_lap: None,
                        projection_ms: None,
                    },
                    None,
                );
            });
        });
    }
}
