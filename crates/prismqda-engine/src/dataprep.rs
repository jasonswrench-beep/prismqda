//! The pure parts of the data editor's later transformations: the parts of a date, a random sample of cases, a summary of each group, and
//! the order cases are shown in. Nothing here touches the project; `variables.rs` reads the variables, calls these, and saves the result.

use std::collections::BTreeMap;

use prismqda_stats::describe::{descriptives, percentile_sorted, sorted_finite};
use prismqda_stats::SplitMix64;
use serde::{Deserialize, Serialize};

use crate::calendar::{civil_from_days, days_from_civil, parse_day};

// ---------------------------------------------------------------- the parts of a date

/// What can be taken out of a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatePart {
    Year,
    /// 1 to 4.
    Quarter,
    /// 1 to 12.
    Month,
    /// The ISO 8601 week, 1 to 53 (the week with the year's first Thursday is week 1, and weeks begin on Monday), as R's `%V`.
    Week,
    /// The day of the month, 1 to 31.
    Day,
    /// 1 to 366.
    DayOfYear,
    /// 1 (Monday) to 7 (Sunday), as R's `%u`.
    Weekday,
    /// 0 to 23, from the time written after the date; a date with no time has none.
    Hour,
}

impl DatePart {
    pub const ALL: [DatePart; 8] = [DatePart::Year, DatePart::Quarter, DatePart::Month, DatePart::Week, DatePart::Day, DatePart::DayOfYear, DatePart::Weekday, DatePart::Hour];

    /// The level of measurement the part has, unless the person chooses another.
    pub fn level(self) -> &'static str {
        match self {
            DatePart::Quarter | DatePart::Month | DatePart::Weekday => "ordinal",
            _ => "interval",
        }
    }

    /// The words a value of the part carries, by stored value (the months, the days of the week, and the quarters).
    pub fn labels(self) -> BTreeMap<String, String> {
        const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
        const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
        match self {
            DatePart::Month => MONTHS.iter().enumerate().map(|(i, m)| ((i + 1).to_string(), (*m).to_string())).collect(),
            DatePart::Weekday => DAYS.iter().enumerate().map(|(i, d)| ((i + 1).to_string(), (*d).to_string())).collect(),
            DatePart::Quarter => (1..=4).map(|q| (q.to_string(), format!("Q{q}"))).collect(),
            _ => BTreeMap::new(),
        }
    }
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// 1 (Monday) to 7 (Sunday) of a day counted from 1970-01-01, which was a Thursday.
fn iso_weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7) + 1
}

/// The number of ISO weeks in a year: 53 when the year begins on a Thursday, or on a Wednesday in a leap year, and 52 otherwise.
fn iso_weeks_in_year(y: i64) -> i64 {
    let jan1 = iso_weekday(days_from_civil(y, 1, 1));
    if jan1 == 4 || (jan1 == 3 && is_leap(y)) {
        53
    } else {
        52
    }
}

/// The ISO week of a day.
fn iso_week(day: i64) -> i64 {
    let (y, _, _) = civil_from_days(day);
    let doy = day - days_from_civil(y, 1, 1) + 1;
    let week = (doy - iso_weekday(day) + 10) / 7;
    if week < 1 {
        iso_weeks_in_year(y - 1)
    } else if week > iso_weeks_in_year(y) {
        1
    } else {
        week
    }
}

/// The hour written after a date (`2024-03-14T09:30` or `2024-03-14 09:30`), when there is one and it is a real hour.
fn hour_of(text: &str) -> Option<i64> {
    let t = text.trim();
    let (_, time) = t.split_once(['T', ' '])?;
    let hour = time.trim().split(':').next()?;
    let h: i64 = hour.parse().ok()?;
    (0..=23).contains(&h).then_some(h)
}

/// One part of a date as written (year, month, day in that order, as the engine reads dates), or `None` when the text is not a date or
/// the date has no such part.
pub fn date_part(text: &str, part: DatePart) -> Option<f64> {
    let day = parse_day(text)?;
    let (y, m, d) = civil_from_days(day);
    let value = match part {
        DatePart::Year => y,
        DatePart::Quarter => (m - 1) / 3 + 1,
        DatePart::Month => m,
        DatePart::Week => iso_week(day),
        DatePart::Day => d,
        DatePart::DayOfYear => day - days_from_civil(y, 1, 1) + 1,
        DatePart::Weekday => iso_weekday(day),
        DatePart::Hour => hour_of(text)?,
    };
    Some(value as f64)
}

