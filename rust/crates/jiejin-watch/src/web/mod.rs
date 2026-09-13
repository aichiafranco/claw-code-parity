//! Local control page and JSON API for jiejin-watch.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::client::EastmoneyTransport;
use crate::date::Date;
use crate::model::{DailyLift, RankedUnlock};
use crate::report::{format_free_date, render_markdown};
use crate::{run_watch, WatchConfig, WatchResult};

const INDEX_HTML: &str = include_str!("index.html");

#[derive(Debug, Clone, Deserialize)]
pub struct FetchRequest {
    pub days: Option<u32>,
    pub top: Option<usize>,
    pub min_yi: Option<f64>,
    pub near: Option<u32>,
    pub as_of: Option<String>,
    pub code: Option<String>,
    pub holders: Option<bool>,
    pub calendar: Option<bool>,
    pub output: Option<String>,
    pub pause_ms: Option<u64>,
}

#[derive(Debug)]
pub enum RequestError {
    BadRequest(String),
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "{msg}"),
        }
    }
}

pub fn serve(bind: &str, default_output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::http(bind).map_err(|err| format!("cannot bind {bind}: {err}"))?;
    eprintln!("jiejin-watch 操作页: http://{bind}/");
    let busy = Arc::new(AtomicBool::new(false));
    for request in server.incoming_requests() {
        if let Err(err) = handle(default_output, busy.as_ref(), request) {
            eprintln!("request error: {err}");
        }
    }
    Ok(())
}

fn handle(
    default_output: &Path,
    busy: &AtomicBool,
    request: Request,
) -> Result<(), Box<dyn std::error::Error>> {
    let method = request.method().clone();
    let url = request.url().split('?').next().unwrap_or("/").to_string();
    match (&method, url.as_str()) {
        (Method::Get, "/" | "/index.html") => {
            respond(request, 200, "text/html; charset=utf-8", INDEX_HTML)
        }
        (Method::Get, "/favicon.ico") => respond(request, 204, "image/x-icon", ""),
        (Method::Get, "/api/defaults") => {
            let body = serde_json::to_string(&json!({
                "as_of": Date::today().to_string(),
                "days": 60,
                "top": 100,
                "min_yi": 1.0,
                "near": 7,
                "output": default_output.display().to_string(),
            }))?;
            respond(request, 200, "application/json; charset=utf-8", &body)
        }
        (Method::Post, "/api/fetch") => handle_fetch(default_output, busy, request),
        _ => respond(request, 404, "text/plain; charset=utf-8", "not found"),
    }
}

fn handle_fetch(
    default_output: &Path,
    busy: &AtomicBool,
    mut request: Request,
) -> Result<(), Box<dyn std::error::Error>> {
    if busy.swap(true, Ordering::SeqCst) {
        return respond(
            request,
            429,
            "application/json; charset=utf-8",
            r#"{"error":"正在抓取，请稍后再试"}"#,
        );
    }
    let mut raw = String::new();
    let outcome: Result<Value, RequestError> = (|| {
        std::io::Read::read_to_string(request.as_reader(), &mut raw)
            .map_err(|err| RequestError::BadRequest(err.to_string()))?;
        let body: FetchRequest = serde_json::from_str(&raw)
            .map_err(|err| RequestError::BadRequest(format!("JSON 无效: {err}")))?;
        let config = build_config(&body, default_output)?;
        let transport =
            EastmoneyTransport::new().map_err(|err| RequestError::BadRequest(err.to_string()))?;
        let result = run_watch(&transport, &config)
            .map_err(|err| RequestError::BadRequest(err.to_string()))?;
        Ok(fetch_payload_with_meta(
            config.as_of,
            config.min_yi,
            config.near_days,
            &result,
        ))
    })();
    busy.store(false, Ordering::SeqCst);
    match outcome {
        Ok(payload) => {
            let body = serde_json::to_string(&payload)?;
            respond(request, 200, "application/json; charset=utf-8", &body)
        }
        Err(RequestError::BadRequest(msg)) => {
            let body = serde_json::to_string(&json!({ "error": msg }))?;
            respond(request, 400, "application/json; charset=utf-8", &body)
        }
    }
}

