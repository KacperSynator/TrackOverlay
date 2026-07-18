# Track Overlay

A desktop app that overlays TrackAddict CSV telemetry (speed, g-force, lap time, GPS position) onto GoPro MP4 footage, with a real-time GPU-rendered preview for syncing video-to-data offset, and a batch export pipeline to render the final video.

## Prerequisites

- Rust toolchain (stable)
- GStreamer development libraries
- FFmpeg (for final video export)

**Ubuntu/Debian setup:**
```bash
sudo apt-get update
sudo apt-get install -y libglib2.0-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev
sudo apt-get install -y gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav
sudo apt-get install -y ffmpeg
```

## Running the App

To launch the `eframe` GUI for syncing your footage and telemetry, simply use:

```bash
cargo run --release
```

Currently, in GUI mode, you can explore the layout options and adjust the playhead and sync offset.

## Exporting

Once your project configuration is correct, you can run the batch export pipeline by providing an export flag and destination via the CLI interface.

```bash
cargo run --release -- --export final_output.mp4
```

> Note: The export feature wraps around the `ffmpeg` tool.

## Tech Stack
- Language: Rust
- GUI: `egui` via `eframe`
- Telemetry parsing: `csv` + `serde`
- Video playback/decoding: `gstreamer-rs`
- Video rendering: FFmpeg (CLI)