// ---------------------------------------------------------------- a random sample of cases

/// How many cases a sample holds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "by", content = "value", rename_all = "snake_case")]
pub enum SampleSize {
    /// This many cases in all.
    Count(usize),
    /// This percent of the cases (of each group, when the sample is drawn within groups), rounded to the nearest whole case.
    Percent(f64),
}

/// Draws a sample of the cases in `eligible` (their positions) and says, for every one of `total` positions, whether it is in the sample
/// (`Some(true)` or `Some(false)`), or `None` when it was not eligible. `groups` gives each eligible position the group it is drawn within
/// (a percent sample takes its share of each group, so a small group is not left out by chance); with no groups the cases are one group.
/// The same seed over the same cases always gives the same sample.
pub fn draw_sample(total: usize, groups: &[Option<usize>], size: SampleSize, seed: u64) -> Result<Vec<Option<bool>>, String> {
    if groups.len() != total {
        return Err("the groups and the cases do not match".to_string());
    }
    let mut by_group: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, g) in groups.iter().enumerate() {
        if let Some(g) = g {
            by_group.entry(*g).or_default().push(i);
        }
    }
    let eligible: usize = by_group.values().map(Vec::len).sum();
    if eligible == 0 {
        return Err("no case can be drawn: none of them has a value to be drawn within".to_string());
    }
    match size {
        SampleSize::Count(n) => {
            if n == 0 {
                return Err("a sample holds at least one case".to_string());
            }
            if n > eligible {
                return Err(format!("a sample of {n} cases cannot be drawn from {eligible}"));
            }
        }
        SampleSize::Percent(p) => {
            if !(p > 0.0 && p <= 100.0) || !p.is_finite() {
                return Err("a percent is more than 0 and at most 100".to_string());
            }
        }
    }
    let mut rng = SplitMix64::new(seed);
    let mut out = vec![None; total];
    let sizes: Vec<usize> = by_group.values().map(Vec::len).collect();
    let wants: Vec<usize> = match size {
        // A count is shared out among the groups by their sizes, by the largest remainder so that the shares add up to the count.
        SampleSize::Count(n) => share_out(n, &sizes),
        SampleSize::Percent(p) => sizes.iter().map(|&s| (((p / 100.0) * s as f64).round() as usize).min(s)).collect(),
    };
    for (members, want) in by_group.values().zip(wants) {
        // A partial Fisher-Yates shuffle: the first `want` of the shuffled members are the sample.
        let mut pool = members.clone();
        for k in 0..want {
            let j = k + rng.below(pool.len() - k);
            pool.swap(k, j);
        }
        for &i in members {
            out[i] = Some(false);
        }
        for &i in &pool[..want] {
            out[i] = Some(true);
        }
    }
    Ok(out)
}

/// `n` shared among groups of these sizes in proportion to the sizes: each gets the whole part of its share, and what is left over goes one
/// each to the groups with the largest remainders (the earlier group first when two tie), never more than a group holds.
fn share_out(n: usize, sizes: &[usize]) -> Vec<usize> {
    let total: usize = sizes.iter().sum();
    if total == 0 {
        return vec![0; sizes.len()];
    }
    let exact: Vec<f64> = sizes.iter().map(|&s| n as f64 * s as f64 / total as f64).collect();
    let mut shares: Vec<usize> = exact.iter().map(|x| x.floor() as usize).collect();
    let mut left = n - shares.iter().sum::<usize>();
    let mut by_remainder: Vec<usize> = (0..sizes.len()).collect();
    by_remainder.sort_by(|&a, &b| (exact[b] - exact[b].floor()).partial_cmp(&(exact[a] - exact[a].floor())).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
    for &g in by_remainder.iter().cycle().take(sizes.len() * 2) {
        if left == 0 {
            break;
        }
        if shares[g] < sizes[g] {
            shares[g] += 1;
            left -= 1;
        }
    }
    shares
}

// ---------------------------------------------------------------- a summary of each group

/// What is worked out for each group of cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupFunction {
    Mean,
    Sum,
    Median,
    Min,
    Max,
    /// The standard deviation (n - 1 in the divisor), as R's `sd`; a group of one value has none.
    Sd,
    /// How many of the group's cases have a value.
    Count,
    /// How many cases are in the group, whether or not they have a value.
    Cases,
}

