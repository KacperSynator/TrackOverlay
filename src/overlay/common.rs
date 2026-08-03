use crate::telemetry::TelemetrySample;
use rusttype::{Font, Scale};
use tiny_skia::{Color, PixmapMut, Rect, Transform};

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

pub fn draw_text_fallback(
    pixmap: &mut PixmapMut,
    center_x: f32,
    center_y: f32,
    w: f32,
    h: f32,
    color: Color,
) {
    if let Some(rect) = Rect::from_xywh(center_x - w / 2.0, center_y - h / 2.0, w, h) {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(
            (color.red() * 255.0) as u8,
            (color.green() * 255.0) as u8,
            (color.blue() * 255.0) as u8,
            (color.alpha() * 255.0) as u8,
        );
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

pub fn draw_text(
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
