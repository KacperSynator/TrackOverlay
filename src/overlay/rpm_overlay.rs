use std::f32::consts::PI;

use crate::overlay::OverlayImpl;
use crate::project::OverlayElement;
use crate::telemetry::TelemetryState;
use crate::trackmap::TrackMap;
use eframe::egui;
use tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, PixmapMut, Rect, Stroke, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RpmStyle {
    Bar,
    Dial,
    Leds,
}

impl RpmStyle {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "dial" => RpmStyle::Dial,
            "leds" => RpmStyle::Leds,
            _ => RpmStyle::Bar,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            RpmStyle::Bar => "Bar",
            RpmStyle::Dial => "Dial",
            RpmStyle::Leds => "Leds",
        }
    }
}

pub struct RpmOverlay;

impl RpmOverlay {
    fn extract_options(el: &OverlayElement) -> (f32, f32, RpmStyle) {
        let mut rpm_max = 6500.0;
        let mut rpm_redline = 6000.0;
        let mut style = RpmStyle::Bar;

        if let Some(opts) = &el.options {
            if let Some(max_val) = opts.get("rpm_max").and_then(|v| v.as_f64()) {
                rpm_max = max_val as f32;
            }
            if let Some(rl_val) = opts.get("rpm_redline").and_then(|v| v.as_f64()) {
                rpm_redline = rl_val as f32;
            }
            if let Some(s) = opts.get("style").and_then(|v| v.as_str()) {
                style = RpmStyle::from_str(s);
            }
        }

        (rpm_max, rpm_redline, style)
    }

