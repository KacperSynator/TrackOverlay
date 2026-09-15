FROM rust:1.98-bookworm AS builder

# Install necessary dependencies for FFmpeg and egui
RUN apt-get update && apt-get install -y \
    clang \
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

# Final lightweight image (CPU-only)
FROM debian:bookworm-slim AS cpu

# Install runtime dependencies
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
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary from the builder phase
COPY --from=builder /usr/src/app/target/release/track-overlay /usr/local/bin/track-overlay

# We set the entrypoint so you can pass arguments directly
ENTRYPOINT ["track-overlay"]

# Final lightweight image (GPU support)
FROM cpu AS gpu

# Add bookworm-backports for newer Mesa drivers to support modern AMD/Intel GPUs
RUN echo "deb http://deb.debian.org/debian bookworm-backports main" > /etc/apt/sources.list.d/backports.list

# Install GPU drivers and wayland clipboard utilities
RUN apt-get update && apt-get install -y -t bookworm-backports \
    mesa-vulkan-drivers \
    libegl-mesa0 \
    libgl1-mesa-dri \
    && apt-get install -y \
    wl-clipboard \
    && rm -rf /var/lib/apt/lists/*
