#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::path::PathBuf;

use clap::Parser;
use jiejin_watch::client::EastmoneyTransport;
use jiejin_watch::date::Date;
use jiejin_watch::{run_watch, WatchConfig};

#[derive(Parser, Debug)]
#[command(
    name = "jiejin-watch",
    about = "自动抓取解禁期前的 A 股限售解禁名单（东方财富公开数据）"
)]
struct Cli {
    /// 向后看多少天（解禁期前窗口，含今天）
    #[arg(long, default_value_t = 60)]
    days: u32,

    /// 按解禁市值取前 N 名
    #[arg(long, default_value_t = 100)]
    top: usize,

    /// 最低解禁市值（亿元），用于筛大额
    #[arg(long, default_value_t = 1.0)]
    min_yi: f64,

    /// 输出目录（可指向 Obsidian 库）
    #[arg(long, default_value = "./jiejin-out")]
    output: PathBuf,

    /// 基准日，默认今天；格式 YYYY-MM-DD 或 YYYYMMDD
    #[arg(long)]
    as_of: Option<String>,

    /// 同时抓取 Top 名单的解禁股东
    #[arg(long, default_value_t = false)]
    holders: bool,

    /// 不写全市场按日解禁日历
    #[arg(long, default_value_t = false)]
    no_calendar: bool,

    /// 只看某一只股票（6 位代码，可带 sh/sz 后缀）
    #[arg(long)]
    code: Option<String>,

    /// 「解禁临近」窗口天数
    #[arg(long, default_value_t = 7)]
    near: u32,

    /// 分页间隔毫秒，避免打太勤
    #[arg(long, default_value_t = 200)]
    pause_ms: u64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let as_of = match cli.as_of {
        Some(value) => value.parse()?,
        None => Date::today(),
    };
    let transport = EastmoneyTransport::new()?;
    let result = run_watch(
        &transport,
        &WatchConfig {
            as_of,
            days: cli.days,
            top: cli.top,
            min_yi: cli.min_yi,
            near_days: cli.near,
            code: cli.code,
            holders: cli.holders,
            calendar: !cli.no_calendar,
            pause_ms: cli.pause_ms,
            output_dir: cli.output,
        },
    )?;
    println!(
        "已生成解禁期前 Top{} 索引：{}\n  窗口 {} ～ {}\n  临近 {} 天内 {} 只 · 日历 {} 天\n  JSON {}",
        result.rows.len(),
        result.markdown_path.display(),
        result.start,
        result.end,
        cli.near,
        result
            .rows
            .iter()
            .filter(|row| {
                row.days_ahead >= 0
                    && row.days_ahead <= i32::try_from(cli.near).unwrap_or(i32::MAX)
            })
            .count(),
        result.calendar.len(),
        result.json_path.display()
    );
    Ok(())
}
