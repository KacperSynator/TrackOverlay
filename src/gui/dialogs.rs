use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::app::{DialogMode, MyApp};
use crate::project::ProjectConfig;
use crate::telemetry::TelemetryLog;
use crate::video::VideoPlayer;

fn handle_pick_video(app: &mut MyApp, ctx: &egui::Context, path_buf: PathBuf) {
    app.config.video_path = path_buf.clone();
    app.playhead_ms = 0;
    app.last_seek_ms = -1;

    let repaint_ctx = ctx.clone();
    match VideoPlayer::new(&path_buf, move || repaint_ctx.request_repaint()) {
        Ok(mut player) => {
            if let Some(dur) = player.duration_ms() {
                app.video_duration_ms = dur;
            }
            app.video_player = Some(player);
            app.video_error = None;
        }
        Err(e) => {
            app.video_player = None;
            app.video_error = Some(format!("Failed to load video: {}", e));
        }
    }
}

fn handle_pick_telemetry(app: &mut MyApp, path_buf: PathBuf) {
    app.config.telemetry_path = path_buf.clone();
    if let Ok(log) = TelemetryLog::load_csv(&path_buf, app.config.speed_source.clone()) {
        app.telemetry = Some(log);
        app.recalculate_telemetry();
    }
}

fn handle_pick_config_load(app: &mut MyApp, ctx: &egui::Context, path_buf: PathBuf) {
    match ProjectConfig::load(&path_buf) {
        Ok(config) => {
            app.config = config;

            // Reload video if possible
            if app.config.video_path.exists() {
                let video_path = app.config.video_path.clone();
                handle_pick_video(app, ctx, video_path);
            }

            // Reload telemetry if possible
            if app.config.telemetry_path.exists() {
                let telemetry_path = app.config.telemetry_path.clone();
                handle_pick_telemetry(app, telemetry_path);
            }
        }
        Err(e) => {
            log::error!("Failed to load config: {}", e);
            // Ideally we'd show this in the UI, but we log for now
        }
    }
}

fn handle_pick_config_save(app: &mut MyApp, path_buf: PathBuf) {
    if let Err(e) = app.config.save(&path_buf) {
        log::error!("Failed to save config: {}", e);
    }
}

fn handle_pick_append_video(app: &mut MyApp, ctx: &egui::Context, path_buf: PathBuf) {
    app.merge_progress = Some("Merging videos...".to_string());

    let original_video = app.config.video_path.clone();
    let new_video = path_buf;
    let tx = app.merge_tx.clone();
    let repaint_ctx = ctx.clone();

    std::thread::spawn(move || {
        let result = crate::merge::merge_videos(&original_video, &new_video);
        let _ = tx.send(result);
        repaint_ctx.request_repaint();
    });
}

fn handle_pick_export_output(app: &mut MyApp, path_buf: PathBuf) {
    let config_clone = app.config.clone();
    let telem_clone = if let Some(t) = &app.telemetry {
        TelemetryLog {
            samples: t.samples.clone(),
            start_time_utc: t.start_time_utc,
            parsed_speed_source: t.parsed_speed_source.clone(),
        }
    } else {
        TelemetryLog {
            samples: vec![],
            start_time_utc: None,
            parsed_speed_source: crate::project::SpeedSource::Auto,
        }
    };

    app.export_progress = Some(format!("Exporting to {:?}...", path_buf));

    let progress_arc = Arc::new(Mutex::new(crate::export::ExportProgress::default()));
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

pub fn handle_dialogs(app: &mut MyApp, ctx: &egui::Context) {
    // Update the file dialog
    app.file_dialog.update(ctx);

    // Check if a file was picked
    if let Some(path) = app.file_dialog.take_picked() {
        let path_buf = path.to_path_buf();
        match app.dialog_mode {
            DialogMode::PickVideo => handle_pick_video(app, ctx, path_buf),
            DialogMode::AppendVideo => handle_pick_append_video(app, ctx, path_buf),
            DialogMode::PickTelemetry => handle_pick_telemetry(app, path_buf),
            DialogMode::PickExportOutput => handle_pick_export_output(app, path_buf),
            DialogMode::PickConfigLoad => handle_pick_config_load(app, ctx, path_buf),
            DialogMode::PickConfigSave => handle_pick_config_save(app, path_buf),
            DialogMode::None => {}
        }
        app.dialog_mode = DialogMode::None;
    }
}