impl GroupFunction {
    pub const ALL: [GroupFunction; 8] = [GroupFunction::Mean, GroupFunction::Sum, GroupFunction::Median, GroupFunction::Min, GroupFunction::Max, GroupFunction::Sd, GroupFunction::Count, GroupFunction::Cases];

    /// True for the functions that count rather than work out something from the numbers.
    pub fn counts(self) -> bool {
        matches!(self, GroupFunction::Count | GroupFunction::Cases)
    }

    pub fn level(self) -> &'static str {
        if self.counts() {
            "ratio"
        } else {
            "interval"
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            GroupFunction::Mean => "mean",
            GroupFunction::Sum => "sum",
            GroupFunction::Median => "median",
            GroupFunction::Min => "min",
            GroupFunction::Max => "max",
            GroupFunction::Sd => "sd",
            GroupFunction::Count => "count",
            GroupFunction::Cases => "cases",
        }
    }
}

/// The summary of one group's values. `None` when the function has nothing to work from (no value, or one value for a standard deviation).
pub fn summarize(values: &[Option<f64>], function: GroupFunction, group_size: usize) -> Option<f64> {
    let have: Vec<f64> = values.iter().flatten().copied().filter(|x| x.is_finite()).collect();
    match function {
        GroupFunction::Cases => Some(group_size as f64),
        GroupFunction::Count => Some(have.len() as f64),
        _ if have.is_empty() => None,
        GroupFunction::Sum => Some(have.iter().sum()),
        GroupFunction::Min => have.iter().copied().reduce(f64::min),
        GroupFunction::Max => have.iter().copied().reduce(f64::max),
        GroupFunction::Mean => descriptives(&have, 0.95).mean,
        GroupFunction::Sd => descriptives(&have, 0.95).sd,
        GroupFunction::Median => percentile_sorted(&sorted_finite(&have), 0.5),
    }
}

// ---------------------------------------------------------------- the order of the cases

/// One key of a sort: a variable's values, and which way.
#[derive(Debug, Clone, PartialEq)]
pub struct SortColumn {
    /// The numbers, when the variable is read as numbers; the order then is numeric.
    pub numbers: Option<Vec<Option<f64>>>,
    /// The values as written, for a variable that is not numbers: they are put in order without regard to capitals.
    pub texts: Vec<Option<String>>,
    pub descending: bool,
}

