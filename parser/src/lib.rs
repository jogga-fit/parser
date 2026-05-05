//! fedisport activity file parser — GPX and FIT.
//!
//! WASM-safe: no `Utc::now()`, no filesystem access, no blocking I/O.
//! `started_at` uses `Option<DateTime<FixedOffset>>` so callers supply a fallback.
use std::io::Cursor;

use chrono::{DateTime, FixedOffset};
use fitparser::{FitDataRecord, Value, profile::MesgNum};
use serde::{Deserialize, Serialize};

/// FIT epoch: 1989-12-31T00:00:00 UTC expressed as Unix seconds.
const FIT_EPOCH_OFFSET: i64 = 631_065_600;
const MAX_TRACK_POINTS: usize = 100_000;
/// Cap on NP grid duration — prevents multi-GB allocation from adversarial FIT timestamps.
const MAX_NP_DURATION_S: u32 = 86_400 * 2; // 48 h covers any real ultra-endurance event

/// A single GPS coordinate with optional elevation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutePoint {
    pub lat: f64,
    pub lon: f64,
    pub ele: Option<f64>,
}

/// Parsed metrics from a GPX or FIT activity file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedActivity {
    /// Sport type from file metadata (e.g. "running", "cycling"). May be absent in GPX.
    pub sport: Option<String>,
    /// Start time from the file. `None` if the file contains no timestamp.
    /// Callers should substitute `Utc::now()` server-side if absent.
    pub started_at: Option<DateTime<FixedOffset>>,
    pub duration_s: i32,
    pub distance_m: f64,
    pub elevation_gain_m: Option<f64>,
    pub avg_pace_s_per_km: Option<f64>,
    pub avg_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub avg_cadence_rpm: Option<f64>,
    pub avg_power_w: Option<f64>,
    pub max_power_w: Option<f64>,
    pub normalized_power_w: Option<f64>,
    pub device: Option<String>,
    /// Route coordinates in (lat, lon, elevation?) order for rendering.
    pub route_coords: Vec<RoutePoint>,
}

/// Unified parse error.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("GPX parse error: {0}")]
    Gpx(String),
    #[error("FIT parse error: {0}")]
    Fit(String),
    #[error("file contains no track points")]
    NoTrackPoints,
    #[error("file contains no session data")]
    NoSession,
    #[error("file exceeds maximum track point limit (100 000)")]
    TooManyPoints,
    #[error("invalid coordinates in file")]
    InvalidCoords,
    #[error("unknown file format (expected GPX or FIT)")]
    UnknownFormat,
}

impl ParsedActivity {
    /// Parse a GPX file from raw bytes.
    pub fn from_gpx(bytes: &[u8]) -> Result<ParsedActivity, ParseError> {
        let gpx = gpx::read(Cursor::new(bytes)).map_err(|e| ParseError::Gpx(e.to_string()))?;

        let points: Vec<&gpx::Waypoint> = gpx
            .tracks
            .iter()
            .flat_map(|t| t.segments.iter())
            .flat_map(|s| s.points.iter())
            .collect();

        if points.is_empty() {
            return Err(ParseError::NoTrackPoints);
        }
        if points.len() > MAX_TRACK_POINTS {
            return Err(ParseError::TooManyPoints);
        }
        for wpt in &points {
            let p = wpt.point();
            if !(-90.0_f64..=90.0).contains(&p.y()) || !(-180.0_f64..=180.0).contains(&p.x()) {
                return Err(ParseError::InvalidCoords);
            }
        }

        let started_at = points
            .first()
            .and_then(|w| w.time.as_ref())
            .and_then(|t| t.format().ok())
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok());

