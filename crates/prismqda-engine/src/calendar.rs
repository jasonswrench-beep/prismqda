//! Reading dates the way the engine promises to read them, with no time zones and no guessing.
//!
//! A date is read as year, month, day, in that order: `2024-03-14`, `2024/3/14`, or either followed by a time
//! (`2024-03-14T09:30:00Z` or `2024-03-14 09:30`). A value written another way (`3/14/2024`, `14 March 2024`) is not
//! read as a date at all and never matches, because "3/4/2024" cannot be told from the 3rd of April without knowing
//! where it was written. A day is counted as the number of days since 1970-01-01 (the first of January 1970 is 0).

/// The day a text names, or `None` when it is not a date in one of the forms above or the day does not exist.
pub fn parse_day(text: &str) -> Option<i64> {
    let t = text.trim();
    // Cut the time off, if there is one: a date may be followed by `T` or a space and then anything.
    let date = t.split(['T', ' ']).next()?;
    let mut parts = date.split(['-', '/']);
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || y.len() != 4 || !(1..=2).contains(&m.len()) || !(1..=2).contains(&d.len()) {
        return None;
    }
    let (y, m, d): (i64, i64, i64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

/// Days since 1970-01-01 for a calendar date (Howard Hinnant's algorithm).
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The calendar date of a day number, as `(year, month, day)`.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `2024-03-14` for a day number.
pub fn day_text(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `2024-03` for a day number: the month it falls in.
pub fn month_text(day: i64) -> String {
    let (y, m, _) = civil_from_days(day);
    format!("{y:04}-{m:02}")
}

/// A number as the engine reads one in a case variable: plain digits with an optional sign and decimal point, with commas
/// between thousands allowed. Anything else is not a number.
pub fn parse_number(text: &str) -> Option<f64> {
    let t = text.trim().replace(',', "");
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+')) {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_read_year_month_day_with_or_without_a_time() {
        assert_eq!(parse_day("1970-01-01"), Some(0));
        assert_eq!(parse_day("1970-01-02"), Some(1));
        assert_eq!(parse_day("2024-03-14"), parse_day("2024/3/14"));
        assert_eq!(parse_day("2024-03-14T09:30:00Z"), parse_day("2024-03-14"));
        assert_eq!(parse_day("2024-03-14 09:30"), parse_day("2024-03-14"));
        assert_eq!(parse_day("  2024-03-14  "), parse_day("2024-03-14"));
        assert_eq!(parse_day("1969-12-31"), Some(-1));
    }

    #[test]
    fn a_value_that_is_not_certainly_a_date_is_not_read() {
        for bad in ["3/14/2024", "14 March 2024", "2024-13-01", "2024-02-30", "2023-02-29", "24-03-14", "2024-03", "", "yesterday", "2024-03-14-01"] {
            assert_eq!(parse_day(bad), None, "{bad:?}");
        }
        assert!(parse_day("2024-02-29").is_some(), "a leap day exists");
    }

    #[test]
    fn day_numbers_go_back_to_dates_and_months() {
        for text in ["1970-01-01", "2000-02-29", "2024-12-31", "1899-07-04", "2100-03-01"] {
            assert_eq!(day_text(parse_day(text).unwrap()), text);
        }
        assert_eq!(month_text(parse_day("2024-03-14").unwrap()), "2024-03");
        assert_eq!(parse_day("2024-03-01").unwrap() - parse_day("2024-02-01").unwrap(), 29, "February 2024 has 29 days");
    }

    #[test]
    fn numbers_allow_thousands_separators_and_nothing_else() {
        assert_eq!(parse_number("42"), Some(42.0));
        assert_eq!(parse_number(" -3.5 "), Some(-3.5));
        assert_eq!(parse_number("1,200"), Some(1200.0));
        for bad in ["", "abc", "12px", "1e5", "--1", "."] {
            assert_eq!(parse_number(bad), None, "{bad:?}");
        }
    }
}
