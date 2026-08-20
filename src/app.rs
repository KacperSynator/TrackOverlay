use eframe::egui;
use egui_file_dialog::FileDialog;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::gui::controls::render_controls_window;
use crate::gui::dialogs::handle_dialogs;
use crate::gui::video_panel::render_video_panel;
use crate::project::ProjectConfig;
use crate::telemetry::TelemetryLog;
use crate::trackmap::TrackMap;
use crate::video::VideoPlayer;

#[derive(PartialEq)]
pub enum DialogMode {
    None,
    PickVideo,
    PickTelemetry,
    PickExportOutput,
}

pub struct MyApp {
    pub config: ProjectConfig,
    pub telemetry: Option<TelemetryLog>,
    pub trackmap: Option<TrackMap>,
    pub playhead_ms: i64,
    pub is_playing: bool,
    pub auto_sync_progress: Option<Arc<Mutex<Option<i64>>>>,
    pub export_progress: Option<String>,
    pub active_export_progress: Option<Arc<Mutex<crate::export::ExportProgress>>>,
    pub export_rx: crossbeam_channel::Receiver<anyhow::Result<()>>,
    pub export_tx: crossbeam_channel::Sender<anyhow::Result<()>>,
    pub export_start_was_active: bool,
    pub export_end_was_active: bool,

    pub file_dialog: FileDialog,
    pub dialog_mode: DialogMode,

    pub video_player: Option<VideoPlayer>,
    pub video_error: Option<String>,
    pub video_texture: Option<egui::TextureHandle>,
    pub last_seek_ms: i64,
    pub video_duration_ms: i64,

    pub telemetry_laps: Vec<(u32, i64)>, // Lap number, start_time_ms
}

impl MyApp {
    pub fn new(config: ProjectConfig, data_dir: Option<PathBuf>) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut fd = FileDialog::new().default_size([600.0, 400.0]);

        if let Some(ref dir) = data_dir {
            fd = fd.initial_directory(dir.clone());
        }

        Self {
            config,
            telemetry: None,
            trackmap: None,
            playhead_ms: 0,
            is_playing: false,
            auto_sync_progress: None,
            export_progress: None,
            active_export_progress: None,
            export_rx: rx,
            export_tx: tx,
            export_start_was_active: false,
            export_end_was_active: false,
            file_dialog: fd,
            dialog_mode: DialogMode::None,
            video_player: None,
            video_error: None,
            video_texture: None,
            last_seek_ms: -1,
            video_duration_ms: 60000,
            telemetry_laps: Vec::new(),
        }
    }

    pub fn format_time(ms: i64) -> String {
        let sign = if ms < 0 { "-" } else { "" };
        let abs_ms = ms.abs();
        let total_seconds = abs_ms / 1000;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{}{:02}:{:02}", sign, minutes, seconds)
    }

    pub fn clamp_playhead_to_trim(&mut self) {
        let start_ms = self.config.export_start_ms.unwrap_or(0);
        let end_ms = match self.config.export_end_ms {
            Some(e) if e >= 0 => e,
            _ => self.video_duration_ms,
        };

        if start_ms <= end_ms {
            self.playhead_ms = self.playhead_ms.clamp(start_ms, end_ms);
        }
    }

    pub fn recalculate_telemetry(&mut self) {
        if let Some(log) = &self.telemetry {
            let view = crate::telemetry::TelemetryView::new(
                log,
                self.config.export_start_ms,
                self.config.export_end_ms,
                self.config.sync.offset_ms,
            );

            self.telemetry_laps = view.extract_laps();

            // Rebuild a temporary TelemetryLog for trackmap creation since from_telemetry requires it currently
            let temp_log = TelemetryLog {
                samples: view.samples.to_vec(),
                start_time_utc: view.start_time_utc,
                parsed_speed_source: crate::project::SpeedSource::Auto,
            };
            self.trackmap = TrackMap::from_telemetry(&temp_log, &self.telemetry_laps);
        }
    }

    pub fn build_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();

        render_controls_window(self, &ctx);
        handle_dialogs(self, &ctx);
        render_video_panel(self, ui);
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if let Ok(res) = self.export_rx.try_recv() {
            self.active_export_progress = None;
            match res {
                Ok(_) => self.export_progress = Some("Export completed successfully.".to_string()),
                Err(e) => self.export_progress = Some(format!("Export failed: {}", e)),
            }
        }
        if self.active_export_progress.is_some() {
            ctx.request_repaint();
        }
        if self.is_playing {
            let dt = ctx.input(|i| i.stable_dt);
            self.playhead_ms += (dt * 1000.0) as i64;

            let end_ms = match self.config.export_end_ms {
                Some(e) if e >= 0 => e,
                _ => self.video_duration_ms,
            };

            if self.playhead_ms >= end_ms {
                self.playhead_ms = end_ms;
                self.is_playing = false;
            }

            ctx.request_repaint();
        }

        self.build_ui(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time_positive() {
        assert_eq!(MyApp::format_time(0), "00:00");
        assert_eq!(MyApp::format_time(999), "00:00");
        assert_eq!(MyApp::format_time(1000), "00:01");
        assert_eq!(MyApp::format_time(59999), "00:59");
        assert_eq!(MyApp::format_time(60000), "01:00");
        assert_eq!(MyApp::format_time(61000), "01:01");
        assert_eq!(MyApp::format_time(3599000), "59:59");
        assert_eq!(MyApp::format_time(3600000), "60:00");
    }

    #[test]
    fn test_format_time_negative() {
        assert_eq!(MyApp::format_time(-999), "-00:00");
        assert_eq!(MyApp::format_time(-1000), "-00:01");
        assert_eq!(MyApp::format_time(-61000), "-01:01");
    }
}
