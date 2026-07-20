use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;
use chrono::{DateTime, Utc};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::format::context::Input;
use ffmpeg_next::software::scaling;
use log::warn;

pub struct DecodedFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct VideoPlayer {
    pub creation_time_utc: Option<DateTime<Utc>>,
    duration_ms: Option<i64>,
    width: u32,
    height: u32,

    input_ctx: Input,
    video_stream_index: usize,
    decoder: ffmpeg::decoder::Video,
    scaler: scaling::Context,
    time_base: f64,
    start_pts: i64,

    // Track the target PTS when we seek, so we decode forward to the exact frame
    target_pts: Option<i64>,
    last_frame: Option<DecodedFrame>,
}

impl VideoPlayer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        ffmpeg::init()?;
        let path_str = path.as_ref().to_string_lossy();

        // Extract creation time
        let mut creation_time_utc = None;
        if let Ok(output) = Command::new("ffprobe")
            .args(&[
                "-v", "quiet",
                "-select_streams", "v:0",
                "-show_entries", "stream_tags=creation_time",
                "-of", "default=noprint_wrappers=1:nokey=1",
                &path_str,
            ])
            .output()
        {
            let time_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !time_str.is_empty() {
                if let Ok(dt) = DateTime::parse_from_rfc3339(&time_str) {
                    creation_time_utc = Some(dt.with_timezone(&Utc));
                }
            }
        }

        let input_ctx = ffmpeg::format::input(&path)?;
        let stream = input_ctx
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or_else(|| anyhow!("No video stream found"))?;

        let video_stream_index = stream.index();
        let tb = stream.time_base();
        let time_base = f64::from(tb.numerator()) / f64::from(tb.denominator());

        let start_pts = stream.start_time().max(0);

        let duration_ms = if stream.duration() >= 0 {
            Some((stream.duration() as f64 * time_base * 1000.0) as i64)
        } else {
            None
        };

        let codec_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        let decoder = codec_ctx.decoder().video()?;

        let width = decoder.width();
        let height = decoder.height();

        let scaler = scaling::Context::get(
            decoder.format(),
            width,
            height,
            ffmpeg::format::Pixel::RGBA,
            width,
            height,
            scaling::flag::Flags::FAST_BILINEAR,
        )?;

        Ok(Self {
            creation_time_utc,
            duration_ms,
            width,
            height,
            input_ctx,
            video_stream_index,
            decoder,
            scaler,
            time_base,
            start_pts,
            target_pts: None,
            last_frame: None,
        })
    }

    pub fn play(&self) -> Result<()> { Ok(()) }
    pub fn pause(&self) -> Result<()> { Ok(()) }

    pub fn seek(&mut self, time_ms: i64) -> Result<()> {
        // Many containers have a non-zero start_pts, so we must add it to the requested seek time
        let target_pts = self.start_pts + (time_ms as f64 / 1000.0 / self.time_base) as i64;
        self.target_pts = Some(target_pts);

        // Seek to the nearest keyframe *before* the target
        self.input_ctx.seek(target_pts, ..target_pts)?;
        self.decoder.flush();

        Ok(())
    }

    pub fn get_frame(&mut self) -> Result<Option<&DecodedFrame>> {
        let mut decoded = ffmpeg::frame::Video::empty();

        // Target PTS to hit, if we're seeking
        let target = self.target_pts.unwrap_or(-1);
        let mut attempt_limit = 1000; // GoPros have dense tracks; don't time out easily

        // Process any frames currently buffered inside the decoder first
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            if let Some(pts) = decoded.pts() {
                if target < 0 || pts >= target {
                    self.target_pts = None;
                    if let Ok(Some(frame)) = self.process_frame(&decoded) {
                        self.last_frame = Some(frame);
                    }
                    return Ok(self.last_frame.as_ref());
                }
            }
        }

        // Read packets and push to decoder
        let mut packet_iter = self.input_ctx.packets();
        while let Some((stream, packet)) = packet_iter.next() {
            if stream.index() == self.video_stream_index {
                if attempt_limit == 0 {
                    warn!("Seeking timed out looking for PTS >= {}. Yielding latest frame.", target);
                    self.target_pts = None;
                    return Ok(self.last_frame.as_ref());
                }
                attempt_limit -= 1;

                self.decoder.send_packet(&packet)?;
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    if let Some(pts) = decoded.pts() {
                        if target < 0 || pts >= target {
                            self.target_pts = None;
                            if let Ok(Some(frame)) = self.process_frame(&decoded) {
                                self.last_frame = Some(frame);
                            }
                            return Ok(self.last_frame.as_ref());
                        }
                    } else {
                        // If no PTS, just return the frame
                        self.target_pts = None;
                        if let Ok(Some(frame)) = self.process_frame(&decoded) {
                            self.last_frame = Some(frame);
                        }
                        return Ok(self.last_frame.as_ref());
                    }
                }
            }
        }

        // If we hit EOF or the loop ended, try flushing the decoder to see if any frames pop out
        let _ = self.decoder.send_eof();
        if self.decoder.receive_frame(&mut decoded).is_ok() {
            self.target_pts = None;
            if let Ok(Some(frame)) = self.process_frame(&decoded) {
                self.last_frame = Some(frame);
            }
            return Ok(self.last_frame.as_ref());
        }

        Ok(self.last_frame.as_ref())
    }

    fn process_frame(&mut self, decoded: &ffmpeg::frame::Video) -> Result<Option<DecodedFrame>> {
        let mut rgb_frame = ffmpeg::frame::Video::empty();
        self.scaler.run(decoded, &mut rgb_frame)?;

        let width = rgb_frame.width() as usize;
        let height = rgb_frame.height() as usize;
        let stride = rgb_frame.stride(0) as usize;

        let mut packed_data = Vec::with_capacity(width * height * 4);
        let raw_data = rgb_frame.data(0);

        for y in 0..height {
            let row_start = y * stride;
            let row_end = row_start + width * 4;
            packed_data.extend_from_slice(&raw_data[row_start..row_end]);
        }

        Ok(Some(DecodedFrame {
            data: packed_data,
            width: width as u32,
            height: height as u32,
        }))
    }

    pub fn duration_ms(&mut self) -> Option<i64> {
        self.duration_ms
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
