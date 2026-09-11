//! Calendar dates as `YYYY-MM-DD` without extra crates.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }
        if day == 0 || day > days_in_month(year, month) {
            return None;
        }
        Some(Self { year, month, day })
    }

    #[must_use]
    pub fn today() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        civil_from_days((i64::try_from(secs / 86_400).unwrap_or(0)) as i32)
    }

    #[must_use]
    pub fn add_days(self, days: i32) -> Self {
        civil_from_days(days_from_civil(self) + days)
    }

    #[must_use]
    pub fn days_until(self, other: Self) -> i32 {
        days_from_civil(other) - days_from_civil(self)
    }

    #[must_use]
    pub fn compact(self) -> String {
        format!("{:04}{:02}{:02}", self.year, self.month, self.day)
    }
}

impl Display for Date {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl FromStr for Date {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let digits = if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
            format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
        } else {
            s.replace('/', "-")
        };
        let parts: Vec<&str> = digits.split('-').collect();
        if parts.len() != 3 {
            return Err(format!("expected YYYY-MM-DD, got {s}"));
        }
        let year: i32 = parts[0].parse().map_err(|_| format!("bad year in {s}"))?;
        let month: u8 = parts[1].parse().map_err(|_| format!("bad month in {s}"))?;
        let day: u8 = parts[2].parse().map_err(|_| format!("bad day in {s}"))?;
        Self::new(year, month, day).ok_or_else(|| format!("invalid date {s}"))
    }
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Howard Hinnant's civil-from-days.
#[allow(clippy::similar_names)]
fn days_from_civil(date: Date) -> i32 {
    let y = if date.month <= 2 {
        date.year - 1
    } else {
        date.year
    };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = i32::from(if date.month > 2 {
        date.month - 3
    } else {
        date.month + 9
    });
    let doy = (153 * mp + 2) / 5 + i32::from(date.day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[allow(clippy::similar_names)]
fn civil_from_days(z: i32) -> Date {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    Date {
        year,
        month: m as u8,
        day: d as u8,
    }
}

#[must_use]
pub fn parse_datetime(value: &str) -> Option<Date> {
    let head = value.split(['T', ' ']).next().unwrap_or(value);
    Date::from_str(head).ok()
}
