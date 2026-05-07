/// Format a duration in seconds as `h:mm:ss` or `m:ss`.
pub fn fmt_duration(s: i32) -> String {
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// Format a distance in metres as `x.xx km` or `x m`.
pub fn fmt_distance(m: f64) -> String {
    if m >= 1000.0 {
        format!("{:.2} km", m / 1000.0)
    } else {
        format!("{:.0} m", m)
    }
}

/// Format an elevation in metres as `x m`.
pub fn fmt_elevation(m: f64) -> String {
    format!("{:.0} m", m)
}

/// Format a pace in seconds-per-km as `m:ss /km`.
pub fn fmt_pace(s_per_km: f64) -> String {
    let secs = s_per_km as i64;
    format!("{}:{:02} /km", secs / 60, secs % 60)
}
