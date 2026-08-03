use crate::overlay::OverlayImpl;
use crate::project::OverlayElement;
use crate::telemetry::TelemetrySample;
use crate::trackmap::TrackMap;
use eframe::egui;
use tiny_skia::{Paint, PathBuilder, PixmapMut, Stroke, Transform};

pub struct TrackMapOverlay;

impl OverlayImpl for TrackMapOverlay {
    fn render_ui(
        &self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        el: &OverlayElement,
        sample: Option<&TelemetrySample>,
        trackmap: Option<&TrackMap>,
    ) {
        let painter = ui.painter_at(rect);
        let center = egui::pos2(
            rect.left() + el.x * rect.width(),
            rect.top() + el.y * rect.height(),
        );

        if let Some(map) = trackmap {
            let map_size = 150.0 * el.scale;
            let map_rect = egui::Rect::from_center_size(center, egui::vec2(map_size, map_size));

            let mut path = Vec::with_capacity(map.outline.len());
            for &(x, y) in &map.outline {
                path.push(egui::pos2(
                    map_rect.left() + x * map_rect.width(),
                    map_rect.top() + y * map_rect.height(),
                ));
            }

            if path.len() > 1 {
                painter.add(egui::Shape::line(
                    path,
                    egui::Stroke::new(2.0 * el.scale, egui::Color32::from_white_alpha(150)),
                ));
            }

            let (p1, p2) = map.start_finish;
            if p1 != (0.0, 0.0) || p2 != (0.0, 0.0) {
                let sp1 = egui::pos2(
                    map_rect.left() + p1.0 * map_rect.width(),
                    map_rect.top() + p1.1 * map_rect.height(),
                );
                let sp2 = egui::pos2(
                    map_rect.left() + p2.0 * map_rect.width(),
                    map_rect.top() + p2.1 * map_rect.height(),
                );
                painter.line_segment(
                    [sp1, sp2],
                    egui::Stroke::new(3.0 * el.scale, egui::Color32::GREEN),
                );

                let mid_p = egui::pos2((sp1.x + sp2.x) / 2.0, (sp1.y + sp2.y) / 2.0);
                painter.circle_filled(mid_p, 3.0 * el.scale, egui::Color32::GREEN);
            }

            if let Some(s) = sample
                && let Some((cx, cy)) = map.point_at_time(s.time_ms)
            {
                let dot_pos = egui::pos2(
                    map_rect.left() + cx * map_rect.width(),
                    map_rect.top() + cy * map_rect.height(),
                );
                painter.circle_filled(dot_pos, 4.0 * el.scale, egui::Color32::RED);
            }
        }
    }

    fn render_skia(
        &self,
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        sample: Option<&TelemetrySample>,
        trackmap: Option<&TrackMap>,
    ) {
        let width = pixmap.width() as f32;
        let height = pixmap.height() as f32;
        let res_scale = height / 720.0;
        let center_x = el.x * width;
        let center_y = el.y * height;

        if let Some(map) = trackmap {
            let map_size = 150.0 * el.scale * res_scale;
            let left = center_x - map_size / 2.0;
            let top = center_y - map_size / 2.0;

            let mut pb = PathBuilder::new();
            let mut first = true;

            for &(x, y) in &map.outline {
                let px = left + x * map_size;
                let py = top + y * map_size;
                if first {
                    pb.move_to(px, py);
                    first = false;
                } else {
                    pb.line_to(px, py);
                }
            }

            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(255, 255, 255, 150);
                paint.anti_alias = true;

                let stroke = Stroke {
                    width: 2.0 * el.scale * res_scale,
                    ..Default::default()
                };

                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }

            let (p1, p2) = map.start_finish;
            if p1 != (0.0, 0.0) || p2 != (0.0, 0.0) {
                let sp1x = left + p1.0 * map_size;
                let sp1y = top + p1.1 * map_size;
                let sp2x = left + p2.0 * map_size;
                let sp2y = top + p2.1 * map_size;

                let mut pb2 = PathBuilder::new();
                pb2.move_to(sp1x, sp1y);
                pb2.line_to(sp2x, sp2y);

                if let Some(path) = pb2.finish() {
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(0, 255, 0, 255);
                    paint.anti_alias = true;
                    let stroke = Stroke {
                        width: 3.0 * el.scale * res_scale,
                        ..Default::default()
                    };
                    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                }

                let mid_x = (sp1x + sp2x) / 2.0;
                let mid_y = (sp1y + sp2y) / 2.0;
                let mut paint = Paint::default();
                paint.set_color_rgba8(0, 255, 0, 255);
                paint.anti_alias = true;

                if let Some(path) =
                    PathBuilder::from_circle(mid_x, mid_y, 3.0 * el.scale * res_scale)
                {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }

            if let Some(s) = sample
                && let Some((cx, cy)) = map.point_at_time(s.time_ms)
            {
                let dot_x = left + cx * map_size;
                let dot_y = top + cy * map_size;

                let mut paint = Paint::default();
                paint.set_color_rgba8(255, 0, 0, 255);
                paint.anti_alias = true;

                if let Some(path) =
                    PathBuilder::from_circle(dot_x, dot_y, 4.0 * el.scale * res_scale)
                {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
    }
}
