use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::TelemetryState;
use crate::trackmap::TrackMap;
use eframe::egui;
use tiny_skia::{Color, PixmapMut};

pub struct LapTimer;

impl OverlayImpl for LapTimer {
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

        let text = common::get_lap_timer_text(state.current_sample.as_ref());

        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(24.0 * el.scale),
            egui::Color32::YELLOW,
        );
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

        let text = common::get_lap_timer_text(state.current_sample.as_ref());
        if let Some(font) = font_opt {
            common::draw_text(
                pixmap,
                font,
                &text,
                center_x,
                center_y,
                24.0 * el.scale * res_scale,
                Color::from_rgba8(255, 255, 0, 255),
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
