use eframe::egui;
use clap::Parser;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use egui_file_dialog::FileDialog;
use log::{info, error, warn};

use track_overlay::project::{ProjectConfig, SyncMode};
use track_overlay::telemetry::TelemetryLog;
use track_overlay::overlay::render_overlay;
use track_overlay::export::export_video;
use track_overlay::gpmf_extract::extract_gopro_gps;
use track_overlay::sync::auto_correlate_gps;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    export: Option<PathBuf>,

    #[arg(short, long)]
    project: Option<PathBuf>,

    /// Default data directory for picking files or exporting
    #[arg(short, long, env = "DATA_DIR")]
    data_dir: Option<PathBuf>,
}

fn main() -> eframe::Result {
    env_logger::init();
    info!("Starting track-overlay application...");

    let args = Args::parse();

    if let Some(output_path) = args.export {
        info!("Export mode requested...");

        let config = if let Some(proj_path) = args.project {
            info!("Loading project config from {:?}", proj_path);
            ProjectConfig::load(proj_path).unwrap_or_default()
        } else {
            ProjectConfig::default()
        };

        // In export mode we must actually load telemetry if available
        let telemetry = if config.telemetry_path.exists() {
            info!("Loading telemetry from {:?}", config.telemetry_path);
            TelemetryLog::load_csv(&config.telemetry_path).unwrap_or_else(|e| {
                error!("Failed to load telemetry: {}", e);
                TelemetryLog { samples: vec![] }
            })
        } else {
            warn!("Telemetry path does not exist: {:?}", config.telemetry_path);
            TelemetryLog { samples: vec![] }
        };

        info!("Beginning batch export to {:?}", output_path);
        match export_video(&config, &telemetry, &output_path) {
            Ok(_) => info!("Export complete: {:?}", output_path),
            Err(e) => error!("Export failed: {}", e),
        }

        return Ok(());
    }

    if std::env::var("HEADLESS_TEST").is_ok() {
        println!("Headless test successful.");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };

    info!("Launching eframe GUI...");
    eframe::run_native(
        "Track Overlay",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::new(args.data_dir)))),
    )
}

struct MyApp {
    config: ProjectConfig,
    telemetry: Option<TelemetryLog>,
    playhead_ms: i64,
    auto_sync_progress: Option<Arc<Mutex<Option<i64>>>>,
    data_dir: Option<PathBuf>,
    export_progress: Option<String>,

    // File dialog instance
    file_dialog: FileDialog,
    // Enum to keep track of what the dialog is currently picking
    dialog_mode: DialogMode,
}

#[derive(PartialEq)]
enum DialogMode {
    None,
    PickVideo,
    PickTelemetry,
    PickExportOutput,
}

