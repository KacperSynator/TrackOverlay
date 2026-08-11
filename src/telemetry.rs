use anyhow::Result;
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
    pub samples: Vec<TelemetrySample>,
    pub start_time_utc: Option<DateTime<Utc>>,
}

impl TelemetryLog {
    pub fn load_csv<P: AsRef<Path>>(
        path: P,
        speed_source: crate::project::SpeedSource,
    ) -> Result<Self> {
        let mut rdr = csv::ReaderBuilder::new()
            .comment(Some(b'#'))
            .from_path(path.as_ref())?;

        let actual_speed_source = if speed_source == crate::project::SpeedSource::Auto {
            let mut gps_count = 0;
            let mut obd_count = 0;
            for result in rdr.deserialize::<RawTelemetryRow>() {
                if let Ok(row) = result {
                    if row.gps_update == 1 {
                        gps_count += 1;
                    }
                    if row.obd_update == 1 {
                        obd_count += 1;
                    }
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
                engine_speed_rpm: row.engine_speed_rpm,
                session_distance_m,
                lap_distance_m,
            });
        }

        Ok(Self {
            samples,
            start_time_utc,
        })
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
                    Some(self.samples[0].clone())
                } else if idx >= self.samples.len() {
                    Some(self.samples.last().unwrap().clone())
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
                    Some(self.adjust_sample(&self.samples[0]))
                } else if idx >= self.samples.len() {
                    Some(self.adjust_sample(self.samples.last().unwrap()))
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

        let mut laps = Vec::new();
        let mut current_lap_start_idx = 0;
        let mut current_lap = self.samples.first().and_then(|s| s.lap_number).unwrap_or(0);

        for (i, s) in self.samples.iter().enumerate() {
            if let Some(lap) = s.lap_number
                && lap != current_lap
            {
                let end_idx = i - 1;
                if end_idx >= current_lap_start_idx {
                    let start_s = &self.samples[current_lap_start_idx];
                    let end_s = &self.samples[end_idx];

                    // Note: Use lap_time_ms from the end_s to get the true lap duration
                    // from the original telemetry, avoiding shortened partial laps.
                    let duration_ms = end_s.lap_time_ms.unwrap_or(end_s.time_ms - start_s.time_ms);

                    laps.push(LapStat {
                        lap_number: current_lap.saturating_sub(self.lap_offset),
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

            let best_lap_samples = self
                .samples
                .iter()
                .filter(|s| s.time_ms >= start_time && s.time_ms <= end_time)
                .collect::<Vec<_>>();

            if !best_lap_samples.is_empty() {
                let target_dist = sample.lap_distance_m;

                let mut best_lap_elapsed = 0; // Fix unused_assignments check
                let _ = best_lap_elapsed;
                match best_lap_samples.binary_search_by(|s| {
                    s.lap_distance_m
                        .partial_cmp(&target_dist)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    Ok(idx) => {
                        best_lap_elapsed = best_lap_samples[idx].time_ms - start_time;
                    }
                    Err(idx) => {
                        if idx == 0 {
                            best_lap_elapsed = best_lap_samples[0].time_ms - start_time;
                        } else if idx >= best_lap_samples.len() {
                            best_lap_elapsed =
                                best_lap_samples.last().unwrap().time_ms - start_time;
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
                            best_lap_elapsed = time_at_dist - start_time;
                        }
                    }
                }

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
}

impl TelemetryLog {
    pub fn get_state(&self, t_ms: i64) -> TelemetryState {
        let current_sample = self.sample_at(t_ms);

        let mut laps = Vec::new();
        let mut current_lap_start_idx = 0;
        let mut current_lap = self.samples.first().and_then(|s| s.lap_number).unwrap_or(0);

        for (i, s) in self.samples.iter().enumerate() {
            if let Some(lap) = s.lap_number
                && lap != current_lap
            {
                let end_idx = i - 1;
                if end_idx >= current_lap_start_idx {
                    let start_s = &self.samples[current_lap_start_idx];
                    let end_s = &self.samples[end_idx];

                    let duration_ms = end_s.lap_time_ms.unwrap_or(end_s.time_ms - start_s.time_ms);

                    laps.push(LapStat {
                        lap_number: current_lap,
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

            let best_lap_samples = self
                .samples
                .iter()
                .filter(|s| s.time_ms >= start_time && s.time_ms <= end_time)
                .collect::<Vec<_>>();

            if !best_lap_samples.is_empty() {
                let target_dist = sample.lap_distance_m;

                let mut best_lap_elapsed = 0; // Fix unused_assignments check
                let _ = best_lap_elapsed;
                match best_lap_samples.binary_search_by(|s| {
                    s.lap_distance_m
                        .partial_cmp(&target_dist)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    Ok(idx) => {
                        best_lap_elapsed = best_lap_samples[idx].time_ms - start_time;
                    }
                    Err(idx) => {
                        if idx == 0 {
                            best_lap_elapsed = best_lap_samples[0].time_ms - start_time;
                        } else if idx >= best_lap_samples.len() {
                            best_lap_elapsed =
                                best_lap_samples.last().unwrap().time_ms - start_time;
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
                            best_lap_elapsed = time_at_dist - start_time;
                        }
                    }
                }

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
}
