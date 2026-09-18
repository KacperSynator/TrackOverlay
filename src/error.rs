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
    Project(#[from] ProjectError),
    #[error("Merge error: {0}")]
    Merge(#[from] MergeError),
    #[error("GPMF error: {0}")]
    Gpmf(#[from] GpmfError),
    #[error("Sync error: {0}")]
    Sync(#[from] SyncError),
    #[error("Overlay error: {0}")]
    Overlay(#[from] OverlayError),
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
    #[error("Stream processing error: {0}")]
    StreamError(String),
}

#[derive(Error, Debug)]
pub enum TelemetryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CSV parsing error: {0}")]
    Csv(#[from] csv::Error),
    #[error("No samples found")]
    NoSamples,
    #[error("Data formatting error: {0}")]
    Data(String),
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
    #[error("Export failed: {0}")]
    Failed(String),
}

#[derive(Error, Debug)]
pub enum ProjectError {
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
    #[error("Parse error")]
    ParseError,
    #[error("Path persist error: {0}")]
    PathPersist(#[from] tempfile::PathPersistError),
}

#[derive(Error, Debug)]
pub enum GpmfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Command failed: {0}")]
    CommandFailed(String),
    #[error("No GPMD stream found in MP4")]
    NoGpmdStream,
    #[error("Failed to extract GPMD data track")]
    ExtractionFailed,
}

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("No overlapping data found for sync")]
    NoOverlap,
    #[error("Calculation error: {0}")]
    Calculation(String),
}

#[derive(Error, Debug)]
pub enum OverlayError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Pixmap creation failed")]
    PixmapFailed,
    #[error("Rect creation failed")]
    RectFailed,
}
