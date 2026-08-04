use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::TelemetryState;
use crate::trackmap::TrackMap;
use eframe::egui;
use rusttype::Font;
use tiny_skia::{Color, PixmapMut};

pub struct AdvancedLapTimer;

impl AdvancedLapTimer {
    fn format_diff(ms: i64) -> String {
        let abs_ms = ms.abs();
        let seconds = abs_ms as f64 / 1000.0;
        let secs = (seconds % 60.0) as i32;
        let millis = (abs_ms % 1000) / 10;
        let sign = if ms > 0 {
            "+"
        } else if ms < 0 {
            "-"
        } else {
            " "
        };
        format!("{}{}.{:02}", sign, secs, millis)
    }

    fn format_time(ms: i64) -> String {
        let seconds = ms as f64 / 1000.0;
        let mins = (seconds / 60.0).floor() as i32;
        let secs = seconds % 60.0;
        format!("{:02}:{:05.2}", mins, secs)
    }
}

impl OverlayImpl for AdvancedLapTimer {
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

        let current_text = common::get_lap_timer_text(state.current_sample.as_ref());
        let mut y_offset = 0.0;
        let line_height = 24.0 * el.scale;

        // Draw current time
        painter.text(
            egui::pos2(center.x, center.y + y_offset),
            egui::Align2::CENTER_CENTER,
            current_text,
            egui::FontId::proportional(32.0 * el.scale),
            egui::Color32::WHITE,
        );
        y_offset += 36.0 * el.scale;

        // Draw projection
        if let Some(diff) = state.projection_ms {
            let color = if diff < 0 {
                egui::Color32::GREEN
            } else {
                egui::Color32::RED
            };
            let diff_text = Self::format_diff(diff);
            painter.text(
                egui::pos2(center.x, center.y + y_offset),
                egui::Align2::CENTER_CENTER,
                diff_text,
                egui::FontId::proportional(24.0 * el.scale),
                color,
            );
            y_offset += line_height;
        }

        // Draw best lap
        if let Some(best) = &state.best_lap {
            let best_text = format!("Best: {}", Self::format_time(best.duration_ms));
            painter.text(
                egui::pos2(center.x, center.y + y_offset),
                egui::Align2::CENTER_CENTER,
                best_text,
                egui::FontId::proportional(16.0 * el.scale),
                egui::Color32::from_rgb(255, 215, 0), // Gold
            );
            y_offset += line_height;
        }

        // Draw previous laps
        for (i, lap) in state.previous_laps.iter().enumerate() {
            let lap_text = format!(
                "L{}: {}",
                lap.lap_number,
                Self::format_time(lap.duration_ms)
            );
            let alpha = 255 - (i as u8 * 50); // fade out older laps
            painter.text(
                egui::pos2(center.x, center.y + y_offset),
                egui::Align2::CENTER_CENTER,
                lap_text,
                egui::FontId::proportional(16.0 * el.scale),
                egui::Color32::from_white_alpha(alpha),
            );
            y_offset += line_height;
        }
    }

    fn render_skia(
        &self,
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        state: &TelemetryState,
        _trackmap: Option<&TrackMap>,
    ) {
        let width = pixmap.width() as f32;
        let height = pixmap.height() as f32;
        let res_scale = height / 720.0;
        let center_x = el.x * width;
        let center_y = el.y * height;

        let font_data = include_bytes!("../font.ttf");
        let font_opt = Font::try_from_bytes(font_data as &[u8]);

        if let Some(font) = &font_opt {
            let current_text = common::get_lap_timer_text(state.current_sample.as_ref());
            let mut y_offset = 0.0;
            let line_height = 24.0 * el.scale * res_scale;

            // Draw current time
            common::draw_text(
                pixmap,
                font,
                &current_text,
                center_x,
                center_y + y_offset,
                32.0 * el.scale * res_scale,
                Color::from_rgba8(255, 255, 255, 255),
            );
            y_offset += 36.0 * el.scale * res_scale;

            // Draw projection
            if let Some(diff) = state.projection_ms {
                let color = if diff < 0 {
                    Color::from_rgba8(0, 255, 0, 255) // Green
                } else {
                    Color::from_rgba8(255, 0, 0, 255) // Red
                };
                let diff_text = Self::format_diff(diff);
                common::draw_text(
                    pixmap,
                    font,
                    &diff_text,
                    center_x,
                    center_y + y_offset,
                    24.0 * el.scale * res_scale,
                    color,
                );
                y_offset += line_height;
            }

            // Draw best lap
            if let Some(best) = &state.best_lap {
                let best_text = format!("Best: {}", Self::format_time(best.duration_ms));
                common::draw_text(
                    pixmap,
                    font,
                    &best_text,
                    center_x,
                    center_y + y_offset,
                    16.0 * el.scale * res_scale,
                    Color::from_rgba8(255, 215, 0, 255), // Gold
                );
                y_offset += line_height;
            }

            // Draw previous laps
            for (i, lap) in state.previous_laps.iter().enumerate() {
                let lap_text = format!(
                    "L{}: {}",
                    lap.lap_number,
                    Self::format_time(lap.duration_ms)
                );
                let alpha = 255 - (i as u8 * 50);
                common::draw_text(
                    pixmap,
                    font,
                    &lap_text,
                    center_x,
                    center_y + y_offset,
                    16.0 * el.scale * res_scale,
                    Color::from_rgba8(255, 255, 255, alpha),
                );
                y_offset += line_height;
            }
        } else {
            common::draw_text_fallback(
                pixmap,
                center_x,
                center_y,
                100.0 * el.scale * res_scale,
                20.0 * el.scale * res_scale,
                Color::from_rgba8(255, 255, 0, 255),
            );
        }
    }
}
