//! Eastmoney datacenter client for lockup-expiry reports.

use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::date::Date;
use crate::model::{parse_event, parse_holder, Holder, UnlockEvent};

pub const DATACENTER: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";
const LIFT_STAGE_COLUMNS: &str = "SECURITY_CODE,SECURITY_NAME_ABBR,FREE_DATE,CURRENT_FREE_SHARES,\
ABLE_FREE_SHARES,LIFT_MARKET_CAP,FREE_RATIO,NEW,B20_ADJCHRATE,A20_ADJCHRATE,FREE_SHARES_TYPE,\
TOTAL_RATIO,NON_FREE_SHARES,BATCH_HOLDER_NUM";
const HOLDER_COLUMNS: &str = "LIMITED_HOLDER_NAME,ADD_LISTING_SHARES,ACTUAL_LISTED_SHARES,\
ADD_LISTING_CAP,LOCK_MONTH,RESIDUAL_LIMITED_SHARES,FREE_SHARES_TYPE,PLAN_FEATURE";

#[derive(Debug)]
pub enum FetchError {
    Http(String),
    Json(String),
    Upstream(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(msg) | Self::Json(msg) | Self::Upstream(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for FetchError {}

pub trait Transport {
    fn get_json(&self, params: &[(&str, String)]) -> Result<Value, FetchError>;
}

pub struct EastmoneyTransport {
    client: reqwest::blocking::Client,
}

impl EastmoneyTransport {
    pub fn new() -> Result<Self, FetchError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("jiejin-watch/0.1 (research; +https://data.eastmoney.com/dxf/detail.html)")
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|err| FetchError::Http(err.to_string()))?;
        Ok(Self { client })
    }
}

impl Transport for EastmoneyTransport {
    fn get_json(&self, params: &[(&str, String)]) -> Result<Value, FetchError> {
        let response = self
            .client
            .get(DATACENTER)
            .header("Referer", "https://data.eastmoney.com/dxf/detail.html")
            .query(params)
            .send()
            .map_err(|err| FetchError::Http(err.to_string()))?;
        if !response.status().is_success() {
            return Err(FetchError::Http(format!("HTTP {}", response.status())));
        }
        response
            .json()
            .map_err(|err| FetchError::Json(err.to_string()))
    }
}

pub fn fetch_window<T: Transport>(
    transport: &T,
    start: Date,
    end: Date,
    pause: Duration,
) -> Result<Vec<UnlockEvent>, FetchError> {
    let mut page = 1_u32;
    let mut events = Vec::new();
    let mut pages = 1_u32;
    while page <= pages {
        let body = transport.get_json(&[
            ("sortColumns", "FREE_DATE,CURRENT_FREE_SHARES".into()),
            ("sortTypes", "1,1".into()),
            ("pageSize", "500".into()),
            ("pageNumber", page.to_string()),
            ("reportName", "RPT_LIFT_STAGE".into()),
            ("columns", LIFT_STAGE_COLUMNS.into()),
            ("source", "WEB".into()),
            ("client", "WEB".into()),
            (
                "filter",
                format!("(FREE_DATE>='{start}')(FREE_DATE<='{end}')"),
            ),
        ])?;
        if body.get("success") != Some(&Value::Bool(true)) {
            return Err(FetchError::Upstream(
                body.get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("eastmoney request failed")
                    .to_string(),
            ));
        }
        let result = body
            .get("result")
            .ok_or_else(|| FetchError::Upstream("missing result".into()))?;
        pages = result
            .get("pages")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1) as u32;
        if let Some(rows) = result.get("data").and_then(Value::as_array) {
            for row in rows {
                if let Some(event) = parse_event(row) {
                    events.push(event);
                }
            }
        }
        page += 1;
        if page <= pages {
            thread::sleep(pause);
        }
    }
    Ok(events)
}

pub fn fetch_holders<T: Transport>(
    transport: &T,
    code: &str,
    free_date: Date,
) -> Result<Vec<Holder>, FetchError> {
    let body = transport.get_json(&[
        ("sortColumns", "ADD_LISTING_SHARES".into()),
        ("sortTypes", "-1".into()),
        ("pageSize", "50".into()),
        ("pageNumber", "1".into()),
        ("reportName", "RPT_LIFT_GD".into()),
        ("columns", HOLDER_COLUMNS.into()),
        ("source", "WEB".into()),
        ("client", "WEB".into()),
        (
            "filter",
            format!("(SECURITY_CODE=\"{code}\")(FREE_DATE='{free_date}')"),
        ),
    ])?;
    if body.get("success") != Some(&Value::Bool(true)) {
        return Ok(Vec::new());
    }
    let Some(rows) = body.pointer("/result/data").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    Ok(rows.iter().filter_map(parse_holder).collect())
}
