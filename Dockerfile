FROM rust:1.85-bookworm AS builder

# Install necessary dependencies for FFmpeg and egui
RUN apt-get update && apt-get install -y \
    libavutil-dev \
    libavformat-dev \
    libavcodec-dev \
    libswscale-dev \
    libavdevice-dev \
    libavfilter-dev \
    ffmpeg \
    pkg-config \
    libx11-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxi-dev \
    libvulkan-dev \
    libwayland-dev \
    wayland-protocols \
    libxkbcommon-dev \
    libegl1-mesa-dev \
    libfontconfig1-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy the source code
COPY . .

# Build the application
RUN cargo build --release

# Final lightweight image
FROM debian:bookworm-slim

# Install runtime dependencies including VA-API drivers for Radeon/Intel hardware acceleration
RUN apt-get update && apt-get install -y \
    ffmpeg \
    libx11-6 \
    libxcursor1 \
    libxrandr2 \
    libxi6 \
    libvulkan1 \
    libwayland-client0 \
    libwayland-cursor0 \
    libwayland-egl1 \
    libxkbcommon0 \
    libegl1 \
    libfontconfig1 \
    mesa-va-drivers \
    libva-drm2 \
    libva-x11-2 \
    libva-wayland2 \
    vainfo \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary from the builder phase
COPY --from=builder /usr/src/app/target/release/track-overlay /usr/local/bin/track-overlay

# We set the entrypoint so you can pass arguments directly
ENTRYPOINT ["track-overlay"]
