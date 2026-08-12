use clap::Parser;
use eframe::egui;
use log::{error, info};
use std::path::PathBuf;

use track_overlay::app::MyApp;
use track_overlay::project::ProjectConfig;
use track_overlay::telemetry::TelemetryLog;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    export: Option<PathBuf>,

    #[arg(short, long)]
    config: PathBuf,

    #[arg(short, long, env = "DATA_DIR")]
    data_dir: Option<PathBuf>,
}

fn main() -> eframe::Result {
    env_logger::init();
    info!("Starting track-overlay application...");

    let args = Args::parse();

    info!("Loading config from {:?}", args.config);
    let config = match ProjectConfig::load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load config file {:?}: {}", args.config, e);
            std::process::exit(1);
        }
    };

    if let Some(output_path) = args.export {
        info!("Export mode requested...");

        let telemetry = if config.telemetry_path.exists() {
            info!("Loading telemetry from {:?}", config.telemetry_path);
            TelemetryLog::load_csv(&config.telemetry_path, config.speed_source.clone())
                .unwrap_or_else(|e| {
                    error!("Failed to load telemetry: {}", e);
                    TelemetryLog {
                        samples: vec![],
                        start_time_utc: None,
                        parsed_speed_source: track_overlay::project::SpeedSource::Auto,
                    }
                })
        } else {
            TelemetryLog {
                samples: vec![],
                start_time_utc: None,
                parsed_speed_source: track_overlay::project::SpeedSource::Auto,
            }
        };

        info!("Beginning batch export to {:?}", output_path);
        let _ = track_overlay::export::export_video(&config, &telemetry, &output_path, None);
        return Ok(());
    }

    if std::env::var("HEADLESS_TEST").is_ok() {
        println!("Headless test successful.");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1920.0, 1080.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Track Overlay",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::new(config, args.data_dir)))),
    )
}
