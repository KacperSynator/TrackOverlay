use anyhow::Result;
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
}

pub struct TelemetryLog {
    pub samples: Vec<TelemetrySample>,
}

impl TelemetryLog {
    pub fn load_csv<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut rdr = csv::ReaderBuilder::new()
            .comment(Some(b'#'))
            .from_path(path)?;

        let mut samples = Vec::new();
        let mut lap_start_time = 0.0;
        let mut current_lap = 0;

        for result in rdr.deserialize() {
            let row: RawTelemetryRow = match result {
                Ok(r) => r,
                Err(_) => continue, // Skip malformed rows
            };

            if row.lap != current_lap {
                current_lap = row.lap;
                lap_start_time = row.time;
            }

            let lap_time_ms = ((row.time - lap_start_time) * 1000.0) as i64;

            samples.push(TelemetrySample {
                time_ms: (row.time * 1000.0) as i64,
                speed_kph: row.speed_kph,
                lat: row.latitude,
                lon: row.longitude,
                accel_lat_g: row.accel_x, // Mapping x to lat, configurable later
                accel_lon_g: row.accel_y, // Mapping y to lon
                lap_number: Some(row.lap),
                lap_time_ms: Some(lap_time_ms),
            });
        }

        Ok(Self { samples })
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
                    })
                }
            }
        }
    }
}
