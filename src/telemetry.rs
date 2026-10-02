use crate::error::TelemetryError;
use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct RawTelemetryRow {
    pub time: f64,
    #[serde(rename = "UTC Time")]
    pub utc_time: f64,
    pub lap: u32,
    #[serde(rename = "Latitude")]
    pub latitude: f64,
    #[serde(rename = "Longitude")]
    pub longitude: f64,
    #[serde(rename = "Speed (Km/h)")]
    pub speed_kph: f32,
    #[serde(rename = "Accel X")]
    pub accel_x: f32,
    #[serde(rename = "Accel Y")]
    pub accel_y: f32,
    #[serde(rename = "Accel Z")]
    pub accel_z: f32,

    // Add Throttle mapping. We use an Option because it might not be in all files,
    // or we can use serde(default) if we want to fallback to 0.0
    #[serde(rename = "Throttle Position (%) *OBD", default)]
    pub throttle_position: f32,

    #[serde(rename = "Brake (calculated)", default)]
    pub brake: f32,

    #[serde(rename = "Engine Speed (RPM) *OBD", default)]
    pub engine_speed_rpm: f32,

    #[serde(rename = "Vehicle Speed (km/h) *OBD", default)]
    pub obd_speed_kph: f32,

    #[serde(rename = "GPS_Update", default)]
    pub gps_update: u8,

    #[serde(rename = "OBD_Update", default)]
    pub obd_update: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TelemetrySample {
    pub time_ms: i64,
    pub speed_kph: f32,
    pub lat: f64,
    pub lon: f64,
    pub accel_lat_g: f32,
    pub accel_lon_g: f32,
    pub lap_number: Option<u32>,
    pub lap_time_ms: Option<i64>,
    pub throttle_pct: f32,
    pub brake: f32,
    pub engine_speed_rpm: f32,

    pub session_distance_m: f64,
    pub lap_distance_m: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LapStat {
    pub lap_number: u32,
    pub start_time_ms: i64,
    pub end_time_ms: i64,
    pub duration_ms: i64,
    pub total_distance_m: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryState {
    pub current_sample: Option<TelemetrySample>,
    pub previous_laps: Vec<LapStat>,
    pub best_lap: Option<LapStat>,
    pub projection_ms: Option<i64>, // Diff to best lap
}

#[derive(Clone)]
pub struct TelemetryLog {
    pub raw_samples: Vec<TelemetrySample>,
    pub samples: Vec<TelemetrySample>,
    pub start_time_utc: Option<DateTime<Utc>>,
    pub parsed_speed_source: crate::project::SpeedSource,
}

impl TelemetryLog {
    pub fn load_csv<P: AsRef<Path>>(
        path: P,
        speed_source: crate::project::SpeedSource,
    ) -> Result<Self, TelemetryError> {
        let mut rdr = csv::ReaderBuilder::new()
            .comment(Some(b'#'))
            .from_path(path.as_ref())?;

        let actual_speed_source = if speed_source == crate::project::SpeedSource::Auto {
            let mut gps_count = 0;
            let mut obd_count = 0;
            for row in rdr.deserialize::<RawTelemetryRow>().flatten() {
                if row.gps_update == 1 {
                    gps_count += 1;
                }
                if row.obd_update == 1 {
                    obd_count += 1;
                }
            }
            if obd_count > gps_count {
                crate::project::SpeedSource::Obd
            } else {
                crate::project::SpeedSource::Gps
            }
        } else {
            speed_source
        };

        // Re-initialize reader for actual parsing
        let mut rdr = csv::ReaderBuilder::new()
            .comment(Some(b'#'))
            .from_path(path.as_ref())?;

        let mut samples = Vec::new();
        let mut lap_start_time = 0.0;
        let mut current_lap = 0;
        let mut start_time_utc = None;
        let mut session_distance_m = 0.0;
        let mut lap_distance_m = 0.0;
        let mut last_time = 0.0;

        for (i, result) in rdr.deserialize().enumerate() {
            let row: RawTelemetryRow = match result {
                Ok(r) => r,
                Err(_) => continue, // Skip malformed rows
            };

            let current_speed_kph = match actual_speed_source {
                crate::project::SpeedSource::Obd => row.obd_speed_kph,
                _ => row.speed_kph,
            };

            if i == 0 {
                // Assuming UTC Time is unix timestamp in seconds
                if let Some(dt) = Utc
                    .timestamp_opt(
                        row.utc_time as i64,
                        ((row.utc_time.fract()) * 1_000_000_000.0) as u32,
                    )
                    .single()
                {
                    start_time_utc = Some(dt);
                }
            }

            if row.lap != current_lap {
                current_lap = row.lap;
                lap_start_time = row.time;
                lap_distance_m = 0.0;
            }

            let dt = row.time - last_time;
            if i > 0 && dt > 0.0 {
                let dist = (current_speed_kph as f64 / 3.6) * dt;
                session_distance_m += dist;
                lap_distance_m += dist;
            }
            last_time = row.time;

            let lap_time_ms = ((row.time - lap_start_time) * 1000.0) as i64;

            samples.push(TelemetrySample {
                time_ms: (row.time * 1000.0) as i64,
                speed_kph: current_speed_kph,
                lat: row.latitude,
                lon: row.longitude,
                accel_lat_g: row.accel_x, // Mapping x to lat, configurable later
                accel_lon_g: row.accel_y, // Mapping y to lon
                lap_number: Some(row.lap),
                lap_time_ms: Some(lap_time_ms),
                throttle_pct: row.throttle_position,
                brake: row.brake,
                engine_speed_rpm: row.engine_speed_rpm,
                session_distance_m,
                lap_distance_m,
            });
        }

        let mut log = Self {
            raw_samples: samples.clone(),
            samples,
            start_time_utc,
            parsed_speed_source: actual_speed_source,
        };

        log.apply_interpolation(&crate::project::InterpolationMode::Linear, 0);

        Ok(log)
    }

    pub fn apply_interpolation(&mut self, mode: &crate::project::InterpolationMode, points: u8) {
        if points == 0 || mode == &crate::project::InterpolationMode::None {
            self.samples = self.raw_samples.clone();
            return;
        }

        let num_intervals = (points + 1) as f32;
        let mut new_samples = Vec::new();

        if self.raw_samples.is_empty() {
            self.samples = new_samples;
            return;
        }

        let n = self.raw_samples.len();

        for i in 0..n - 1 {
            let p1 = &self.raw_samples[i];
            let p2 = &self.raw_samples[i + 1];

            new_samples.push(p1.clone());

            let dt = (p2.time_ms - p1.time_ms) as f32;
            if dt <= 0.0 {
                continue; // Prevent issues with identical timestamps
            }

            // Get points for Cubic (Catmull-Rom) interpolation
            let p0 = if i > 0 { &self.raw_samples[i - 1] } else { p1 };
            let p3 = if i + 2 < n {
                &self.raw_samples[i + 2]
            } else {
                p2
            };

            for step in 1..=points {
                let t = step as f32 / num_intervals;

                let interpolated = match mode {
                    crate::project::InterpolationMode::Linear => Self::lerp_sample(p1, p2, t),
                    crate::project::InterpolationMode::Cubic => {
                        Self::cubic_sample(p0, p1, p2, p3, t)
                    }
                    crate::project::InterpolationMode::None => p1.clone(),
                };
                new_samples.push(interpolated);
            }
        }

        // Add the very last point
        if let Some(last) = self.raw_samples.last() {
            new_samples.push(last.clone());
        }

        self.samples = new_samples;
    }

    fn lerp(v1: f32, v2: f32, t: f32) -> f32 {
        v1 + (v2 - v1) * t
    }

    fn lerp_f64(v1: f64, v2: f64, t: f32) -> f64 {
        v1 + (v2 - v1) * t as f64
    }

    fn cubic(v0: f32, v1: f32, v2: f32, v3: f32, t: f32) -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * ((2.0 * v1)
            + (-v0 + v2) * t
            + (2.0 * v0 - 5.0 * v1 + 4.0 * v2 - v3) * t2
            + (-v0 + 3.0 * v1 - 3.0 * v2 + v3) * t3)
    }

    fn cubic_f64(v0: f64, v1: f64, v2: f64, v3: f64, t: f32) -> f64 {
        let t = t as f64;
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * ((2.0 * v1)
            + (-v0 + v2) * t
            + (2.0 * v0 - 5.0 * v1 + 4.0 * v2 - v3) * t2
            + (-v0 + 3.0 * v1 - 3.0 * v2 + v3) * t3)
    }

    fn lerp_sample(s1: &TelemetrySample, s2: &TelemetrySample, t: f32) -> TelemetrySample {
        TelemetrySample {
            time_ms: s1.time_ms + ((s2.time_ms - s1.time_ms) as f32 * t) as i64,
            speed_kph: Self::lerp(s1.speed_kph, s2.speed_kph, t),
            lat: Self::lerp_f64(s1.lat, s2.lat, t),
            lon: Self::lerp_f64(s1.lon, s2.lon, t),
            accel_lat_g: Self::lerp(s1.accel_lat_g, s2.accel_lat_g, t),
            accel_lon_g: Self::lerp(s1.accel_lon_g, s2.accel_lon_g, t),
            lap_number: s1.lap_number,
            lap_time_ms: s1.lap_time_ms.map(|l1| {
                let l2 = s2.lap_time_ms.unwrap_or(l1);
                l1 + ((l2 - l1) as f32 * t) as i64
            }),
            throttle_pct: Self::lerp(s1.throttle_pct, s2.throttle_pct, t),
            brake: Self::lerp(s1.brake, s2.brake, t),
            engine_speed_rpm: Self::lerp(s1.engine_speed_rpm, s2.engine_speed_rpm, t),
            session_distance_m: Self::lerp_f64(s1.session_distance_m, s2.session_distance_m, t),
            lap_distance_m: Self::lerp_f64(s1.lap_distance_m, s2.lap_distance_m, t),
        }
    }

    fn cubic_sample(
        s0: &TelemetrySample,
        s1: &TelemetrySample,
        s2: &TelemetrySample,
        s3: &TelemetrySample,
        t: f32,
    ) -> TelemetrySample {
        TelemetrySample {
            time_ms: s1.time_ms + ((s2.time_ms - s1.time_ms) as f32 * t) as i64,
            speed_kph: Self::cubic(s0.speed_kph, s1.speed_kph, s2.speed_kph, s3.speed_kph, t),
            lat: Self::cubic_f64(s0.lat, s1.lat, s2.lat, s3.lat, t),
            lon: Self::cubic_f64(s0.lon, s1.lon, s2.lon, s3.lon, t),
            accel_lat_g: Self::cubic(
                s0.accel_lat_g,
                s1.accel_lat_g,
                s2.accel_lat_g,
                s3.accel_lat_g,
                t,
            ),
            accel_lon_g: Self::cubic(
                s0.accel_lon_g,
                s1.accel_lon_g,
                s2.accel_lon_g,
                s3.accel_lon_g,
                t,
            ),
            lap_number: s1.lap_number,
            lap_time_ms: s1.lap_time_ms.map(|l1| {
                let l2 = s2.lap_time_ms.unwrap_or(l1);
                l1 + ((l2 - l1) as f32 * t) as i64
            }),
            throttle_pct: Self::cubic(
                s0.throttle_pct,
                s1.throttle_pct,
                s2.throttle_pct,
                s3.throttle_pct,
                t,
            )
            .clamp(0.0, 100.0),
            brake: Self::cubic(s0.brake, s1.brake, s2.brake, s3.brake, t).clamp(0.0, 100.0),
            engine_speed_rpm: Self::cubic(
                s0.engine_speed_rpm,
                s1.engine_speed_rpm,
                s2.engine_speed_rpm,
                s3.engine_speed_rpm,
                t,
            )
            .max(0.0),
            session_distance_m: Self::cubic_f64(
                s0.session_distance_m,
                s1.session_distance_m,
                s2.session_distance_m,
                s3.session_distance_m,
                t,
            ),
            lap_distance_m: Self::cubic_f64(
                s0.lap_distance_m,
                s1.lap_distance_m,
                s2.lap_distance_m,
                s3.lap_distance_m,
                t,
            ),
        }
    }

    /// Returns a list of (lap_number, start_time_ms) by scanning the samples
    pub fn extract_laps(&self) -> Vec<(u32, i64)> {
        let mut laps = Vec::new();
        let mut current_lap = None;
        for s in &self.samples {
            if let Some(lap) = s.lap_number
                && Some(lap) != current_lap
            {
                current_lap = Some(lap);
                laps.push((lap, s.time_ms));
            }
        }
        laps
    }

    pub fn sample_at(&self, t_ms: i64) -> Option<TelemetrySample> {
        if self.samples.is_empty() {
            return None;
        }

        match self.samples.binary_search_by_key(&t_ms, |s| s.time_ms) {
            Ok(idx) => Some(self.samples[idx].clone()),
            Err(idx) => {
                if idx == 0 {
                    self.samples.first().cloned()
                } else if idx >= self.samples.len() {
                    self.samples.last().cloned()
                } else {
                    let s1 = &self.samples[idx - 1];
                    let s2 = &self.samples[idx];

                    let dt = (s2.time_ms - s1.time_ms) as f32;
                    let t = if dt > 0.0 {
                        (t_ms - s1.time_ms) as f32 / dt
                    } else {
                        0.0
                    };

                    Some(TelemetrySample {
                        time_ms: t_ms,
                        speed_kph: s1.speed_kph + (s2.speed_kph - s1.speed_kph) * t,
                        lat: s1.lat + (s2.lat - s1.lat) * t as f64,
                        lon: s1.lon + (s2.lon - s1.lon) * t as f64,
                        accel_lat_g: s1.accel_lat_g + (s2.accel_lat_g - s1.accel_lat_g) * t,
                        accel_lon_g: s1.accel_lon_g + (s2.accel_lon_g - s1.accel_lon_g) * t,
                        lap_number: s1.lap_number,
                        lap_time_ms: s1.lap_time_ms.map(|l1| {
                            let l2 = s2.lap_time_ms.unwrap_or(l1);
                            l1 + ((l2 - l1) as f32 * t) as i64
                        }),
                        throttle_pct: s1.throttle_pct + (s2.throttle_pct - s1.throttle_pct) * t,
                        brake: s1.brake + (s2.brake - s1.brake) * t,
                        engine_speed_rpm: s1.engine_speed_rpm
                            + (s2.engine_speed_rpm - s1.engine_speed_rpm) * t,
                        session_distance_m: s1.session_distance_m
                            + (s2.session_distance_m - s1.session_distance_m) * t as f64,
                        lap_distance_m: s1.lap_distance_m
                            + (s2.lap_distance_m - s1.lap_distance_m) * t as f64,
                    })
                }
            }
        }
    }
}

pub struct TelemetryView<'a> {
    pub samples: &'a [TelemetrySample],
    pub start_time_utc: Option<DateTime<Utc>>,
    pub lap_offset: u32,
    pub sync_offset_ms: i64,
}

impl<'a> TelemetryView<'a> {
    pub fn new(
        log: &'a TelemetryLog,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        sync_offset_ms: i64,
    ) -> Self {
        if log.samples.is_empty() {
            return Self {
                samples: &[],
                start_time_utc: log.start_time_utc,
                lap_offset: 0,
                sync_offset_ms,
            };
        }

        let start = start_ms.unwrap_or(0);
        let end = end_ms.unwrap_or(i64::MAX);
        let end = if end < 0 { i64::MAX } else { end };

        // Find the indices that fall within the specified export range.
        // Convert video timestamps back to telemetry time domain for the search.
        let start_telem_time = start + sync_offset_ms;
        // Don't shift i64::MAX to prevent overflow logic breaking
        let end_telem_time = if end == i64::MAX {
            i64::MAX
        } else {
            end + sync_offset_ms
        };

        let start_idx = log
            .samples
            .binary_search_by_key(&start_telem_time, |s| s.time_ms)
            .unwrap_or_else(|idx| idx);

        let end_idx = log
            .samples
            .binary_search_by_key(&end_telem_time, |s| s.time_ms)
            .unwrap_or_else(|idx| idx);

        let start_idx = start_idx.min(log.samples.len());
        let end_idx = end_idx.min(log.samples.len());

        let slice = if start_idx < end_idx {
            &log.samples[start_idx..end_idx]
        } else {
            &[]
        };

        let lap_offset = if let Some(first_sample) = slice.first() {
            first_sample.lap_number.unwrap_or(0)
        } else {
            0
        };
        let lap_offset = lap_offset.saturating_sub(1);

        Self {
            samples: slice,
            start_time_utc: log.start_time_utc,
            lap_offset,
            sync_offset_ms,
        }
    }

    pub fn extract_laps(&self) -> Vec<(u32, i64)> {
        let mut laps = Vec::new();
        let mut current_lap = None;
        for s in self.samples {
            if let Some(lap) = s.lap_number {
                let adjusted_lap = lap.saturating_sub(self.lap_offset);
                if Some(adjusted_lap) != current_lap {
                    current_lap = Some(adjusted_lap);
                    laps.push((adjusted_lap, s.time_ms));
                }
            }
        }
        laps
    }

    pub fn sample_at(&self, t_ms: i64) -> Option<TelemetrySample> {
        if self.samples.is_empty() {
            return None;
        }

        match self.samples.binary_search_by_key(&t_ms, |s| s.time_ms) {
            Ok(idx) => Some(self.adjust_sample(&self.samples[idx])),
            Err(idx) => {
                if idx == 0 {
                    self.samples.first().map(|s| self.adjust_sample(s))
                } else if idx >= self.samples.len() {
                    self.samples.last().map(|s| self.adjust_sample(s))
                } else {
                    let s1 = &self.samples[idx - 1];
                    let s2 = &self.samples[idx];

                    let dt = (s2.time_ms - s1.time_ms) as f32;
                    let t = if dt > 0.0 {
                        (t_ms - s1.time_ms) as f32 / dt
                    } else {
                        0.0
                    };

                    let interpolated = TelemetrySample {
                        time_ms: t_ms,
                        speed_kph: s1.speed_kph + (s2.speed_kph - s1.speed_kph) * t,
                        lat: s1.lat + (s2.lat - s1.lat) * t as f64,
                        lon: s1.lon + (s2.lon - s1.lon) * t as f64,
                        accel_lat_g: s1.accel_lat_g + (s2.accel_lat_g - s1.accel_lat_g) * t,
                        accel_lon_g: s1.accel_lon_g + (s2.accel_lon_g - s1.accel_lon_g) * t,
                        lap_number: s1.lap_number,
                        lap_time_ms: s1.lap_time_ms.map(|l1| {
                            let l2 = s2.lap_time_ms.unwrap_or(l1);
                            l1 + ((l2 - l1) as f32 * t) as i64
                        }),
                        throttle_pct: s1.throttle_pct + (s2.throttle_pct - s1.throttle_pct) * t,
                        brake: s1.brake + (s2.brake - s1.brake) * t,
                        engine_speed_rpm: s1.engine_speed_rpm
                            + (s2.engine_speed_rpm - s1.engine_speed_rpm) * t,
                        session_distance_m: s1.session_distance_m
                            + (s2.session_distance_m - s1.session_distance_m) * t as f64,
                        lap_distance_m: s1.lap_distance_m
                            + (s2.lap_distance_m - s1.lap_distance_m) * t as f64,
                    };
                    Some(self.adjust_sample(&interpolated))
                }
            }
        }
    }

    fn adjust_sample(&self, s: &TelemetrySample) -> TelemetrySample {
        let mut sample = s.clone();
        if let Some(lap) = sample.lap_number {
            sample.lap_number = Some(lap.saturating_sub(self.lap_offset));
        }
        sample
    }

    pub fn get_state(&self, t_ms: i64) -> TelemetryState {
        let current_sample = self.sample_at(t_ms);
        compute_telemetry_state(self.samples, t_ms, current_sample, self.lap_offset)
    }
}

impl TelemetryLog {
    pub fn get_state(&self, t_ms: i64) -> TelemetryState {
        let current_sample = self.sample_at(t_ms);
        compute_telemetry_state(&self.samples, t_ms, current_sample, 0)
    }
}

fn compute_telemetry_state(
    samples: &[TelemetrySample],
    t_ms: i64,
    current_sample: Option<TelemetrySample>,
    lap_offset: u32,
) -> TelemetryState {
    let mut laps = Vec::new();
    let mut current_lap_start_idx = 0;
    let mut current_lap = samples.first().and_then(|s| s.lap_number).unwrap_or(0);

    for (i, s) in samples.iter().enumerate() {
        if let Some(lap) = s.lap_number
            && lap != current_lap
        {
            let end_idx = i - 1;
            if end_idx >= current_lap_start_idx {
                let start_s = &samples[current_lap_start_idx];
                let end_s = &samples[end_idx];

                // Note: Use lap_time_ms from the end_s to get the true lap duration
                // from the original telemetry, avoiding shortened partial laps.
                let duration_ms = end_s.lap_time_ms.unwrap_or(end_s.time_ms - start_s.time_ms);

                laps.push(LapStat {
                    lap_number: current_lap.saturating_sub(lap_offset),
                    start_time_ms: start_s.time_ms,
                    end_time_ms: end_s.time_ms,
                    duration_ms,
                    total_distance_m: end_s.lap_distance_m,
                });
            }
            current_lap = lap;
            current_lap_start_idx = i;
        }
    }

    let mut completed_laps = Vec::new();
    for lap in laps {
        if lap.end_time_ms <= t_ms {
            completed_laps.push(lap);
        }
    }

    let best_lap = completed_laps.iter().min_by_key(|l| l.duration_ms).cloned();

    let mut previous_laps = completed_laps.clone();
    previous_laps.sort_by_key(|l| std::cmp::Reverse(l.end_time_ms)); // most recent first
    previous_laps.truncate(3);

    let mut projection_ms = None;
    if let (Some(sample), Some(best)) = (&current_sample, &best_lap)
        && sample.lap_time_ms.unwrap_or(0) > 0
    {
        // Only project if we are actually in a lap
        let start_time = best.start_time_ms;
        let end_time = best.end_time_ms;

        let best_lap_samples = samples
            .iter()
            .filter(|s| s.time_ms >= start_time && s.time_ms <= end_time)
            .collect::<Vec<_>>();

        if !best_lap_samples.is_empty() {
            let target_dist = sample.lap_distance_m;

            let best_lap_elapsed = match best_lap_samples.binary_search_by(|s| {
                s.lap_distance_m
                    .partial_cmp(&target_dist)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                Ok(idx) => best_lap_samples[idx].time_ms - start_time,
                Err(idx) => {
                    if idx == 0 {
                        best_lap_samples
                            .first()
                            .map(|s| s.time_ms)
                            .unwrap_or(start_time)
                            - start_time
                    } else if idx >= best_lap_samples.len() {
                        best_lap_samples
                            .last()
                            .map(|s| s.time_ms)
                            .unwrap_or(start_time)
                            - start_time
                    } else {
                        let s1 = best_lap_samples[idx - 1];
                        let s2 = best_lap_samples[idx];

                        let dd = s2.lap_distance_m - s1.lap_distance_m;
                        let t = if dd > 0.0 {
                            (target_dist - s1.lap_distance_m) / dd
                        } else {
                            0.0
                        };

                        let time_at_dist =
                            s1.time_ms + ((s2.time_ms - s1.time_ms) as f64 * t) as i64;
                        time_at_dist - start_time
                    }
                }
            };

            let current_elapsed = sample.lap_time_ms.unwrap_or(0);
            projection_ms = Some(current_elapsed - best_lap_elapsed);
        }
    }

    TelemetryState {
        current_sample,
        previous_laps,
        best_lap,
        projection_ms,
    }
}
