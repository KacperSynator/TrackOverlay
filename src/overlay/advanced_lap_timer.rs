use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::{LapStat, TelemetryState};
use crate::trackmap::TrackMap;
use eframe::egui;
use rusttype::Font;
use tiny_skia::{Color, PixmapMut};

pub struct AdvancedLapTimer;

struct UiDrawContext<'a> {
    painter: &'a egui::Painter,
    center: egui::Pos2,
    el: &'a OverlayElement,
    y_offset: f32,
    line_height: f32,
}

struct SkiaDrawContext<'a, 'b> {
    pixmap: &'a mut PixmapMut<'b>,
    font: &'a Font<'a>,
    center_x: f32,
    center_y: f32,
    res_scale: f32,
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

    fn draw_ui_current_time(ctx: &mut UiDrawContext, state: &TelemetryState) {
        let current_text = common::get_lap_timer_text(state.current_sample.as_ref());
        ctx.painter.text(
            egui::pos2(ctx.center.x, ctx.center.y + ctx.y_offset),
            egui::Align2::CENTER_CENTER,
            current_text,
            egui::FontId::proportional(32.0 * ctx.el.scale),
            egui::Color32::WHITE,
        );
        ctx.y_offset += 36.0 * ctx.el.scale;
    }

    fn draw_ui_projection(ctx: &mut UiDrawContext, diff: i64) {
        let color = if diff < 0 {
            egui::Color32::GREEN
        } else {
            egui::Color32::RED
        };
        let diff_text = Self::format_diff(diff);
        ctx.painter.text(
            egui::pos2(ctx.center.x, ctx.center.y + ctx.y_offset),
            egui::Align2::CENTER_CENTER,
            diff_text,
            egui::FontId::proportional(24.0 * ctx.el.scale),
            color,
        );
        ctx.y_offset += ctx.line_height;
    }

    fn draw_ui_best_lap(ctx: &mut UiDrawContext, best: &LapStat) {
        let best_text = format!(
            "Best (L{}): {}",
            best.lap_number,
            Self::format_time(best.duration_ms)
        );
        ctx.painter.text(
            egui::pos2(ctx.center.x, ctx.center.y + ctx.y_offset),
            egui::Align2::CENTER_CENTER,
            best_text,
            egui::FontId::proportional(16.0 * ctx.el.scale),
            egui::Color32::from_rgb(255, 215, 0), // Gold
        );
        ctx.y_offset += ctx.line_height;
    }

    fn draw_ui_previous_laps(ctx: &mut UiDrawContext, previous_laps: &[LapStat]) {
        for (i, lap) in previous_laps.iter().enumerate() {
            let lap_text = format!(
                "L{}: {}",
                lap.lap_number,
                Self::format_time(lap.duration_ms)
            );
            let alpha = 255 - (i as u8 * 50); // fade out older laps
            ctx.painter.text(
                egui::pos2(ctx.center.x, ctx.center.y + ctx.y_offset),
                egui::Align2::CENTER_CENTER,
                lap_text,
                egui::FontId::proportional(16.0 * ctx.el.scale),
                egui::Color32::from_white_alpha(alpha),
            );
            ctx.y_offset += ctx.line_height;
        }
    }

    fn draw_skia_current_time(ctx: &mut SkiaDrawContext, state: &TelemetryState) {
        let current_text = common::get_lap_timer_text(state.current_sample.as_ref());
        common::draw_text(
            ctx.pixmap,
            ctx.font,
            &current_text,
            ctx.center_x,
            ctx.center_y + ctx.y_offset,
            32.0 * ctx.el.scale * ctx.res_scale,
            Color::from_rgba8(255, 255, 255, 255),
        );
        ctx.y_offset += 36.0 * ctx.el.scale * ctx.res_scale;
    }

    fn draw_skia_projection(ctx: &mut SkiaDrawContext, diff: i64) {
        let color = if diff < 0 {
            Color::from_rgba8(0, 255, 0, 255) // Green
        } else {
            Color::from_rgba8(255, 0, 0, 255) // Red
        };
        let diff_text = Self::format_diff(diff);
        common::draw_text(
            ctx.pixmap,
            ctx.font,
            &diff_text,
            ctx.center_x,
            ctx.center_y + ctx.y_offset,
            24.0 * ctx.el.scale * ctx.res_scale,
            color,
        );
        ctx.y_offset += ctx.line_height;
    }

