use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Video error: {0}")]
    Video(#[from] VideoError),
    #[error("Telemetry error: {0}")]
    Telemetry(#[from] TelemetryError),
    #[error("Export error: {0}")]
    Export(#[from] ExportError),
    #[error("Project error: {0}")]
    Config(#[from] ConfigError),
    #[error("Merge error: {0}")]
    Merge(#[from] MergeError),
    #[error("GPMF error: {0}")]
    Gpmf(#[from] GpmfError),
    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[derive(Error, Debug)]
pub enum VideoError {
    #[error("FFmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg_next::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No video stream found")]
    NoVideoStream,
}

#[derive(Error, Debug)]
pub enum TelemetryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CSV parsing error: {0}")]
    Csv(#[from] csv::Error),
}

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("FFmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg_next::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No video path specified for export")]
    NoVideoPath,
    #[error("No video stream found")]
    NoVideoStream,
    #[error("H264 encoder not found")]
    NoEncoder,
    #[error("Output stream not found")]
    NoOutputStream,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum MergeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Command failed with status: {0}")]
    CommandFailed(std::process::ExitStatus),
    #[error("Path persist error: {0}")]
    PathPersist(#[from] tempfile::PathPersistError),
}

#[derive(Error, Debug)]
pub enum GpmfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No GPMD stream found in MP4")]
    NoGpmdStream,
    #[error("Failed to extract GPMD data track")]
    ExtractionFailed,
}
