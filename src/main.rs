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
use track_overlay::video::VideoPlayer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    export: Option<PathBuf>,

    #[arg(short, long)]
    project: Option<PathBuf>,

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

        let telemetry = if config.telemetry_path.exists() {
            info!("Loading telemetry from {:?}", config.telemetry_path);
            TelemetryLog::load_csv(&config.telemetry_path).unwrap_or_else(|e| {
                error!("Failed to load telemetry: {}", e);
                TelemetryLog { samples: vec![], start_time_utc: None }
            })
        } else {
            TelemetryLog { samples: vec![], start_time_utc: None }
        };

        info!("Beginning batch export to {:?}", output_path);
        let _ = export_video(&config, &telemetry, &output_path);
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
        Box::new(|_cc| Ok(Box::new(MyApp::new(args.data_dir)))),
    )
}

struct MyApp {
    config: ProjectConfig,
    telemetry: Option<TelemetryLog>,
    playhead_ms: i64,
    is_playing: bool,
    auto_sync_progress: Option<Arc<Mutex<Option<i64>>>>,
    data_dir: Option<PathBuf>,
    export_progress: Option<String>,

    file_dialog: FileDialog,
    dialog_mode: DialogMode,

    video_player: Option<VideoPlayer>,
    video_texture: Option<egui::TextureHandle>,
    last_seek_ms: i64,
    video_duration_ms: i64,

    telemetry_laps: Vec<(u32, i64)>, // Lap number, start_time_ms
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
        let mut fd = FileDialog::new()
            .default_size([600.0, 400.0]);

        if let Some(ref dir) = data_dir {
            fd = fd.initial_directory(dir.clone());
        }