    // --- BAR UI ---
    fn render_bar_ui(
        painter: &egui::Painter,
        center: egui::Pos2,
        el: &OverlayElement,
        rpm: f32,
        rpm_max: f32,
        rpm_redline: f32,
    ) {
        let max_width = 300.0 * el.scale;
        let height = 20.0 * el.scale;
        let bg_rect = egui::Rect::from_center_size(center, egui::vec2(max_width, height));

        let max_k = (rpm_max / 1000.0).floor() as i32;
        for i in 1..=max_k {
            let ratio = (i as f32 * 1000.0) / rpm_max;
            let x = bg_rect.left() + ratio * max_width;

            let color = if (i as f32 * 1000.0) >= rpm_redline {
                egui::Color32::RED
            } else {
                egui::Color32::WHITE
            };

            let text = format!("{}", i);
            painter.text(
                egui::pos2(x, bg_rect.top() - 10.0 * el.scale),
                egui::Align2::CENTER_BOTTOM,
                text,
                egui::FontId::proportional(14.0 * el.scale),
                color,
            );
        }

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
                painter.rect_filled(fill_rect, 2.0, egui::Color32::WHITE);
            } else {
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

    // --- DIAL UI ---
    fn render_dial_ui(
        painter: &egui::Painter,
        center: egui::Pos2,
        el: &OverlayElement,
        rpm: f32,
        rpm_max: f32,
        rpm_redline: f32,
    ) {
        let radius = 100.0 * el.scale;

        let start_angle = -135.0_f32.to_radians();
        let end_angle = 135.0_f32.to_radians();
        let angle_range = end_angle - start_angle;

        // Draw background arc
        let num_points = 64;
        let mut bg_points = Vec::new();
        for i in 0..=num_points {
            let t = i as f32 / num_points as f32;
            let angle = start_angle + t * angle_range;
            // Note: in standard math, 0 is right, negative is up.
            // In screen space, y grows downwards. To make -135 bottom left and +135 bottom right,
            // we rotate by -90 deg so that 0 is UP.
            let rotated_angle = angle - PI / 2.0;
            bg_points.push(
                center + egui::vec2(rotated_angle.cos() * radius, rotated_angle.sin() * radius),
            );
        }
        painter.add(egui::Shape::line(
            bg_points,
            egui::Stroke::new(10.0 * el.scale, egui::Color32::from_black_alpha(150)),
        ));

        // Draw redline arc
        if rpm_redline < rpm_max {
            let red_start_t = rpm_redline / rpm_max;
            let mut red_points = Vec::new();
            for i in 0..=16 {
                let t = red_start_t + (1.0 - red_start_t) * (i as f32 / 16.0);
                let angle = start_angle + t * angle_range;
                let rotated_angle = angle - PI / 2.0;
                red_points.push(
                    center + egui::vec2(rotated_angle.cos() * radius, rotated_angle.sin() * radius),
                );
            }
            painter.add(egui::Shape::line(
                red_points,
                egui::Stroke::new(10.0 * el.scale, egui::Color32::RED),
            ));
        }

        // Draw numbers
        let max_k = (rpm_max / 1000.0).floor() as i32;
        for i in 0..=max_k {
            let ratio = (i as f32 * 1000.0) / rpm_max;
            let angle = start_angle + ratio * angle_range;
            let rotated_angle = angle - PI / 2.0;

            let label_radius = radius - 20.0 * el.scale;
            let text_pos = center
                + egui::vec2(
                    rotated_angle.cos() * label_radius,
                    rotated_angle.sin() * label_radius,
                );

            let color = if (i as f32 * 1000.0) >= rpm_redline {
                egui::Color32::RED
            } else {
                egui::Color32::WHITE
            };

            painter.text(
                text_pos,
                egui::Align2::CENTER_CENTER,
                format!("{}", i),
                egui::FontId::proportional(14.0 * el.scale),
                color,
            );
        }

        // Draw needle
        let rpm_ratio = rpm / rpm_max;
        let needle_angle = start_angle + rpm_ratio * angle_range;
        let rotated_needle = needle_angle - PI / 2.0;
        let needle_end = center
            + egui::vec2(
                rotated_needle.cos() * radius * 0.9,
                rotated_needle.sin() * radius * 0.9,
            );

        painter.line_segment(
            [center, needle_end],
            egui::Stroke::new(3.0 * el.scale, egui::Color32::WHITE),
        );
        painter.circle_filled(center, 5.0 * el.scale, egui::Color32::WHITE);
    }

    // --- LEDS UI ---
    fn render_leds_ui(
        painter: &egui::Painter,
        center: egui::Pos2,
        el: &OverlayElement,
        rpm: f32,
        rpm_max: f32,
        rpm_redline: f32,
    ) {
        // 3 green, 3 yellow, 2 red = 8 total
        let num_leds = 8;
        let led_radius = 10.0 * el.scale;
        let spacing = 25.0 * el.scale;
        let total_width = (num_leds - 1) as f32 * spacing;
        let start_x = center.x - total_width / 2.0;

        // RPM thresholds
        let green_yellow_thresholds = 6;
        let red_thresholds = 2;

        let green_yellow_step = rpm_redline / green_yellow_thresholds as f32;
        let red_step = (rpm_max - rpm_redline) / red_thresholds as f32;

        for i in 0..num_leds {
            let x = start_x + (i as f32) * spacing;
            let led_pos = egui::pos2(x, center.y);

            let (threshold, base_color) = if i < 3 {
                ((i + 1) as f32 * green_yellow_step, egui::Color32::GREEN)
            } else if i < 6 {
                ((i + 1) as f32 * green_yellow_step, egui::Color32::YELLOW)
            } else {
                (rpm_redline + (i - 5) as f32 * red_step, egui::Color32::RED)
            };

            let is_on = rpm >= threshold;
            let mut color = base_color;
            if !is_on {
                // Dim
                color = egui::Color32::from_rgba_premultiplied(
                    (color.r() as f32 * 0.2) as u8,
                    (color.g() as f32 * 0.2) as u8,
                    (color.b() as f32 * 0.2) as u8,
                    255,
                );
            }

            painter.circle_filled(led_pos, led_radius, color);
            painter.circle_stroke(
                led_pos,
                led_radius,
                egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(150)),
            );
        }
    }

