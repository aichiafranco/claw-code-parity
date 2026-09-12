//! Markdown / JSON reports for Obsidian-style unlock indexes.

use crate::date::{parse_datetime, Date};
use crate::model::{DailyLift, RankedUnlock};

#[must_use]
pub fn render_markdown(
    title_date: Date,
    start: Date,
    end: Date,
    min_yi: f64,
    near_days: u32,
    rows: &[RankedUnlock],
    calendar: &[DailyLift],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# {compact}-大额解禁Top{}图谱索引_{}只\n\n",
        rows.len(),
        rows.len(),
        compact = title_date.compact()
    ));
    out.push_str("- 数据源: [东方财富 · 限售股解禁](https://data.eastmoney.com/dxf/detail.html)\n");
    out.push_str(&format!("- 窗口: {start} ～ {end}（解禁期前，含当日）\n"));
    out.push_str(&format!("- 门槛: 解禁市值 ≥ {min_yi:.2} 亿元\n"));
    out.push_str(&format!("- 抓取日: {title_date}（北京时间）\n"));
    out.push_str("- 说明: 市值来自东财公开 JSON（万元），本表换算为亿元。链接指向个股解禁页、公告页与「简称+解禁」资讯搜索。\n\n");

    let near_limit = i32::try_from(near_days).unwrap_or(i32::MAX);
    let near: Vec<&RankedUnlock> = rows
        .iter()
        .filter(|row| row.days_ahead >= 0 && row.days_ahead <= near_limit)
        .collect();
    if !near.is_empty() {
        out.push_str(&format!("## 解禁临近（{near_days} 天内）\n\n"));
        out.push_str("| 距今 | 代码 | 简称 | 解禁日 | 解禁市值(亿) | 类型 |\n| ---: | --- | --- | --- | ---: | --- |\n");
        for row in near {
            let date = format_date(&row.event.free_date);
            out.push_str(&format!(
                "| {} | {} | {} | {date} | {:.2} | {} |\n",
                row.days_ahead,
                row.event.code,
                row.event.name,
                row.event.lift_yi(),
                row.event.share_type
            ));
        }
        out.push('\n');
    }

    if !calendar.is_empty() {
        out.push_str("## 全市场解禁日历\n\n");
        out.push_str("| 日期 | 家数 | 解禁市值(亿) |\n| --- | ---: | ---: |\n");
        for day in calendar {
            out.push_str(&format!(
                "| {} | {} | {:.2} |\n",
                format_date(&day.date),
                day.org_num,
                day.lift_yi()
            ));
        }
        out.push('\n');
    }

    out.push_str("## 大额解禁个股\n\n");
    out.push_str("| 排名 | 代码 | 简称 | 解禁日 | 距今(天) | 解禁市值(亿) | 占流通% | 类型 | 解禁页 | 公告 | 资讯 |\n");
    out.push_str("| ---: | --- | --- | --- | ---: | ---: | ---: | --- | --- | --- | --- |\n");
    for row in rows {
        let date = format_date(&row.event.free_date);
        out.push_str(&format!(
            "| {} | {} | {} | {date} | {} | {:.2} | {:.2} | {} | [解禁]({}) | [公告]({}) | [资讯]({}) |\n",
            row.rank,
            row.event.code,
            row.event.name,
            row.days_ahead,
            row.event.lift_yi(),
            row.event.free_ratio * 100.0,
            row.event.share_type,
            row.event.unlock_url(),
            row.event.notice_url(),
            row.event.news_url()
        ));
    }
    if rows.iter().any(|row| !row.holders.is_empty()) {
        out.push_str("\n## 解禁股东（Top 名单）\n");
        for row in rows {
            if row.holders.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "\n### {} {}（{}）\n",
                row.event.code,
                row.event.name,
                format_date(&row.event.free_date)
            ));
            out.push_str("| 股东 | 解禁股数 | 解禁市值(元) | 锁定期(月) | 类型 |\n| --- | ---: | ---: | ---: | --- |\n");
            for holder in &row.holders {
                out.push_str(&format!(
                    "| {} | {:.0} | {:.0} | {} | {} |\n",
                    holder.name,
                    holder.add_listing_shares,
                    holder.add_listing_cap,
                    holder
                        .lock_month
                        .map_or_else(|| "-".into(), |m| format!("{m:.0}")),
                    holder.share_type
                ));
            }
        }
    }
    out.push('\n');
    out
}

fn format_date(value: &str) -> String {
    parse_datetime(value).map_or_else(|| value.to_string(), |d| d.to_string())
}

pub fn render_json(
    rows: &[RankedUnlock],
    calendar: &[DailyLift],
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&serde_json::json!({
        "rows": rows,
        "calendar": calendar
    }))
}
