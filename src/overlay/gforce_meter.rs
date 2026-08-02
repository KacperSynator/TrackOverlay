use crate::overlay::{common, OverlayImpl};
use crate::project::OverlayElement;
use crate::telemetry::TelemetrySample;
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
        sample: Option<&TelemetrySample>,
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

        let (dx, dy) = common::get_gforce_dot(sample, radius);
        let dot_pos = center + egui::vec2(dx, dy);
        painter.circle_filled(dot_pos, 5.0 * el.scale, egui::Color32::RED);
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

        let (dx, dy) = common::get_gforce_dot(sample, radius);
        let mut paint_red = Paint::default();
        paint_red.set_color_rgba8(255, 0, 0, 255);
        paint_red.anti_alias = true;

        if let Some(path) = PathBuilder::from_circle(
            center_x + dx,
            center_y + dy,
            5.0 * el.scale * res_scale,
        ) {
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