        Self {
            config: ProjectConfig::default(),
            telemetry: None,
            playhead_ms: 0,
            is_playing: false,
            auto_sync_progress: None,
            data_dir,
            export_progress: None,
            file_dialog: fd,
            dialog_mode: DialogMode::None,
            video_player: None,
            video_texture: None,
            last_seek_ms: -1,
            video_duration_ms: 60000,
            telemetry_laps: Vec::new(),
        }
    }

    fn format_time(ms: i64) -> String {
        let total_seconds = ms / 1000;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }

    fn build_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("Controls").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Project Files");

                ui.horizontal(|ui| {
                    if ui.button("Load Video").clicked() {
                        self.dialog_mode = DialogMode::PickVideo;
                        self.file_dialog.pick_file();
                    }
                    ui.label(self.config.video_path.file_name().unwrap_or_default().to_string_lossy().as_ref());
                });
                if let Some(vp) = &self.video_player {
                    if let Some(dt) = vp.creation_time_utc {
                        ui.label(format!("  Timestamp: {}", dt.format("%Y-%m-%d %H:%M:%S UTC")));
                    }
                    ui.label(format!("  Duration: {}s", self.video_duration_ms / 1000));
                }

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Load Telemetry").clicked() {
                        self.dialog_mode = DialogMode::PickTelemetry;
                        self.file_dialog.pick_file();
                    }
                    ui.label(self.config.telemetry_path.file_name().unwrap_or_default().to_string_lossy().as_ref());
                });
                if let Some(telem) = &self.telemetry {
                    if let Some(dt) = telem.start_time_utc {
                        ui.label(format!("  Timestamp: {}", dt.format("%Y-%m-%d %H:%M:%S UTC")));
                    }
                    if !telem.samples.is_empty() {
                        let telem_dur = telem.samples.last().unwrap().time_ms - telem.samples.first().unwrap().time_ms;
                        ui.label(format!("  Data Length: {}s", telem_dur / 1000));
                    }

                    if !self.telemetry_laps.is_empty() {
                        ui.collapsing("Laps", |ui| {
                            for (lap_num, start_time) in &self.telemetry_laps {
                                if ui.button(format!("Jump to Lap {} ({}s)", lap_num, start_time / 1000)).clicked() {
                                    let target_playhead = start_time - self.config.sync.offset_ms;
                                    if target_playhead >= 0 && target_playhead <= self.video_duration_ms {
                                        self.playhead_ms = target_playhead;
                                    }
                                }
                            }
                        });
                    }
                }

                ui.separator();
                ui.heading("Settings");
                ui.checkbox(&mut self.config.flip_vertical, "Flip Video Vertically");
                ui.checkbox(&mut self.config.flip_horizontal, "Flip Video Horizontally");

                ui.separator();
                ui.heading("Sync");

                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.config.sync.mode, SyncMode::Manual, "Manual Sync");
                    ui.radio_value(&mut self.config.sync.mode, SyncMode::Auto, "Auto Sync");
                });

                if self.config.sync.mode == SyncMode::Auto {
                    if self.auto_sync_progress.is_none() {
                        if ui.button("Run Auto-Sync").clicked() {
                            let progress = Arc::new(Mutex::new(None));
                            self.auto_sync_progress = Some(progress.clone());

                            let video_path = self.config.video_path.to_string_lossy().to_string();
                            let telem_clone = if let Some(t) = &self.telemetry {
                                TelemetryLog { samples: t.samples.clone(), start_time_utc: t.start_time_utc }
                            } else {
                                TelemetryLog { samples: vec![], start_time_utc: None }
                            };

                            thread::spawn(move || {
                                if let Ok(gps_data) = extract_gopro_gps(&video_path) {
                                    if let Some(offset) = auto_correlate_gps(&gps_data, &telem_clone) {
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
                    ui.add(egui::Slider::new(&mut self.config.sync.offset_ms, -120000..=120000).text("Sync Offset (ms)"));
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
                    self.dialog_mode = DialogMode::PickExportOutput;
                    self.file_dialog.save_file();
                }

                if let Some(msg) = &self.export_progress {
                    ui.label(msg);
                }
            });
        });

        // Update the file dialog
        self.file_dialog.update(ctx);

        // Check if a file was picked
        if let Some(path) = self.file_dialog.take_picked() {
            let path_buf = path.to_path_buf();
            match self.dialog_mode {
                DialogMode::PickVideo => {
                    self.config.video_path = path_buf.clone();
                    self.playhead_ms = 0;
                    self.last_seek_ms = -1;

                    if let Ok(mut player) = VideoPlayer::new(&path_buf) {
                        let _ = player.play();
                        let _ = player.pause();
                        if let Some(dur) = player.duration_ms() {
                            self.video_duration_ms = dur;
                        }
                        self.video_player = Some(player);
                    }
                }
                DialogMode::PickTelemetry => {
                    self.config.telemetry_path = path_buf.clone();
                    if let Ok(log) = TelemetryLog::load_csv(&path_buf) {
                        self.telemetry_laps.clear();
                        let mut current_lap = None;
                        for s in &log.samples {
                            if let Some(lap) = s.lap_number {
                                if Some(lap) != current_lap {
                                    current_lap = Some(lap);
                                    self.telemetry_laps.push((lap, s.time_ms));
                                }
                            }
                        }
                        self.telemetry = Some(log);
                    }
                }
                DialogMode::PickExportOutput => {
                    let config_clone = self.config.clone();
                    let telem_clone = if let Some(t) = &self.telemetry {
                        TelemetryLog { samples: t.samples.clone(), start_time_utc: t.start_time_utc }
                    } else {
                        TelemetryLog { samples: vec![], start_time_utc: None }
                    };

                    self.export_progress = Some(format!("Exporting to {:?}...", path_buf));

                    match export_video(&config_clone, &telem_clone, &path_buf) {
                        Ok(_) => self.export_progress = Some("Export completed successfully.".to_string()),
                        Err(e) => self.export_progress = Some(format!("Export failed: {}", e)),
                    }
                }
                DialogMode::None => {}
            }
            self.dialog_mode = DialogMode::None;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let mut available_video_area = rect;

            // Bottom controls height
            let controls_height = 40.0;
            available_video_area.set_bottom(rect.bottom() - controls_height);

            ui.painter().rect_filled(available_video_area, 0.0, egui::Color32::from_rgb(20, 20, 20));

            // Fetch frame from video player
            if let Some(player) = &mut self.video_player {
                if self.playhead_ms != self.last_seek_ms {
                    if let Err(e) = player.seek(self.playhead_ms) {
                        warn!("Seek error: {}", e);
                    }
                    self.last_seek_ms = self.playhead_ms;
                }

                if let Ok(Some(frame)) = player.get_frame() {
                    let w = frame.width as usize;
                    let h = frame.height as usize;

                    if w > 0 && h > 0 {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [w, h],
                            &frame.data,
                        );

                        let texture = ui.ctx().load_texture(
                            "video_frame",
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.video_texture = Some(texture);
                    }
                }
            }

            // By default, assume 16:9
            let mut aspect = 16.0 / 9.0;
            if let Some(tex) = &self.video_texture {
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
            if let Some(tex) = &self.video_texture {
                let mut min_pos = egui::pos2(0.0, 0.0);
                let mut max_pos = egui::pos2(1.0, 1.0);

                if self.config.flip_horizontal {
                    let tmp = min_pos.x;
                    min_pos.x = max_pos.x;
                    max_pos.x = tmp;
                }
                if self.config.flip_vertical {
                    let tmp = min_pos.y;
                    min_pos.y = max_pos.y;
                    max_pos.y = tmp;
                }

                ui.painter().image(
                    tex.id(),
                    draw_rect,
                    egui::Rect::from_min_max(min_pos, max_pos),
                    egui::Color32::WHITE,
                );
            }

            let sample = self.telemetry.as_ref().and_then(|log| {
                log.sample_at(self.playhead_ms + self.config.sync.offset_ms)
            });

            // Bind the telemetry overlay rendering entirely to the draw_rect of the video stream
            render_overlay(ui, draw_rect, &mut self.config.elements, sample.as_ref(), false);

            let mut control_rect = rect;
            control_rect.set_top(rect.bottom() - controls_height);

            // Constrain controls to the same width as the video so it aligns nicely
            let mut centered_controls = control_rect;
            centered_controls.set_left(draw_rect.left());
            centered_controls.set_right(draw_rect.right());

            ui.scope_builder(egui::UiBuilder::new().max_rect(centered_controls), |ui| {
                ui.horizontal(|ui| {
                    let btn_text = if self.is_playing { "⏸ Pause" } else { "▶ Play" };
                    if ui.button(btn_text).clicked() {
                        self.is_playing = !self.is_playing;
                    }

                    ui.label(format!("{} / {}", Self::format_time(self.playhead_ms), Self::format_time(self.video_duration_ms)));

                    let slider = egui::Slider::new(&mut self.playhead_ms, 0..=self.video_duration_ms)
                        .show_value(false)
                        .trailing_fill(true);
                    ui.add_sized(ui.available_size(), slider);
                });
            });
        });
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.is_playing {
            let dt = ctx.input(|i| i.stable_dt);
            self.playhead_ms += (dt * 1000.0) as i64;
            if self.playhead_ms > self.video_duration_ms {
                self.playhead_ms = self.video_duration_ms;
                self.is_playing = false;
            }
            ctx.request_repaint();
        }

        self.build_ui(ctx);
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
}
