//! `file.stat`: what the launcher needs to know about a recent project without opening it.
//!
//! Only the file system is asked. The file is never opened as a project, so nothing is read from it,
//! no backup is made, and no `-wal` or `-shm` file appears beside it.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::ApiError;

/// Whether a path exists, whether it is a file, its name, when it was last changed (UTC, in the same form
/// the project file uses), and its size in bytes. A path that is not there is a normal answer, not an
/// error. A folder is `is_file: false` with size 0.
pub(crate) fn stat(path: &str) -> Result<Value, ApiError> {
    if path.trim().is_empty() {
        return Err(ApiError::new("invalid", "say which file to look at"));
    }
    let path = Path::new(path);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    match fs::metadata(path) {
        Ok(meta) => {
            let modified = meta.modified().ok().and_then(utc_text);
            let size = if meta.is_file() { meta.len() } else { 0 };
            // Only the first bytes are read, to tell whether the file is encrypted (so the window asks for the passphrase).
            let encrypted = meta.is_file() && prismqda_engine::file_looks_encrypted(path).unwrap_or(false);
            Ok(json!({ "exists": true, "is_file": meta.is_file(), "name": name, "modified": modified, "size": size, "encrypted": encrypted }))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(json!({ "exists": false, "is_file": false, "name": name, "modified": null, "size": 0, "encrypted": false })),
        Err(e) => Err(ApiError::new("io", format!("cannot look at {}: {e}", path.display()))),
    }
}

/// A moment as UTC text with milliseconds, like `2026-11-04T15:30:00.123Z`. Moments before 1970 give `None`.
fn utc_text(time: SystemTime) -> Option<String> {
    let since = time.duration_since(UNIX_EPOCH).ok()?;
    let secs = since.as_secs();
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (year, month, day) = civil_from_days(days as i64);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60,
        since.subsec_millis()
    ))
}

/// The calendar date (proleptic Gregorian) that is `days` after 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(secs: u64, millis: u32) -> Option<String> {
        utc_text(UNIX_EPOCH + Duration::new(secs, millis * 1_000_000))
    }

    #[test]
    fn dates_come_out_right_across_leap_years_and_centuries() {
        assert_eq!(at(0, 0).unwrap(), "1970-01-01T00:00:00.000Z");
        assert_eq!(at(1_700_000_000, 123).unwrap(), "2023-11-14T22:13:20.123Z");
        // 2024 is a leap year: the 29th of February exists.
        assert_eq!(at(1_709_208_000, 0).unwrap(), "2024-02-29T12:00:00.000Z");
        assert_eq!(at(1_709_208_000 + 43_200, 0).unwrap(), "2024-03-01T00:00:00.000Z");
        // 2100 is not: the day after the 28th of February is the 1st of March.
        assert_eq!(at(4_107_542_400 - 1, 999).unwrap(), "2100-02-28T23:59:59.999Z");
        assert_eq!(at(4_107_542_400, 0).unwrap(), "2100-03-01T00:00:00.000Z");
        // 2000 is: divisible by 400.
        assert_eq!(at(951_782_400, 0).unwrap(), "2000-02-29T00:00:00.000Z");
        assert_eq!(at(1_793_806_200, 7).unwrap(), "2026-11-04T15:30:00.007Z");
    }

    #[test]
    fn a_moment_before_1970_is_not_invented() {
        assert_eq!(utc_text(UNIX_EPOCH - Duration::from_secs(5)), None);
    }

    #[test]
    fn every_day_of_four_years_round_trips_through_the_calendar() {
        // Walk day by day from 1999 to 2003 and compare with counting the calendar by hand.
        let mut expected = (1999_i64, 1_i64, 1_i64);
        let first = 10_592; // 1999-01-01 is this many days after 1970-01-01
        for offset in 0..(365 * 4 + 366) {
            assert_eq!(civil_from_days(first + offset), expected, "day {offset}");
            let leap = (expected.0 % 4 == 0 && expected.0 % 100 != 0) || expected.0 % 400 == 0;
            let length = match expected.1 {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                _ if leap => 29,
                _ => 28,
            };
            expected = if expected.2 < length {
                (expected.0, expected.1, expected.2 + 1)
            } else if expected.1 < 12 {
                (expected.0, expected.1 + 1, 1)
            } else {
                (expected.0 + 1, 1, 1)
            };
        }
    }
}