        let ended_at: Option<DateTime<FixedOffset>> = points
            .last()
            .and_then(|w| w.time.as_ref())
            .and_then(|t| t.format().ok())
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok());

        let duration_s = match (started_at, ended_at) {
            (Some(s), Some(e)) => (e - s).num_seconds().max(0).min(i64::from(i32::MAX)) as i32,
            _ => 0,
        };

        let distance_m: f64 = points
            .windows(2)
            .map(|w| {
                let p1 = w[0].point();
                let p2 = w[1].point();
                haversine_m(p1.y(), p1.x(), p2.y(), p2.x())
            })
            .sum();

        let elevation_gain_m: Option<f64> = {
            let gains: Vec<f64> = points
                .windows(2)
                .filter_map(|w| {
                    let e1 = w[0].elevation?;
                    let e2 = w[1].elevation?;
                    Some((e2 - e1).max(0.0))
                })
                .collect();
            if gains.is_empty() { None } else { Some(gains.iter().sum()) }
        };

        let avg_pace_s_per_km = if duration_s > 0 && distance_m > 0.0 {
            Some(1000.0 / (distance_m / duration_s as f64))
        } else {
            None
        };

        let route_coords: Vec<RoutePoint> = points
            .iter()
            .map(|wpt| {
                let p = wpt.point();
                RoutePoint { lat: p.y(), lon: p.x(), ele: wpt.elevation }
            })
            .collect();

        let sport = gpx.tracks.first().and_then(|t| t.type_.clone());

        Ok(ParsedActivity {
            sport,
            started_at,
            duration_s,
            distance_m,
            elevation_gain_m,
            avg_pace_s_per_km,
            avg_heart_rate_bpm: None,
            max_heart_rate_bpm: None,
            avg_cadence_rpm: None,
            avg_power_w: None,
            max_power_w: None,
            normalized_power_w: None,
            device: None,
            route_coords,
        })
    }

    /// Parse a FIT file from raw bytes.
    pub fn from_fit(bytes: &[u8]) -> Result<ParsedActivity, ParseError> {
        let mut cursor = Cursor::new(bytes);
        let records =
            fitparser::from_reader(&mut cursor).map_err(|e| ParseError::Fit(e.to_string()))?;

        let mut session_records: Vec<&FitDataRecord> = Vec::new();
        let mut track_records: Vec<&FitDataRecord> = Vec::new();
        let mut device_info: Vec<&FitDataRecord> = Vec::new();

        for rec in &records {
            match rec.kind() {
                MesgNum::Session => session_records.push(rec),
                MesgNum::Record => track_records.push(rec),
                MesgNum::DeviceInfo => device_info.push(rec),
                _ => {}
            }
        }

        // Use the last session record: for multi-sport FIT files (triathlons) the last
        // session record holds the aggregate totals, matching the accumulated track_records.
        let session = session_records.last().ok_or(ParseError::NoSession)?;

        let started_at = extract_timestamp(session, "start_time")
            .or_else(|| extract_timestamp(session, "timestamp"));

        let duration_s = extract_f64(session, "total_elapsed_time")
            .or_else(|| extract_f64(session, "total_timer_time"))
            .map(|s| s.max(0.0).min(i32::MAX as f64) as i32)
            .unwrap_or(0);

        let distance_m_session = extract_f64(session, "total_distance");
        let avg_heart_rate_bpm = extract_i32(session, "avg_heart_rate");
        let max_heart_rate_bpm = extract_i32(session, "max_heart_rate");
        let avg_cadence_rpm = extract_f64(session, "avg_cadence");
        let avg_power_w = extract_f64(session, "avg_power");
        let max_power_w = extract_f64(session, "max_power");
        let avg_speed_ms = extract_f64(session, "avg_speed");
        let avg_pace_s_per_km = avg_speed_ms.filter(|&s| s > 0.0).map(|s| 1000.0 / s);

        let sport = extract_string(session, "sport");

        if track_records.len() > MAX_TRACK_POINTS {
            return Err(ParseError::TooManyPoints);
        }

        let mut route_coords: Vec<RoutePoint> = Vec::new();
        // (elapsed_seconds_from_first_record, power_watts) for NP calculation.
        let mut power_samples: Vec<(u32, f64)> = Vec::new();
        let mut first_record_ts: Option<DateTime<FixedOffset>> = None;
        let mut prev_alt: Option<f64> = None;
        let mut elevation_gain = 0.0_f64;
        let mut has_elevation = false;
        let mut distance_from_records = 0.0_f64;

        for rec in &track_records {
            // Set time anchor from the first record that carries a timestamp.
            if first_record_ts.is_none() {
                first_record_ts = extract_timestamp(rec, "timestamp");
            }

            let lat = extract_semicircle(rec, "position_lat");
            let lon = extract_semicircle(rec, "position_long");
            let alt = extract_f64(rec, "altitude");

            if let (Some(lat), Some(lon)) = (lat, lon)
                && (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)
            {
                route_coords.push(RoutePoint { lat, lon, ele: alt });
            }

            if let Some(a) = alt {
                has_elevation = true;
                if let Some(prev) = prev_alt {
                    let gain = a - prev;
                    if gain > 0.0 {
                        elevation_gain += gain;
                    }
                }
                prev_alt = Some(a);
            }

            // Collect timestamped power for accurate 1Hz upsampling before NP.
            if let Some(p) = extract_f64(rec, "power")
                && let (Some(anchor), Some(ts)) =
                    (first_record_ts, extract_timestamp(rec, "timestamp"))
            {
                let elapsed = (ts - anchor).num_seconds().max(0) as u32;
                power_samples.push((elapsed, p));
            }

            if let Some(d) = extract_f64(rec, "distance") {
                distance_from_records = d;
            }
        }

        let distance_m = distance_m_session.unwrap_or(distance_from_records).max(0.0);

        if distance_m == 0.0 && route_coords.is_empty() {
            return Err(ParseError::NoTrackPoints);
        }

        let elevation_gain_m = if has_elevation { Some(elevation_gain) } else { None };

        // Sort by elapsed time to handle FIT files with non-chronological records.
        power_samples.sort_unstable_by_key(|s| s.0);
        // Normalized power — Allen & Coggan with correct 1Hz upsampling for non-1Hz FIT files.
        let normalized_power_w = compute_normalized_power(&power_samples);

        let device = device_info.first().and_then(|rec| {
            extract_string(rec, "product_name")
                .or_else(|| extract_string(rec, "garmin_product"))
                .or_else(|| extract_string(rec, "manufacturer"))
        });

        Ok(ParsedActivity {
            sport,
            started_at,
            duration_s,
            distance_m,
            elevation_gain_m,
            avg_pace_s_per_km,
            avg_heart_rate_bpm,
            max_heart_rate_bpm,
            avg_cadence_rpm,
            avg_power_w,
            max_power_w,
            normalized_power_w,
            device,
            route_coords,
        })
    }

    /// Detect file format from magic bytes and dispatch to `from_gpx` or `from_fit`.
    ///
    /// FIT detection: bytes 8–11 are the ASCII string `.FIT` per the FIT protocol spec.
    /// GPX detection: first non-whitespace byte is `<` (XML).
    pub fn parse_auto(bytes: &[u8]) -> Result<ParsedActivity, ParseError> {
        if bytes.get(8..12) == Some(b".FIT") {
            return Self::from_fit(bytes);
        }
        if bytes.iter().find(|&&b| !b.is_ascii_whitespace()) == Some(&b'<') {
            return Self::from_gpx(bytes);
        }
        Err(ParseError::UnknownFormat)
    }
}

fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6_371_000.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

fn get_field<'a>(rec: &'a FitDataRecord, name: &str) -> Option<&'a Value> {
    rec.fields().iter().find(|f| f.name() == name).map(|f| f.value())
}

fn extract_f64(rec: &FitDataRecord, name: &str) -> Option<f64> {
    match get_field(rec, name)? {
        Value::Float64(v) => Some(*v),
        Value::Float32(v) => Some(*v as f64),
        Value::UInt32(v) => Some(*v as f64),
        Value::UInt16(v) => Some(*v as f64),
        Value::SInt32(v) => Some(*v as f64),
        Value::SInt16(v) => Some(*v as f64),
        Value::UInt8(v) => Some(*v as f64),
        _ => None,
    }
}

fn extract_i32(rec: &FitDataRecord, name: &str) -> Option<i32> {
    match get_field(rec, name)? {
        Value::UInt8(v) => Some(*v as i32),
        Value::UInt16(v) => Some(*v as i32),
        Value::UInt32(v) => Some(*v as i32),
        Value::SInt32(v) => Some(*v),
        Value::SInt16(v) => Some(*v as i32),
        Value::Float64(v) => Some(*v as i32),
        _ => None,
    }
}

fn extract_semicircle(rec: &FitDataRecord, name: &str) -> Option<f64> {
    const SEMICIRCLE_TO_DEG: f64 = 180.0 / 2_147_483_648.0;
    match get_field(rec, name)? {
        Value::SInt32(v) => Some(*v as f64 * SEMICIRCLE_TO_DEG),
        Value::Float64(v) => Some(*v),
        _ => None,
    }
}