    fn draw_skia_best_lap(ctx: &mut SkiaDrawContext, best: &LapStat) {
        let best_text = format!(
            "Best (L{}): {}",
            best.lap_number,
            Self::format_time(best.duration_ms)
        );
        common::draw_text(
            ctx.pixmap,
            ctx.font,
            &best_text,
            ctx.center_x,
            ctx.center_y + ctx.y_offset,
            16.0 * ctx.el.scale * ctx.res_scale,
            Color::from_rgba8(255, 215, 0, 255), // Gold
        );
        ctx.y_offset += ctx.line_height;
    }

    fn draw_skia_previous_laps(ctx: &mut SkiaDrawContext, previous_laps: &[LapStat]) {
        for (i, lap) in previous_laps.iter().enumerate() {
            let lap_text = format!(
                "L{}: {}",
                lap.lap_number,
                Self::format_time(lap.duration_ms)
            );
            let alpha = 255 - (i as u8 * 50);
            common::draw_text(
                ctx.pixmap,
                ctx.font,
                &lap_text,
                ctx.center_x,
                ctx.center_y + ctx.y_offset,
                16.0 * ctx.el.scale * ctx.res_scale,
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

        let mut ctx = UiDrawContext {
            painter: &painter,
            center,
            el,
            y_offset: 0.0,
            line_height: 24.0 * el.scale,
        };

        Self::draw_ui_current_time(&mut ctx, state);

        if let Some(diff) = state.projection_ms {
            Self::draw_ui_projection(&mut ctx, diff);
        }

        if let Some(best) = &state.best_lap {
            Self::draw_ui_best_lap(&mut ctx, best);
        }

        Self::draw_ui_previous_laps(&mut ctx, &state.previous_laps);
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
            let mut ctx = SkiaDrawContext {
                pixmap,
                font,
                center_x,
                center_y,
                res_scale,
                el,
                y_offset: 0.0,
                line_height: 24.0 * el.scale * res_scale,
            };

            Self::draw_skia_current_time(&mut ctx, state);

            if let Some(diff) = state.projection_ms {
                Self::draw_skia_projection(&mut ctx, diff);
            }

            if let Some(best) = &state.best_lap {
                Self::draw_skia_best_lap(&mut ctx, best);
            }

            Self::draw_skia_previous_laps(&mut ctx, &state.previous_laps);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::OverlayKind;
    use crate::telemetry::TelemetrySample;

    fn create_test_state() -> TelemetryState {
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
            session_distance_m: 1000.0,
            lap_distance_m: 500.0,
        };

        let best_lap = LapStat {
            lap_number: 1,
            start_time_ms: 0,
            end_time_ms: 90000,
            duration_ms: 90000,
            total_distance_m: 2000.0,
        };

        let previous_laps = vec![best_lap.clone()];

        TelemetryState {
            current_sample: Some(sample),
            previous_laps,
            best_lap: Some(best_lap),
            projection_ms: Some(-1500),
        }
    }

    #[test]
    fn test_advanced_lap_timer_render_skia() {
        let timer = AdvancedLapTimer;
        let mut data = vec![0; 800 * 600 * 4];
        let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

        let el = OverlayElement {
            enabled: true,
            kind: OverlayKind::AdvancedLapTimer,
            x: 0.5,
            y: 0.5,
            scale: 1.0,
        };

        let state = create_test_state();

        // With font
        timer.render_skia(&mut pixmap, &el, &state, None);

        // Ensure it doesn't panic on missing fields
        let empty_state = TelemetryState {
            current_sample: None,
            previous_laps: vec![],
            best_lap: None,
            projection_ms: None,
        };
        timer.render_skia(&mut pixmap, &el, &empty_state, None);
    }

    #[test]
    fn test_advanced_lap_timer_render_ui() {
        let timer = AdvancedLapTimer;

        let el = OverlayElement {
            enabled: true,
            kind: OverlayKind::AdvancedLapTimer,
            x: 0.5,
            y: 0.5,
            scale: 1.0,
        };

        let state = create_test_state();
        let empty_state = TelemetryState {
            current_sample: None,
            previous_laps: vec![],
            best_lap: None,
            projection_ms: None,
        };

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show_inside(ctx, |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
                timer.render_ui(ui, rect, &el, &state, None);
                timer.render_ui(ui, rect, &el, &empty_state, None);
            });
        });
    }
}
