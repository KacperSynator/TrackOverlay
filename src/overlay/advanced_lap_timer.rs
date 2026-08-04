use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::{LapStat, TelemetryState};
use crate::trackmap::TrackMap;
use eframe::egui;
use rusttype::Font;
use tiny_skia::{Color, PixmapMut};

pub struct AdvancedLapTimer;

struct DrawContext<'a> {
    el: &'a OverlayElement,
    y_offset: f32,
    line_height: f32,
}

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

    fn draw_ui_current_time(
        painter: &egui::Painter,
        center: egui::Pos2,
        ctx: &mut DrawContext,
        state: &TelemetryState,
    ) {
        let current_text = common::get_lap_timer_text(state.current_sample.as_ref());
        painter.text(
            egui::pos2(center.x, center.y + ctx.y_offset),
            egui::Align2::CENTER_CENTER,
            current_text,
            egui::FontId::proportional(32.0 * ctx.el.scale),
            egui::Color32::WHITE,
        );
        ctx.y_offset += 36.0 * ctx.el.scale;
    }

    fn draw_ui_projection(
        painter: &egui::Painter,
        center: egui::Pos2,
        ctx: &mut DrawContext,
        diff: i64,
    ) {
        let color = if diff < 0 {
            egui::Color32::GREEN
        } else {
            egui::Color32::RED
        };
        let diff_text = Self::format_diff(diff);
        painter.text(
            egui::pos2(center.x, center.y + ctx.y_offset),
            egui::Align2::CENTER_CENTER,
            diff_text,
            egui::FontId::proportional(24.0 * ctx.el.scale),
            color,
        );
        ctx.y_offset += ctx.line_height;
    }

    fn draw_ui_best_lap(
        painter: &egui::Painter,
        center: egui::Pos2,
        ctx: &mut DrawContext,
        best: &LapStat,
    ) {
        let best_text = format!(
            "Best (L{}): {}",
            best.lap_number,
            Self::format_time(best.duration_ms)
        );
        painter.text(
            egui::pos2(center.x, center.y + ctx.y_offset),
            egui::Align2::CENTER_CENTER,
            best_text,
            egui::FontId::proportional(16.0 * ctx.el.scale),
            egui::Color32::from_rgb(255, 215, 0), // Gold
        );
        ctx.y_offset += ctx.line_height;
    }

    fn draw_ui_previous_laps(
        painter: &egui::Painter,
        center: egui::Pos2,
        ctx: &mut DrawContext,
        previous_laps: &[LapStat],
    ) {
        for (i, lap) in previous_laps.iter().enumerate() {
            let lap_text = format!(
                "L{}: {}",
                lap.lap_number,
                Self::format_time(lap.duration_ms)
            );
            let alpha = 255 - (i as u8 * 50); // fade out older laps
            painter.text(
                egui::pos2(center.x, center.y + ctx.y_offset),
                egui::Align2::CENTER_CENTER,
                lap_text,
                egui::FontId::proportional(16.0 * ctx.el.scale),
                egui::Color32::from_white_alpha(alpha),
            );
            ctx.y_offset += ctx.line_height;
        }
    }

    fn draw_skia_current_time(
        pixmap: &mut PixmapMut,
        font: &Font,
        center_x: f32,
        center_y: f32,
        res_scale: f32,
        ctx: &mut DrawContext,
        state: &TelemetryState,
    ) {
        let current_text = common::get_lap_timer_text(state.current_sample.as_ref());
        common::draw_text(
            pixmap,
            font,
            &current_text,
            center_x,
            center_y + ctx.y_offset,
            32.0 * ctx.el.scale * res_scale,
            Color::from_rgba8(255, 255, 255, 255),
        );
        ctx.y_offset += 36.0 * ctx.el.scale * res_scale;
    }

    fn draw_skia_projection(
        pixmap: &mut PixmapMut,
        font: &Font,
        center_x: f32,
        center_y: f32,
        res_scale: f32,
        ctx: &mut DrawContext,
        diff: i64,
    ) {
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
            center_y + ctx.y_offset,
            24.0 * ctx.el.scale * res_scale,
            color,
        );
        ctx.y_offset += ctx.line_height;
    }

    fn draw_skia_best_lap(
        pixmap: &mut PixmapMut,
        font: &Font,
        center_x: f32,
        center_y: f32,
        res_scale: f32,
        ctx: &mut DrawContext,
        best: &LapStat,
    ) {
        let best_text = format!(
            "Best (L{}): {}",
            best.lap_number,
            Self::format_time(best.duration_ms)
        );
        common::draw_text(
            pixmap,
            font,
            &best_text,
            center_x,
            center_y + ctx.y_offset,
            16.0 * ctx.el.scale * res_scale,
            Color::from_rgba8(255, 215, 0, 255), // Gold
        );
        ctx.y_offset += ctx.line_height;
    }

    fn draw_skia_previous_laps(
        pixmap: &mut PixmapMut,
        font: &Font,
        center_x: f32,
        center_y: f32,
        res_scale: f32,
        ctx: &mut DrawContext,
        previous_laps: &[LapStat],
    ) {
        for (i, lap) in previous_laps.iter().enumerate() {
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
                center_y + ctx.y_offset,
                16.0 * ctx.el.scale * res_scale,
                Color::from_rgba8(255, 255, 255, alpha),
            );
            ctx.y_offset += ctx.line_height;
        }
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

        let mut ctx = DrawContext {
            el,
            y_offset: 0.0,
            line_height: 24.0 * el.scale,
        };

        Self::draw_ui_current_time(&painter, center, &mut ctx, state);

        if let Some(diff) = state.projection_ms {
            Self::draw_ui_projection(&painter, center, &mut ctx, diff);
        }

        if let Some(best) = &state.best_lap {
            Self::draw_ui_best_lap(&painter, center, &mut ctx, best);
        }

        Self::draw_ui_previous_laps(&painter, center, &mut ctx, &state.previous_laps);
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
            let mut ctx = DrawContext {
                el,
                y_offset: 0.0,
                line_height: 24.0 * el.scale * res_scale,
            };

            Self::draw_skia_current_time(
                pixmap, font, center_x, center_y, res_scale, &mut ctx, state,
            );

            if let Some(diff) = state.projection_ms {
                Self::draw_skia_projection(
                    pixmap, font, center_x, center_y, res_scale, &mut ctx, diff,
                );
            }

            if let Some(best) = &state.best_lap {
                Self::draw_skia_best_lap(
                    pixmap, font, center_x, center_y, res_scale, &mut ctx, best,
                );
            }

            Self::draw_skia_previous_laps(
                pixmap,
                font,
                center_x,
                center_y,
                res_scale,
                &mut ctx,
                &state.previous_laps,
            );
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
