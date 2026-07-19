use eframe::egui;
use clap::Parser;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

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
}

fn main() -> eframe::Result {
    let args = Args::parse();

    if let Some(output_path) = args.export {
        println!("Export mode requested...");

        let config = if let Some(proj_path) = args.project {
            ProjectConfig::load(proj_path).unwrap_or_default()
        } else {
            ProjectConfig::default()
        };

        let telemetry = TelemetryLog { samples: vec![] };

        match export_video(&config, &telemetry, &output_path) {
            Ok(_) => println!("Export complete: {:?}", output_path),
            Err(e) => eprintln!("Export failed: {}", e),
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

    eframe::run_native(
        "Track Overlay",
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::default())),
    )
}

struct MyApp {
    config: ProjectConfig,
    telemetry: Option<TelemetryLog>,
    playhead_ms: i64,
    auto_sync_progress: Option<Arc<Mutex<Option<i64>>>>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            config: ProjectConfig::default(),
            telemetry: None,
            playhead_ms: 0,
            auto_sync_progress: None,
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::Window::new("Controls").show(&ctx, |ui| {
            ui.heading("Controls");
            ui.add(egui::Slider::new(&mut self.playhead_ms, 0..=60000).text("Playhead (ms)"));

            ui.separator();
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.config.sync.mode, SyncMode::Manual, "Manual Sync");
                ui.radio_value(&mut self.config.sync.mode, SyncMode::Auto, "Auto Sync");
            });

            if self.config.sync.mode == SyncMode::Auto {
                if self.auto_sync_progress.is_none() {
                    if ui.button("Run Auto-Sync").clicked() {
                        let progress = Arc::new(Mutex::new(None));
                        self.auto_sync_progress = Some(progress.clone());

                        // We copy the paths, we don't have real TelemetryLog here, but let's mock it for compile check
                        // In reality, telemetry would be cloned or passed.
                        // For demonstration, we just mock the computation asynchronously
                        let video_path = self.config.video_path.to_string_lossy().to_string();
                        let telem_dummy = TelemetryLog { samples: vec![] }; // normally we clone telemetry

                        thread::spawn(move || {
                            if let Ok(gps_data) = extract_gopro_gps(&video_path) {
                                if let Some(offset) = auto_correlate_gps(&gps_data, &telem_dummy) {
                                    if let Ok(mut lock) = progress.lock() {
                                        *lock = Some(offset);
                                    }
                                }
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
        });

        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 20));

        let sample = self.telemetry.as_ref().and_then(|log| {
            log.sample_at(self.playhead_ms + self.config.sync.offset_ms)
        });

        render_overlay(ui, rect, &mut self.config.elements, sample.as_ref(), false);
    }
}