pub fn build_config(
    body: &FetchRequest,
    default_output: &Path,
) -> Result<WatchConfig, RequestError> {
    let days = body.days.unwrap_or(60);
    if !(1..=365).contains(&days) {
        return Err(RequestError::BadRequest("--days 需在 1～365".into()));
    }
    let top = body.top.unwrap_or(100);
    if !(1..=500).contains(&top) {
        return Err(RequestError::BadRequest("--top 需在 1～500".into()));
    }
    let min_yi = body.min_yi.unwrap_or(1.0);
    if !min_yi.is_finite() || min_yi < 0.0 {
        return Err(RequestError::BadRequest("最低市值不能为负".into()));
    }
    let near_days = body.near.unwrap_or(7);
    if near_days > 365 {
        return Err(RequestError::BadRequest("临近天数过大".into()));
    }
    let pause_ms = body.pause_ms.unwrap_or(200);
    if pause_ms > 5_000 {
        return Err(RequestError::BadRequest("分页间隔过大".into()));
    }
    let as_of = match body
        .as_of
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(value) => value
            .parse()
            .map_err(|err| RequestError::BadRequest(format!("基准日无效: {err}")))?,
        None => Date::today(),
    };
    let code = body
        .code
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let output_dir = body
        .output
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(|| default_output.to_path_buf(), PathBuf::from);
    Ok(WatchConfig {
        as_of,
        days,
        top,
        min_yi,
        near_days,
        code,
        holders: body.holders.unwrap_or(false),
        calendar: body.calendar.unwrap_or(true),
        pause_ms,
        output_dir,
    })
}

#[must_use]
pub fn fetch_payload_with_meta(
    as_of: Date,
    min_yi: f64,
    near_days: u32,
    result: &WatchResult,
) -> Value {
    json!({
        "start": result.start.to_string(),
        "end": result.end.to_string(),
        "markdown_path": result.markdown_path.display().to_string(),
        "json_path": result.json_path.display().to_string(),
        "markdown": render_markdown(
            as_of,
            result.start,
            result.end,
            min_yi,
            near_days,
            &result.rows,
            &result.calendar,
        ),
        "rows": result.rows.iter().map(row_json).collect::<Vec<_>>(),
        "calendar": result.calendar.iter().map(day_json).collect::<Vec<_>>(),
    })
}

fn row_json(row: &RankedUnlock) -> Value {
    json!({
        "rank": row.rank,
        "code": row.event.code,
        "name": row.event.name,
        "free_date": format_free_date(&row.event.free_date),
        "days_ahead": row.days_ahead,
        "lift_yi": (row.event.lift_yi() * 100.0).round() / 100.0,
        "free_ratio_pct": (row.event.free_ratio * 10_000.0).round() / 100.0,
        "share_type": row.event.share_type,
        "unlock_url": row.event.unlock_url(),
        "notice_url": row.event.notice_url(),
        "news_url": row.event.news_url(),
        "quote_url": row.event.quote_url(),
        "holders": row.holders,
    })
}

fn day_json(day: &DailyLift) -> Value {
    json!({
        "date": format_free_date(&day.date),
        "org_num": day.org_num,
        "lift_yi": (day.lift_yi() * 100.0).round() / 100.0,
    })
}

fn respond(
    request: Request,
    status: u16,
    content_type: &str,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let header = Header::from_bytes(b"Content-Type", content_type.as_bytes())
        .expect("content-type header is ASCII");
    let response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(header);
    request.respond(response)?;
    Ok(())
}

#[must_use]
pub fn index_html() -> &'static str {
    INDEX_HTML
}
