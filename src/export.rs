#![allow(clippy::collapsible_if)]
use crate::project::ProjectConfig;
use crate::telemetry::TelemetryLog;
use anyhow::{Result, anyhow};
use ffmpeg_next as ffmpeg;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct ExportProgress {
    pub frames_done: usize,
    pub total_frames: usize,
    pub start_time: Option<Instant>,
}

impl Default for ExportProgress {
    fn default() -> Self {
        Self {
            frames_done: 0,
            total_frames: 0,
            start_time: Some(Instant::now()),
        }
    }
}

pub fn export_video(
    config: &ProjectConfig,
    telemetry: &TelemetryLog,
    output_path: &Path,
    progress: Option<Arc<Mutex<ExportProgress>>>,
) -> Result<()> {
    println!("Starting export for {:?}", config.video_path);

    let video_path = config.video_path.to_str().unwrap_or("").to_string();
    if video_path.is_empty() {
        return Err(anyhow!("No video path specified for export"));
    }

    ffmpeg::init()?;

    let mut input_ctx = ffmpeg::format::input(&video_path)?;
    let input_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| anyhow!("No video stream found"))?;

    let video_stream_index = input_stream.index();
    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = decoder_ctx.decoder().video()?;

    let width = decoder.width();
    let height = decoder.height();
    let time_base = input_stream.time_base();
    let frame_rate = input_stream.rate();

    let temp_path = output_path.with_extension("temp.mp4");
    let mut output_ctx = ffmpeg::format::output(&temp_path)?;

    let encoder = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
        .ok_or_else(|| anyhow!("H264 encoder not found"))?;

    let mut output_stream = output_ctx.add_stream(encoder)?;

    let encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(encoder);

    let mut encoder_ctx_video = encoder_ctx.encoder().video()?;
    encoder_ctx_video.set_width(width);
    encoder_ctx_video.set_height(height);
    encoder_ctx_video.set_format(ffmpeg::format::Pixel::YUV420P);
    encoder_ctx_video.set_time_base(time_base);
    encoder_ctx_video.set_frame_rate(Some(frame_rate));

    let mut opts = ffmpeg::Dictionary::new();
    opts.set("preset", "medium");
    let mut encoder = encoder_ctx_video.open_as_with(encoder, opts)?;

    output_stream.set_parameters(&encoder);

    output_ctx.write_header()?;

    let mut scaler_to_rgba = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        width,
        height,
        ffmpeg::format::Pixel::RGBA,
        width,
        height,
        ffmpeg::software::scaling::flag::Flags::FAST_BILINEAR,
    )?;

    let mut scaler_to_yuv = ffmpeg::software::scaling::Context::get(
        ffmpeg::format::Pixel::RGBA,
        width,
        height,
        ffmpeg::format::Pixel::YUV420P,
        width,
        height,
        ffmpeg::software::scaling::flag::Flags::FAST_BILINEAR,
    )?;

    let mut decoded = ffmpeg::frame::Video::empty();
    let mut rgba_frame = ffmpeg::frame::Video::empty();
    let mut yuv_frame = ffmpeg::frame::Video::empty();

    // Create telemetry view to handle bounding and laps cleanly
    let telemetry_view = crate::telemetry::TelemetryView::new(
        telemetry,
        config.export_start_ms,
        config.export_end_ms,
        config.sync.offset_ms,
    );

    // Build a temporary TelemetryLog to generate trackmap correctly
    let temp_log = crate::telemetry::TelemetryLog {
        samples: telemetry_view.samples.to_vec(),
        start_time_utc: telemetry_view.start_time_utc,
    };
    let trackmap =
        crate::trackmap::TrackMap::from_telemetry(&temp_log, &telemetry_view.extract_laps());

    let fps = f64::from(input_stream.rate().numerator())
        / f64::from(input_stream.rate().denominator());

    let mut start_s = 0.0;
    if let Some(s) = config.export_start_ms {
        if s > 0 {
            start_s = s as f64 / 1000.0;
        }
    }

    let end_s = if let Some(e) = config.export_end_ms {
        if e >= 0 {
            e as f64 / 1000.0
        } else {
            let duration = input_stream.duration();
            if duration > 0 {
                let tb = input_stream.time_base();
                let time_base_f = f64::from(tb.numerator()) / f64::from(tb.denominator());
                duration as f64 * time_base_f
            } else {
                0.0
            }
        }
    } else {
        let duration = input_stream.duration();
        if duration > 0 {
            let tb = input_stream.time_base();
            let time_base_f = f64::from(tb.numerator()) / f64::from(tb.denominator());
            duration as f64 * time_base_f
        } else {
            0.0
        }
    };

    let duration_s = (end_s - start_s).max(0.0);
    let total_frames = (duration_s * fps).ceil() as usize;

    if let Some(p) = &progress {
        if let Ok(mut lock) = p.lock() {
            lock.start_time = Some(Instant::now());
            lock.total_frames = total_frames;
        }
    }

    let mut frames_done = 0;

    if let Some(start_ms) = config.export_start_ms {
        if start_ms > 0 {
            // Seek expects timestamps in AV_TIME_BASE (microseconds)
            let pts = (start_ms as f64 * 1000.0) as i64;
            input_ctx.seek(pts, ..pts).unwrap_or_else(|e| {
                eprintln!("Failed to seek to start: {}", e);
            });
        }
    }

    let mut finished = false;
    let mut first_pts: Option<i64> = None;
    let mut packed_data = Vec::new();

    for (stream, packet) in input_ctx.packets() {
        if finished {
            break;
        }

        if stream.index() == video_stream_index {
            decoder.send_packet(&packet)?;

            while decoder.receive_frame(&mut decoded).is_ok() {
                let pts_ms = decoded.pts().unwrap_or(0) as f64 * time_base.numerator() as f64
                    / time_base.denominator() as f64
                    * 1000.0;

                if let Some(start_ms) = config.export_start_ms {
                    if (pts_ms as i64) < start_ms {
                        frames_done += 1;
                        if let Some(p) = &progress {
                            if let Ok(mut lock) = p.lock() {
                                lock.frames_done = frames_done;
                                lock.total_frames = total_frames.max(frames_done);
                            }
                        }
                        continue;
                    }
                }

                if let Some(end_ms) = config.export_end_ms {
                    if end_ms >= 0 && (pts_ms as i64) > end_ms {
                        finished = true;
                        break;
                    }
                }

                if first_pts.is_none() {
                    first_pts = Some(decoded.pts().unwrap_or(0));
                }

                frames_done += 1;
                if let Some(p) = &progress {
                    if let Ok(mut lock) = p.lock() {
                        lock.frames_done = frames_done;
                        lock.total_frames = total_frames.max(frames_done);
                    }
                }

                scaler_to_rgba.run(&decoded, &mut rgba_frame)?;

                let w = rgba_frame.width();
                let h = rgba_frame.height();
                let stride = rgba_frame.stride(0);
                let raw_data = rgba_frame.data_mut(0);

                packed_data.resize((w * h * 4) as usize, 0);
                for y in 0..h as usize {
                    let src_y = if config.flip_vertical {
                        (h as usize - 1) - y
                    } else {
                        y
                    };

                    let src_start = src_y * stride;
                    let dst_start = y * (w * 4) as usize;

                    if config.flip_horizontal {
                        for x in 0..w as usize {
                            let src_x = (w as usize - 1) - x;
                            let src_idx = src_start + src_x * 4;
                            let dst_idx = dst_start + x * 4;
                            packed_data[dst_idx..dst_idx + 4]
                                .copy_from_slice(&raw_data[src_idx..src_idx + 4]);
                        }
                    } else {
                        packed_data[dst_start..dst_start + (w * 4) as usize]
                            .copy_from_slice(&raw_data[src_start..src_start + (w * 4) as usize]);
                    }
                }

                if let Some(mut pixmap) = tiny_skia::PixmapMut::from_bytes(&mut packed_data, w, h) {
                    let pts_ms = decoded.pts().unwrap_or(0) as f64 * time_base.numerator() as f64
                        / time_base.denominator() as f64
                        * 1000.0;
                    let state = telemetry_view.get_state(pts_ms as i64 + config.sync.offset_ms);
                    crate::overlay::render_overlay_skia(
                        &mut pixmap,
                        &config.elements,
                        &state,
                        trackmap.as_ref(),
                    );
                }

                for y in 0..h as usize {
                    let src_start = y * (w * 4) as usize;
                    let dst_start = y * stride;
                    raw_data[dst_start..dst_start + (w * 4) as usize]
                        .copy_from_slice(&packed_data[src_start..src_start + (w * 4) as usize]);
                }

                scaler_to_yuv.run(&rgba_frame, &mut yuv_frame)?;

                // Adjust PTS so it starts at 0
                let adjusted_pts = decoded
                    .pts()
                    .unwrap_or(0)
                    .saturating_sub(first_pts.unwrap_or(0));
                yuv_frame.set_pts(Some(adjusted_pts));
                encoder.send_frame(&yuv_frame)?;

                let mut encoded = ffmpeg::Packet::empty();
                while encoder.receive_packet(&mut encoded).is_ok() {
                    encoded.set_stream(0);
                    encoded.rescale_ts(time_base, output_ctx.stream(0).unwrap().time_base());
                    encoded.write_interleaved(&mut output_ctx)?;
                }
            }
        }
    }

    decoder.send_eof()?;
    while decoder.receive_frame(&mut decoded).is_ok() {
        frames_done += 1;
        if let Some(p) = &progress
            && let Ok(mut lock) = p.lock()
        {
            lock.frames_done = frames_done;
            lock.total_frames = total_frames.max(frames_done);
        }

        scaler_to_rgba.run(&decoded, &mut rgba_frame)?;

        let w = rgba_frame.width();
        let h = rgba_frame.height();
        let stride = rgba_frame.stride(0);
        let raw_data = rgba_frame.data_mut(0);

        packed_data.resize((w * h * 4) as usize, 0);
        for y in 0..h as usize {
            let src_start = y * stride;
            let dst_start = y * (w * 4) as usize;
            packed_data[dst_start..dst_start + (w * 4) as usize]
                .copy_from_slice(&raw_data[src_start..src_start + (w * 4) as usize]);
        }

        if let Some(mut pixmap) = tiny_skia::PixmapMut::from_bytes(&mut packed_data, w, h) {
            let pts_ms = decoded.pts().unwrap_or(0) as f64 * time_base.numerator() as f64
                / time_base.denominator() as f64
                * 1000.0;
            let state = telemetry_view.get_state(pts_ms as i64 + config.sync.offset_ms);
            crate::overlay::render_overlay_skia(
                &mut pixmap,
                &config.elements,
                &state,
                trackmap.as_ref(),
            );
        }

        for y in 0..h as usize {
            let src_start = y * (w * 4) as usize;
            let dst_start = y * stride;
            raw_data[dst_start..dst_start + (w * 4) as usize]
                .copy_from_slice(&packed_data[src_start..src_start + (w * 4) as usize]);
        }

        scaler_to_yuv.run(&rgba_frame, &mut yuv_frame)?;

        let adjusted_pts = decoded
            .pts()
            .unwrap_or(0)
            .saturating_sub(first_pts.unwrap_or(0));
        yuv_frame.set_pts(Some(adjusted_pts));
        encoder.send_frame(&yuv_frame)?;

        let mut encoded = ffmpeg::Packet::empty();
        while encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(0);
            encoded.rescale_ts(time_base, output_ctx.stream(0).unwrap().time_base());
            encoded.write_interleaved(&mut output_ctx)?;
        }
    }

    encoder.send_eof()?;
    let mut encoded = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded).is_ok() {
        encoded.set_stream(0);
        encoded.rescale_ts(time_base, output_ctx.stream(0).unwrap().time_base());
        encoded.write_interleaved(&mut output_ctx)?;
    }

    output_ctx.write_trailer()?;

    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        temp_path.to_str().unwrap_or("").to_string(),
    ];

    // Add start offset for audio from the original video if trimming
    if let Some(start_ms) = config.export_start_ms {
        if start_ms > 0 {
            args.push("-ss".to_string());
            args.push(format!("{:.3}", start_ms as f64 / 1000.0));
        }
    }

    // Output duration is what matters since we trim both inputs appropriately
    if let Some(end_ms) = config.export_end_ms {
        let start_ms = config.export_start_ms.unwrap_or(0).max(0);
        if end_ms >= 0 && end_ms > start_ms {
            args.push("-t".to_string());
            args.push(format!("{:.3}", (end_ms - start_ms) as f64 / 1000.0));
        }
    }

    args.extend(vec![
        "-i".to_string(),
        video_path.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-c:a".to_string(),
        "copy".to_string(),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "1:a:0?".to_string(),
        output_path.to_str().unwrap_or("output.mp4").to_string(),
    ]);

    let status = std::process::Command::new("ffmpeg").args(args).status()?;

    if !status.success() {
        std::fs::copy(&temp_path, output_path)?;
    }
    let _ = std::fs::remove_file(&temp_path);

    Ok(())
}
