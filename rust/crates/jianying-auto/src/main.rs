#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::path::PathBuf;

use clap::Parser;
use jianying_auto::media::us_to_seconds;
use jianying_auto::project::{build_project, install_draft, BuildInput};
use jianying_auto::timeline::{CanvasPreset, EditOptions};

#[derive(Parser, Debug)]
#[command(
    name = "jianying-auto",
    about = "把视频素材和文案排进剪映时间线，生成可打开的草稿"
)]
struct Cli {
    /// 视频/图片所在文件夹（或单个文件）
    #[arg(long)]
    media: PathBuf,

    /// 文案文件（.txt / .srt / .md）
    #[arg(long)]
    script: Option<PathBuf>,

    /// 草稿名称
    #[arg(long, default_value = "自动成片")]
    name: String,

    /// 输出草稿目录
    #[arg(long, default_value = "./jianying-draft")]
    output: PathBuf,

    /// 画布比例：9:16 / 16:9 / 1:1
    #[arg(long, default_value = "9:16")]
    ratio: String,

    /// 每秒口播字数，用来估算每句字幕时长
    #[arg(long, default_value_t = 6.0)]
    chars_per_second: f64,

    /// 片头裁掉的秒数
    #[arg(long, default_value_t = 0.0)]
    head_trim: f64,

    /// 片尾裁掉的秒数
    #[arg(long, default_value_t = 0.0)]
    tail_trim: f64,

    /// 静音原声
    #[arg(long, default_value_t = false)]
    mute: bool,

    /// 不铺满画布（contain 而不是 cover）
    #[arg(long, default_value_t = false)]
    contain: bool,

    /// ffprobe 不可用时，假设每个视频这么长（秒）
    #[arg(long)]
    assume_seconds: Option<f64>,

    /// 把草稿复制进剪映草稿根目录（全局设置里的草稿位置）
    #[arg(long)]
    install: Option<PathBuf>,

    /// 素材路径替换：把生成环境路径改成剪映电脑上的路径
    #[arg(long)]
    rewrite_from: Option<String>,

    /// 与 --rewrite-from 配对
    #[arg(long)]
    rewrite_to: Option<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let canvas = CanvasPreset::from_ratio(&cli.ratio)
        .ok_or_else(|| format!("unsupported --ratio {} (use 9:16, 16:9, 1:1)", cli.ratio))?;
    let script = match cli.script {
        Some(path) => Some(std::fs::read_to_string(path)?),
        None => None,
    };
    let input = BuildInput {
        media_dir: cli.media,
        script,
        name: cli.name.clone(),
        output_dir: cli.output,
        assume_seconds: cli.assume_seconds,
        rewrite_from: cli.rewrite_from,
        rewrite_to: cli.rewrite_to,
        options: EditOptions {
            canvas,
            fps: 30,
            chars_per_second: cli.chars_per_second,
            min_caption_us: 1_200_000,
            head_trim_us: (cli.head_trim * 1_000_000.0).round() as u64,
            tail_trim_us: (cli.tail_trim * 1_000_000.0).round() as u64,
            mute_original: cli.mute,
            cover_fill: !cli.contain,
        },
    };
    let result = build_project(&input)?;
    println!(
        "已生成剪映草稿: {}\n  时长 {:.2}s · {} 个镜头 · {} 条字幕\n  工程: {}\n  时间线: {}",
        cli.name,
        us_to_seconds(result.plan.duration_us),
        result.plan.clips.len(),
        result.plan.captions.len(),
        result.draft.folder.display(),
        result.draft.plan_path.display()
    );
    println!(
        "用法: 把该文件夹放进剪映「草稿位置」后重启剪映；新版剪映若加密草稿，可用明文 JSON 作为导入底稿。"
    );
    if let Some(root) = cli.install {
        let dest = install_draft(&result.draft, &root)?;
        println!("已安装到: {}", dest.display());
    }
    Ok(())
}
