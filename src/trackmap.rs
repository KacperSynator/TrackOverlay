use crate::telemetry::TelemetryLog;

/// A track outline projected to local flat coordinates, computed once per session.
#[derive(Debug, Clone)]
pub struct TrackMap {
    /// Track outline points, already normalized to a 0.0..=1.0 square
    pub outline: Vec<(f32, f32)>,

    /// The timestamps corresponding to each point in the outline
    pub times_ms: Vec<i64>,

    /// Start/finish line as a short segment in the same normalized coordinate space.
    pub start_finish: ((f32, f32), (f32, f32)),
}

const EARTH_RADIUS_M: f64 = 6371000.0;

use crate::telemetry::TelemetrySample;

impl TrackMap {
    pub fn from_telemetry(log: &TelemetryLog, lap_boundaries_ms: &[(u32, i64)]) -> Option<Self> {
        if log.samples.len() < 10 {
            return None;
        }

        let (lat_ref, lon_ref) = Self::calculate_reference_point(&log.samples);
        let (projected, min_x, max_x, min_y, max_y) =
            Self::project_coords(&log.samples, lat_ref, lon_ref);
        let outline = Self::normalize_coords(&projected, min_x, max_x, min_y, max_y);

        let times_ms: Vec<i64> = log.samples.iter().map(|s| s.time_ms).collect();
        let start_finish = Self::calculate_start_finish(&outline, &times_ms, lap_boundaries_ms);

        Some(Self {
            outline,
            times_ms,
            start_finish,
        })
    }

    fn calculate_reference_point(samples: &[TelemetrySample]) -> (f64, f64) {
        let mut sum_lat = 0.0;
        let mut sum_lon = 0.0;
        for s in samples {
            sum_lat += s.lat;
            sum_lon += s.lon;
        }
        let count = samples.len() as f64;
        (sum_lat / count, sum_lon / count)
    }

