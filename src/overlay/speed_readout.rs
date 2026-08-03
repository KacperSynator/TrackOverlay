use crate::overlay::{OverlayImpl, common};
use crate::project::OverlayElement;
use crate::telemetry::TelemetrySample;
use crate::trackmap::TrackMap;
use eframe::egui;
use rusttype::Font;
use tiny_skia::{Color, PixmapMut};

pub struct SpeedReadout;

impl OverlayImpl for SpeedReadout {
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

        let text = common::get_speed_text(sample);
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(32.0 * el.scale),
            egui::Color32::WHITE,
        );
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

        let font_data = include_bytes!("../font.ttf");
        let font_opt = Font::try_from_bytes(font_data as &[u8]);

        let text = common::get_speed_text(sample);
        if let Some(font) = &font_opt {
            common::draw_text(
                pixmap,
                font,
                &text,
                center_x,
                center_y,
                32.0 * el.scale * res_scale,
                Color::WHITE,
            );
        } else {
            common::draw_text_fallback(
                pixmap,
                center_x,
                center_y,
                100.0 * el.scale * res_scale,
                30.0 * el.scale * res_scale,
                Color::WHITE,
            );
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
            kind: crate::project::OverlayKind::SpeedReadout,
            x: 0.5,
            y: 0.5,
            scale: 1.0,
        }
    }

    #[test]
    fn test_speed_readout_render_skia() {
        let el = create_test_element();
        let mut data = vec![0; 800 * 600 * 4];
        let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

        let readout = SpeedReadout;
        readout.render_skia(&mut pixmap, &el, None, None);

        let sample = crate::overlay::common::create_test_sample();
        readout.render_skia(&mut pixmap, &el, Some(&sample), None);
    }

    #[test]
    fn test_speed_readout_render_ui() {
        let el = create_test_element();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show_inside(ctx, |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

                let readout = SpeedReadout;
                readout.render_ui(ui, rect, &el, None, None);

                let sample = crate::overlay::common::create_test_sample();
                readout.render_ui(ui, rect, &el, Some(&sample), None);
            });
        });
    }
}
