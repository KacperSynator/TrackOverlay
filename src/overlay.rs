use crate::project::{OverlayElement, OverlayKind};
use crate::telemetry::TelemetrySample;
use crate::trackmap::TrackMap;
use eframe::egui;
use rusttype::{Font, Scale};
use tiny_skia::{Color, Paint, PathBuilder, PixmapMut, Rect, Stroke, Transform};

pub struct OverlayData<'a> {
    pub sample: Option<&'a TelemetrySample>,
    pub trackmap: Option<&'a TrackMap>,
}

pub fn get_speed_text(sample: Option<&TelemetrySample>) -> String {
    let speed = sample.map_or(0.0, |s| s.speed_kph);
    format!("{:.0} km/h", speed)
}

pub fn get_gforce_dot(sample: Option<&TelemetrySample>, radius: f32) -> (f32, f32) {
    let lat_g = sample.map_or(0.0, |s| s.accel_lat_g);
    let lon_g = sample.map_or(0.0, |s| s.accel_lon_g);
    (lat_g * radius, -lon_g * radius)
}

pub fn get_lap_timer_text(sample: Option<&TelemetrySample>) -> String {
    let time_ms = sample.and_then(|s| s.lap_time_ms).unwrap_or(0);
    let seconds = time_ms as f64 / 1000.0;
    let mins = (seconds / 60.0).floor() as i32;
    let secs = seconds % 60.0;
    format!("{:02}:{:05.2}", mins, secs)
}

pub fn get_throttle_ratio(sample: Option<&TelemetrySample>) -> f32 {
    sample.map_or(0.0, |s| s.throttle_pct).clamp(0.0, 100.0) / 100.0
}

pub fn render_overlay(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    elements: &mut [OverlayElement],
    sample: Option<&TelemetrySample>,
    trackmap: Option<&TrackMap>,
    _is_dragging: bool,
) {
    let painter = ui.painter_at(rect);

    for el in elements.iter_mut() {
        let center = egui::pos2(
            rect.left() + el.x * rect.width(),
            rect.top() + el.y * rect.height(),
        );

        match el.kind {
            OverlayKind::SpeedReadout => {
                let text = get_speed_text(sample);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(32.0 * el.scale),
                    egui::Color32::WHITE,
                );
            }
            OverlayKind::GForceMeter => {
                let radius = 40.0 * el.scale;
                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(2.0 * el.scale, egui::Color32::WHITE),
                );

                let (dx, dy) = get_gforce_dot(sample, radius);
                let dot_pos = center + egui::vec2(dx, dy);
                painter.circle_filled(dot_pos, 5.0 * el.scale, egui::Color32::RED);
            }
            OverlayKind::LapTimer => {
                let text = get_lap_timer_text(sample);

                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(24.0 * el.scale),
                    egui::Color32::YELLOW,
                );
            }
            OverlayKind::TrackMap => {
                if let Some(map) = trackmap {
                    let map_size = 150.0 * el.scale;
                    let map_rect =
                        egui::Rect::from_center_size(center, egui::vec2(map_size, map_size));

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
                        && let Some((cx, cy)) = map.point_at_time(s.time_ms) {
                            let dot_pos = egui::pos2(
                                map_rect.left() + cx * map_rect.width(),
                                map_rect.top() + cy * map_rect.height(),
                            );
                            painter.circle_filled(dot_pos, 4.0 * el.scale, egui::Color32::RED);
                        }
                }
            }
            OverlayKind::ThrottleBar => {
                let throttle = get_throttle_ratio(sample);

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
        }
    }
}

