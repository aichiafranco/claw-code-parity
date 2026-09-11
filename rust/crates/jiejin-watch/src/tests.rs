use crate::client::{fetch_window, Transport};
use crate::date::Date;
use crate::model::{parse_event, rank_events, UnlockEvent};
use crate::report::render_markdown;
use crate::write_reports;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::VecDeque;

struct ScriptedTransport {
    responses: RefCell<VecDeque<Value>>,
}

impl Transport for ScriptedTransport {
    fn get_json(&self, _params: &[(&str, String)]) -> Result<Value, crate::client::FetchError> {
        self.responses
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| crate::client::FetchError::Upstream("no scripted response".into()))
    }
}

fn sample_row() -> Value {
    json!({
        "SECURITY_CODE": "300008",
        "SECURITY_NAME_ABBR": "天海防务",
        "FREE_DATE": "2026-09-20 00:00:00",
        "CURRENT_FREE_SHARES": 6300.0,
        "ABLE_FREE_SHARES": 6300.0,
        "LIFT_MARKET_CAP": 41895.0,
        "FREE_RATIO": 0.036_466_720_447,
        "NEW": 6.65,
        "B20_ADJCHRATE": 10.58,
        "A20_ADJCHRATE": -2.06,
        "FREE_SHARES_TYPE": "定向增发机构配售股份",
        "TOTAL_RATIO": 0.036,
        "NON_FREE_SHARES": 42.6,
        "BATCH_HOLDER_NUM": 1
    })
}

#[test]
fn date_add_and_parse() {
    let d = Date::new(2026, 9, 8).unwrap();
    assert_eq!(d.to_string(), "2026-09-08");
    assert_eq!(d.add_days(3).to_string(), "2026-09-11");
    assert_eq!(d.days_until(Date::new(2026, 9, 11).unwrap()), 3);
    assert_eq!("20260908".parse::<Date>().unwrap(), d);
}

#[test]
fn parse_and_rank_large_unlocks() {
    let small = UnlockEvent {
        code: "600000".into(),
        name: "浦发银行".into(),
        free_date: "2026-09-15".into(),
        able_free_shares_wan: 10.0,
        current_free_shares_wan: 10.0,
        lift_market_cap_wan: 800.0,
        free_ratio: 0.01,
        prev_close: None,
        b20_change_pct: None,
        share_type: "首发原股东限售股份".into(),
        holder_count: None,
    };
    let large = parse_event(&sample_row()).unwrap();
    let today = Date::new(2026, 9, 11).unwrap();
    let ranked = rank_events(vec![small, large], today, 1.0, 100);
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].event.code, "300008");
    assert!((ranked[0].event.lift_yi() - 4.1895).abs() < 0.000_1);
    assert_eq!(ranked[0].days_ahead, 9);
}

#[test]
fn fetch_window_reads_scripted_pages() {
    let transport = ScriptedTransport {
        responses: RefCell::new(VecDeque::from([json!({
            "success": true,
            "result": {
                "pages": 1,
                "count": 1,
                "data": [sample_row()]
            }
        })])),
    };
    let start = Date::new(2026, 9, 11).unwrap();
    let events = fetch_window(
        &transport,
        start,
        start.add_days(30),
        std::time::Duration::from_millis(0),
    )
    .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].name, "天海防务");
}

#[test]
fn writes_obsidian_style_index() {
    let today = Date::new(2026, 9, 8).unwrap();
    let event = parse_event(&sample_row()).unwrap();
    let rows = rank_events(vec![event], today, 0.0, 10);
    let dir = std::env::temp_dir().join("jiejin-watch-test");
    let _ = std::fs::remove_dir_all(&dir);
    let result = write_reports(today, today, today.add_days(60), 1.0, &dir, &rows).unwrap();
    assert!(result
        .markdown_path
        .ends_with("20260908-大额解禁Top1图谱索引_1只.md"));
    let md = std::fs::read_to_string(&result.markdown_path).unwrap();
    assert!(md.contains("天海防务"));
    assert!(md.contains("data.eastmoney.com/dxf/q/300008.html"));
    let rendered = render_markdown(today, today, today.add_days(60), 1.0, &rows);
    assert!(rendered.contains("解禁期前"));
}

#[test]
fn past_unlocks_are_dropped() {
    let past = UnlockEvent {
        code: "000001".into(),
        name: "平安银行".into(),
        free_date: "2026-01-01".into(),
        able_free_shares_wan: 1.0,
        current_free_shares_wan: 1.0,
        lift_market_cap_wan: 50_000.0,
        free_ratio: 0.1,
        prev_close: None,
        b20_change_pct: None,
        share_type: "首发原股东限售股份".into(),
        holder_count: None,
    };
    let today = Date::new(2026, 9, 11).unwrap();
    assert!(rank_events(vec![past], today, 0.0, 10).is_empty());
}