/// The positions of the cases in the order the keys give: the first key first, the next to break its ties, and the order the cases already
/// have to break the rest. A case with no value comes last whichever way the key runs.
pub fn sort_order(count: usize, keys: &[SortColumn]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by(|&a, &b| {
        for key in keys {
            let ordering = match &key.numbers {
                Some(numbers) => match (numbers[a], numbers[b]) {
                    (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                    (Some(_), None) => return std::cmp::Ordering::Less,
                    (None, Some(_)) => return std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                },
                None => match (&key.texts[a], &key.texts[b]) {
                    (Some(x), Some(y)) => x.to_lowercase().cmp(&y.to_lowercase()).then_with(|| x.cmp(y)),
                    (Some(_), None) => return std::cmp::Ordering::Less,
                    (None, Some(_)) => return std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                },
            };
            let ordering = if key.descending { ordering.reverse() } else { ordering };
            if ordering != std::cmp::Ordering::Equal {
                return ordering;
            }
        }
        std::cmp::Ordering::Equal
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_of_a_date_are_the_ones_a_calendar_gives() {
        let p = |t: &str, part| date_part(t, part);
        assert_eq!(p("2024-03-14", DatePart::Year), Some(2024.0));
        assert_eq!(p("2024-03-14", DatePart::Quarter), Some(1.0));
        assert_eq!(p("2024-03-14", DatePart::Month), Some(3.0));
        assert_eq!(p("2024-03-14", DatePart::Day), Some(14.0));
        assert_eq!(p("2024-03-14", DatePart::DayOfYear), Some(74.0), "a leap year: 31 + 29 + 14");
        assert_eq!(p("2024-03-14", DatePart::Weekday), Some(4.0), "a Thursday");
        assert_eq!(p("2024-03-14", DatePart::Week), Some(11.0));
        assert_eq!(p("2024-03-14", DatePart::Hour), None, "no time was written");
        assert_eq!(p("2024-03-14T09:30:00Z", DatePart::Hour), Some(9.0));
        assert_eq!(p("2024/3/14 23:59", DatePart::Hour), Some(23.0));
        assert_eq!(p("2024-03-14 25:00", DatePart::Hour), None, "not an hour");
        assert_eq!(p("14 March 2024", DatePart::Year), None, "not a date the engine reads");
    }

    #[test]
    fn the_iso_week_runs_across_the_new_year_the_way_the_standard_says() {
        let w = |t: &str| date_part(t, DatePart::Week).unwrap() as i64;
        assert_eq!(w("2021-01-03"), 53, "a Sunday that belongs to the last week of 2020");
        assert_eq!(w("2021-01-04"), 1);
        assert_eq!(w("2024-12-30"), 1, "a Monday that belongs to the first week of 2025");
        assert_eq!(w("2020-12-31"), 53, "2020 has 53 weeks");
        assert_eq!(w("2019-12-31"), 1);
        assert_eq!(w("2016-01-01"), 53, "a Friday, the end of 2015's 53 weeks");
        assert_eq!(w("2022-01-01"), 52);
    }

    #[test]
    fn months_days_and_quarters_have_their_words() {
        assert_eq!(DatePart::Month.labels()["12"], "December");
        assert_eq!(DatePart::Weekday.labels()["7"], "Sunday");
        assert_eq!(DatePart::Quarter.labels()["3"], "Q3");
        assert!(DatePart::Year.labels().is_empty());
        assert_eq!(DatePart::Month.level(), "ordinal");
        assert_eq!(DatePart::Year.level(), "interval");
    }

    fn all_one_group(n: usize) -> Vec<Option<usize>> {
        vec![Some(0); n]
    }

    #[test]
    fn a_sample_has_the_size_asked_for_and_the_same_seed_gives_the_same_sample() {
        let flags = draw_sample(50, &all_one_group(50), SampleSize::Count(12), 7).unwrap();
        assert_eq!(flags.iter().filter(|f| **f == Some(true)).count(), 12);
        assert_eq!(flags.iter().filter(|f| **f == Some(false)).count(), 38);
        assert_eq!(flags, draw_sample(50, &all_one_group(50), SampleSize::Count(12), 7).unwrap(), "the seed fixes the sample");
        assert_ne!(flags, draw_sample(50, &all_one_group(50), SampleSize::Count(12), 8).unwrap(), "another seed draws another");
        let tenth = draw_sample(50, &all_one_group(50), SampleSize::Percent(10.0), 7).unwrap();
        assert_eq!(tenth.iter().filter(|f| **f == Some(true)).count(), 5);
    }

    #[test]
    fn a_case_that_cannot_be_drawn_is_left_out_and_a_percent_is_taken_within_each_group() {
        // Positions 0..9: groups a (0-5), b (6-7), and two cases with no group.
        let groups = vec![Some(0), Some(0), Some(0), Some(0), Some(0), Some(0), Some(1), Some(1), None, None];
        let flags = draw_sample(10, &groups, SampleSize::Percent(50.0), 3).unwrap();
        assert_eq!(flags[8], None);
        assert_eq!(flags[9], None);
        assert_eq!(flags[..6].iter().filter(|f| **f == Some(true)).count(), 3, "half of the six");
        assert_eq!(flags[6..8].iter().filter(|f| **f == Some(true)).count(), 1, "and half of the two, so the small group is in the sample");
    }

    #[test]
    fn a_count_shared_among_groups_adds_up_to_the_count() {
        assert_eq!(share_out(10, &[6, 3, 1]), [6, 3, 1]);
        assert_eq!(share_out(5, &[6, 3, 1]), [3, 2, 0], "3.0, 1.5, and 0.5: the larger remainder takes the spare case, and the earlier group wins a tie");
        assert_eq!(share_out(4, &[2, 2, 2]), [2, 1, 1]);
        assert_eq!(share_out(3, &[1, 1, 1, 1]), [1, 1, 1, 0]);
        assert_eq!(share_out(7, &[0, 0]), [0, 0]);
        for n in 0..=20 {
            assert_eq!(share_out(n, &[5, 7, 8]).iter().sum::<usize>(), n, "{n}");
        }
        let flags = draw_sample(10, &[Some(0), Some(0), Some(0), Some(0), Some(0), Some(0), Some(1), Some(1), Some(1), Some(1)], SampleSize::Count(5), 2).unwrap();
        assert_eq!(flags.iter().filter(|f| **f == Some(true)).count(), 5, "exactly the count asked for, drawn across both groups");
    }

    #[test]
    fn a_sample_that_cannot_be_drawn_says_why() {
        assert!(draw_sample(5, &all_one_group(5), SampleSize::Count(6), 1).unwrap_err().contains("cannot be drawn from 5"));
        assert!(draw_sample(5, &all_one_group(5), SampleSize::Count(0), 1).is_err());
        assert!(draw_sample(5, &all_one_group(5), SampleSize::Percent(0.0), 1).is_err());
        assert!(draw_sample(5, &all_one_group(5), SampleSize::Percent(101.0), 1).is_err());
        assert!(draw_sample(5, &[None; 5], SampleSize::Count(1), 1).is_err());
        assert!(draw_sample(5, &all_one_group(4), SampleSize::Count(1), 1).is_err());
    }

    #[test]
    fn a_group_is_summarized_the_way_r_summarizes_it() {
        let v = [Some(2.0), Some(4.0), None, Some(9.0)];
        assert_eq!(summarize(&v, GroupFunction::Mean, 4), Some(5.0));
        assert_eq!(summarize(&v, GroupFunction::Sum, 4), Some(15.0));
        assert_eq!(summarize(&v, GroupFunction::Median, 4), Some(4.0));
        assert_eq!(summarize(&v, GroupFunction::Min, 4), Some(2.0));
        assert_eq!(summarize(&v, GroupFunction::Max, 4), Some(9.0));
        assert_eq!(summarize(&v, GroupFunction::Count, 4), Some(3.0));
        assert_eq!(summarize(&v, GroupFunction::Cases, 4), Some(4.0));
        let sd = summarize(&v, GroupFunction::Sd, 4).unwrap();
        assert!((sd - 3.605_551_275_463_989).abs() < 1e-12, "{sd}");
        assert_eq!(summarize(&[Some(1.0)], GroupFunction::Sd, 1), None, "one value has no spread");
        assert_eq!(summarize(&[None, None], GroupFunction::Mean, 2), None);
        assert_eq!(summarize(&[None, None], GroupFunction::Count, 2), Some(0.0));
    }

    #[test]
    fn cases_are_put_in_order_by_the_keys_with_those_that_have_no_value_last() {
        let key = |numbers: Vec<Option<f64>>, descending| SortColumn { texts: vec![None; numbers.len()], numbers: Some(numbers), descending };
        let ages = key(vec![Some(30.0), None, Some(20.0), Some(30.0)], false);
        assert_eq!(sort_order(4, std::slice::from_ref(&ages)), [2, 0, 3, 1], "ties keep the order the cases already had; no value last");
        let down = key(vec![Some(30.0), None, Some(20.0), Some(30.0)], true);
        assert_eq!(sort_order(4, &[down]), [0, 3, 2, 1], "no value is last going down too");
        let words = SortColumn { numbers: None, texts: vec![Some("b".into()), Some("A".into()), None, Some("a".into())], descending: false };
        assert_eq!(sort_order(4, &[words]), [1, 3, 0, 2], "without regard to capitals, then a capital before the same word in small letters");
        let second = key(vec![Some(1.0), Some(1.0), Some(1.0), Some(0.0)], false);
        assert_eq!(sort_order(4, &[second, ages]), [3, 2, 0, 1], "the next key breaks the ties of the first");
        assert_eq!(sort_order(3, &[]), [0, 1, 2], "no key leaves the order as it is");
    }
}
