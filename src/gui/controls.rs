use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::app::{DialogMode, MyApp};
use crate::gpmf_extract::extract_gopro_gps;
use crate::gui::common::{format_time_str, parse_time_str};
use crate::project::SyncMode;
use crate::sync::auto_correlate_gps;
use crate::telemetry::TelemetryLog;

fn render_load_video(app: &mut MyApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button("Load Video").clicked() {
            app.dialog_mode = DialogMode::PickVideo;
            app.file_dialog.pick_file();
        }
        ui.label(
            app.config
                .video_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );
    });
    if let Some(vp) = &app.video_player {
        if let Some(dt) = vp.creation_time_utc {
            ui.label(format!(
                "  Timestamp: {}",
                dt.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }
        ui.label(format!("  Duration: {}s", app.video_duration_ms / 1000));
    }
}

fn render_load_telemetry(app: &mut MyApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button("Load Telemetry").clicked() {
            app.dialog_mode = DialogMode::PickTelemetry;
            app.file_dialog.pick_file();
        }
        ui.label(
            app.config
                .telemetry_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );
    });
    if let Some(telem) = &app.telemetry {
        if let Some(dt) = telem.start_time_utc {
            ui.label(format!(
                "  Timestamp: {}",
                dt.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }
        if !telem.samples.is_empty() {
            let telem_dur =
                telem.samples.last().unwrap().time_ms - telem.samples.first().unwrap().time_ms;
            ui.label(format!("  Data Length: {}s", telem_dur / 1000));
        }

        if !app.telemetry_laps.is_empty() {
            ui.collapsing("Laps", |ui| {
                for (lap_num, start_time) in &app.telemetry_laps {
                    if ui
                        .button(format!("Jump to Lap {} ({}s)", lap_num, start_time / 1000))
                        .clicked()
                    {
                        let target_playhead = start_time - app.config.sync.offset_ms;
                        if target_playhead >= 0 && target_playhead <= app.video_duration_ms {
                            app.playhead_ms = target_playhead;
                        }
                    }
                }
            });
        }
    }
}

fn render_project_files_section(app: &mut MyApp, ui: &mut egui::Ui) {
    ui.heading("Project Files");
    render_load_video(app, ui);
    ui.add_space(10.0);
    render_load_telemetry(app, ui);
}

fn render_settings_section(app: &mut MyApp, ui: &mut egui::Ui) {
    ui.heading("Settings");
    ui.checkbox(&mut app.config.flip_vertical, "Flip Video Vertically");
    ui.checkbox(&mut app.config.flip_horizontal, "Flip Video Horizontally");
}

fn render_export_start(app: &mut MyApp, ui: &mut egui::Ui) -> bool {
    let mut needs_telemetry_recalc = false;
    ui.horizontal(|ui| {
        let mut start_sec = app.config.export_start_ms.unwrap_or(0) as f64 / 1000.0;
        let response = ui.add(
            egui::DragValue::new(&mut start_sec)
                .speed(0.1)
                .prefix("Start: ")
                .custom_parser(parse_time_str)
                .custom_formatter(|n, _| format_time_str(n)),
        );

        if response.changed() {
            app.config.export_start_ms = Some((start_sec * 1000.0) as i64);
            app.clamp_playhead_to_trim();
        }

        let is_active = response.has_focus() || response.dragged();
        if app.export_start_was_active && !is_active {
            needs_telemetry_recalc = true;
        }
        app.export_start_was_active = is_active;

        if ui.button("Jump").clicked() {
            app.playhead_ms = app.config.export_start_ms.unwrap_or(0);
        }
    });
    needs_telemetry_recalc
}

fn render_export_end(app: &mut MyApp, ui: &mut egui::Ui) -> bool {
    let mut needs_telemetry_recalc = false;
    ui.horizontal(|ui| {
        let mut end_sec = if let Some(ms) = app.config.export_end_ms {
            if ms >= 0 { ms as f64 / 1000.0 } else { -1.0 }
        } else {
            -1.0
        };

        let response = ui.add(
            egui::DragValue::new(&mut end_sec)
                .speed(0.1)
                .prefix("End: ")
                .custom_parser(parse_time_str)
                .custom_formatter(|n, _| format_time_str(n)),
        );

        if response.changed() {
            app.config.export_end_ms = if end_sec < 0.0 {
                Some(-1)
            } else {
                Some((end_sec * 1000.0) as i64)
            };

            app.clamp_playhead_to_trim();
        }
        let end_active = response.has_focus() || response.dragged();
        if app.export_end_was_active && !end_active {
            needs_telemetry_recalc = true;
        }

        app.export_end_was_active = end_active;

        if ui.button("Jump").clicked() {
            if let Some(end_val) = app.config.export_end_ms {
                if end_val >= 0 {
                    app.playhead_ms = end_val;
                } else {
                    app.playhead_ms = app.video_duration_ms;
                }
            } else {
                app.playhead_ms = app.video_duration_ms;
            }
        }
        ui.label("(-1 for end)");
    });
    needs_telemetry_recalc
}

fn render_export_progress(app: &MyApp, ui: &mut egui::Ui) {
    if let Some(arc) = &app.active_export_progress {
        if let Ok(lock) = arc.lock() {
            let done = lock.frames_done;
            let total = lock.total_frames.max(1);
            let progress_pct = (done as f32 / total as f32).clamp(0.0, 1.0);

            ui.add(
                egui::ProgressBar::new(progress_pct).text(format!("{:.1}%", progress_pct * 100.0)),
            );

            let elapsed_s = lock.start_time.map_or(0.0, |t| t.elapsed().as_secs_f32());

            let fps = if elapsed_s > 0.0 {
                done as f32 / elapsed_s
            } else {
                0.0
            };

            let remaining = total.saturating_sub(done);
            let eta_s = if fps > 0.0 {
                remaining as f32 / fps
            } else {
                0.0
            };

            ui.label(format!("Frames: {} / {}", done, total));
            ui.label(format!("Speed: {:.1} fps", fps));

            let elapsed_str = format!(
                "{:02}:{:02}",
                (elapsed_s / 60.0).floor(),
                (elapsed_s % 60.0).floor()
            );
            let eta_str = format!(
                "{:02}:{:02}",
                (eta_s / 60.0).floor(),
                (eta_s % 60.0).floor()
            );

            ui.label(format!("Elapsed: {} | ETA: {}", elapsed_str, eta_str));
        }
    } else if let Some(msg) = &app.export_progress {
        ui.label(msg);
    }
}

fn render_export_section(app: &mut MyApp, ui: &mut egui::Ui) -> bool {
    ui.heading("Export");
    let needs_telemetry_recalc_start = render_export_start(app, ui);
    let needs_telemetry_recalc_end = render_export_end(app, ui);

    if ui.button("Export Final Video").clicked() {
        app.dialog_mode = DialogMode::PickExportOutput;
        app.file_dialog.save_file();
    }

    render_export_progress(app, ui);

    needs_telemetry_recalc_start || needs_telemetry_recalc_end
}

fn render_auto_sync(app: &mut MyApp, ui: &mut egui::Ui) -> bool {
    let mut needs_telemetry_recalc = false;
    if app.auto_sync_progress.is_none() {
        if ui.button("Run Auto-Sync").clicked() {
            let progress = Arc::new(Mutex::new(None));
            app.auto_sync_progress = Some(progress.clone());

            let video_path = app.config.video_path.to_string_lossy().to_string();
            let telem_clone = if let Some(t) = &app.telemetry {
                TelemetryLog {
                    samples: t.samples.clone(),
                    start_time_utc: t.start_time_utc,
                }
            } else {
                TelemetryLog {
                    samples: vec![],
                    start_time_utc: None,
                }
            };
            let max_auto_sync_offset_ms = app.config.sync.max_auto_sync_offset_ms;

            thread::spawn(move || {
                if let Ok(gps_data) = extract_gopro_gps(&video_path)
                    && let Some(offset) =
                        auto_correlate_gps(&gps_data, &telem_clone, max_auto_sync_offset_ms)
                    && let Ok(mut lock) = progress.lock()
                {
                    *lock = Some(offset);
                }
            });
        }
    } else {
        let mut done = false;
        if let Ok(lock) = app.auto_sync_progress.as_ref().unwrap().lock()
            && let Some(offset) = *lock
        {
            app.config.sync.offset_ms = offset;
            done = true;
            needs_telemetry_recalc = true;
        }
        if done {
            app.auto_sync_progress = None;
        } else {
            ui.label("Syncing...");
            ui.ctx().request_repaint(); // ensure we re-draw to check progress
        }
    }

    ui.label(format!("Computed offset: {} ms", app.config.sync.offset_ms));
    needs_telemetry_recalc
}

fn render_manual_sync(app: &mut MyApp, ui: &mut egui::Ui) -> bool {
    let mut needs_telemetry_recalc = false;
    if ui
        .add(
            egui::Slider::new(&mut app.config.sync.offset_ms, -120000..=120000)
                .text("Sync Offset (ms)"),
        )
        .changed()
    {
        needs_telemetry_recalc = true;
    }
    needs_telemetry_recalc
}

fn render_sync_section(app: &mut MyApp, ui: &mut egui::Ui) -> bool {
    ui.heading("Sync");

    ui.horizontal(|ui| {
        ui.radio_value(&mut app.config.sync.mode, SyncMode::Manual, "Manual Sync");
        ui.radio_value(&mut app.config.sync.mode, SyncMode::Auto, "Auto Sync");
    });

    if app.config.sync.mode == SyncMode::Auto {
        render_auto_sync(app, ui)
    } else {
        render_manual_sync(app, ui)
    }
}

fn render_layout_editor_section(app: &mut MyApp, ui: &mut egui::Ui) {
    ui.label("Layout Editor");

    for el in app.config.elements.iter_mut() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut el.enabled, format!("{:?}", el.kind));
            ui.add(egui::Slider::new(&mut el.x, 0.0..=1.0).text("X"));
            ui.add(egui::Slider::new(&mut el.y, 0.0..=1.0).text("Y"));
            ui.add(egui::Slider::new(&mut el.scale, 0.5..=3.0).text("Scale"));
        });

        crate::overlay::get_impl(&el.kind).custom_ui(ui, el);
    }
}

pub fn render_controls_window(app: &mut MyApp, ctx: &egui::Context) {
    egui::Window::new("Controls").show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            render_project_files_section(app, ui);
            ui.separator();

            render_settings_section(app, ui);
            ui.separator();

            let mut needs_telemetry_recalc = render_export_section(app, ui);
            ui.separator();

            needs_telemetry_recalc |= render_sync_section(app, ui);

            if needs_telemetry_recalc {
                app.recalculate_telemetry();
            }

            ui.separator();
            render_layout_editor_section(app, ui);
        });
    });
}