impl MyApp {
    fn new(data_dir: Option<PathBuf>) -> Self {
        let mut fd = FileDialog::new();

        if let Some(ref dir) = data_dir {
            fd = fd.initial_directory(dir.clone());
        }

        Self {
            config: ProjectConfig::default(),
            telemetry: None,
            playhead_ms: 0,
            auto_sync_progress: None,
            data_dir,
            export_progress: None,
            file_dialog: fd,
            dialog_mode: DialogMode::None,
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::Window::new("Controls").show(&ctx, |ui| {
            ui.heading("Project Files");

            // --- VIDEO FILE PICKER ---
            ui.horizontal(|ui| {
                if ui.button("Load Video").clicked() {
                    info!("Opening file dialog for Video");
                    self.dialog_mode = DialogMode::PickVideo;
                    self.file_dialog.pick_file();
                }
                ui.label(self.config.video_path.file_name().unwrap_or_default().to_string_lossy().as_ref());
            });

            // --- TELEMETRY FILE PICKER ---
            ui.horizontal(|ui| {
                if ui.button("Load Telemetry").clicked() {
                    info!("Opening file dialog for Telemetry");
                    self.dialog_mode = DialogMode::PickTelemetry;
                    self.file_dialog.pick_file();
                }
                ui.label(self.config.telemetry_path.file_name().unwrap_or_default().to_string_lossy().as_ref());
            });

            ui.separator();

            ui.heading("Sync");
            ui.add(egui::Slider::new(&mut self.playhead_ms, 0..=60000).text("Playhead (ms)"));

            ui.horizontal(|ui| {
                ui.radio_value(&mut self.config.sync.mode, SyncMode::Manual, "Manual Sync");
                ui.radio_value(&mut self.config.sync.mode, SyncMode::Auto, "Auto Sync");
            });

            if self.config.sync.mode == SyncMode::Auto {
                if self.auto_sync_progress.is_none() {
                    if ui.button("Run Auto-Sync").clicked() {
                        info!("Starting Auto-Sync...");
                        let progress = Arc::new(Mutex::new(None));
                        self.auto_sync_progress = Some(progress.clone());

                        let video_path = self.config.video_path.to_string_lossy().to_string();
                        let telem_clone = if let Some(t) = &self.telemetry {
                            TelemetryLog { samples: t.samples.clone() }
                        } else {
                            warn!("No telemetry loaded for Auto-Sync!");
                            TelemetryLog { samples: vec![] }
                        };

                        thread::spawn(move || {
                            info!("Extracting GoPro GPS from {}", video_path);
                            if let Ok(gps_data) = extract_gopro_gps(&video_path) {
                                info!("Extracted {} GPS points. Running correlation...", gps_data.len());
                                if let Some(offset) = auto_correlate_gps(&gps_data, &telem_clone) {
                                    info!("Auto-sync computed offset: {} ms", offset);
                                    if let Ok(mut lock) = progress.lock() {
                                        *lock = Some(offset);
                                    }
                                } else {
                                    warn!("Auto-correlation failed or produced no valid result.");
                                }
                            } else {
                                error!("Failed to extract GoPro GPS data.");
                            }
                        });
                    }
                } else {
                    let mut done = false;
                    if let Ok(lock) = self.auto_sync_progress.as_ref().unwrap().lock() {
                        if let Some(offset) = *lock {
                            self.config.sync.offset_ms = offset;
                            done = true;
                        }
                    }
                    if done {
                        self.auto_sync_progress = None;
                    } else {
                        ui.label("Syncing...");
                        ui.ctx().request_repaint(); // ensure we re-draw to check progress
                    }
                }

                ui.label(format!("Computed offset: {} ms", self.config.sync.offset_ms));
            } else {
                ui.add(egui::Slider::new(&mut self.config.sync.offset_ms, -10000..=10000).text("Sync Offset (ms)"));
            }

            ui.separator();
            ui.label("Layout Editor");
            for (_i, el) in self.config.elements.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{:?}", el.kind));
                    ui.add(egui::Slider::new(&mut el.x, 0.0..=1.0).text("X"));
                    ui.add(egui::Slider::new(&mut el.y, 0.0..=1.0).text("Y"));
                    ui.add(egui::Slider::new(&mut el.scale, 0.5..=3.0).text("Scale"));
                });
            }

            ui.separator();

            if ui.button("Export Final Video").clicked() {
                info!("Opening file dialog for Export");
                self.dialog_mode = DialogMode::PickExportOutput;
                self.file_dialog.save_file();
            }

            if let Some(msg) = &self.export_progress {
                ui.label(msg);
            }
        });

        // Update the file dialog
        self.file_dialog.update(&ctx);

        // Check if a file was picked
        if let Some(path) = self.file_dialog.take_picked() {
            let path_buf = path.to_path_buf();
            match self.dialog_mode {
                DialogMode::PickVideo => {
                    info!("Video file selected: {:?}", path_buf);
                    self.config.video_path = path_buf;
                    self.playhead_ms = 0;
                }
                DialogMode::PickTelemetry => {
                    info!("Telemetry file selected: {:?}", path_buf);
                    self.config.telemetry_path = path_buf.clone();
                    match TelemetryLog::load_csv(&path_buf) {
                        Ok(log) => {
                            info!("Successfully loaded telemetry: {} samples", log.samples.len());
                            self.telemetry = Some(log);
                        }
                        Err(e) => error!("Failed to load CSV: {}", e),
                    }
                }
                DialogMode::PickExportOutput => {
                    info!("Export destination selected: {:?}", path_buf);
                    let config_clone = self.config.clone();
                    let telem_clone = if let Some(t) = &self.telemetry {
                        TelemetryLog { samples: t.samples.clone() }
                    } else {
                        TelemetryLog { samples: vec![] }
                    };

                    self.export_progress = Some(format!("Exporting to {:?}...", path_buf));
                    info!("Starting export...");

                    match export_video(&config_clone, &telem_clone, &path_buf) {
                        Ok(_) => {
                            info!("Export completed successfully.");
                            self.export_progress = Some("Export completed successfully.".to_string());
                        }
                        Err(e) => {
                            error!("Export failed: {}", e);
                            self.export_progress = Some(format!("Export failed: {}", e));
                        }
                    }
                }
                DialogMode::None => {}
            }
            self.dialog_mode = DialogMode::None;
        }

        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 20));

        let sample = self.telemetry.as_ref().and_then(|log| {
            log.sample_at(self.playhead_ms + self.config.sync.offset_ms)
        });

        render_overlay(ui, rect, &mut self.config.elements, sample.as_ref(), false);
    }
}