    // --- BAR SKIA ---
    #[allow(clippy::too_many_arguments)]
    fn render_bar_skia(
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        center_x: f32,
        center_y: f32,
        res_scale: f32,
        rpm: f32,
        rpm_max: f32,
        rpm_redline: f32,
        font_opt: Option<&rusttype::Font>,
    ) {
        let max_w = 300.0 * el.scale * res_scale;
        let h = 20.0 * el.scale * res_scale;
        let left = center_x - max_w / 2.0;
        let top = center_y - h / 2.0;

        let max_k = (rpm_max / 1000.0).floor() as i32;
        for i in 1..=max_k {
            let ratio = (i as f32 * 1000.0) / rpm_max;
            let x = left + ratio * max_w;

            let color = if (i as f32 * 1000.0) >= rpm_redline {
                tiny_skia::Color::from_rgba8(255, 0, 0, 255)
            } else {
                tiny_skia::Color::from_rgba8(255, 255, 255, 255)
            };

            let text = format!("{}", i);
            let y = top - 8.0 * el.scale * res_scale;

            if let Some(font) = font_opt {
                crate::overlay::common::draw_text(
                    pixmap,
                    font,
                    &text,
                    x,
                    y,
                    16.0 * el.scale * res_scale,
                    color,
                );
            } else {
                crate::overlay::common::draw_text_fallback(
                    pixmap,
                    x,
                    y,
                    10.0 * el.scale * res_scale,
                    10.0 * el.scale * res_scale,
                    color,
                );
            }
        }

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

    // --- DIAL SKIA ---
    #[allow(clippy::too_many_arguments)]
    fn render_dial_skia(
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        center_x: f32,
        center_y: f32,
        res_scale: f32,
        rpm: f32,
        rpm_max: f32,
        rpm_redline: f32,
        font_opt: Option<&rusttype::Font>,
    ) {
        let radius = 100.0 * el.scale * res_scale;

        let start_angle = -135.0_f32.to_radians();
        let end_angle = 135.0_f32.to_radians();
        let angle_range = end_angle - start_angle;

        // Draw background arc
        let mut pb_bg = PathBuilder::new();
        let num_points = 64;
        for i in 0..=num_points {
            let t = i as f32 / num_points as f32;
            let angle = start_angle + t * angle_range;
            let rotated_angle = angle - PI / 2.0;
            let x = center_x + rotated_angle.cos() * radius;
            let y = center_y + rotated_angle.sin() * radius;
            if i == 0 {
                pb_bg.move_to(x, y);
            } else {
                pb_bg.line_to(x, y);
            }
        }
        if let Some(path) = pb_bg.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, 150);
            let stroke = Stroke {
                width: 10.0 * el.scale * res_scale,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }

        // Draw redline arc
        if rpm_redline < rpm_max {
            let red_start_t = rpm_redline / rpm_max;
            let mut pb_red = PathBuilder::new();
            for i in 0..=16 {
                let t = red_start_t + (1.0 - red_start_t) * (i as f32 / 16.0);
                let angle = start_angle + t * angle_range;
                let rotated_angle = angle - PI / 2.0;
                let x = center_x + rotated_angle.cos() * radius;
                let y = center_y + rotated_angle.sin() * radius;
                if i == 0 {
                    pb_red.move_to(x, y);
                } else {
                    pb_red.line_to(x, y);
                }
            }
            if let Some(path) = pb_red.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(255, 0, 0, 255);
                let stroke = Stroke {
                    width: 10.0 * el.scale * res_scale,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Default::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }

        // Draw numbers
        let max_k = (rpm_max / 1000.0).floor() as i32;
        for i in 0..=max_k {
            let ratio = (i as f32 * 1000.0) / rpm_max;
            let angle = start_angle + ratio * angle_range;
            let rotated_angle = angle - PI / 2.0;

            let label_radius = radius - 20.0 * el.scale * res_scale;
            let text_x = center_x + rotated_angle.cos() * label_radius;
            // tiny_skia text renders from bottom left, approximate center alignment
            let text_y =
                center_y + rotated_angle.sin() * label_radius + (5.0 * el.scale * res_scale);

            let color = if (i as f32 * 1000.0) >= rpm_redline {
                tiny_skia::Color::from_rgba8(255, 0, 0, 255)
            } else {
                tiny_skia::Color::from_rgba8(255, 255, 255, 255)
            };

            let text = format!("{}", i);
            if let Some(font) = font_opt {
                crate::overlay::common::draw_text(
                    pixmap,
                    font,
                    &text,
                    text_x,
                    text_y,
                    16.0 * el.scale * res_scale,
                    color,
                );
            } else {
                crate::overlay::common::draw_text_fallback(
                    pixmap,
                    text_x,
                    text_y,
                    10.0 * el.scale * res_scale,
                    10.0 * el.scale * res_scale,
                    color,
                );
            }
        }

        // Draw needle
        let rpm_ratio = rpm / rpm_max;
        let needle_angle = start_angle + rpm_ratio * angle_range;
        let rotated_needle = needle_angle - PI / 2.0;
        let needle_end_x = center_x + rotated_needle.cos() * radius * 0.9;
        let needle_end_y = center_y + rotated_needle.sin() * radius * 0.9;

        let mut pb_needle = PathBuilder::new();
        pb_needle.move_to(center_x, center_y);
        pb_needle.line_to(needle_end_x, needle_end_y);
        if let Some(path) = pb_needle.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(255, 255, 255, 255);
            let stroke = Stroke {
                width: 3.0 * el.scale * res_scale,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }

        // Draw center dot
        let mut pb_center = PathBuilder::new();
        pb_center.push_circle(center_x, center_y, 5.0 * el.scale * res_scale);
        if let Some(path) = pb_center.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(255, 255, 255, 255);
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    // --- LEDS SKIA ---
    #[allow(clippy::too_many_arguments)]
    fn render_leds_skia(
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        center_x: f32,
        center_y: f32,
        res_scale: f32,
        rpm: f32,
        rpm_max: f32,
        rpm_redline: f32,
    ) {
        let num_leds = 8;
        let led_radius = 10.0 * el.scale * res_scale;
        let spacing = 25.0 * el.scale * res_scale;
        let total_width = (num_leds - 1) as f32 * spacing;
        let start_x = center_x - total_width / 2.0;

        let green_yellow_thresholds = 6;
        let red_thresholds = 2;
        let green_yellow_step = rpm_redline / green_yellow_thresholds as f32;
        let red_step = (rpm_max - rpm_redline) / red_thresholds as f32;

        for i in 0..num_leds {
            let x = start_x + (i as f32) * spacing;
            let (threshold, r, g, b) = if i < 3 {
                ((i + 1) as f32 * green_yellow_step, 0, 255, 0)
            } else if i < 6 {
                ((i + 1) as f32 * green_yellow_step, 255, 255, 0)
            } else {
                (rpm_redline + (i - 5) as f32 * red_step, 255, 0, 0)
            };

            let is_on = rpm >= threshold;
            let (r, g, b) = if is_on {
                (r, g, b)
            } else {
                (
                    (r as f32 * 0.2) as u8,
                    (g as f32 * 0.2) as u8,
                    (b as f32 * 0.2) as u8,
                )
            };

            let mut pb_led = PathBuilder::new();
            pb_led.push_circle(x, center_y, led_radius);
            if let Some(path) = pb_led.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(r, g, b, 255);
                pixmap.fill_path(
                    &path,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    Transform::identity(),
                    None,
                );

                let mut stroke_paint = Paint::default();
                stroke_paint.set_color_rgba8(0, 0, 0, 150);
                let stroke = Stroke {
                    width: 1.0,
                    ..Default::default()
                };
                pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
            }
        }
    }
}

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

