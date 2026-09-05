use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::TelemetryState;
use crate::trackmap::TrackMap;
use eframe::egui;
use tiny_skia::{Paint, PathBuilder, PixmapMut, Stroke, Transform};

pub struct GForceMeter;

impl GForceMeter {
    fn extract_options(el: &OverlayElement) -> (bool, bool, bool) {
        let mut invert_x = false;
        let mut invert_y = false;
        let mut swap_axes = false;

        if let Some(opts) = &el.options {
            if let Some(val) = opts.get("invert_x").and_then(|v| v.as_bool()) {
                invert_x = val;
            }
            if let Some(val) = opts.get("invert_y").and_then(|v| v.as_bool()) {
                invert_y = val;
            }
            if let Some(val) = opts.get("swap_axes").and_then(|v| v.as_bool()) {
                swap_axes = val;
            }
        }
        (invert_x, invert_y, swap_axes)
    }

    fn apply_axis_config(
        dx: f32,
        dy: f32,
        invert_x: bool,
        invert_y: bool,
        swap_axes: bool,
    ) -> (f32, f32) {
        let mut final_dx = dx;
        let mut final_dy = dy;

        if swap_axes {
            std::mem::swap(&mut final_dx, &mut final_dy);
        }

        if invert_x {
            final_dx = -final_dx;
        }
        if invert_y {
            final_dy = -final_dy;
        }

        (final_dx, final_dy)
    }
}

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

        let (invert_x, invert_y, swap_axes) = Self::extract_options(el);
        let (raw_dx, raw_dy) = common::get_gforce_dot(state.current_sample.as_ref(), radius);
        let (dx, dy) = Self::apply_axis_config(raw_dx, raw_dy, invert_x, invert_y, swap_axes);

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

        let (invert_x, invert_y, swap_axes) = Self::extract_options(el);
        let (raw_dx, raw_dy) = common::get_gforce_dot(state.current_sample.as_ref(), radius);
        let (dx, dy) = Self::apply_axis_config(raw_dx, raw_dy, invert_x, invert_y, swap_axes);

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

    fn custom_ui(&self, ui: &mut egui::Ui, el: &mut OverlayElement) {
        let (mut invert_x, mut invert_y, mut swap_axes) = Self::extract_options(el);
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label("  "); // Indent

            if ui.checkbox(&mut swap_axes, "Swap X/Y").changed() {
                changed = true;
            }
            if ui.checkbox(&mut invert_x, "Invert X").changed() {
                changed = true;
            }
            if ui.checkbox(&mut invert_y, "Invert Y").changed() {
                changed = true;
            }
        });

        if changed {
            let mut new_opts = match &el.options {
                Some(serde_json::Value::Object(map)) => map.clone(),
                _ => serde_json::Map::new(),
            };
            new_opts.insert("invert_x".to_string(), serde_json::Value::Bool(invert_x));
            new_opts.insert("invert_y".to_string(), serde_json::Value::Bool(invert_y));
            new_opts.insert("swap_axes".to_string(), serde_json::Value::Bool(swap_axes));
            el.options = Some(serde_json::Value::Object(new_opts));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    pub fn create_test_element() -> OverlayElement {
        let mut opts = serde_json::Map::new();
        opts.insert("invert_x".to_string(), serde_json::Value::Bool(false));
        opts.insert("invert_y".to_string(), serde_json::Value::Bool(false));
        opts.insert("swap_axes".to_string(), serde_json::Value::Bool(false));
        OverlayElement {
            enabled: true,
            kind: crate::project::OverlayKind::GForceMeter,
            x: 0.5,
            y: 0.5,
            scale: 1.0,
            options: Some(serde_json::Value::Object(opts)),
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
    fn test_extract_options() {
        let mut el = create_test_element();
        let (inv_x, inv_y, swap) = GForceMeter::extract_options(&el);
        assert!(!inv_x);
        assert!(!inv_y);
        assert!(!swap);

        let mut opts = serde_json::Map::new();
        opts.insert("invert_x".to_string(), serde_json::Value::Bool(true));
        opts.insert("swap_axes".to_string(), serde_json::Value::Bool(true));
        el.options = Some(serde_json::Value::Object(opts));

        let (inv_x, inv_y, swap) = GForceMeter::extract_options(&el);
        assert!(inv_x);
        assert!(!inv_y);
        assert!(swap);
    }

    #[test]
    fn test_apply_axis_config() {
        // Base case
        let (dx, dy) = GForceMeter::apply_axis_config(10.0, 5.0, false, false, false);
        assert_eq!(dx, 10.0);
        assert_eq!(dy, 5.0);

        // Swap axes
        let (dx, dy) = GForceMeter::apply_axis_config(10.0, 5.0, false, false, true);
        assert_eq!(dx, 5.0);
        assert_eq!(dy, 10.0);

        // Invert X
        let (dx, dy) = GForceMeter::apply_axis_config(10.0, 5.0, true, false, false);
        assert_eq!(dx, -10.0);
        assert_eq!(dy, 5.0);

        // Invert Y
        let (dx, dy) = GForceMeter::apply_axis_config(10.0, 5.0, false, true, false);
        assert_eq!(dx, 10.0);
        assert_eq!(dy, -5.0);

        // Swap and Invert X (which was originally Y)
        let (dx, dy) = GForceMeter::apply_axis_config(10.0, 5.0, true, false, true);
        assert_eq!(dx, -5.0);
        assert_eq!(dy, 10.0);
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