pub fn render_overlay_skia(
    pixmap: &mut PixmapMut,
    elements: &[OverlayElement],
    sample: Option<&TelemetrySample>,
    trackmap: Option<&TrackMap>,
) {
    let width = pixmap.width() as f32;
    let height = pixmap.height() as f32;

    let font_data =
        std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").unwrap_or_default();
    let font_opt = Font::try_from_vec(font_data);

    for el in elements.iter() {
        let center_x = el.x * width;
        let center_y = el.y * height;

        match el.kind {
            OverlayKind::SpeedReadout => {
                let text = get_speed_text(sample);
                if let Some(font) = &font_opt {
                    draw_text(
                        pixmap,
                        font,
                        &text,
                        center_x,
                        center_y,
                        32.0 * el.scale,
                        Color::WHITE,
                    );
                } else {
                    draw_text_fallback(
                        pixmap,
                        center_x,
                        center_y,
                        100.0 * el.scale,
                        30.0 * el.scale,
                        Color::WHITE,
                    );
                }
            }
            OverlayKind::GForceMeter => {
                let radius = 40.0 * el.scale;

                let mut paint = Paint::default();
                paint.set_color_rgba8(255, 255, 255, 255);
                paint.anti_alias = true;

                let stroke = Stroke { width: 2.0 * el.scale, ..Default::default() };

                if let Some(path) = PathBuilder::from_circle(center_x, center_y, radius) {
                    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                }

                let (dx, dy) = get_gforce_dot(sample, radius);
                let mut paint_red = Paint::default();
                paint_red.set_color_rgba8(255, 0, 0, 255);
                paint_red.anti_alias = true;

                if let Some(path) =
                    PathBuilder::from_circle(center_x + dx, center_y + dy, 5.0 * el.scale)
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
            OverlayKind::LapTimer => {
                let text = get_lap_timer_text(sample);
                if let Some(font) = &font_opt {
                    draw_text(
                        pixmap,
                        font,
                        &text,
                        center_x,
                        center_y,
                        24.0 * el.scale,
                        Color::from_rgba8(255, 255, 0, 255),
                    );
                } else {
                    draw_text_fallback(
                        pixmap,
                        center_x,
                        center_y,
                        100.0 * el.scale,
                        20.0 * el.scale,
                        Color::from_rgba8(255, 255, 0, 255),
                    );
                }
            }
            OverlayKind::TrackMap => {
                if let Some(map) = trackmap {
                    let map_size = 150.0 * el.scale;
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

                        let stroke = Stroke { width: 2.0 * el.scale, ..Default::default() };

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
                            let stroke = Stroke { width: 3.0 * el.scale, ..Default::default() };
                            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                        }

                        let mid_x = (sp1x + sp2x) / 2.0;
                        let mid_y = (sp1y + sp2y) / 2.0;
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(0, 255, 0, 255);
                        paint.anti_alias = true;

                        if let Some(path) = PathBuilder::from_circle(mid_x, mid_y, 3.0 * el.scale) {
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
                        && let Some((cx, cy)) = map.point_at_time(s.time_ms) {
                            let dot_x = left + cx * map_size;
                            let dot_y = top + cy * map_size;

                            let mut paint = Paint::default();
                            paint.set_color_rgba8(255, 0, 0, 255);
                            paint.anti_alias = true;

                            if let Some(path) =
                                PathBuilder::from_circle(dot_x, dot_y, 4.0 * el.scale)
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
            OverlayKind::ThrottleBar => {
                let throttle = get_throttle_ratio(sample);

                let w = 20.0 * el.scale;
                let max_h = 100.0 * el.scale;

                let left = center_x - w / 2.0;
                let top = center_y - max_h / 2.0;

                let bg_rect = Rect::from_xywh(left, top, w, max_h).unwrap();

                let mut paint_bg = Paint::default();
                paint_bg.set_color_rgba8(0, 0, 0, 150);
                pixmap.fill_rect(bg_rect, &paint_bg, Transform::identity(), None);

                let mut paint_stroke = Paint::default();
                paint_stroke.set_color_rgba8(255, 255, 255, 255);
                let stroke = Stroke { width: 1.0_f32, ..Default::default() };

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
    }
}

fn draw_text_fallback(
    pixmap: &mut PixmapMut,
    center_x: f32,
    center_y: f32,
    w: f32,
    h: f32,
    color: Color,
) {
    if let Some(rect) = Rect::from_xywh(center_x - w / 2.0, center_y - h / 2.0, w, h) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(
            (color.red() * 255.0) as u8,
            (color.green() * 255.0) as u8,
            (color.blue() * 255.0) as u8,
            (color.alpha() * 255.0) as u8,
        );
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn draw_text(
    pixmap: &mut PixmapMut,
    font: &Font,
    text: &str,
    center_x: f32,
    center_y: f32,
    scale_val: f32,
    color: Color,
) {
    let scale = Scale::uniform(scale_val);
    let v_metrics = font.v_metrics(scale);
    let offset = rusttype::point(0.0, v_metrics.ascent);

    let glyphs: Vec<_> = font.layout(text, scale, offset).collect();
    if glyphs.is_empty() {
        return;
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for g in &glyphs {
        if let Some(bb) = g.pixel_bounding_box() {
            min_x = min_x.min(bb.min.x as f32);
            max_x = max_x.max(bb.max.x as f32);
            min_y = min_y.min(bb.min.y as f32);
            max_y = max_y.max(bb.max.y as f32);
        }
    }

    if min_x == f32::MAX {
        return;
    }

    let width = max_x - min_x;
    let height = max_y - min_y;

    let start_x = center_x - width / 2.0;
    let start_y = center_y - height / 2.0;

    for g in glyphs {
        if let Some(bb) = g.pixel_bounding_box() {
            g.draw(|x, y, v| {
                if v > 0.0 {
                    let px = (start_x + bb.min.x as f32 + x as f32) as i32;
                    let py = (start_y + bb.min.y as f32 + y as f32) as i32;

                    if px >= 0
                        && px < pixmap.width() as i32
                        && py >= 0
                        && py < pixmap.height() as i32
                    {
                        let mut c = color;
                        c.set_alpha(v);

                        let idx = (py as u32 * pixmap.width() + px as u32) as usize;
                        let existing = pixmap.pixels_mut()[idx];
                        let ea = existing.alpha() as f32 / 255.0;
                        let er = existing.red() as f32 / 255.0;
                        let eg = existing.green() as f32 / 255.0;
                        let eb = existing.blue() as f32 / 255.0;

                        let na = c.alpha() + ea * (1.0 - c.alpha());
                        if na > 0.0 {
                            let nr = (c.red() * c.alpha() + er * ea * (1.0 - c.alpha())) / na;
                            let ng = (c.green() * c.alpha() + eg * ea * (1.0 - c.alpha())) / na;
                            let nb = (c.blue() * c.alpha() + eb * ea * (1.0 - c.alpha())) / na;

                            if let Some(new_color) = tiny_skia::Color::from_rgba(nr, ng, nb, na) {
                                pixmap.pixels_mut()[idx] = new_color.premultiply().to_color_u8();
                            }
                        }
                    }
                }
            });
        }
    }
}
