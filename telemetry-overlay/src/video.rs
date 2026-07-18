use anyhow::{anyhow, Result};
use gstreamer::prelude::*;
use gstreamer::{ElementFactory, State, ClockTime};
use gstreamer_app::AppSink;
use std::path::Path;

pub struct VideoPlayer {
    pipeline: gstreamer::Pipeline,
    appsink: AppSink,
    duration: Option<ClockTime>,
    width: u32,
    height: u32,
}

impl VideoPlayer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        gstreamer::init()?;

        let path_str = path.as_ref().to_string_lossy();
        let uri = format!("file://{}", path_str.replace(" ", "%20"));

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
            .max_buffers(1) // Keep memory low
            .build();

        let pipeline = gstreamer::Pipeline::new();
        pipeline.add_many(&[&source, &videoconvert, appsink.upcast_ref()])?;

        // We link uridecodebin dynamically because it creates pads when it knows the stream type
        let _pipeline_clone = pipeline.clone();
        let videoconvert_clone = videoconvert.clone();
        source.connect_pad_added(move |_src, src_pad| {
            let is_video = src_pad.current_caps()
                .and_then(|caps| caps.structure(0).map(|s| s.name().starts_with("video/")))
                .unwrap_or(false);

            if is_video {
                let sink_pad = videoconvert_clone.static_pad("sink").unwrap();
                if sink_pad.is_linked() {
                    return; // Already linked
                }
                let _ = src_pad.link(&sink_pad);
            }
        });

        gstreamer::Element::link_many(&[&videoconvert, appsink.upcast_ref()])?;

        let player = Self {
            pipeline,
            appsink,
            duration: None,
            width: 0, // Filled during playback
            height: 0,
        };

        Ok(player)
    }

    pub fn play(&self) -> Result<()> {
        self.pipeline.set_state(State::Playing)?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.pipeline.set_state(State::Paused)?;
        Ok(())
    }

    pub fn seek(&self, time_ms: i64) -> Result<()> {
        let time = ClockTime::from_mseconds(time_ms as u64);
        self.pipeline.seek_simple(
            gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT,
            time,
        )?;
        Ok(())
    }

    pub fn get_frame(&mut self) -> Result<Option<gstreamer::Sample>> {
        if self.duration.is_none() {
            // Try to query duration
            if let Some(dur) = self.pipeline.query_duration::<ClockTime>() {
                self.duration = Some(dur);
            }
        }

        let sample = self.appsink.try_pull_sample(gstreamer::ClockTime::from_mseconds(10));

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

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
