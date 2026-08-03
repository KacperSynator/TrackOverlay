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
