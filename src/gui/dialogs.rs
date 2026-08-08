use eframe::egui;
use std::sync::{Arc, Mutex};

use crate::app::{DialogMode, MyApp};
use crate::telemetry::TelemetryLog;
use crate::video::VideoPlayer;

pub fn handle_dialogs(app: &mut MyApp, ctx: &egui::Context) {
    // Update the file dialog
    app.file_dialog.update(ctx);

    // Check if a file was picked
    if let Some(path) = app.file_dialog.take_picked() {
        let path_buf = path.to_path_buf();
        match app.dialog_mode {
            DialogMode::PickVideo => {
                app.config.video_path = path_buf.clone();
                app.playhead_ms = 0;
                app.last_seek_ms = -1;

                let repaint_ctx = ctx.clone();
                if let Ok(mut player) =
                    VideoPlayer::new(&path_buf, move || repaint_ctx.request_repaint())
                {
                    if let Some(dur) = player.duration_ms() {
                        app.video_duration_ms = dur;
                    }
                    app.video_player = Some(player);
                }
            }
            DialogMode::PickTelemetry => {
                app.config.telemetry_path = path_buf.clone();
                if let Ok(log) = TelemetryLog::load_csv(&path_buf) {
                    app.telemetry = Some(log);
                    app.recalculate_telemetry();
                }
            }
            DialogMode::PickExportOutput => {
                let config_clone = app.config.clone();
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

                app.export_progress = Some(format!("Exporting to {:?}...", path_buf));

                let progress_arc =
                    Arc::new(Mutex::new(crate::export::ExportProgress::default()));
                app.active_export_progress = Some(progress_arc.clone());
                let path_clone = path_buf.clone();
                let tx = app.export_tx.clone();
                std::thread::spawn(move || {
                    let res = crate::export::export_video(
                        &config_clone,
                        &telem_clone,
                        &path_clone,
                        Some(progress_arc),
                    );
                    let _ = tx.send(res);
                });
                app.export_progress = Some("Exporting in background...".to_string());
            }
            DialogMode::None => {}
        }
        app.dialog_mode = DialogMode::None;
    }
}
