use crate::project::ProjectConfig;
use crate::telemetry::TelemetryLog;
use anyhow::{Result, anyhow};
use ffmpeg_next as ffmpeg;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct ExportProgress {
    pub frames_done: usize,
    pub total_frames: usize,
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

    let mut encoder = encoder_ctx_video.open_as(encoder)?;

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

    let total_frames = if input_stream.frames() > 0 {
        input_stream.frames() as usize
    } else {
        let duration = input_stream.duration();
        if duration > 0 {
            let tb = input_stream.time_base();
            let time_base_f = f64::from(tb.numerator()) / f64::from(tb.denominator());
            let fps = f64::from(input_stream.rate().numerator())
                / f64::from(input_stream.rate().denominator());
            (duration as f64 * time_base_f * fps) as usize
        } else {
            0
        }
    };
    let mut frames_done = 0;

    for (stream, packet) in input_ctx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet)?;

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

                let mut packed_data = vec![0u8; (w * h * 4) as usize];
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
                    let sample = telemetry.sample_at(pts_ms as i64 + config.sync.offset_ms);
                    crate::overlay::render_overlay_skia(
                        &mut pixmap,
                        &config.elements,
                        sample.as_ref(),
                        None,
                    );
                }

                for y in 0..h as usize {
                    let src_start = y * (w * 4) as usize;
                    let dst_start = y * stride;
                    raw_data[dst_start..dst_start + (w * 4) as usize]
                        .copy_from_slice(&packed_data[src_start..src_start + (w * 4) as usize]);
                }

                scaler_to_yuv.run(&rgba_frame, &mut yuv_frame)?;

                yuv_frame.set_pts(decoded.pts());
                encoder.send_frame(&yuv_frame)?;

                let mut encoded = ffmpeg::Packet::empty();
                while encoder.receive_packet(&mut encoded).is_ok() {
                    encoded.set_stream(0);
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

        let mut packed_data = vec![0u8; (w * h * 4) as usize];
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
            let sample = telemetry.sample_at(pts_ms as i64 + config.sync.offset_ms);
            crate::overlay::render_overlay_skia(
                &mut pixmap,
                &config.elements,
                sample.as_ref(),
                None,
            );
        }

        for y in 0..h as usize {
            let src_start = y * (w * 4) as usize;
            let dst_start = y * stride;
            raw_data[dst_start..dst_start + (w * 4) as usize]
                .copy_from_slice(&packed_data[src_start..src_start + (w * 4) as usize]);
        }

        scaler_to_yuv.run(&rgba_frame, &mut yuv_frame)?;
        yuv_frame.set_pts(decoded.pts());
        encoder.send_frame(&yuv_frame)?;

        let mut encoded = ffmpeg::Packet::empty();
        while encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(0);
            encoded.write_interleaved(&mut output_ctx)?;
        }
    }

    encoder.send_eof()?;
    let mut encoded = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded).is_ok() {
        encoded.set_stream(0);
        encoded.write_interleaved(&mut output_ctx)?;
    }

    output_ctx.write_trailer()?;

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            temp_path.to_str().unwrap_or(""),
            "-i",
            &video_path,
            "-c:v",
            "copy",
            "-c:a",
            "copy",
            "-map",
            "0:v:0",
            "-map",
            "1:a:0?",
            output_path.to_str().unwrap_or("output.mp4"),
        ])
        .status()?;

    if !status.success() {
        std::fs::copy(&temp_path, output_path)?;
    }
    let _ = std::fs::remove_file(&temp_path);

    Ok(())
}
