use crate::video::{DecodedFrame, PlayerCommand};
use crossbeam_channel::Receiver;
use ffmpeg_next as ffmpeg;
use log::warn;
use lru::LruCache;
use std::sync::{Arc, Mutex};

pub(crate) struct VideoDecoderWorker {
    pub input_ctx: ffmpeg::format::context::Input,
    pub decoder: ffmpeg::decoder::Video,
    pub scaler: ffmpeg::software::scaling::Context,
    pub frame_cache: LruCache<i64, DecodedFrame>,
    pub time_base: f64,
    pub start_pts: i64,
    pub video_stream_index: usize,
    pub latest_frame_bg: Arc<Mutex<Option<DecodedFrame>>>,
    pub repaint_cb: Box<dyn Fn() + Send + 'static>,
}

impl VideoDecoderWorker {
    fn drain_commands(&self, cmd_rx: &Receiver<PlayerCommand>) -> Option<i64> {
        let mut final_time_ms = match cmd_rx.recv() {
            Ok(PlayerCommand::Seek(ms)) => ms,
            Ok(PlayerCommand::Quit) => return None,
            Err(_) => return None,
        };

        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                PlayerCommand::Seek(ms) => final_time_ms = ms,
                PlayerCommand::Quit => return None,
            }
        }
        Some(final_time_ms)
    }

    fn find_cached_frame(&mut self, final_time_ms: i64) -> Option<DecodedFrame> {
        for (pts, frame) in self.frame_cache.iter() {
            let pts_ms = (*pts as f64 * self.time_base * 1000.0) as i64;
            if (pts_ms - final_time_ms).abs() < 40 {
                return Some(frame.clone());
            }
        }
        None
    }

    fn seek_decoder_if_needed(
        &mut self,
        final_time_ms: i64,
        target_pts: i64,
        current_decoder_pts: &mut i64,
    ) {
        let pts_diff = target_pts - *current_decoder_pts;
        let ms_diff = pts_diff as f64 * self.time_base * 1000.0;

        if !(0.0..=2000.0).contains(&ms_diff) {
            // Seek operates in AV_TIME_BASE by default with this ffmpeg wrapper
            let seek_ts_av = final_time_ms * 1000;
            if self.input_ctx.seek(seek_ts_av, ..).is_ok() {
                self.decoder.flush();
                *current_decoder_pts = target_pts;
            } else {
                let _ = self.input_ctx.seek(seek_ts_av, ..);
                self.decoder.flush();
                *current_decoder_pts = target_pts;
            }
        }
    }

    fn decode_to_target(&mut self, target_pts: i64, current_decoder_pts: &mut i64) {
        let mut decoded = ffmpeg::frame::Video::empty();
        let packet_iter = self.input_ctx.packets();

        let mut attempt_limit = 1000;

        for (stream, packet) in packet_iter {
            if attempt_limit == 0 {
                warn!("Timed out decoding forward to PTS {}", target_pts);
                break;
            }
            attempt_limit -= 1;

            if stream.index() == self.video_stream_index {
                if self.decoder.send_packet(&packet).is_err() {
                    continue;
                }

                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    let current_pts = decoded.pts().unwrap_or(*current_decoder_pts);
                    *current_decoder_pts = current_pts;

                    let mut rgb_frame = ffmpeg::frame::Video::empty();
                    if self.scaler.run(&decoded, &mut rgb_frame).is_ok() {
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
                            pts_ms: (current_pts as f64 * self.time_base * 1000.0) as i64,
                        };

                        self.frame_cache.put(current_pts, frame.clone());

                        if current_pts >= target_pts {
                            if let Ok(mut lf) = self.latest_frame_bg.lock() {
                                *lf = Some(frame);
                            }
                            (self.repaint_cb)();
                            return;
                        }
                    }
                }

                if *current_decoder_pts >= target_pts {
                    return;
                }
            }
        }
    }

    pub(crate) fn run(mut self, cmd_rx: Receiver<PlayerCommand>) {
        let mut current_decoder_pts = self.start_pts;

        loop {
            let final_time_ms = match self.drain_commands(&cmd_rx) {
                Some(ms) => ms,
                None => return,
            };

            let target_pts =
                self.start_pts + (final_time_ms as f64 / 1000.0 / self.time_base) as i64;

            if let Some(frame) = self.find_cached_frame(final_time_ms) {
                if let Ok(mut lf) = self.latest_frame_bg.lock() {
                    *lf = Some(frame);
                }
                (self.repaint_cb)();
                continue;
            }

            self.seek_decoder_if_needed(final_time_ms, target_pts, &mut current_decoder_pts);
            self.decode_to_target(target_pts, &mut current_decoder_pts);
        }
    }
}
