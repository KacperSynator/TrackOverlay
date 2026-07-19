use anyhow::{anyhow, Result};
use gstreamer::prelude::*;
use gstreamer::{ClockTime, ElementFactory, MessageView, State};
use gstreamer_app::AppSink;
use std::path::Path;
use std::process::Command;
use chrono::{DateTime, Utc};

pub struct VideoPlayer {
    pipeline: gstreamer::Pipeline,
    appsink: AppSink,
    duration: Option<ClockTime>,
    width: u32,
    height: u32,
    pub creation_time_utc: Option<DateTime<Utc>>,
}

impl VideoPlayer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        gstreamer::init()?;

        let path_str = path.as_ref().to_string_lossy();

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

        let uri = format!("file://{}", path_str.replace(' ', "%20"));

        let source = ElementFactory::make("uridecodebin")
            .property("uri", &uri)
            .build()
            .map_err(|e| anyhow!("Failed to create uridecodebin: {}", e))?;

        let videoconvert = ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| anyhow!("Failed to create videoconvert: {}", e))?;

        let appsink = AppSink::builder()
            .caps(
                &gstreamer::Caps::builder("video/x-raw")
                    .field("format", "RGBA")
                    .build(),
            )
            .drop(true)
            .max_buffers(2)
            .build();

        // Emit frames as soon as they're decoded; don't wait for pipeline clock sync,
        // since we're driving playback manually via seeks.
        appsink.set_property("sync", false);

        let pipeline = gstreamer::Pipeline::new();
        pipeline.add_many([&source, &videoconvert, appsink.upcast_ref()])?;

        let videoconvert_clone = videoconvert.clone();
        source.connect_pad_added(move |_src, src_pad| {
            let is_video = src_pad
                .current_caps()
                .and_then(|caps| caps.structure(0).map(|s| s.name().starts_with("video/")))
                .unwrap_or(false);

            if is_video {
                let sink_pad = videoconvert_clone.static_pad("sink").unwrap();
                if sink_pad.is_linked() {
                    return; // Already linked
                }
                if let Err(e) = src_pad.link(&sink_pad) {
                    eprintln!("Failed to link video pad: {:?}", e);
                }
            }
        });

        gstreamer::Element::link_many([&videoconvert, appsink.upcast_ref()])?;

        // NOTE: we deliberately do NOT use bus.add_watch() here. add_watch() attaches
        // a GLib source to the default main context, which only gets serviced by an
        // actual running glib::MainLoop. eframe/egui drives its own event loop, not
        // GLib's, so an add_watch callback here would silently never fire — state
        // change failures would show up as opaque StateChangeError with no detail.
        // Instead, drain_bus_errors() below is called synchronously after every
        // state change / seek to pull real error messages off the bus.

        let player = Self {
            pipeline,
            appsink,
            duration: None,
            width: 0,
            height: 0,
            creation_time_utc,
        };

        Ok(player)
    }

    /// Synchronously pop any pending Error/Warning messages off the bus. Must be
    /// called explicitly (there is no running GLib main loop to do this for us).
    /// Returns the first error's message text, if any occurred.
    fn drain_bus_errors(&self) -> Option<String> {
        let bus = self.pipeline.bus()?;
        let mut first_error = None;
        while let Some(msg) = bus.pop() {
            match msg.view() {
                MessageView::Error(err) => {
                    let text = format!(
                        "{} ({:?}) from {:?}",
                        err.error(),
                        err.debug(),
                        err.src().map(|s| s.path_string())
                    );
                    eprintln!("GStreamer error: {}", text);
                    if first_error.is_none() {
                        first_error = Some(text);
                    }
                }
                MessageView::Warning(warn) => {
                    eprintln!(
                        "GStreamer warning: {} ({:?})",
                        warn.error(),
                        warn.debug()
                    );
                }
                _ => {}
            }
        }
        first_error
    }

    pub fn play(&self) -> Result<()> {
        self.pipeline.set_state(State::Playing)?;
        let (result, _, _) = self.pipeline.state(ClockTime::from_seconds(5));
        if let Err(e) = result {
            let detail = self.drain_bus_errors();
            return Err(anyhow!(
                "Failed to reach PLAYING state: {:?}{}",
                e,
                detail.map(|d| format!(" — {}", d)).unwrap_or_default()
            ));
        }
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.pipeline.set_state(State::Paused)?;
        let (result, _, _) = self.pipeline.state(ClockTime::from_seconds(5));
        result.map_err(|e| anyhow!("Failed to reach PAUSED state: {:?}", e))?;
        Ok(())
    }

    pub fn seek(&mut self, time_ms: i64) -> Result<()> {
        let time = ClockTime::from_mseconds(time_ms as u64);

        // Seeking requires the pipeline to have at least reached PAUSED (and
        // completed preroll). If something upstream failed silently, current_state()
        // will still be NULL/READY here, which is a much clearer signal than the
        // generic "Failed to seek" gstreamer error alone.
        let current = self.pipeline.current_state();
        if current < State::Paused {
            return Err(anyhow!(
                "Cannot seek: pipeline is in {:?} state, not yet PAUSED/prerolled. \
                 Check earlier log output for GStreamer errors during load.",
                current
            ));
        }

        // KEY_UNIT for faster seeking, FLUSH to clear stale buffers, ACCURATE to land
        // on the exact requested time rather than the nearest keyframe.
        let flags = gstreamer::SeekFlags::FLUSH
            | gstreamer::SeekFlags::KEY_UNIT
            | gstreamer::SeekFlags::ACCURATE;

        self.pipeline
            .seek_simple(flags, time)
            .map_err(|e| anyhow!("seek_simple failed at {}ms: {:?}", time_ms, e))?;

        // Seeks are asynchronous too — wait for the pipeline to settle back into a
        // steady state before the caller tries to pull a frame.
        let (result, _, _) = self.pipeline.state(ClockTime::from_seconds(5));
        result.map_err(|e| anyhow!("Failed to settle after seek: {:?}", e))?;

        Ok(())
    }

    pub fn get_frame(&mut self) -> Result<Option<gstreamer::Sample>> {
        if self.duration.is_none() {
            if let Some(dur) = self.pipeline.query_duration::<ClockTime>() {
                self.duration = Some(dur);
            }
        }

        // Give the decoder a more generous window to produce a frame, especially
        // right after a seek/state change where decode-from-keyframe takes a moment.
        let sample = self
            .appsink
            .try_pull_sample(gstreamer::ClockTime::from_mseconds(500));

        if let Some(s) = &sample {
            if self.width == 0 || self.height == 0 {
                let caps = s.caps().ok_or_else(|| anyhow!("Sample without caps"))?;
                let s_struct = caps.structure(0).unwrap();
                self.width = s_struct.get::<i32>("width").unwrap() as u32;
                self.height = s_struct.get::<i32>("height").unwrap() as u32;
            }
        }

        Ok(sample)
    }

    pub fn duration_ms(&mut self) -> Option<i64> {
        if self.duration.is_none() {
            if let Some(dur) = self.pipeline.query_duration::<ClockTime>() {
                self.duration = Some(dur);
            }
        }
        self.duration.map(|d| d.mseconds() as i64)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        // Make sure the pipeline is torn down cleanly rather than left dangling.
        let _ = self.pipeline.set_state(State::Null);
    }
}
