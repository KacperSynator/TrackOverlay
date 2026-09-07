use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::TelemetryState;
use crate::trackmap::TrackMap;
use eframe::egui;
use serde::{Deserialize, Serialize};
use tiny_skia::{Paint, PathBuilder, PixmapMut, Rect, Stroke, Transform};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleBarConfig {
    #[serde(default = "default_show_brake")]
    pub show_brake: bool,
}

fn default_show_brake() -> bool {
    true
}

impl Default for ThrottleBarConfig {
    fn default() -> Self {
        Self { show_brake: true }
    }
}

fn draw_brake_ui(painter: egui::Painter, bg_rect: egui::Rect, scale: f32, state: &TelemetryState) {
    let brake_active = common::get_brake_active(state.current_sample.as_ref());
    let brake_height = 15.0 * scale;
    // Draw a small red box directly on top of the throttle bar
    let mut brake_rect = bg_rect;
    brake_rect.set_bottom(bg_rect.top());
    brake_rect.set_top(brake_rect.bottom() - brake_height);
    // Move it slightly up so it's disjoint or touching the top
    brake_rect = brake_rect.translate(egui::vec2(0.0, -2.0 * scale));

    painter.rect_filled(brake_rect, 2.0, egui::Color32::from_black_alpha(150));
    painter.rect_stroke(
        brake_rect,
        2.0,
        egui::Stroke::new(1.0_f32, egui::Color32::WHITE),
        egui::StrokeKind::Inside,
    );

    if brake_active {
        painter.rect_filled(brake_rect, 2.0, egui::Color32::RED);
    }
}

fn draw_brake_skia(
    pixmap: &mut PixmapMut,
    left: f32,
    top: f32,
    w: f32,
    scale: f32,
    state: &TelemetryState,
) {
    let brake_active = common::get_brake_active(state.current_sample.as_ref());
    let brake_h = 15.0 * scale;
    let brake_top = top - brake_h - (2.0 * scale);
    if let Some(brake_rect) = Rect::from_xywh(left, brake_top, w, brake_h) {
        let mut paint_bg = Paint::default();
        paint_bg.set_color_rgba8(0, 0, 0, 150);
        pixmap.fill_rect(brake_rect, &paint_bg, Transform::identity(), None);

        let mut pb = PathBuilder::new();
        pb.move_to(left, brake_top);
        pb.line_to(left + w, brake_top);
        pb.line_to(left + w, brake_top + brake_h);
        pb.line_to(left, brake_top + brake_h);
        pb.close();
        if let Some(path) = pb.finish() {
            let mut paint_stroke = Paint::default();
            paint_stroke.set_color_rgba8(255, 255, 255, 255);
            let stroke = Stroke {
                width: 1.0_f32,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &paint_stroke, &stroke, Transform::identity(), None);
        }

        if brake_active {
            let mut paint_fill = Paint::default();
            paint_fill.set_color_rgba8(255, 0, 0, 255);
            pixmap.fill_rect(brake_rect, &paint_fill, Transform::identity(), None);
        }
    }
}

pub struct ThrottleBar;

impl OverlayImpl for ThrottleBar {
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

        let throttle = common::get_throttle_ratio(state.current_sample.as_ref());

        let config: ThrottleBarConfig = el
            .options
            .clone()
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        let width = 20.0 * el.scale;
        let max_height = 100.0 * el.scale;

        let bg_rect = egui::Rect::from_center_size(center, egui::vec2(width, max_height));
        painter.rect_filled(bg_rect, 2.0, egui::Color32::from_black_alpha(150));
        painter.rect_stroke(
            bg_rect,
            2.0,
            egui::Stroke::new(1.0_f32, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );

        let fill_height = max_height * throttle;
        let mut fill_rect = bg_rect;
        fill_rect.set_top(bg_rect.bottom() - fill_height);

        painter.rect_filled(fill_rect, 2.0, egui::Color32::GREEN);

        if config.show_brake {
            draw_brake_ui(painter, bg_rect, el.scale, state);
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

        let config: ThrottleBarConfig = el
            .options
            .clone()
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        let throttle = common::get_throttle_ratio(state.current_sample.as_ref());

        let w = 20.0 * el.scale * res_scale;
        let max_h = 100.0 * el.scale * res_scale;

        let left = center_x - w / 2.0;
        let top = center_y - max_h / 2.0;

        let bg_rect = Rect::from_xywh(left, top, w, max_h).unwrap();

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
        pb.line_to(left + w, top);
        pb.line_to(left + w, top + max_h);
        pb.line_to(left, top + max_h);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint_stroke, &stroke, Transform::identity(), None);
        }

        let fill_h = max_h * throttle;
        if fill_h > 0.0
            && let Some(fill_rect) = Rect::from_xywh(left, top + max_h - fill_h, w, fill_h)
        {
            let mut paint_fill = Paint::default();
            paint_fill.set_color_rgba8(0, 255, 0, 255);
            pixmap.fill_rect(fill_rect, &paint_fill, Transform::identity(), None);
        }

        if config.show_brake {
            draw_brake_skia(pixmap, left, top, w, el.scale * res_scale, state);
        }
    }


    fn custom_ui(&self, ui: &mut egui::Ui, el: &mut OverlayElement) {
        let mut config: ThrottleBarConfig = el
            .options
            .clone()
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label("Show Brake Indicator:");
            if ui.checkbox(&mut config.show_brake, "").changed() {
                changed = true;
            }
        });

        if changed {
            el.options = Some(serde_json::to_value(config).unwrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    fn create_test_element() -> OverlayElement {
        OverlayElement {
            enabled: true,
            kind: crate::project::OverlayKind::ThrottleBar,
            x: 0.5,
            y: 0.5,
            scale: 1.0,
            options: None,
        }
    }

    #[test]
    fn test_throttle_bar_render_skia() {
        let el = create_test_element();
        let mut data = vec![0; 800 * 600 * 4];
        let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

        let bar = ThrottleBar;
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

        let sample = crate::overlay::common::create_test_sample();
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
    }

    #[test]
    fn test_throttle_bar_render_ui() {
        let el = create_test_element();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show_inside(ctx, |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

                let bar = ThrottleBar;
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

                let sample = crate::overlay::common::create_test_sample();
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
            });
        });
    }
}
