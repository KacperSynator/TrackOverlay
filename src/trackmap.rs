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

impl TrackMap {
    pub fn from_telemetry(log: &TelemetryLog, lap_boundaries_ms: &[(u32, i64)]) -> Option<Self> {
        if log.samples.len() < 10 {
            return None;
        }

        // Determine reference point (mean lat/lon)
        let mut sum_lat = 0.0;
        let mut sum_lon = 0.0;
        for s in &log.samples {
            sum_lat += s.lat;
            sum_lon += s.lon;
        }
        let count = log.samples.len() as f64;
        let lat_ref = sum_lat / count;
        let lon_ref = sum_lon / count;

        let lat_ref_rad = lat_ref.to_radians();

        // Equirectangular projection
        let mut projected = Vec::with_capacity(log.samples.len());
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for s in &log.samples {
            let x = ((s.lon - lon_ref).to_radians() * lat_ref_rad.cos() * EARTH_RADIUS_M) as f32;
            let y = ((s.lat - lat_ref).to_radians() * EARTH_RADIUS_M) as f32;

            if x < min_x { min_x = x; }
            if x > max_x { max_x = x; }
            if y < min_y { min_y = y; }
            if y > max_y { max_y = y; }

            projected.push((x, y));
        }

        // Normalization (maintain aspect ratio)
        let width = max_x - min_x;
        let height = max_y - min_y;
        // Avoid division by zero on degenerate tracks
        let scale = if width > 0.0 && height > 0.0 {
            if width > height { 1.0 / width } else { 1.0 / height }
        } else {
            1.0
        };

        let offset_x = -min_x;
        let offset_y = -min_y;

        // Apply normalization
        let mut outline = Vec::with_capacity(projected.len());
        let mut times_ms = Vec::with_capacity(projected.len());

        for (i, (x, y)) in projected.into_iter().enumerate() {
            // center the shorter axis
            let nx = (x + offset_x) * scale + if height > width { (1.0 - width * scale) / 2.0 } else { 0.0 };
            let ny = (y + offset_y) * scale + if width > height { (1.0 - height * scale) / 2.0 } else { 0.0 };

            outline.push((nx, 1.0 - ny)); // Invert Y so North is Up on screen
            times_ms.push(log.samples[i].time_ms);
        }

        // Determine Start/Finish Line using the fully NORMALIZED coords so it draws correctly
        let sf_line = if let Some(&(_lap_num, start_time)) = lap_boundaries_ms.first() {
            if let Some(idx) = times_ms.iter().position(|&t| t >= start_time) {
                // Find adjacent points to compute vector
                let p1 = if idx > 0 { outline[idx - 1] } else { outline[idx] };
                let p2 = if idx + 1 < outline.len() { outline[idx + 1] } else { outline[idx] };

                let dx = p2.0 - p1.0;
                let dy = p2.1 - p1.1;
                let len = (dx * dx + dy * dy).sqrt();

                if len > 0.0 {
                    // Perpendicular vector normalized
                    let px = -dy / len;
                    let py = dx / len;

                    // The line extends 5% of the bounding box width on either side of the track
                    let width_fraction = 0.05;
                    let p = outline[idx];

                    (
                        (p.0 - px * width_fraction, p.1 - py * width_fraction),
                        (p.0 + px * width_fraction, p.1 + py * width_fraction)
                    )
                } else {
                    ((0.0, 0.0), (0.0, 0.0))
                }
            } else {
                ((0.0, 0.0), (0.0, 0.0))
            }
        } else {
            ((0.0, 0.0), (0.0, 0.0))
        };

        Some(Self {
            outline,
            times_ms,
            start_finish: sf_line,
        })
    }

    /// Interpolates smoothly along the physical drawn lines to ensure the dot never jitters off-track.
    pub fn point_at_time(&self, time_ms: i64) -> Option<(f32, f32)> {
        if self.times_ms.is_empty() { return None; }

        match self.times_ms.binary_search(&time_ms) {
            Ok(idx) => Some(self.outline[idx]),
            Err(idx) => {
                if idx == 0 {
                    Some(self.outline[0])
                } else if idx >= self.times_ms.len() {
                    Some(*self.outline.last().unwrap())
                } else {
                    let t1 = self.times_ms[idx - 1];
                    let t2 = self.times_ms[idx];
                    let p1 = self.outline[idx - 1];
                    let p2 = self.outline[idx];

                    let dt = (t2 - t1) as f32;
                    let t = if dt > 0.0 { (time_ms - t1) as f32 / dt } else { 0.0 };

                    Some((
                        p1.0 + (p2.0 - p1.0) * t,
                        p1.1 + (p2.1 - p1.1) * t
                    ))
                }
            }
        }
    }
}
