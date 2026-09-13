//! Fetch upcoming A-share lockup expiries before the unlock date.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

pub mod client;
pub mod date;
pub mod model;
pub mod report;
pub mod web;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::client::{fetch_calendar, fetch_holders, fetch_window, FetchError, Transport};
use crate::date::Date;
use crate::model::{rank_events, DailyLift, RankedUnlock};
use crate::report::{render_json, render_markdown};

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub as_of: Date,
    pub days: u32,
    pub top: usize,
    pub min_yi: f64,
    pub near_days: u32,
    pub code: Option<String>,
    pub holders: bool,
    pub calendar: bool,
    pub pause_ms: u64,
    pub output_dir: PathBuf,
}

#[derive(Debug)]
pub struct WatchResult {
    pub start: Date,
    pub end: Date,
    pub rows: Vec<RankedUnlock>,
    pub calendar: Vec<DailyLift>,
    pub markdown_path: PathBuf,
    pub json_path: PathBuf,
}

#[derive(Debug)]
pub enum WatchError {
    Fetch(FetchError),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fetch(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for WatchError {}

impl From<FetchError> for WatchError {
    fn from(value: FetchError) -> Self {
        Self::Fetch(value)
    }
}

impl From<std::io::Error> for WatchError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for WatchError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn run_watch<T: Transport>(
    transport: &T,
    config: &WatchConfig,
) -> Result<WatchResult, WatchError> {
    let start = config.as_of;
    let end = config
        .as_of
        .add_days(i32::try_from(config.days).unwrap_or(i32::MAX));
    let events = fetch_window(
        transport,
        start,
        end,
        Duration::from_millis(config.pause_ms),
        config.code.as_deref(),
    )?;
    let mut rows = rank_events(events, config.as_of, config.min_yi, config.top);
    if config.holders {
        for row in &mut rows {
            if let Some(date) = row.event.free_date_parsed() {
                row.holders = fetch_holders(transport, &row.event.code, date).unwrap_or_default();
                std::thread::sleep(Duration::from_millis(config.pause_ms));
            }
        }
    }
    let calendar = if config.calendar {
        fetch_calendar(
            transport,
            start,
            end,
            Duration::from_millis(config.pause_ms),
        )
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    write_reports(
        config.as_of,
        start,
        end,
        config.min_yi,
        config.near_days,
        &config.output_dir,
        &rows,
        &calendar,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn write_reports(
    as_of: Date,
    start: Date,
    end: Date,
    min_yi: f64,
    near_days: u32,
    output_dir: &Path,
    rows: &[RankedUnlock],
    calendar: &[DailyLift],
) -> Result<WatchResult, WatchError> {
    std::fs::create_dir_all(output_dir)?;
    let stem = format!(
        "{}-大额解禁Top{}图谱索引_{}只",
        as_of.compact(),
        rows.len(),
        rows.len()
    );
    let markdown_path = output_dir.join(format!("{stem}.md"));
    let json_path = output_dir.join(format!("{stem}.json"));
    std::fs::write(
        &markdown_path,
        render_markdown(as_of, start, end, min_yi, near_days, rows, calendar),
    )?;
    std::fs::write(&json_path, render_json(rows, calendar)?)?;
    Ok(WatchResult {
        start,
        end,
        rows: rows.to_vec(),
        calendar: calendar.to_vec(),
        markdown_path,
        json_path,
    })
}

#[cfg(test)]
mod tests;