fn extract_string(rec: &FitDataRecord, name: &str) -> Option<String> {
    match get_field(rec, name)? {
        Value::String(s) => Some(s.clone()),
        Value::Enum(e) => Some(format!("{e:?}")),
        _ => None,
    }
}

/// Extract a timestamp from a FIT record without calling `now()`.
fn extract_timestamp(rec: &FitDataRecord, name: &str) -> Option<DateTime<FixedOffset>> {
    match get_field(rec, name)? {
        // `DateTime<Local>::fixed_offset()` gives us `DateTime<FixedOffset>` directly,
        // avoiding the previous fragile Debug-format → RFC3339 round-trip.
        Value::Timestamp(ts) => Some(ts.fixed_offset()),
        Value::UInt32(secs) => {
            let unix = *secs as i64 + FIT_EPOCH_OFFSET;
            DateTime::from_timestamp(unix, 0).map(|dt| dt.fixed_offset())
        }
        _ => None,
    }
}

/// Compute normalized power from timestamped `(elapsed_secs, watts)` samples.
///
/// Allen & Coggan algorithm with 1Hz upsampling:
/// 1. Forward-fill power onto a 1-second grid.
/// 2. Apply 30-second rolling average.
/// 3. Raise each value to 4th power, average, take 4th root.
///
/// Returns `None` if total duration < 30 s or no samples provided.
fn compute_normalized_power(samples: &[(u32, f64)]) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let total_duration = samples.last()?.0;
    if total_duration < 30 {
        return None;
    }
    if total_duration > MAX_NP_DURATION_S {
        return None;
    }

    // Forward-fill to 1Hz grid. Slots before the first power sample stay 0.
    let grid_len = (total_duration + 1) as usize;
    let mut grid: Vec<f64> = vec![0.0; grid_len];
    for i in 0..samples.len() {
        let start = samples[i].0 as usize;
        let end = if i + 1 < samples.len() { samples[i + 1].0 as usize } else { grid_len };
        let power = samples[i].1;
        for cell in grid.iter_mut().take(end.min(grid_len)).skip(start) {
            *cell = power;
        }
    }

    const WINDOW: usize = 30; // Allen & Coggan 30-second rolling average
    if grid.len() < WINDOW {
        return None;
    }
    let rolling_avgs: Vec<f64> =
        grid.windows(WINDOW).map(|w| w.iter().sum::<f64>() / WINDOW as f64).collect();
    let mean_fourth =
        rolling_avgs.iter().map(|&x| x.powi(4)).sum::<f64>() / rolling_avgs.len() as f64;
    Some(mean_fourth.powf(0.25))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_GPX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <type>running</type>
    <trkseg>
      <trkpt lat="51.5074" lon="-0.1278">
        <ele>10.0</ele>
        <time>2024-01-15T09:00:00Z</time>
      </trkpt>
      <trkpt lat="51.5080" lon="-0.1270">
        <ele>12.5</ele>
        <time>2024-01-15T09:05:00Z</time>
      </trkpt>
      <trkpt lat="51.5090" lon="-0.1260">
        <ele>11.0</ele>
        <time>2024-01-15T09:10:00Z</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    #[test]
    fn parse_gpx_happy_path() {
        let result = ParsedActivity::from_gpx(MINIMAL_GPX.as_bytes()).unwrap();
        assert_eq!(result.route_coords.len(), 3);
        assert!(result.distance_m > 0.0);
        assert_eq!(result.duration_s, 600); // 10 minutes
        assert_eq!(result.sport.as_deref(), Some("running"));
        assert!(result.started_at.is_some());
        // elevation: 10 → 12.5 = +2.5 gain; 12.5 → 11 = loss, ignored
        assert!((result.elevation_gain_m.unwrap() - 2.5).abs() < 0.01);
        // all FIT-only fields should be None
        assert!(result.avg_heart_rate_bpm.is_none());
        assert!(result.avg_power_w.is_none());
    }

    #[test]
    fn parse_gpx_no_elevation() {
        let gpx = r#"<?xml version="1.0"?>
<gpx version="1.1" creator="test">
  <trk><trkseg>
    <trkpt lat="51.5074" lon="-0.1278"><time>2024-01-15T09:00:00Z</time></trkpt>
    <trkpt lat="51.5080" lon="-0.1270"><time>2024-01-15T09:05:00Z</time></trkpt>
  </trkseg></trk>
</gpx>"#;
        let result = ParsedActivity::from_gpx(gpx.as_bytes()).unwrap();
        assert!(result.elevation_gain_m.is_none());
        assert!(result.route_coords.iter().all(|p| p.ele.is_none()));
    }

    #[test]
    fn parse_gpx_no_timestamps() {
        let gpx = r#"<?xml version="1.0"?>
<gpx version="1.1" creator="test">
  <trk><trkseg>
    <trkpt lat="51.5074" lon="-0.1278"/>
    <trkpt lat="51.5080" lon="-0.1270"/>
  </trkseg></trk>
</gpx>"#;
        let result = ParsedActivity::from_gpx(gpx.as_bytes()).unwrap();
        assert!(result.started_at.is_none());
        assert_eq!(result.duration_s, 0);
    }

    #[test]
    fn parse_gpx_empty_returns_error() {
        assert!(ParsedActivity::from_gpx(b"not xml").is_err());
    }

    #[test]
    fn parse_gpx_no_track_points_returns_error() {
        let gpx = r#"<?xml version="1.0"?>
<gpx version="1.1" creator="test"><trk><trkseg></trkseg></trk></gpx>"#;
        let err = ParsedActivity::from_gpx(gpx.as_bytes()).unwrap_err();
        assert!(matches!(err, ParseError::NoTrackPoints));
    }

    #[test]
    fn parse_gpx_invalid_coords_returns_error() {
        // The gpx crate rejects lat=999.0 at the XML-parse level, so we get
        // ParseError::Gpx. Our own InvalidCoords check catches values that
        // slip past the crate (e.g. values just outside ±90/±180).
        let gpx = r#"<?xml version="1.0"?>
<gpx version="1.1" creator="test">
  <trk><trkseg>
    <trkpt lat="999.0" lon="-0.1278"/>
  </trkseg></trk>
</gpx>"#;
        assert!(ParsedActivity::from_gpx(gpx.as_bytes()).is_err());
    }

    #[test]
    fn haversine_london_to_paris() {
        // London (51.5074, -0.1278) to Paris (48.8566, 2.3522) ≈ 340 km
        let dist = haversine_m(51.5074, -0.1278, 48.8566, 2.3522);
        assert!((dist - 340_000.0).abs() < 5_000.0, "got {dist}");
    }

    #[test]
    fn parse_auto_dispatches_gpx() {
        let result = ParsedActivity::parse_auto(MINIMAL_GPX.as_bytes()).unwrap();
        assert_eq!(result.sport.as_deref(), Some("running"));
    }

    #[test]
    fn parse_auto_unknown_format_error() {
        let err = ParsedActivity::parse_auto(b"hello world").unwrap_err();
        assert!(matches!(err, ParseError::UnknownFormat));
    }

    #[test]
    fn parse_auto_empty_unknown_format() {
        let err = ParsedActivity::parse_auto(b"").unwrap_err();
        assert!(matches!(err, ParseError::UnknownFormat));
    }

    #[test]
    fn np_uniform_200w_returns_200w() {
        let samples: Vec<(u32, f64)> = (0u32..60).map(|s| (s, 200.0)).collect();
        let np = compute_normalized_power(&samples).unwrap();
        assert!((np - 200.0).abs() < 0.001, "expected 200W, got {np}");
    }

    #[test]
    fn np_below_30s_returns_none() {
        let samples: Vec<(u32, f64)> = (0u32..20).map(|s| (s, 250.0)).collect();
        assert!(compute_normalized_power(&samples).is_none());
    }

    #[test]
    fn np_empty_returns_none() {
        assert!(compute_normalized_power(&[]).is_none());
    }

    #[test]
    fn np_sparse_2s_intervals_correct() {
        // Smart-recording at 2-second intervals, 60 samples × 2s = 120s at 200W.
        // After 1Hz forward-fill every slot is 200W → NP = 200W.
        let samples: Vec<(u32, f64)> = (0u32..60).map(|s| (s * 2, 200.0)).collect();
        let np = compute_normalized_power(&samples).unwrap();
        assert!((np - 200.0).abs() < 0.001, "expected 200W, got {np}");
    }

    #[test]
    fn np_higher_than_avg_for_variable_power() {
        // 30s@100W then 30s@300W → avg = 200W, NP > 200W due to 4th-power weighting.
        let mut samples: Vec<(u32, f64)> = (0u32..30).map(|s| (s, 100.0)).collect();
        samples.extend((30u32..60).map(|s| (s, 300.0)));
        let np = compute_normalized_power(&samples).unwrap();
        assert!(np > 200.0, "NP {np} should exceed avg 200W for variable power");
    }

    #[test]
    fn np_at_30s_boundary() {
        // total_duration == 29 (elapsed 0..=29) → guard fires (< 30) → None
        let below: Vec<(u32, f64)> = (0u32..30).map(|s| (s, 200.0)).collect();
        assert!(compute_normalized_power(&below).is_none());
        // total_duration == 30 (elapsed 0..=30) → guard passes → Some
        let at: Vec<(u32, f64)> = (0u32..31).map(|s| (s, 200.0)).collect();
        assert!(compute_normalized_power(&at).is_some());
    }

    #[test]
    fn np_all_zero_returns_zero() {
        let samples: Vec<(u32, f64)> = (0u32..60).map(|s| (s, 0.0)).collect();
        let np = compute_normalized_power(&samples).unwrap();
        assert!(np.abs() < 0.001, "expected 0W, got {np}");
    }

    #[test]
    fn haversine_same_point_is_zero() {
        let dist = haversine_m(51.5074, -0.1278, 51.5074, -0.1278);
        assert!(dist.abs() < 0.001, "same-point distance should be 0, got {dist}");
    }

    #[test]
    fn parse_auto_gpx_leading_whitespace_dispatches_not_unknown() {
        // parse_auto finds '<' as first non-whitespace → dispatches to from_gpx.
        // xml-rs rejects whitespace before the XML declaration (invalid XML), so
        // the result is Gpx(...) not UnknownFormat.
        let padded = format!("  {}", MINIMAL_GPX);
        match ParsedActivity::parse_auto(padded.as_bytes()) {
            Err(ParseError::UnknownFormat) => panic!("should dispatch to GPX, not UnknownFormat"),
            _ => {} // Gpx parse error or Ok — both mean dispatch was correct
        }
    }

    #[test]
    fn gpx_avg_pace_none_when_zero_duration() {
        // Two points with identical timestamps → duration_s = 0 → avg_pace = None.
        let gpx = r#"<?xml version="1.0"?>
<gpx version="1.1" creator="test">
  <trk><trkseg>
    <trkpt lat="51.5074" lon="-0.1278"><time>2024-01-15T09:00:00Z</time></trkpt>
    <trkpt lat="51.5080" lon="-0.1270"><time>2024-01-15T09:00:00Z</time></trkpt>
  </trkseg></trk>
</gpx>"#;
        let result = ParsedActivity::from_gpx(gpx.as_bytes()).unwrap();
        assert_eq!(result.duration_s, 0);
        assert!(result.avg_pace_s_per_km.is_none());
    }
}
