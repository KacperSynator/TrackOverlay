use eframe::egui;
use log::warn;

use crate::app::MyApp;
use crate::overlay::render_overlay;
use crate::telemetry::TelemetryState;
use crate::telemetry::TelemetryView;

pub fn render_video_panel(app: &mut MyApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        let rect = ui.available_rect_before_wrap();
        let mut available_video_area = rect;

        // Bottom controls height
        let controls_height = 40.0;
        available_video_area.set_bottom(rect.bottom() - controls_height);

        ui.painter().rect_filled(
            available_video_area,
            0.0,
            egui::Color32::from_rgb(20, 20, 20),
        );

        // Fetch frame from video player
        if let Some(player) = &mut app.video_player {
            if app.playhead_ms != app.last_seek_ms {
                if let Err(e) = player.seek(app.playhead_ms) {
                    warn!("Seek error: {}", e);
                }
                app.last_seek_ms = app.playhead_ms;
            }

            if let Some(frame) = player.get_frame() {
                let w = frame.width as usize;
                let h = frame.height as usize;

                if w > 0 && h > 0 {
                    let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &frame.data);

                    let texture = ui.ctx().load_texture(
                        "video_frame",
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    app.video_texture = Some(texture);
                }
            }
        }

        // By default, assume 16:9
        let mut aspect = 16.0 / 9.0;
        if let Some(tex) = &app.video_texture {
            aspect = tex.aspect_ratio();
        }

        let mut w = available_video_area.width();
        let mut h = w / aspect;
        if h > available_video_area.height() {
            h = available_video_area.height();
            w = h * aspect;
        }

        let center = available_video_area.center();
        let draw_rect = egui::Rect::from_center_size(center, egui::vec2(w, h));

        // Draw the video texture if available
        if let Some(tex) = &app.video_texture {
            let mut min_pos = egui::pos2(0.0, 0.0);
            let mut max_pos = egui::pos2(1.0, 1.0);

            if app.config.flip_horizontal {
                std::mem::swap(&mut min_pos.x, &mut max_pos.x);
            }
            if app.config.flip_vertical {
                std::mem::swap(&mut min_pos.y, &mut max_pos.y);
            }

            ui.painter().image(
                tex.id(),
                draw_rect,
                egui::Rect::from_min_max(min_pos, max_pos),
                egui::Color32::WHITE,
            );
        }

        let state = if let Some(log) = &app.telemetry {
            let view = TelemetryView::new(
                log,
                app.config.export_start_ms,
                app.config.export_end_ms,
                app.config.sync.offset_ms,
            );
            // The telemetry viewer expects absolute telemetry time to evaluate states against.
            // However, playhead_ms tracks video time. Therefore, to match telemetry time,
            // we add the sync offset to the playhead video time.
            let telemetry_time_ms = app.playhead_ms + app.config.sync.offset_ms;
            view.get_state(telemetry_time_ms)
        } else {
            TelemetryState {
                current_sample: None,
                previous_laps: vec![],
                best_lap: None,
                projection_ms: None,
            }
        };

        // Bind the telemetry overlay rendering entirely to the draw_rect of the video stream
        render_overlay(
            ui,
            draw_rect,
            &mut app.config.elements,
            &state,
            app.trackmap.as_ref(),
            false,
        );

        let mut control_rect = rect;
        control_rect.set_top(rect.bottom() - controls_height);

        // Constrain controls to the same width as the video so it aligns nicely
        let mut centered_controls = control_rect;
        centered_controls.set_left(draw_rect.left());
        centered_controls.set_right(draw_rect.right());

        ui.scope_builder(egui::UiBuilder::new().max_rect(centered_controls), |ui| {
            ui.horizontal(|ui| {
                let btn_text = if app.is_playing { "Pause" } else { "Play" };
                if ui.button(btn_text).clicked() {
                    app.is_playing = !app.is_playing;
                }

                ui.label(format!(
                    "{} / {}",
                    MyApp::format_time(app.playhead_ms),
                    MyApp::format_time(app.video_duration_ms)
                ));

                let remaining = ui.available_width();
                ui.spacing_mut().slider_width = remaining.max(0.0);

                let slider =
                    egui::Slider::new(&mut app.playhead_ms, 0..=app.video_duration_ms)
                        .show_value(false)
                        .trailing_fill(true)
                        .clamping(egui::SliderClamping::Edits);

                let response = ui.add(slider);

                let rect = response.rect;
                let duration = app.video_duration_ms.max(1) as f32;

                let start_ms = app.config.export_start_ms.unwrap_or(0);
                let end_ms = match app.config.export_end_ms {
                    Some(e) if e >= 0 => e,
                    _ => app.video_duration_ms,
                };

                // Clamp the playhead itself whenever the user drags/clicks the slider,
                // so it can't land inside the trimmed-out (red) regions.
                if response.dragged() || response.changed() {
                    app.playhead_ms = app.playhead_ms.clamp(start_ms, end_ms);
                }

                // --- visual overlay (as before, using f32 versions) ---
                let start_ms_f = start_ms as f32;
                let end_ms_f = end_ms as f32;

                let x_at = |ms: f32| -> f32 {
                    egui::remap_clamp(ms, 0.0..=duration, rect.left()..=rect.right())
                };

                let painter = ui.painter();
                let dim_color = egui::Color32::from_rgba_unmultiplied(200, 40, 40, 90);
                let y_range = rect.top()..=rect.bottom();

                if start_ms_f > 0.0 {
                    let r = egui::Rect::from_x_y_ranges(
                        rect.left()..=x_at(start_ms_f),
                        y_range.clone(),
                    );
                    painter.rect_filled(r, 0.0, dim_color);
                }
                if end_ms_f < duration {
                    let r = egui::Rect::from_x_y_ranges(
                        x_at(end_ms_f)..=rect.right(),
                        y_range.clone(),
                    );
                    painter.rect_filled(r, 0.0, dim_color);
                }
            });
        });
    });
}
