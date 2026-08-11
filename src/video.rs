use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use crossbeam_channel::{Sender, unbounded};

use ffmpeg_next as ffmpeg;
use log::error;
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

pub(crate) enum PlayerCommand {
    Seek(i64),
    Quit,
}

use crate::video_worker::VideoDecoderWorker;

pub struct VideoPlayer {
    pub creation_time_utc: Option<DateTime<Utc>>,
    duration_ms: Option<i64>,
    width: u32,
    height: u32,

    cmd_tx: Sender<PlayerCommand>,
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    error_state: Arc<Mutex<Option<String>>>,
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(PlayerCommand::Quit);
    }
}

impl VideoPlayer {
    pub fn new<P: AsRef<Path>, F: Fn() + Send + 'static>(path: P, repaint_cb: F) -> Result<Self> {
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
                && let Ok(dt) = DateTime::parse_from_rfc3339(&time_str)
            {
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

        let error_state = Arc::new(Mutex::new(None));
        let error_state_bg = error_state.clone();

        let (cmd_tx, cmd_rx) = unbounded::<PlayerCommand>();

        let path_for_thread = path_str.clone();
        let repaint_cb_box = Box::new(repaint_cb);

        thread::spawn(move || {
            let input_ctx = match ffmpeg::format::input(&path_for_thread) {
                Ok(ctx) => ctx,
                Err(e) => {
                    error!("Failed to open video in bg thread: {}", e);
                    return;
                }
            };
            let stream = match input_ctx.streams().best(ffmpeg::media::Type::Video) {
                Some(s) => s,
                None => {
                    let msg = "No video stream found".to_string();
                    error!("Background video error: {}", msg);
                    if let Ok(mut lock) = error_state_bg.lock() {
                        *lock = Some(msg);
                    }
                    return;
                }
            };
            let codec_ctx =
                match ffmpeg::codec::context::Context::from_parameters(stream.parameters()) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        let msg = format!("Failed to get codec context parameters: {}", e);
                        error!("Background video error: {}", msg);
                        if let Ok(mut lock) = error_state_bg.lock() {
                            *lock = Some(msg);
                        }
                        return;
                    }
                };
            let decoder = match codec_ctx.decoder().video() {
                Ok(d) => d,
                Err(e) => {
                    let msg = format!("Failed to create video decoder: {}", e);
                    error!("Background video error: {}", msg);
                    if let Ok(mut lock) = error_state_bg.lock() {
                        *lock = Some(msg);
                    }
                    return;
                }
            };

            let scaler = match ffmpeg::software::scaling::Context::get(
                decoder.format(),
                width,
                height,
                ffmpeg::format::Pixel::RGBA,
                width,
                height,
                ffmpeg::software::scaling::flag::Flags::FAST_BILINEAR,
            ) {
                Ok(s) => s,
                Err(e) => {
                    let msg = format!("Failed to create scaling context: {}", e);
                    error!("Background video error: {}", msg);
                    if let Ok(mut lock) = error_state_bg.lock() {
                        *lock = Some(msg);
                    }
                    return;
                }
            };

            let frame_cache: LruCache<i64, DecodedFrame> =
                LruCache::new(NonZeroUsize::new(200).unwrap());

            let worker = VideoDecoderWorker {
                input_ctx,
                decoder,
                scaler,
                frame_cache,
                time_base,
                start_pts,
                video_stream_index,
                latest_frame_bg,
                repaint_cb: repaint_cb_box,
            };
            worker.run(cmd_rx);
        });

        Ok(Self {
            creation_time_utc,
            duration_ms,
            width,
            height,
            cmd_tx,
            latest_frame,
            error_state,
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

    pub fn get_error(&self) -> Option<String> {
        if let Ok(lock) = self.error_state.lock() {
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
