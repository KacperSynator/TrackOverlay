use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::TelemetrySample;
use crate::trackmap::TrackMap;
use eframe::egui;
use tiny_skia::{Paint, PathBuilder, PixmapMut, Rect, Stroke, Transform};

pub struct ThrottleBar;

impl OverlayImpl for ThrottleBar {
    fn render_ui(
        &self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        el: &OverlayElement,
        sample: Option<&TelemetrySample>,
        _trackmap: Option<&TrackMap>,
    ) {
        let painter = ui.painter_at(rect);
        let center = egui::pos2(
            rect.left() + el.x * rect.width(),
            rect.top() + el.y * rect.height(),
        );

        let throttle = common::get_throttle_ratio(sample);

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
    }

    fn render_skia(
        &self,
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        sample: Option<&TelemetrySample>,
        _trackmap: Option<&TrackMap>,
    ) {
        let width = pixmap.width() as f32;
        let height = pixmap.height() as f32;
        let res_scale = height / 720.0;
        let center_x = el.x * width;
        let center_y = el.y * height;

        let throttle = common::get_throttle_ratio(sample);

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
        }
    }

    #[test]
    fn test_throttle_bar_render_skia() {
        let el = create_test_element();
        let mut data = vec![0; 800 * 600 * 4];
        let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

        let bar = ThrottleBar;
        bar.render_skia(&mut pixmap, &el, None, None);

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
        };
        bar.render_skia(&mut pixmap, &el, Some(&sample), None);
    }

    #[test]
    fn test_throttle_bar_render_ui() {
        let el = create_test_element();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show_inside(ctx, |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

                let bar = ThrottleBar;
                bar.render_ui(ui, rect, &el, None, None);

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
                };
                bar.render_ui(ui, rect, &el, Some(&sample), None);
            });
        });
    }
}
