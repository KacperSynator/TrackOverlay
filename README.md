# Track Overlay

A desktop app that overlays TrackAddict CSV telemetry (speed, g-force, lap time, GPS position) onto GoPro MP4 footage, with a real-time preview for syncing video-to-data offset, and a batch export pipeline to render the final video. Auto-sync via GoPro GPMF GPS tracking is also supported!

## Features

- **Auto-Sync:** Automatically synchronizes video and telemetry by correlating GoPro GPS (GPMF) data with TrackAddict GPS telemetry.
- **Manual Sync:** Tools to manually adjust the sync offset between video and telemetry data.
- **Configurable Layouts:** Customize the size, position, and visibility of various overlay elements using JSON configuration files and a UI layout editor.
- **Real-Time Preview:** Video playback with real-time rendering of overlay gauges to verify synchronization.
- **Batch Export:** Headless export capabilities via CLI to render the final composited video using FFmpeg.

### Available Overlay Elements
- **Speed Readout**
- **G-Force Meter** (Friction circle)
- **Lap Timer**
- **Advanced Lap Timer** (shows current, best, history, and live projection)
- **Track Map** (Driven path visualization)
- **Throttle Bar**

## Prerequisites

If you plan to run the app natively on your machine, you need:

- Rust toolchain (stable)
- FFmpeg development libraries (for video decoding and export)

**Ubuntu/Debian setup:**
```bash
sudo apt-get update
sudo apt-get install -y pkg-config libavutil-dev libavformat-dev libavcodec-dev libswscale-dev libavdevice-dev libavfilter-dev ffmpeg
```

Alternatively, you can run the app using **Docker** without installing dependencies natively (see Docker instructions below).

## Running the App Natively

To launch the app (either GUI or CLI), you must provide a required configuration file via the `--config` flag.
A default layout configuration is provided in the repository as `default_config.json`.

```bash
RUST_LOG=info cargo run --release -- --config default_config.json
```

You can optionally specify a default directory for loading/saving files using the `--data-dir` argument:

```bash
RUST_LOG=info cargo run --release -- --config default_config.json --data-dir /path/to/my/videos
```

Currently, in GUI mode, you can:
1. Load a GoPro MP4
2. Load a TrackAddict CSV
3. Adjust the sync offset (or use Auto Sync)
4. Tweak the layout gauges
5. Click **"Export Final Video"** to export the result.

## Exporting via CLI

If you've already configured your project (e.g. by saving it in the GUI or crafting it manually), you can run the batch export pipeline by providing an export flag and destination via the CLI interface.

```bash
RUST_LOG=info cargo run --release -- --export final_output.mp4 --config my_project.json
```

> Note: The export feature wraps around the `ffmpeg` tool.

## Using Docker

If you don't want to install dependencies locally, you can build and run `track-overlay` via Docker. The app uses an embedded `egui` file picker so DBus host permissions aren't strictly required.

### Building the Docker Image

You can build the Docker image with CPU-only support or with GPU hardware acceleration support.

**Build CPU-only image:**
```bash
docker build --target cpu -t track-overlay:cpu .
```

**Build GPU-accelerated image (Recommended for better performance):**
```bash
docker build --target gpu -t track-overlay:gpu .
```

### Running with Docker

Because the app is graphical and needs file access, you must map your display server to the container and map a local directory as your data directory so the file picker can access it.

#### 1. CPU-Only Mode

**For X11:**
```bash
xhost +local:docker
docker run --rm \
  -e DISPLAY=$DISPLAY \
  -e RUST_LOG=info \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -v $(pwd)/data:/app/data \
  track-overlay:cpu --config /app/data/default_config.json --data-dir /app/data
```

**For Wayland:**
```bash
docker run --rm \
  -e WAYLAND_DISPLAY=$WAYLAND_DISPLAY \
  -e XDG_RUNTIME_DIR=/tmp \
  -e XDG_SESSION_TYPE=wayland \
  -e WINIT_UNIX_BACKEND=wayland \
  -e RUST_LOG=info \
  -v $XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/tmp/$WAYLAND_DISPLAY \
  -v $(pwd)/data:/app/data \
  track-overlay:cpu --config /app/data/default_config.json --data-dir /app/data
```

#### 2. GPU-Accelerated Mode

When running with the GPU-accelerated image, you must pass your host's GPU device to the container.

**For AMD/Intel (Wayland):**
```bash
docker run --rm \
  -e WAYLAND_DISPLAY=$WAYLAND_DISPLAY \
  -e XDG_RUNTIME_DIR=/tmp \
  -e XDG_SESSION_TYPE=wayland \
  -e WINIT_UNIX_BACKEND=wayland \
  -e RUST_LOG=info \
  -v $XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/tmp/$WAYLAND_DISPLAY \
  -v $(pwd)/data:/app/data \
  --device /dev/dri \
  track-overlay:gpu --config /app/data/default_config.json --data-dir /app/data
```

**For NVIDIA (Wayland, requires NVIDIA Container Toolkit):**
```bash
docker run --rm \
  -e WAYLAND_DISPLAY=$WAYLAND_DISPLAY \
  -e XDG_RUNTIME_DIR=/tmp \
  -e XDG_SESSION_TYPE=wayland \
  -e WINIT_UNIX_BACKEND=wayland \
  -e RUST_LOG=info \
  -v $XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/tmp/$WAYLAND_DISPLAY \
  -v $(pwd)/data:/app/data \
  --gpus all \
  track-overlay:gpu --config /app/data/default_config.json --data-dir /app/data
```

> **Note on X11 with GPU:** If you are using X11 instead of Wayland, replace the Wayland environment variables and volume mounts with the X11 equivalents shown in the CPU-Only section, while keeping the `--device /dev/dri` or `--gpus all` flags.

#### 3. Export Mode (CLI - No GUI required)
If you just want to export a project and avoid messing with display servers entirely, you just need to mount your files.

```bash
docker run --rm \
  -e RUST_LOG=info \
  -v $(pwd)/data:/app/data \
  track-overlay:cpu --export /app/data/final_output.mp4 --config /app/data/my_project.json
```


## Tech Stack
- Language: Rust
- GUI: `egui` via `eframe`
- File Picker: `egui-file-dialog` (Cross-platform, embedded inside egui window)
- Telemetry parsing: `csv` + `serde`
- Video playback/decoding: `ffmpeg-next`
- Sync Strategy: GPMF extraction via `ffprobe` + cross-correlation
- Video rendering: FFmpeg (CLI)
