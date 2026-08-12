pub fn parse_time_str(s: &str) -> Option<f64> {
    let s = s.trim();
    let is_negative = s.starts_with('-');
    let s_unsigned = if is_negative { &s[1..] } else { s };

    let parts: Vec<&str> = s_unsigned.split(':').collect();
    let val = if parts.len() == 2 {
        let m: f64 = parts[0].parse().ok()?;
        let sec: f64 = parts[1].parse().ok()?;
        // If there are negative values in parts, they shouldn't exist after removing the leading '-'
        // but if someone types "1:-30", `parts[1]` will have `-30` which parses as negative, we want to reject that.
        if m < 0.0 || sec < 0.0 {
            return None;
        }
        m * 60.0 + sec
    } else {
        let val: f64 = s_unsigned.parse().ok()?;
        if val < 0.0 {
            return None;
        }
        val
    };

    Some(if is_negative { -val } else { val })
}

pub fn format_time_str(seconds: f64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_str_minutes_and_seconds() {
        assert_eq!(parse_time_str("1:30"), Some(90.0));
        assert_eq!(parse_time_str("0:45"), Some(45.0));
        assert_eq!(parse_time_str("10:00"), Some(600.0));
    }

    #[test]
    fn test_parse_time_str_seconds_only() {
        assert_eq!(parse_time_str("90"), Some(90.0));
        assert_eq!(parse_time_str("45"), Some(45.0));
        assert_eq!(parse_time_str("0"), Some(0.0));
    }

    #[test]
    fn test_parse_time_str_fractions() {
        assert_eq!(parse_time_str("3.5"), Some(3.5));
        assert_eq!(parse_time_str("1:03.5"), Some(63.5));
        assert_eq!(parse_time_str("0:00.123"), Some(0.123));
    }

    #[test]
    fn test_parse_time_str_large_numbers() {
        assert_eq!(parse_time_str("600"), Some(600.0));
        assert_eq!(parse_time_str("600:00"), Some(36000.0));
    }

    #[test]
    fn test_parse_time_str_negative_times() {
        assert_eq!(parse_time_str("-1:30"), Some(-90.0));
        assert_eq!(parse_time_str("-0:30"), Some(-30.0));
        assert_eq!(parse_time_str("-90"), Some(-90.0));
        assert_eq!(parse_time_str(" -1:30 "), Some(-90.0));
    }

    #[test]
    fn test_parse_time_str_invalid() {
        assert_eq!(parse_time_str("-1:-30"), None);
        assert_eq!(parse_time_str("1:-30"), None);
        assert_eq!(parse_time_str("1:-30:00"), None);
        assert_eq!(parse_time_str("abc"), None);
        assert_eq!(parse_time_str("1:2:3"), None);
        assert_eq!(parse_time_str("1:abc"), None);
        assert_eq!(parse_time_str(""), None);
        assert_eq!(parse_time_str(":"), None);
    }

    #[test]
    fn test_format_time_str_under_minute() {
        assert_eq!(format_time_str(0.0), "0.00");
        assert_eq!(format_time_str(5.5), "5.50");
        assert_eq!(format_time_str(45.123), "45.12");
    }

    #[test]
    fn test_format_time_str_over_minute() {
        assert_eq!(format_time_str(60.0), "1:00.00");
        assert_eq!(format_time_str(65.123), "1:05.12");
        assert_eq!(format_time_str(125.999), "2:06.00");
        assert_eq!(format_time_str(600.0), "10:00.00");
    }

    #[test]
    fn test_format_time_str_negative() {
        assert_eq!(format_time_str(-1.0), "-1");
        assert_eq!(format_time_str(-0.5), "-1");
    }

    #[test]
    fn test_format_time_str_floating_limits() {
        assert_eq!(format_time_str(f64::NAN), "NaN");
        assert_eq!(format_time_str(f64::INFINITY), "4294967295:00NaN");
        assert_eq!(format_time_str(f64::NEG_INFINITY), "-1");
    }
}
