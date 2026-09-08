//! Plan a timeline from clips + captions.

use serde::Serialize;

use crate::media::{us_to_seconds, MediaFile, MediaKind};
use crate::script::{character_weight, Caption};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasPreset {
    Landscape,
    Portrait,
    Square,
}

impl CanvasPreset {
    #[must_use]
    pub fn from_ratio(value: &str) -> Option<Self> {
        match value.trim() {
            "16:9" | "landscape" => Some(Self::Landscape),
            "9:16" | "portrait" => Some(Self::Portrait),
            "1:1" | "square" => Some(Self::Square),
            _ => None,
        }
    }

    #[must_use]
    pub fn size(self) -> (u32, u32) {
        match self {
            Self::Landscape => (1920, 1080),
            Self::Portrait => (1080, 1920),
            Self::Square => (1080, 1080),
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Landscape => "16:9",
            Self::Portrait => "9:16",
            Self::Square => "1:1",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditOptions {
    pub canvas: CanvasPreset,
    pub fps: u32,
    pub chars_per_second: f64,
    pub min_caption_us: u64,
    pub head_trim_us: u64,
    pub tail_trim_us: u64,
    pub mute_original: bool,
    pub cover_fill: bool,
}

impl Default for EditOptions {
    fn default() -> Self {
        Self {
            canvas: CanvasPreset::Portrait,
            fps: 30,
            chars_per_second: 6.0,
            min_caption_us: 1_200_000,
            head_trim_us: 0,
            tail_trim_us: 0,
            mute_original: false,
            cover_fill: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedClip {
    pub media_index: usize,
    pub path: String,
    pub source_start_us: u64,
    pub source_duration_us: u64,
    pub timeline_start_us: u64,
    pub duration_us: u64,
    pub speed: f64,
    pub volume: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedCaption {
    pub text: String,
    pub timeline_start_us: u64,
    pub duration_us: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelinePlan {
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub fps: u32,
    pub duration_us: u64,
    pub clips: Vec<PlannedClip>,
    pub captions: Vec<PlannedCaption>,
}

#[derive(Debug)]
pub enum PlanError {
    NoVisuals,
    Empty,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoVisuals => write!(f, "no video or image clips to place on the timeline"),
            Self::Empty => write!(f, "timeline would be empty"),
        }
    }
}

impl std::error::Error for PlanError {}

pub fn plan_timeline(
    visuals: &[MediaFile],
    captions: &[Caption],
    options: &EditOptions,
) -> Result<TimelinePlan, PlanError> {
    if visuals.is_empty() {
        return Err(PlanError::NoVisuals);
    }
    let (canvas_width, canvas_height) = options.canvas.size();
    if captions
        .iter()
        .all(|caption| caption.start_us.is_some() && caption.end_us.is_some())
        && !captions.is_empty()
    {
        return Ok(plan_from_srt(
            visuals,
            captions,
            options,
            canvas_width,
            canvas_height,
        ));
    }
    if captions.is_empty() {
        return plan_clips_only(visuals, options, canvas_width, canvas_height);
    }
    Ok(plan_script_driven(
        visuals,
        captions,
        options,
        canvas_width,
        canvas_height,
    ))
}

fn usable_range(file: &MediaFile, options: &EditOptions) -> (u64, u64) {
    let duration = if file.kind == MediaKind::Image {
        file.duration_us
    } else {
        file.duration_us
            .saturating_sub(options.head_trim_us)
            .saturating_sub(options.tail_trim_us)
    };
    let start = if file.kind == MediaKind::Image {
        0
    } else {
        options.head_trim_us.min(file.duration_us)
    };
    (start, duration.max(1))
}

fn cover_scale(file: &MediaFile, canvas_w: u32, canvas_h: u32, cover_fill: bool) -> f64 {
    if !cover_fill {
        return 1.0;
    }
    let src_w = f64::from(file.width.max(1));
    let src_h = f64::from(file.height.max(1));
    let canvas_w = f64::from(canvas_w);
    let canvas_h = f64::from(canvas_h);
    (canvas_w / src_w).max(canvas_h / src_h).max(1.0)
}

fn volume(options: &EditOptions) -> f64 {
    if options.mute_original {
        0.0
    } else {
        1.0
    }
}

fn caption_duration(caption: &Caption, options: &EditOptions) -> u64 {
    if let (Some(start), Some(end)) = (caption.start_us, caption.end_us) {
        return end.saturating_sub(start).max(options.min_caption_us);
    }
    let seconds = character_weight(&caption.text) as f64 / options.chars_per_second.max(0.5);
    seconds_to_us_clamped(seconds, options.min_caption_us)
}

fn seconds_to_us_clamped(seconds: f64, min_us: u64) -> u64 {
    ((seconds * 1_000_000.0).round() as u64).max(min_us)
}

fn plan_clips_only(
    visuals: &[MediaFile],
    options: &EditOptions,
    canvas_width: u32,
    canvas_height: u32,
) -> Result<TimelinePlan, PlanError> {
    let mut clips = Vec::new();
    let mut cursor = 0_u64;
    for (index, file) in visuals.iter().enumerate() {
        let (source_start, usable) = usable_range(file, options);
        clips.push(PlannedClip {
            media_index: index,
            path: file.path.display().to_string(),
            source_start_us: source_start,
            source_duration_us: usable,
            timeline_start_us: cursor,
            duration_us: usable,
            speed: 1.0,
            volume: volume(options),
            scale: cover_scale(file, canvas_width, canvas_height, options.cover_fill),
        });
        cursor += usable;
    }
    if cursor == 0 {
        return Err(PlanError::Empty);
    }
    Ok(TimelinePlan {
        canvas_width,
        canvas_height,
        fps: options.fps,
        duration_us: cursor,
        clips,
        captions: Vec::new(),
    })
}

fn plan_script_driven(
    visuals: &[MediaFile],
    captions: &[Caption],
    options: &EditOptions,
    canvas_width: u32,
    canvas_height: u32,
) -> TimelinePlan {
    let mut remaining: Vec<(usize, u64, u64)> = visuals
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let (start, usable) = usable_range(file, options);
            (index, start, usable)
        })
        .collect();
    let mut clips = Vec::new();
    let mut planned_captions = Vec::new();
    let mut cursor = 0_u64;
    let mut media_cursor = 0_usize;

    for caption in captions {
        let needed = caption_duration(caption, options);
        let (media_index, source_start, take, speed) =
            take_from_pool(&mut remaining, &mut media_cursor, visuals.len(), needed);
        let file = &visuals[media_index];
        clips.push(PlannedClip {
            media_index,
            path: file.path.display().to_string(),
            source_start_us: source_start,
            source_duration_us: take,
            timeline_start_us: cursor,
            duration_us: needed,
            speed,
            volume: volume(options),
            scale: cover_scale(file, canvas_width, canvas_height, options.cover_fill),
        });
        planned_captions.push(PlannedCaption {
            text: caption.text.clone(),
            timeline_start_us: cursor,
            duration_us: needed,
        });
        cursor += needed;
    }

    TimelinePlan {
        canvas_width,
        canvas_height,
        fps: options.fps,
        duration_us: cursor,
        clips,
        captions: planned_captions,
    }
}

fn take_from_pool(
    remaining: &mut [(usize, u64, u64)],
    media_cursor: &mut usize,
    visual_count: usize,
    needed: u64,
) -> (usize, u64, u64, f64) {
    for _ in 0..visual_count {
        let slot = &mut remaining[*media_cursor % remaining.len()];
        *media_cursor += 1;
        if slot.2 == 0 {
            continue;
        }
        let take = slot.2.min(needed);
        let source_start = slot.1;
        slot.1 += take;
        slot.2 -= take;
        let speed = take as f64 / needed as f64;
        return (slot.0, source_start, take, speed.max(0.05));
    }
    // All clips exhausted: loop the first visual at high speed.
    let (index, start, leftover) = remaining[0];
    let take = leftover.max(needed / 8).max(1);
    (index, start, take, take as f64 / needed as f64)
}

fn plan_from_srt(
    visuals: &[MediaFile],
    captions: &[Caption],
    options: &EditOptions,
    canvas_width: u32,
    canvas_height: u32,
) -> TimelinePlan {
    let mut remaining: Vec<(usize, u64, u64)> = visuals
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let (start, usable) = usable_range(file, options);
            (index, start, usable)
        })
        .collect();
    let mut clips = Vec::new();
    let mut planned_captions = Vec::new();
    let mut media_cursor = 0_usize;
    let mut duration_us = 0_u64;

    for caption in captions {
        let start = caption.start_us.unwrap_or(duration_us);
        let end = caption.end_us.unwrap_or(start + options.min_caption_us);
        let needed = end.saturating_sub(start).max(1);
        let (media_index, source_start, take, speed) =
            take_from_pool(&mut remaining, &mut media_cursor, visuals.len(), needed);
        let file = &visuals[media_index];
        clips.push(PlannedClip {
            media_index,
            path: file.path.display().to_string(),
            source_start_us: source_start,
            source_duration_us: take,
            timeline_start_us: start,
            duration_us: needed,
            speed,
            volume: volume(options),
            scale: cover_scale(file, canvas_width, canvas_height, options.cover_fill),
        });
        planned_captions.push(PlannedCaption {
            text: caption.text.clone(),
            timeline_start_us: start,
            duration_us: needed,
        });
        duration_us = duration_us.max(start + needed);
    }

    TimelinePlan {
        canvas_width,
        canvas_height,
        fps: options.fps,
        duration_us,
        clips,
        captions: planned_captions,
    }
}

#[must_use]
pub fn render_plan_markdown(plan: &TimelinePlan, name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {name}\n\n"));
    out.push_str(&format!(
        "- 画布: {}x{} @ {}fps\n- 总时长: {:.2}s\n- 视频片段: {}\n- 字幕: {}\n\n",
        plan.canvas_width,
        plan.canvas_height,
        plan.fps,
        us_to_seconds(plan.duration_us),
        plan.clips.len(),
        plan.captions.len()
    ));
    out.push_str("| 时间 | 时长 | 素材 | 速度 | 字幕 |\n| --- | --- | --- | --- | --- |\n");
    for (index, clip) in plan.clips.iter().enumerate() {
        let caption = plan
            .captions
            .get(index)
            .map_or("", |caption| caption.text.as_str());
        let name = std::path::Path::new(&clip.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&clip.path);
        out.push_str(&format!(
            "| {:.2}s | {:.2}s | {name} | {:.2}x | {caption} |\n",
            us_to_seconds(clip.timeline_start_us),
            us_to_seconds(clip.duration_us),
            clip.speed
        ));
    }
    out
}
