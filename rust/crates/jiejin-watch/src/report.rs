//! Markdown / JSON reports for Obsidian-style unlock indexes.

use crate::date::Date;
use crate::model::RankedUnlock;

#[must_use]
pub fn render_markdown(
    title_date: Date,
    start: Date,
    end: Date,
    min_yi: f64,
    rows: &[RankedUnlock],
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
    out.push_str(&format!("- 抓取日: {title_date}\n"));
    out.push_str("- 说明: 市值来自东财 `LIFT_MARKET_CAP`（万元），本表换算为亿元。链接指向东财个股解禁页、公告页与「简称+解禁」资讯搜索，便于解禁前跟踪。\n\n");
    out.push_str("| 排名 | 代码 | 简称 | 解禁日 | 距今(天) | 解禁市值(亿) | 占流通% | 类型 | 解禁页 | 公告 | 资讯 |\n");
    out.push_str("| ---: | --- | --- | --- | ---: | ---: | ---: | --- | --- | --- | --- |\n");
    for row in rows {
        let date = row
            .event
            .free_date_parsed()
            .map_or_else(|| row.event.free_date.clone(), |d| d.to_string());
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
                row.event.code, row.event.name, row.event.free_date
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

pub fn render_json(rows: &[RankedUnlock]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(rows)
}
