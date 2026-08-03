use crate::project::{OverlayElement, OverlayKind};
use crate::telemetry::TelemetrySample;
use crate::trackmap::TrackMap;
use eframe::egui;
use tiny_skia::PixmapMut;

pub mod common;
pub mod gforce_meter;
pub mod lap_timer;
pub mod speed_readout;
pub mod throttle_bar;
pub mod track_map;

pub trait OverlayImpl {
    fn render_ui(
        &self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        el: &OverlayElement,
        sample: Option<&TelemetrySample>,
        trackmap: Option<&TrackMap>,
    );
    fn render_skia(
        &self,
        pixmap: &mut PixmapMut,
        el: &OverlayElement,
        sample: Option<&TelemetrySample>,
        trackmap: Option<&TrackMap>,
    );
}

fn get_impl(kind: &OverlayKind) -> Box<dyn OverlayImpl> {
    match kind {
        OverlayKind::SpeedReadout => Box::new(speed_readout::SpeedReadout),
        OverlayKind::GForceMeter => Box::new(gforce_meter::GForceMeter),
        OverlayKind::LapTimer => Box::new(lap_timer::LapTimer),
        OverlayKind::TrackMap => Box::new(track_map::TrackMapOverlay),
        OverlayKind::ThrottleBar => Box::new(throttle_bar::ThrottleBar),
    }
}

pub fn render_overlay(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    elements: &mut [OverlayElement],
    sample: Option<&TelemetrySample>,
    trackmap: Option<&TrackMap>,
    _is_dragging: bool,
) {
    for el in elements.iter_mut() {
        if el.enabled {
            let implementation = get_impl(&el.kind);
            implementation.render_ui(ui, rect, el, sample, trackmap);
        }
    }
}

pub fn render_overlay_skia(
    pixmap: &mut PixmapMut,
    elements: &[OverlayElement],
    sample: Option<&TelemetrySample>,
    trackmap: Option<&TrackMap>,
) {
    for el in elements.iter() {
        if el.enabled {
            let implementation = get_impl(&el.kind);
            implementation.render_skia(pixmap, el, sample, trackmap);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    pub fn create_test_elements() -> Vec<OverlayElement> {
        vec![
            OverlayElement {
                enabled: true,
                kind: OverlayKind::SpeedReadout,
                x: 0.5,
                y: 0.5,
                scale: 1.0,
            },
            OverlayElement {
                enabled: false,
                kind: OverlayKind::GForceMeter,
                x: 0.5,
                y: 0.5,
                scale: 1.0,
            },
        ]
    }

    #[test]
    fn test_render_overlay_skia() {
        let mut data = vec![0; 800 * 600 * 4];
        let mut pixmap = PixmapMut::from_bytes(&mut data, 800, 600).unwrap();

        let elements = create_test_elements();

        render_overlay_skia(&mut pixmap, &elements, None, None);
    }

    #[test]
    fn test_render_overlay_ui() {
        let mut elements = create_test_elements();

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show_inside(ctx, |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
                render_overlay(ui, rect, &mut elements, None, None, false);
            });
        });
    }
}
