# Track Overlay

A desktop app that overlays TrackAddict CSV telemetry (speed, g-force, lap time, GPS position) onto GoPro MP4 footage, with a real-time GPU-rendered preview for syncing video-to-data offset, and a batch export pipeline to render the final video.

## Prerequisites

If you plan to run the app natively on your machine, you need:

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

Alternatively, you can run the app using **Docker** without installing dependencies natively (see Docker instructions below).

## Running the App Natively

To launch the `eframe` GUI for syncing your footage and telemetry, simply use:

```bash
cargo run --release
```

Currently, in GUI mode, you can explore the layout options and adjust the playhead and sync offset.

## Exporting Natively

Once your project configuration is correct, you can run the batch export pipeline by providing an export flag and destination via the CLI interface.

```bash
cargo run --release -- --export final_output.mp4
```

> Note: The export feature wraps around the `ffmpeg` tool.

## Using Docker

If you don't want to install dependencies locally, you can build and run `track-overlay` via Docker.

### Building the Docker Image

From the project root:

```bash
docker build -t track-overlay .
```

### Running with Docker

Since the app requires X11/Wayland display access for the GUI, you will need to share your display environment with the container. Or, if you only want to use the CLI exporter, you just need to mount your files.

**Export Mode (CLI):**
To mount your local directory and run an export:

```bash
docker run --rm -v $(pwd):/app track-overlay --export /app/final_output.mp4
```

**GUI Mode (X11 Example):**
```bash
xhost +local:docker
docker run --rm -e DISPLAY=$DISPLAY -v /tmp/.X11-unix:/tmp/.X11-unix -v $(pwd):/app track-overlay
```
*(Note: GPU acceleration sharing inside Docker might require additional flags like `--device /dev/dri` depending on your host OS).*

## Tech Stack
- Language: Rust
- GUI: `egui` via `eframe`
- Telemetry parsing: `csv` + `serde`
- Video playback/decoding: `gstreamer-rs`
- Video rendering: FFmpeg (CLI)
