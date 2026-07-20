use crate::telemetry::TelemetryLog;

/// A track outline projected to local flat coordinates, computed once per session.
#[derive(Debug, Clone)]
pub struct TrackMap {
    /// Track outline points, already normalized to a 0.0..=1.0 square
    pub outline: Vec<(f32, f32)>,

    /// Start/finish line as a short segment in the same normalized coordinate space.
    pub start_finish: ((f32, f32), (f32, f32)),

    // Internal projection state
    lat_ref: f64,
    lon_ref: f64,
    scale_x: f32,
    scale_y: f32,
    offset_x: f32,
    offset_y: f32,
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
        let scale = if width > height { 1.0 / width } else { 1.0 / height };

        let offset_x = -min_x;
        let offset_y = -min_y;

        // Apply normalization
        let mut outline = Vec::with_capacity(projected.len());
        for (x, y) in projected {
            // center the shorter axis
            let nx = (x + offset_x) * scale + if height > width { (1.0 - width * scale) / 2.0 } else { 0.0 };
            let ny = (y + offset_y) * scale + if width > height { (1.0 - height * scale) / 2.0 } else { 0.0 };

            outline.push((nx, 1.0 - ny)); // Invert Y so North is Up on screen
        }

        // Determine Start/Finish Line using normalized coords
        let sf_line = if let Some(&(_lap_num, start_time)) = lap_boundaries_ms.first() {
            if let Some(idx) = log.samples.iter().position(|s| s.time_ms >= start_time) {
                if idx > 0 && idx + 1 < outline.len() {
                    let p1 = outline[idx - 1];
                    let p2 = outline[idx + 1];

                    let dx = p2.0 - p1.0;
                    let dy = p2.1 - p1.1;
                    let len = (dx * dx + dy * dy).sqrt();

                    if len > 0.0 {
                        let px = -dy / len;
                        let py = dx / len;

                        let width_fraction = 0.05; // 5% of bounding box
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
            }
        } else {
            ((0.0, 0.0), (0.0, 0.0))
        };

        Some(Self {
            outline,
            start_finish: sf_line,
            lat_ref,
            lon_ref,
            scale_x: scale,
            scale_y: scale,
            offset_x: offset_x + if height > width { (1.0 / scale - width) / 2.0 } else { 0.0 },
            offset_y: offset_y + if width > height { (1.0 / scale - height) / 2.0 } else { 0.0 },
        })
    }

    pub fn project(&self, lat: f64, lon: f64) -> (f32, f32) {
        let lat_ref_rad = self.lat_ref.to_radians();
        let x = ((lon - self.lon_ref).to_radians() * lat_ref_rad.cos() * EARTH_RADIUS_M) as f32;
        let y = ((lat - self.lat_ref).to_radians() * EARTH_RADIUS_M) as f32;

        let nx = (x + self.offset_x) * self.scale_x;
        let ny = (y + self.offset_y) * self.scale_y;

        (nx, 1.0 - ny)
    }
}
