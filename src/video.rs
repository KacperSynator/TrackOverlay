use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use crossbeam_channel::{Sender, unbounded};
use eframe::egui;
use ffmpeg_next as ffmpeg;
use log::{error, warn};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
pub struct DecodedFrame {
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub pts_ms: i64,
}

enum PlayerCommand {
    Seek(i64),
    Quit,
}

pub struct VideoPlayer {
    pub creation_time_utc: Option<DateTime<Utc>>,
    duration_ms: Option<i64>,
    width: u32,
    height: u32,

    cmd_tx: Sender<PlayerCommand>,
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(PlayerCommand::Quit);
    }
}

impl VideoPlayer {
    pub fn new<P: AsRef<Path>>(path: P, ctx: egui::Context) -> Result<Self> {
        ffmpeg::init()?;
        let path_str = path.as_ref().to_string_lossy().to_string();

        let mut creation_time_utc = None;
        if let Ok(output) = Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream_tags=creation_time",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                &path_str,
            ])
            .output()
        {
            let time_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !time_str.is_empty()
                && let Ok(dt) = DateTime::parse_from_rfc3339(&time_str) {
                    creation_time_utc = Some(dt.with_timezone(&Utc));
                }
        }

        let input_ctx = ffmpeg::format::input(&path_str)?;
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

        let latest_frame = Arc::new(Mutex::new(None));
        let latest_frame_bg = latest_frame.clone();

        let (cmd_tx, cmd_rx) = unbounded::<PlayerCommand>();

        let path_for_thread = path_str.clone();

        thread::spawn(move || {
            let mut input_ctx = match ffmpeg::format::input(&path_for_thread) {
                Ok(ctx) => ctx,
                Err(e) => {
                    error!("Failed to open video in bg thread: {}", e);
                    return;
                }
            };
            let stream = input_ctx
                .streams()
                .best(ffmpeg::media::Type::Video)
                .unwrap();
            let codec_ctx =
                ffmpeg::codec::context::Context::from_parameters(stream.parameters()).unwrap();
            let mut decoder = codec_ctx.decoder().video().unwrap();

            let mut scaler = ffmpeg::software::scaling::Context::get(
                decoder.format(),
                width,
                height,
                ffmpeg::format::Pixel::RGBA,
                width,
                height,
                ffmpeg::software::scaling::flag::Flags::FAST_BILINEAR,
            )
            .unwrap();

            let mut frame_cache: LruCache<i64, DecodedFrame> =
                LruCache::new(NonZeroUsize::new(200).unwrap());
            let mut current_decoder_pts = start_pts;

            loop {
                let target_time_ms = match cmd_rx.recv() {
                    Ok(PlayerCommand::Seek(ms)) => ms,
                    Ok(PlayerCommand::Quit) => return,
                    Err(_) => return,
                };

                let mut final_time_ms = target_time_ms;
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        PlayerCommand::Seek(ms) => final_time_ms = ms,
                        PlayerCommand::Quit => return,
                    }
                }

                let target_pts = start_pts + (final_time_ms as f64 / 1000.0 / time_base) as i64;

                let mut found_cached = None;
                for (pts, frame) in frame_cache.iter() {
                    let pts_ms = (*pts as f64 * time_base * 1000.0) as i64;
                    if (pts_ms - final_time_ms).abs() < 40 {
                        found_cached = Some(frame.clone());
                        break;
                    }
                }

                if let Some(frame) = found_cached {
                    if let Ok(mut lf) = latest_frame_bg.lock() {
                        *lf = Some(frame);
                    }
                    ctx.request_repaint();
                    continue;
                }

                let pts_diff = target_pts - current_decoder_pts;
                let ms_diff = pts_diff as f64 * time_base * 1000.0;

                if !(0.0..=2000.0).contains(&ms_diff) {
                    // Seek operates in AV_TIME_BASE by default with this ffmpeg wrapper
                    let seek_ts_av = final_time_ms * 1000;
                    if input_ctx.seek(seek_ts_av, ..).is_ok() {
                        decoder.flush();
                        current_decoder_pts = target_pts;
                    } else {
                        let _ = input_ctx.seek(seek_ts_av, ..);
                        decoder.flush();
                        current_decoder_pts = target_pts;
                    }
                }

                let mut decoded = ffmpeg::frame::Video::empty();
                let mut packet_iter = input_ctx.packets();

                let mut attempt_limit = 1000;

                for (stream, packet) in packet_iter {
                    if attempt_limit == 0 {
                        warn!("Timed out decoding forward to PTS {}", target_pts);
                        break;
                    }
                    attempt_limit -= 1;

                    if stream.index() == video_stream_index {
                        if decoder.send_packet(&packet).is_err() {
                            continue;
                        }

                        while decoder.receive_frame(&mut decoded).is_ok() {
                            let current_pts = decoded.pts().unwrap_or(current_decoder_pts);
                            current_decoder_pts = current_pts;

                            let mut rgb_frame = ffmpeg::frame::Video::empty();
                            if scaler.run(&decoded, &mut rgb_frame).is_ok() {
                                let w = rgb_frame.width() as usize;
                                let h = rgb_frame.height() as usize;
                                let stride = rgb_frame.stride(0);

                                let mut packed_data = Vec::with_capacity(w * h * 4);
                                let raw_data = rgb_frame.data(0);

                                for y in 0..h {
                                    let row_start = y * stride;
                                    let row_end = row_start + w * 4;
                                    packed_data.extend_from_slice(&raw_data[row_start..row_end]);
                                }

                                let frame = DecodedFrame {
                                    data: Arc::new(packed_data),
                                    width: w as u32,
                                    height: h as u32,
                                    pts_ms: (current_pts as f64 * time_base * 1000.0) as i64,
                                };

                                frame_cache.put(current_pts, frame.clone());

                                if current_pts >= target_pts {
                                    if let Ok(mut lf) = latest_frame_bg.lock() {
                                        *lf = Some(frame);
                                    }
                                    ctx.request_repaint();
                                    break;
                                }
                            }
                        }

                        if current_decoder_pts >= target_pts {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self {
            creation_time_utc,
            duration_ms,
            width,
            height,
            cmd_tx,
            latest_frame,
        })
    }

    pub fn seek(&mut self, time_ms: i64) -> Result<()> {
        let _ = self.cmd_tx.send(PlayerCommand::Seek(time_ms));
        Ok(())
    }

    pub fn get_frame(&mut self) -> Option<DecodedFrame> {
        if let Ok(lock) = self.latest_frame.lock() {
            lock.clone()
        } else {
            None
        }
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
