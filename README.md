# Track Overlay

A desktop app that overlays TrackAddict CSV telemetry (speed, g-force, lap time, GPS position) onto GoPro MP4 footage, with a real-time GPU-rendered preview for syncing video-to-data offset, and a batch export pipeline to render the final video. Auto-sync via GoPro GPMF GPS tracking is also supported!

## Prerequisites

If you plan to run the app natively on your machine, you need:

- Rust toolchain (stable)
- GStreamer development libraries
- FFmpeg (for final video export)
- Native UI file dialog libraries (e.g., `zenity`, `kdialog`, or portal support for Wayland)

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

You can optionally specify a default directory for loading/saving files using the `--data-dir` argument:

```bash
cargo run --release -- --data-dir /path/to/my/videos
```

Currently, in GUI mode, you can:
1. Load a GoPro MP4
2. Load a TrackAddict CSV
3. Adjust the sync offset (or use Auto Sync)
4. Tweak the layout gauges
5. Click **"Export Final Video"** to export the result.

## Exporting via CLI

If you've already configured your `ProjectConfig` (e.g. by saving it in the GUI or crafting it manually), you can run the batch export pipeline by providing an export flag and destination via the CLI interface.

```bash
cargo run --release -- --export final_output.mp4 --project my_project.json
```

> Note: The export feature wraps around the `ffmpeg` tool.

## Using Docker

If you don't want to install dependencies locally, you can build and run `track-overlay` via Docker. The Dockerfile comes pre-installed with `mesa-va-drivers` allowing for hardware acceleration on AMD/Intel GPUs.

### Building the Docker Image

From the project root:

```bash
docker build -t track-overlay .
```

### Running with Docker

Because the app is graphical and needs file access, you must map your display server to the container and map a local directory as your data directory so the file picker can access it.

#### 1. Basic GUI Mode (Software Rendering / No GPU access)
Use this if you don't need hardware acceleration, or if you run into driver issues.

```bash
xhost +local:docker
docker run --rm \
  -e DISPLAY=$DISPLAY \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -v $(pwd)/data:/app/data \
  track-overlay --data-dir /app/data
```

#### 2. GPU Accelerated Mode (Radeon/AMD, Intel)
Passing `--device /dev/dri` exposes your GPU to the container. The Docker image has the necessary `mesa-va-drivers` to utilize VA-API for decoding and rendering.

**For X11:**
```bash
xhost +local:docker
docker run --rm \
  --device /dev/dri \
  -e DISPLAY=$DISPLAY \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -v $(pwd)/data:/app/data \
  track-overlay --data-dir /app/data
```

**For Wayland (e.g., Cachy OS default):**
```bash
docker run --rm \
  --device /dev/dri \
  -e WAYLAND_DISPLAY=$WAYLAND_DISPLAY \
  -e XDG_RUNTIME_DIR=/tmp \
  -v $XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/tmp/$WAYLAND_DISPLAY \
  -v $(pwd)/data:/app/data \
  track-overlay --data-dir /app/data
```

#### 3. Export Mode (CLI - No GUI required)
If you just want to export a project and avoid messing with display servers entirely, you just need to mount your files. You can optionally include `--device /dev/dri` for hardware decoding speedups.

```bash
docker run --rm \
  --device /dev/dri \
  -v $(pwd)/data:/app/data \
  track-overlay --export /app/data/final_output.mp4 --project /app/data/my_project.json
```

*(Note: NVIDIA GPUs require the proprietary `nvidia-container-toolkit` and the `--gpus all` flag instead of `/dev/dri`. The provided Dockerfile uses Mesa drivers, so NVIDIA users will fallback to software decoding unless the image is adapted for CUDA).*

## Tech Stack
- Language: Rust
- GUI: `egui` via `eframe`
- File Picker: `rfd`
- Telemetry parsing: `csv` + `serde`
- Video playback/decoding: `gstreamer-rs`
- Sync Strategy: GPMF extraction via `ffprobe` + cross-correlation
- Video rendering: FFmpeg (CLI)
