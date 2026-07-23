use crate::project::{OverlayElement, OverlayKind};
use crate::telemetry::TelemetrySample;
use crate::trackmap::TrackMap;
use eframe::egui;

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
                let speed = sample.map_or(0.0, |s| s.speed_kph);
                let text = format!("{:.0} km/h", speed);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(32.0 * el.scale),
                    egui::Color32::WHITE,
                );
            }
            OverlayKind::GForceMeter => {
                let lat_g = sample.map_or(0.0, |s| s.accel_lat_g);
                let lon_g = sample.map_or(0.0, |s| s.accel_lon_g);

                let radius = 40.0 * el.scale;
                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(2.0 * el.scale, egui::Color32::WHITE),
                );

                let dot_pos = center + egui::vec2(lat_g * radius, -lon_g * radius);
                painter.circle_filled(dot_pos, 5.0 * el.scale, egui::Color32::RED);
            }
            OverlayKind::LapTimer => {
                let time_ms = sample.and_then(|s| s.lap_time_ms).unwrap_or(0);
                let seconds = time_ms as f64 / 1000.0;
                let mins = (seconds / 60.0).floor() as i32;
                let secs = seconds % 60.0;
                let text = format!("{:02}:{:05.2}", mins, secs);

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
                    // Draw map bounds based on element scale
                    let map_size = 150.0 * el.scale;
                    let map_rect =
                        egui::Rect::from_center_size(center, egui::vec2(map_size, map_size));

                    // Draw outline
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

                    // Draw start/finish line and dot
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

                    // Draw car live position dot based on interpolated time
                    if let Some(s) = sample
                        && let Some((cx, cy)) = map.point_at_time(s.time_ms)
                    {
                        let dot_pos = egui::pos2(
                            map_rect.left() + cx * map_rect.width(),
                            map_rect.top() + cy * map_rect.height(),
                        );
                        painter.circle_filled(dot_pos, 4.0 * el.scale, egui::Color32::RED);
                    }
                }
            }
            OverlayKind::ThrottleBar => {
                let throttle = sample.map_or(0.0, |s| s.throttle_pct).clamp(0.0, 100.0) / 100.0;

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
