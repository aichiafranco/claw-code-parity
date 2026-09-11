//! Eastmoney lockup-expiry records.

use serde::{Deserialize, Serialize};

use crate::date::{parse_datetime, Date};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnlockEvent {
    pub code: String,
    pub name: String,
    pub free_date: String,
    /// 解禁数量（万股）
    pub able_free_shares_wan: f64,
    /// 实际解禁数量（万股）
    pub current_free_shares_wan: f64,
    /// 实际解禁市值（万元）
    pub lift_market_cap_wan: f64,
    /// 占解禁前流通市值比例（小数）
    pub free_ratio: f64,
    pub prev_close: Option<f64>,
    pub b20_change_pct: Option<f64>,
    pub share_type: String,
    pub holder_count: Option<u32>,
}

impl UnlockEvent {
    #[must_use]
    pub fn lift_yi(&self) -> f64 {
        self.lift_market_cap_wan / 10_000.0
    }

    #[must_use]
    pub fn free_date_parsed(&self) -> Option<Date> {
        parse_datetime(&self.free_date)
    }

    #[must_use]
    pub fn days_ahead(&self, today: Date) -> Option<i32> {
        self.free_date_parsed().map(|d| today.days_until(d))
    }

    #[must_use]
    pub fn quote_url(&self) -> String {
        let prefix = match self.code.as_bytes().first() {
            Some(b'6') => "sh",
            Some(b'8' | b'4') => "bj",
            _ => "sz",
        };
        format!("https://quote.eastmoney.com/{prefix}{}.html", self.code)
    }

    #[must_use]
    pub fn unlock_url(&self) -> String {
        format!("https://data.eastmoney.com/dxf/q/{}.html", self.code)
    }

    #[must_use]
    pub fn notice_url(&self) -> String {
        format!(
            "https://data.eastmoney.com/notices/stock/{}.html",
            self.code
        )
    }

    #[must_use]
    pub fn news_url(&self) -> String {
        format!(
            "https://so.eastmoney.com/news/s?keyword={}",
            urlencoding_minimal(&format!("{} 解禁", self.name))
        )
    }
}

fn urlencoding_minimal(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            for byte in ch.encode_utf8(&mut [0; 4]).as_bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Holder {
    pub name: String,
    pub add_listing_shares: f64,
    pub actual_listed_shares: f64,
    pub add_listing_cap: f64,
    pub lock_month: Option<f64>,
    pub share_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RankedUnlock {
    pub rank: usize,
    pub event: UnlockEvent,
    pub days_ahead: i32,
    pub holders: Vec<Holder>,
}

pub fn parse_event(row: &serde_json::Value) -> Option<UnlockEvent> {
    let code = row.get("SECURITY_CODE")?.as_str()?.to_string();
    let name = row
        .get("SECURITY_NAME_ABBR")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let free_date = row.get("FREE_DATE")?.as_str()?.to_string();
    Some(UnlockEvent {
        code,
        name,
        free_date,
        able_free_shares_wan: json_f64(row.get("ABLE_FREE_SHARES")),
        current_free_shares_wan: json_f64(row.get("CURRENT_FREE_SHARES")),
        lift_market_cap_wan: json_f64(row.get("LIFT_MARKET_CAP")),
        free_ratio: json_f64(row.get("FREE_RATIO")),
        prev_close: row.get("NEW").and_then(as_f64),
        b20_change_pct: row.get("B20_ADJCHRATE").and_then(as_f64),
        share_type: row
            .get("FREE_SHARES_TYPE")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        holder_count: row
            .get("BATCH_HOLDER_NUM")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
    })
}

pub fn parse_holder(row: &serde_json::Value) -> Option<Holder> {
    Some(Holder {
        name: row
            .get("LIMITED_HOLDER_NAME")
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        add_listing_shares: json_f64(row.get("ADD_LISTING_SHARES")),
        actual_listed_shares: json_f64(row.get("ACTUAL_LISTED_SHARES")),
        add_listing_cap: json_f64(row.get("ADD_LISTING_CAP")),
        lock_month: row.get("LOCK_MONTH").and_then(as_f64),
        share_type: row
            .get("FREE_SHARES_TYPE")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

fn as_f64(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_i64().map(|n| n as f64))
}

fn json_f64(value: Option<&serde_json::Value>) -> f64 {
    value.and_then(as_f64).unwrap_or(0.0)
}

#[must_use]
pub fn rank_events(
    mut events: Vec<UnlockEvent>,
    today: Date,
    min_yi: f64,
    top: usize,
) -> Vec<RankedUnlock> {
    events.retain(|event| {
        event
            .days_ahead(today)
            .is_some_and(|days| days >= 0 && event.lift_yi() + f64::EPSILON >= min_yi)
    });
    events.sort_by(|a, b| {
        b.lift_yi()
            .partial_cmp(&a.lift_yi())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.free_date.cmp(&b.free_date))
            .then_with(|| a.code.cmp(&b.code))
    });
    events
        .into_iter()
        .take(top)
        .enumerate()
        .map(|(index, event)| {
            let days_ahead = event.days_ahead(today).unwrap_or(0);
            RankedUnlock {
                rank: index + 1,
                event,
                days_ahead,
                holders: Vec::new(),
            }
        })
        .collect()
}
