fn parse_time(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let m: f64 = parts[0].parse().ok()?;
        let s: f64 = parts[1].parse().ok()?;
        Some(m * 60.0 + s)
    } else {
        s.parse().ok()
    }
}

fn format_time(seconds: f64) -> String {
    if seconds < 0.0 {
        return "-1".to_string();
    }
    let m = (seconds / 60.0).floor() as u32;
    let s = seconds % 60.0;
    if m > 0 {
        format!("{m}:{s:05.2}")
    } else {
        format!("{s:.2}")
    }
}

fn main() {
    println!("{:?}", parse_time("2:34"));
    println!("{:?}", parse_time("154"));
    println!("{:?}", format_time(154.0));
    println!("{:?}", format_time(2.5));
}