        let (rpm_max, rpm_redline, style) = Self::extract_options(el);

        let rpm = state
            .current_sample
            .as_ref()
            .map_or(0.0, |s| s.engine_speed_rpm)
            .clamp(0.0, rpm_max);

        match style {
            RpmStyle::Bar => Self::render_bar_ui(&painter, center, el, rpm, rpm_max, rpm_redline),
            RpmStyle::Dial => Self::render_dial_ui(&painter, center, el, rpm, rpm_max, rpm_redline),
            RpmStyle::Leds => Self::render_leds_ui(&painter, center, el, rpm, rpm_max, rpm_redline),
        }
    }

    fn custom_ui(&self, ui: &mut egui::Ui, el: &mut OverlayElement) {
        let (rpm_max, rpm_redline, mut current_style) = Self::extract_options(el);
        let mut max_u = rpm_max as u32;
        let mut redline_u = rpm_redline as u32;

        let mut changed = false;
        let kind = el.kind.clone();

        ui.horizontal(|ui| {
            ui.label("  "); // Indent

            let mut style_str = current_style.as_str().to_string();
            egui::ComboBox::from_id_salt(format!("rpm_style_{}", kind as u32))
                .selected_text(style_str.clone())
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut style_str, "Bar".to_string(), "Bar")
                        .clicked()
                    {
                        changed = true;
                    }
                    if ui
                        .selectable_value(&mut style_str, "Dial".to_string(), "Dial")
                        .clicked()
                    {
                        changed = true;
                    }
                    if ui
                        .selectable_value(&mut style_str, "Leds".to_string(), "LEDs")
                        .clicked()
                    {
                        changed = true;
                    }
                });

            if changed {
                current_style = RpmStyle::from_str(&style_str);
            }

            if ui
                .add(egui::DragValue::new(&mut max_u).prefix("Max RPM: "))
                .changed()
            {
                changed = true;
            }
            if ui
                .add(egui::DragValue::new(&mut redline_u).prefix("Redline: "))
                .changed()
            {
                changed = true;
            }
        });

        if changed {
            let mut new_opts = serde_json::Map::new();
            new_opts.insert(
                "rpm_max".to_string(),
                serde_json::Value::Number(max_u.into()),
            );
            new_opts.insert(
                "rpm_redline".to_string(),
                serde_json::Value::Number(redline_u.into()),
            );
            new_opts.insert(
                "style".to_string(),
                serde_json::Value::String(current_style.as_str().to_string()),
            );
            el.options = Some(serde_json::Value::Object(new_opts));
        }
    }

    fn render_skia(
        &self,
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        state: &TelemetryState,
        _trackmap: Option<&TrackMap>,
        font_opt: Option<&rusttype::Font>,
    ) {
        let width = pixmap.width() as f32;
        let height = pixmap.height() as f32;
        let res_scale = height / 720.0;
        let center_x = el.x * width;
        let center_y = el.y * height;

        let (rpm_max, rpm_redline, style) = Self::extract_options(el);

        let rpm = state
            .current_sample
            .as_ref()
            .map_or(0.0, |s| s.engine_speed_rpm)
            .clamp(0.0, rpm_max);

        match style {
            RpmStyle::Bar => Self::render_bar_skia(
                pixmap,
                el,
                center_x,
                center_y,
                res_scale,
                rpm,
                rpm_max,
                rpm_redline,
                font_opt,
            ),
            RpmStyle::Dial => Self::render_dial_skia(
                pixmap,
                el,
                center_x,
                center_y,
                res_scale,
                rpm,
                rpm_max,
                rpm_redline,
                font_opt,
            ),
            RpmStyle::Leds => Self::render_leds_skia(
                pixmap,
                el,
                center_x,
                center_y,
                res_scale,
                rpm,
                rpm_max,
                rpm_redline,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    fn create_test_element(style: &str) -> OverlayElement {
        let mut opts = serde_json::Map::new();
        opts.insert(
            "rpm_max".to_string(),
            serde_json::Value::Number(7000.into()),
        );
        opts.insert(
            "rpm_redline".to_string(),
            serde_json::Value::Number(6500.into()),
        );
        opts.insert(
            "style".to_string(),
            serde_json::Value::String(style.to_string()),
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
    fn test_rpm_overlay_render_skia_all_styles() {
        let styles = ["bar", "dial", "leds"];
        for style in styles {
            let el = create_test_element(style);
            let mut data = vec![0; 800 * 600 * 4];
            let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

            let overlay = RpmOverlay;

            // Render without state
            overlay.render_skia(
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
            overlay.render_skia(
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
            overlay.render_skia(
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
    }

    #[test]
    fn test_rpm_overlay_render_ui_all_styles() {
        let styles = ["bar", "dial", "leds"];
        for style in styles {
            let el = create_test_element(style);
            let ctx = egui::Context::default();
            let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show_inside(ctx, |ui| {
                    let rect =
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

                    let overlay = RpmOverlay;

                    // Render without state
                    overlay.render_ui(
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
                    overlay.render_ui(
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
                    overlay.render_ui(
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
}