    fn project_coords(
        samples: &[TelemetrySample],
        lat_ref: f64,
        lon_ref: f64,
    ) -> (Vec<(f32, f32)>, f32, f32, f32, f32) {
        let lat_ref_rad = lat_ref.to_radians();
        let mut projected = Vec::with_capacity(samples.len());
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for s in samples {
            let x = ((s.lon - lon_ref).to_radians() * lat_ref_rad.cos() * EARTH_RADIUS_M) as f32;
            let y = ((s.lat - lat_ref).to_radians() * EARTH_RADIUS_M) as f32;

            if x < min_x {
                min_x = x;
            }
            if x > max_x {
                max_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if y > max_y {
                max_y = y;
            }

            projected.push((x, y));
        }

        (projected, min_x, max_x, min_y, max_y)
    }

    fn normalize_coords(
        projected: &[(f32, f32)],
        min_x: f32,
        max_x: f32,
        min_y: f32,
        max_y: f32,
    ) -> Vec<(f32, f32)> {
        let width = max_x - min_x;
        let height = max_y - min_y;
        let scale = if width > height {
            1.0 / width
        } else {
            1.0 / height
        };

        let offset_x = -min_x;
        let offset_y = -min_y;

        let mut outline = Vec::with_capacity(projected.len());

        for &(x, y) in projected {
            let nx = (x + offset_x) * scale
                + if height > width {
                    (1.0 - width * scale) / 2.0
                } else {
                    0.0
                };
            let ny = (y + offset_y) * scale
                + if width > height {
                    (1.0 - height * scale) / 2.0
                } else {
                    0.0
                };

            outline.push((nx, 1.0 - ny)); // Invert Y so North is Up on screen
        }

        outline
    }

    fn calculate_start_finish(
        outline: &[(f32, f32)],
        times_ms: &[i64],
        lap_boundaries_ms: &[(u32, i64)],
    ) -> ((f32, f32), (f32, f32)) {
        let mut sf_line = ((0.0, 0.0), (0.0, 0.0));

        for &(_lap_num, start_time) in lap_boundaries_ms {
            if let Some(idx) = times_ms.iter().position(|&t| t >= start_time) {
                let (i1, i2) = if idx > 0 && idx + 1 < outline.len() {
                    (idx - 1, idx + 1)
                } else if idx == 0 && outline.len() >= 2 {
                    (0, 1)
                } else {
                    continue;
                };

                let p1 = outline[i1];
                let p2 = outline[i2];

                let dx = p2.0 - p1.0;
                let dy = p2.1 - p1.1;
                let len = (dx * dx + dy * dy).sqrt();

                if len > 0.0 {
                    let px = -dy / len;
                    let py = dx / len;

                    let width_fraction = 0.05; // 5% of bounding box
                    let p = outline[idx];

                    sf_line = (
                        (p.0 - px * width_fraction, p.1 - py * width_fraction),
                        (p.0 + px * width_fraction, p.1 + py * width_fraction),
                    );
                    break;
                }
            }
        }

        sf_line
    }

    /// Calculates exactly where on the polyline this specific time falls.
    /// This guarantees perfectly smooth dots that don't jitter off the line.
    pub fn point_at_time(&self, time_ms: i64) -> Option<(f32, f32)> {
        if self.times_ms.is_empty() || self.outline.is_empty() {
            return None;
        }

        match self.times_ms.binary_search(&time_ms) {
            Ok(idx) => Some(self.outline[idx]),
            Err(idx) => {
                if idx == 0 {
                    Some(self.outline[0])
                } else if idx >= self.times_ms.len() {
                    self.outline.last().copied()
                } else {
                    let t1 = self.times_ms[idx - 1];
                    let t2 = self.times_ms[idx];
                    let p1 = self.outline[idx - 1];
                    let p2 = self.outline[idx];

                    let dt = (t2 - t1) as f64;
                    let t = if dt > 0.0 {
                        ((time_ms - t1) as f64 / dt) as f32
                    } else {
                        0.0
                    };

                    Some((p1.0 + (p2.0 - p1.0) * t, p1.1 + (p2.1 - p1.1) * t))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_reference_point() {
        let samples = vec![
            TelemetrySample {
                time_ms: 0,
                speed_kph: 0.0,
                lat: 10.0,
                lon: 20.0,
                accel_lat_g: 0.0,
                accel_lon_g: 0.0,
                lap_number: None,
                lap_time_ms: None,
                throttle_pct: 0.0,
                brake: 0.0,
                engine_speed_rpm: 0.0,
                session_distance_m: 0.0,
                lap_distance_m: 0.0,
            },
            TelemetrySample {
                time_ms: 1000,
                speed_kph: 0.0,
                lat: -10.0,
                lon: -20.0,
                accel_lat_g: 0.0,
                accel_lon_g: 0.0,
                lap_number: None,
                lap_time_ms: None,
                throttle_pct: 0.0,
                brake: 0.0,
                engine_speed_rpm: 0.0,
                session_distance_m: 0.0,
                lap_distance_m: 0.0,
            },
        ];

        let (lat_ref, lon_ref) = TrackMap::calculate_reference_point(&samples);
        assert_eq!(lat_ref, 0.0);
        assert_eq!(lon_ref, 0.0);
    }

    #[test]
    fn test_normalize_coords() {
        let projected = vec![(0.0, 0.0), (10.0, 5.0)];
        let min_x = 0.0;
        let max_x = 10.0;
        let min_y = 0.0;
        let max_y = 5.0;

        let outline = TrackMap::normalize_coords(&projected, min_x, max_x, min_y, max_y);

        // width (10) > height (5). Scale = 1/10 = 0.1
        // offset_x = 0, offset_y = 0
        // nx = (x + 0) * 0.1 + 0 = x * 0.1
        // ny = (y + 0) * 0.1 + (1 - 5 * 0.1) / 2 = y * 0.1 + 0.25

        // Point 1: (0, 0) => nx: 0.0, ny: 0.25. Outline (nx, 1 - ny) = (0.0, 0.75)
        assert!((outline[0].0 - 0.0).abs() < f32::EPSILON);
        assert!((outline[0].1 - 0.75).abs() < f32::EPSILON);

        // Point 2: (10, 5) => nx: 1.0, ny: 0.75. Outline (nx, 1 - ny) = (1.0, 0.25)
        assert!((outline[1].0 - 1.0).abs() < f32::EPSILON);
        assert!((outline[1].1 - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_calculate_start_finish() {
        let outline = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let times_ms = vec![0, 1000, 2000, 3000];
        let lap_boundaries_ms = vec![(1, 1000)];

        let ((x1, y1), (x2, y2)) =
            TrackMap::calculate_start_finish(&outline, &times_ms, &lap_boundaries_ms);

        // idx = 1 (time: 1000). Outline point: (1.0, 0.0).
        // i1 = 0 (0.0, 0.0), i2 = 2 (1.0, 1.0)
        // dx = 1.0, dy = 1.0, len = sqrt(2)
        // px = -1.0 / sqrt(2), py = 1.0 / sqrt(2)
        // width = 0.05

        let dx = 1.0_f32;
        let dy = 1.0_f32;
        let len = (dx * dx + dy * dy).sqrt();
        let px = -dy / len;
        let py = dx / len;
        let width = 0.05;

        assert!((x1 - (1.0 - px * width)).abs() < 1e-5);
        assert!((y1 - (0.0 - py * width)).abs() < 1e-5);
        assert!((x2 - (1.0 + px * width)).abs() < 1e-5);
        assert!((y2 - (0.0 + py * width)).abs() < 1e-5);
    }
}
