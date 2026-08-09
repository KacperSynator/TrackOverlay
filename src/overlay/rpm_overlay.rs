use crate::overlay::OverlayImpl;
use crate::project::OverlayElement;
use crate::telemetry::TelemetryState;
use crate::trackmap::TrackMap;
use eframe::egui;
use tiny_skia::{Paint, PathBuilder, PixmapMut, Rect, Stroke, Transform};

pub struct RpmOverlay;

impl OverlayImpl for RpmOverlay {
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

        let mut rpm_max = 6500.0;
        let mut rpm_redline = 6000.0;

        if let Some(opts) = &el.options {
            if let Some(max_val) = opts.get("rpm_max").and_then(|v| v.as_f64()) {
                rpm_max = max_val as f32;
            }
            if let Some(rl_val) = opts.get("rpm_redline").and_then(|v| v.as_f64()) {
                rpm_redline = rl_val as f32;
            }
        }

        let rpm = state
            .current_sample
            .as_ref()
            .map_or(0.0, |s| s.engine_speed_rpm)
            .clamp(0.0, rpm_max);

        let max_width = 300.0 * el.scale;
        let height = 20.0 * el.scale;

        let bg_rect = egui::Rect::from_center_size(center, egui::vec2(max_width, height));
        painter.rect_filled(bg_rect, 2.0, egui::Color32::from_black_alpha(150));
        painter.rect_stroke(
            bg_rect,
            2.0,
            egui::Stroke::new(1.0_f32, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );

        if rpm > 0.0 {
            let fill_width = (rpm / rpm_max) * max_width;
            let mut fill_rect = bg_rect;
            fill_rect.set_right(bg_rect.left() + fill_width);

            if rpm <= rpm_redline {
                // Entirely below redline, draw white
                painter.rect_filled(fill_rect, 2.0, egui::Color32::WHITE);
            } else {
                // Above redline: draw white part up to redline, then red part
                let redline_width = (rpm_redline / rpm_max) * max_width;

                let mut white_rect = fill_rect;
                white_rect.set_right(bg_rect.left() + redline_width);
                painter.rect_filled(white_rect, 2.0, egui::Color32::WHITE);

                let mut red_rect = fill_rect;
                red_rect.set_left(bg_rect.left() + redline_width);
                painter.rect_filled(red_rect, 2.0, egui::Color32::RED);
            }
        }
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

        let mut rpm_max = 6500.0;
        let mut rpm_redline = 6000.0;

        if let Some(opts) = &el.options {
            if let Some(max_val) = opts.get("rpm_max").and_then(|v| v.as_f64()) {
                rpm_max = max_val as f32;
            }
            if let Some(rl_val) = opts.get("rpm_redline").and_then(|v| v.as_f64()) {
                rpm_redline = rl_val as f32;
            }
        }

        let rpm = state
            .current_sample
            .as_ref()
            .map_or(0.0, |s| s.engine_speed_rpm)
            .clamp(0.0, rpm_max);

        let max_w = 300.0 * el.scale * res_scale;
        let h = 20.0 * el.scale * res_scale;

        let left = center_x - max_w / 2.0;
        let top = center_y - h / 2.0;

        let bg_rect = Rect::from_xywh(left, top, max_w, h).unwrap();

        let mut paint_bg = Paint::default();
        paint_bg.set_color_rgba8(0, 0, 0, 150);
        pixmap.fill_rect(bg_rect, &paint_bg, Transform::identity(), None);

        let mut paint_stroke = Paint::default();
        paint_stroke.set_color_rgba8(255, 255, 255, 255);
        let stroke = Stroke {
            width: 1.0_f32,
            ..Default::default()
        };

        let mut pb = PathBuilder::new();
        pb.move_to(left, top);
        pb.line_to(left + max_w, top);
        pb.line_to(left + max_w, top + h);
        pb.line_to(left, top + h);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint_stroke, &stroke, Transform::identity(), None);
        }

        if rpm > 0.0 {
            let fill_w = (rpm / rpm_max) * max_w;

            if rpm <= rpm_redline {
                if let Some(fill_rect) = Rect::from_xywh(left, top, fill_w, h) {
                    let mut paint_fill = Paint::default();
                    paint_fill.set_color_rgba8(255, 255, 255, 255);
                    pixmap.fill_rect(fill_rect, &paint_fill, Transform::identity(), None);
                }
            } else {
                let redline_w = (rpm_redline / rpm_max) * max_w;

                if let Some(white_rect) = Rect::from_xywh(left, top, redline_w, h) {
                    let mut paint_white = Paint::default();
                    paint_white.set_color_rgba8(255, 255, 255, 255);
                    pixmap.fill_rect(white_rect, &paint_white, Transform::identity(), None);
                }

                if let Some(red_rect) =
                    Rect::from_xywh(left + redline_w, top, fill_w - redline_w, h)
                {
                    let mut paint_red = Paint::default();
                    paint_red.set_color_rgba8(255, 0, 0, 255);
                    pixmap.fill_rect(red_rect, &paint_red, Transform::identity(), None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    fn create_test_element() -> OverlayElement {
        let mut opts = serde_json::Map::new();
        opts.insert(
            "rpm_max".to_string(),
            serde_json::Value::Number(7000.into()),
        );
        opts.insert(
            "rpm_redline".to_string(),
            serde_json::Value::Number(6500.into()),
        );

        OverlayElement {
            enabled: true,
            kind: crate::project::OverlayKind::RpmOverlay,
            x: 0.5,
            y: 0.5,
            scale: 1.0,
            options: Some(serde_json::Value::Object(opts)),
        }
    }

    #[test]
    fn test_rpm_overlay_render_skia() {
        let el = create_test_element();
        let mut data = vec![0; 800 * 600 * 4];
        let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

        let bar = RpmOverlay;

        // Render without state
        bar.render_skia(
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

        // Render with state (below redline)
        let mut sample = crate::overlay::common::create_test_sample();
        sample.engine_speed_rpm = 3000.0;
        bar.render_skia(
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

        // Render with state (above redline)
        sample.engine_speed_rpm = 6800.0;
        bar.render_skia(
            &mut pixmap,
            &el,
            &crate::telemetry::TelemetryState {
                current_sample: Some(sample),
                previous_laps: vec![],
                best_lap: None,
                projection_ms: None,
            },
            None,
            None,
        );
    }

    #[test]
    fn test_rpm_overlay_render_ui() {
        let el = create_test_element();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show_inside(ctx, |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

                let bar = RpmOverlay;

                // Render without state
                bar.render_ui(
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

                // Render with state (below redline)
                let mut sample = crate::overlay::common::create_test_sample();
                sample.engine_speed_rpm = 3000.0;
                bar.render_ui(
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

                // Render with state (above redline)
                sample.engine_speed_rpm = 6800.0;
                bar.render_ui(
                    ui,
                    rect,
                    &el,
                    &crate::telemetry::TelemetryState {
                        current_sample: Some(sample),
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
