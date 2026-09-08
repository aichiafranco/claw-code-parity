//! Jianying 5.9-compatible plaintext draft writer.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::id::{hex_id, upper_guid};
use crate::media::MediaFile;
use crate::timeline::TimelinePlan;

pub struct DraftRequest<'a> {
    pub name: &'a str,
    pub plan: &'a TimelinePlan,
    pub visuals: &'a [MediaFile],
    pub output_dir: &'a Path,
    pub rewrite_from: Option<&'a str>,
    pub rewrite_to: Option<&'a str>,
}

#[derive(Debug)]
pub struct WrittenDraft {
    pub folder: PathBuf,
    pub content_path: PathBuf,
    pub meta_path: PathBuf,
    pub plan_path: PathBuf,
}

#[derive(Debug)]
pub enum DraftError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for DraftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for DraftError {}

impl From<std::io::Error> for DraftError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for DraftError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn write_draft(request: &DraftRequest<'_>) -> Result<WrittenDraft, DraftError> {
    std::fs::create_dir_all(request.output_dir)?;
    let content = build_draft_content(request);
    let meta = build_meta_info(request, content["duration"].as_u64().unwrap_or(0));
    let content_path = request.output_dir.join("draft_content.json");
    let meta_path = request.output_dir.join("draft_meta_info.json");
    let plan_path = request.output_dir.join("timeline.md");
    std::fs::write(&content_path, serde_json::to_string_pretty(&content)?)?;
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
    std::fs::write(
        &plan_path,
        crate::timeline::render_plan_markdown(request.plan, request.name),
    )?;
    std::fs::write(
        request.output_dir.join("timeline.json"),
        serde_json::to_string_pretty(request.plan)?,
    )?;
    Ok(WrittenDraft {
        folder: request.output_dir.to_path_buf(),
        content_path,
        meta_path,
        plan_path,
    })
}

fn rewrite_path(path: &str, from: Option<&str>, to: Option<&str>) -> String {
    match (from, to) {
        (Some(from), Some(to)) if !from.is_empty() => path.replace(from, to),
        _ => path.to_string(),
    }
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}

fn timerange(start: u64, duration: u64) -> Value {
    json!({ "start": start, "duration": duration })
}

fn empty_materials() -> Value {
    let keys = [
        "ai_translates",
        "audio_balances",
        "audio_effects",
        "audio_fades",
        "audio_track_indexes",
        "audios",
        "beats",
        "canvases",
        "chromas",
        "color_curves",
        "digital_humans",
        "drafts",
        "effects",
        "flowers",
        "green_screens",
        "handwrites",
        "hsl",
        "images",
        "log_color_wheels",
        "loudnesses",
        "manual_deformations",
        "masks",
        "material_animations",
        "material_colors",
        "multi_language_refs",
        "placeholders",
        "plugin_effects",
        "primary_color_wheels",
        "realtime_denoises",
        "shapes",
        "smart_crops",
        "smart_relights",
        "sound_channel_mappings",
        "speeds",
        "stickers",
        "tail_leaders",
        "text_templates",
        "texts",
        "time_marks",
        "transitions",
        "video_effects",
        "video_trackings",
        "videos",
        "vocal_beautifys",
        "vocal_separations",
    ];
    let mut map = serde_json::Map::new();
    for key in keys {
        map.insert(key.to_string(), Value::Array(Vec::new()));
    }
    Value::Object(map)
}

fn video_material(file: &MediaFile, material_id: &str, path: &str) -> Value {
    let kind = if file.kind == crate::media::MediaKind::Image {
        "photo"
    } else {
        "video"
    };
    json!({
        "audio_fade": null,
        "category_id": "",
        "category_name": "local",
        "check_flag": 63487,
        "crop": {
            "upper_left_x": 0.0,
            "upper_left_y": 0.0,
            "upper_right_x": 1.0,
            "upper_right_y": 0.0,
            "lower_left_x": 0.0,
            "lower_left_y": 1.0,
            "lower_right_x": 1.0,
            "lower_right_y": 1.0
        },
        "crop_ratio": "free",
        "crop_scale": 1.0,
        "duration": file.duration_us,
        "height": file.height,
        "id": material_id,
        "local_material_id": "",
        "material_id": material_id,
        "material_name": file.name,
        "media_path": "",
        "path": path,
        "type": kind,
        "width": file.width
    })
}

fn speed_material(id: &str, speed: f64) -> Value {
    json!({
        "curve_speed": null,
        "id": id,
        "mode": 0,
        "speed": speed,
        "type": "speed"
    })
}

fn canvas_material(id: &str) -> Value {
    json!({
        "album_image": "",
        "blur": 0.0,
        "color": "",
        "id": id,
        "image": "",
        "image_id": "",
        "image_name": "",
        "source_platform": 0,
        "team_id": "",
        "type": "canvas_color"
    })
}

fn sound_channel_mapping(id: &str) -> Value {
    json!({
        "audio_channel_mapping": 0,
        "id": id,
        "is_config_open": false,
        "type": "none"
    })
}

fn vocal_separation(id: &str) -> Value {
    json!({
        "choice": 0,
        "id": id,
        "production_path": "",
        "time_range": null,
        "type": "vocal_separation"
    })
}

fn video_segment(
    material_id: &str,
    extra_refs: &[String],
    clip: &crate::timeline::PlannedClip,
    render_index: u32,
) -> Value {
    json!({
        "cartoon": false,
        "clip": {
            "alpha": 1.0,
            "flip": { "horizontal": false, "vertical": false },
            "rotation": 0.0,
            "scale": { "x": clip.scale, "y": clip.scale },
            "transform": { "x": 0.0, "y": 0.0 }
        },
        "common_keyframes": [],
        "enable_adjust": true,
        "enable_color_correct_adjust": false,
        "enable_color_curves": true,
        "enable_color_match_adjust": false,
        "enable_color_wheels": true,
        "enable_lut": true,
        "enable_smart_color_adjust": false,
        "extra_material_refs": extra_refs,
        "hdr_settings": { "intensity": 1.0, "mode": 1, "nits": 1000 },
        "id": hex_id(),
        "intensifies_audio": false,
        "is_placeholder": false,
        "is_tone_modify": false,
        "keyframe_refs": [],
        "last_nonzero_volume": 1.0,
        "material_id": material_id,
        "render_index": render_index,
        "render_timerange": timerange(clip.timeline_start_us, clip.duration_us),
        "reverse": false,
        "source_timerange": timerange(clip.source_start_us, clip.source_duration_us),
        "speed": clip.speed,
        "target_timerange": timerange(clip.timeline_start_us, clip.duration_us),
        "template_id": "",
        "template_scene": "default",
        "track_attribute": 0,
        "track_render_index": 0,
        "uniform_scale": { "on": true, "value": 1.0 },
        "visible": true,
        "volume": clip.volume
    })
}

fn text_material(id: &str, text: &str) -> Value {
    let content = json!({
        "styles": [{
            "fill": {
                "alpha": 1.0,
                "content": {
                    "render_type": "solid",
                    "solid": { "alpha": 1.0, "color": [1.0, 1.0, 1.0] }
                }
            },
            "range": [0, text.chars().count()],
            "size": 8.0,
            "bold": false,
            "italic": false,
            "underline": false,
            "strokes": [{
                "content": { "solid": { "alpha": 1.0, "color": [0.0, 0.0, 0.0] } },
                "width": 0.08
            }]
        }],
        "text": text
    });
    json!({
        "alignment": 1,
        "border_color": "",
        "border_width": 0.0,
        "check_flag": 15,
        "content": content.to_string(),
        "font_category_id": "",
        "font_category_name": "",
        "font_id": "",
        "font_name": "",
        "font_path": "",
        "font_resource_id": "",
        "font_size": 8.0,
        "font_source_platform": 0,
        "force_apply_line_max_width": false,
        "global_alpha": 1.0,
        "group_id": "",
        "has_shadow": false,
        "id": id,
        "initial_scale": 1.0,
        "is_rich_text": false,
        "italic_degree": 0,
        "ktv_color": "",
        "layer_weight": 1,
        "letter_spacing": 0.0,
        "line_feed": 1,
        "line_max_width": 0.82,
        "line_spacing": 0.02,
        "name": "",
        "original_size": [],
        "preset_id": "",
        "recognize_task_id": "",
        "recognize_type": 0,
        "relevance_segment": [],
        "shadow_alpha": 0.9,
        "shadow_angle": -45.0,
        "shadow_color": "",
        "shadow_distance": 5.0,
        "shadow_point": { "x": 0.636_396_103_067_892_7, "y": -0.636_396_103_067_892_7 },
        "shadow_smoothing": 0.45,
        "shape_clip_x": false,
        "shape_clip_y": false,
        "style_name": "",
        "sub_type": 0,
        "subtitle_keywords": null,
        "text_alpha": 1.0,
        "text_color": "#FFFFFF",
        "text_preset_resource_id": "",
        "text_size": 30,
        "text_to_audio_ids": [],
        "tts_auto_update": false,
        "type": "subtitle",
        "typesetting": 0,
        "underline": false,
        "underline_offset": 0.22,
        "underline_width": 0.05,
        "use_effect_default_color": true,
        "words": []
    })
}

fn text_segment(material_id: &str, start: u64, duration: u64, render_index: u32) -> Value {
    json!({
        "clip": {
            "alpha": 1.0,
            "flip": { "horizontal": false, "vertical": false },
            "rotation": 0.0,
            "scale": { "x": 1.0, "y": 1.0 },
            "transform": { "x": 0.0, "y": -0.8 }
        },
        "common_keyframes": [],
        "enable_adjust": true,
        "enable_color_correct_adjust": false,
        "enable_color_curves": true,
        "enable_color_match_adjust": false,
        "enable_color_wheels": true,
        "enable_lut": true,
        "enable_smart_color_adjust": false,
        "extra_material_refs": [],
        "id": hex_id(),
        "is_placeholder": false,
        "is_tone_modify": false,
        "keyframe_refs": [],
        "last_nonzero_volume": 1.0,
        "material_id": material_id,
        "render_index": render_index,
        "render_timerange": timerange(start, duration),
        "reverse": false,
        "source_timerange": null,
        "speed": 1.0,
        "target_timerange": timerange(start, duration),
        "track_attribute": 0,
        "track_render_index": 0,
        "uniform_scale": { "on": true, "value": 1.0 },
        "visible": true,
        "volume": 1.0
    })
}

fn track(track_type: &str, name: &str, render_index: u32, segments: Vec<Value>) -> Value {
    let mut object = serde_json::Map::new();
    object.insert("attribute".into(), json!(0));
    object.insert("flag".into(), json!(0));
    object.insert("id".into(), json!(hex_id()));
    object.insert("is_default_name".into(), json!(true));
    object.insert("name".into(), json!(name));
    object.insert("segments".into(), Value::Array(segments));
    object.insert("type".into(), json!(track_type));
    object.insert("render_index".into(), json!(render_index));
    Value::Object(object)
}

#[allow(clippy::too_many_lines)]
fn build_draft_content(request: &DraftRequest<'_>) -> Value {
    let mut materials = empty_materials();
    let mut video_materials = Vec::new();
    let mut speeds = Vec::new();
    let mut canvases = Vec::new();
    let mut sound_maps = Vec::new();
    let mut vocals = Vec::new();
    let mut texts = Vec::new();
    let mut video_segments = Vec::new();

    let mut material_ids = Vec::new();
    for file in request.visuals {
        let id = hex_id();
        let path = rewrite_path(
            &file.path.display().to_string(),
            request.rewrite_from,
            request.rewrite_to,
        );
        video_materials.push(video_material(file, &id, &path));
        material_ids.push(id);
    }

    for clip in &request.plan.clips {
        let material_id = material_ids
            .get(clip.media_index)
            .cloned()
            .unwrap_or_else(hex_id);
        let speed_id = hex_id();
        let canvas_id = hex_id();
        let sound_id = hex_id();
        let vocal_id = hex_id();
        speeds.push(speed_material(&speed_id, clip.speed));
        canvases.push(canvas_material(&canvas_id));
        sound_maps.push(sound_channel_mapping(&sound_id));
        vocals.push(vocal_separation(&vocal_id));
        let refs = vec![speed_id, canvas_id, sound_id, vocal_id];
        video_segments.push(video_segment(&material_id, &refs, clip, 0));
    }

    let mut text_segments = Vec::new();
    for caption in &request.plan.captions {
        let id = hex_id();
        texts.push(text_material(&id, &caption.text));
        text_segments.push(text_segment(
            &id,
            caption.timeline_start_us,
            caption.duration_us,
            1,
        ));
    }

    materials["videos"] = Value::Array(video_materials);
    materials["speeds"] = Value::Array(speeds);
    materials["canvases"] = Value::Array(canvases);
    materials["sound_channel_mappings"] = Value::Array(sound_maps);
    materials["vocal_separations"] = Value::Array(vocals);
    materials["texts"] = Value::Array(texts);

    let mut tracks = vec![track("video", "video", 0, video_segments)];
    if !text_segments.is_empty() {
        tracks.push(track("text", "text", 1, text_segments));
    }

    json!({
        "canvas_config": {
            "height": request.plan.canvas_height,
            "ratio": "original",
            "width": request.plan.canvas_width
        },
        "color_space": 0,
        "config": {
            "adjust_max_index": 1,
            "attachment_info": [],
            "combination_max_index": 1,
            "export_range": null,
            "extract_audio_last_index": 1,
            "lyrics_recognition_id": "",
            "lyrics_sync": true,
            "lyrics_taskinfo": [],
            "maintrack_adsorb": true,
            "material_save_mode": 0,
            "multi_language_current": "none",
            "multi_language_list": [],
            "multi_language_main": "none",
            "multi_language_mode": "none",
            "original_sound_last_index": 1,
            "record_audio_last_index": 1,
            "sticker_max_index": 1,
            "subtitle_keywords_config": null,
            "subtitle_recognition_id": "",
            "subtitle_sync": true,
            "subtitle_taskinfo": [],
            "system_font_list": [],
            "video_mute": false,
            "zoom_info_params": null
        },
        "cover": null,
        "create_time": now_us(),
        "duration": request.plan.duration_us,
        "extra_info": null,
        "fps": f64::from(request.plan.fps),
        "free_render_index_mode_on": false,
        "group_container": null,
        "id": upper_guid(),
        "keyframe_graph_list": [],
        "keyframes": {
            "adjusts": [],
            "audios": [],
            "effects": [],
            "filters": [],
            "handwrites": [],
            "stickers": [],
            "texts": [],
            "videos": []
        },
        "last_modified_platform": {
            "app_id": 3704,
            "app_source": "lv",
            "app_version": "5.9.0",
            "os": "windows"
        },
        "platform": {
            "app_id": 3704,
            "app_source": "lv",
            "app_version": "5.9.0",
            "os": "windows"
        },
        "materials": materials,
        "mutable_config": null,
        "name": request.name,
        "new_version": "110.0.0",
        "relationships": [],
        "render_index_track_mode_on": false,
        "retouch_cover": null,
        "source": "default",
        "static_cover_image_path": "",
        "time_marks": null,
        "tracks": tracks,
        "update_time": now_us(),
        "version": 360_000
    })
}

#[allow(clippy::too_many_lines)]
fn build_meta_info(request: &DraftRequest<'_>, duration_us: u64) -> Value {
    let fold = rewrite_path(
        &request.output_dir.display().to_string(),
        request.rewrite_from,
        request.rewrite_to,
    );
    let mut material_values = Vec::new();
    for file in request.visuals {
        let path = rewrite_path(
            &file.path.display().to_string(),
            request.rewrite_from,
            request.rewrite_to,
        );
        material_values.push(json!({
            "create_time": now_us(),
            "duration": file.duration_us,
            "extra_info": file.name,
            "file_Path": path,
            "height": file.height,
            "id": hex_id(),
            "import_time": now_us(),
            "import_time_ms": now_us() / 1000,
            "item_source": 1,
            "md5": "",
            "metetype": if file.kind == crate::media::MediaKind::Image { "photo" } else { "video" },
            "roughcut_time_range": { "duration": -1, "start": -1 },
            "sub_time_range": { "duration": -1, "start": -1 },
            "type": 0,
            "width": file.width
        }));
    }
    json!({
        "cloud_package_completed_time": "",
        "draft_cloud_capcut_purchase_info": "",
        "draft_cloud_last_action_download": false,
        "draft_cloud_materials": [],
        "draft_cloud_purchase_info": "",
        "draft_cloud_template_id": "",
        "draft_cloud_tutorial_info": "",
        "draft_cloud_videocut_purchase_info": "",
        "draft_cover": "",
        "draft_deeplink_url": "",
        "draft_enterprise_info": {
            "draft_enterprise_extra": "",
            "draft_enterprise_id": "",
            "draft_enterprise_name": "",
            "enterprise_material": []
        },
        "draft_fold_path": fold,
        "draft_id": upper_guid(),
        "draft_is_ai_packaging_used": false,
        "draft_is_ai_shorts": false,
        "draft_is_ai_translate": false,
        "draft_is_article_video_draft": false,
        "draft_is_from_deeplink": "false",
        "draft_is_invisible": false,
        "draft_materials": [
            { "type": 0, "value": material_values },
            { "type": 1, "value": [] },
            { "type": 2, "value": [] },
            { "type": 3, "value": [] },
            { "type": 6, "value": [] },
            { "type": 7, "value": [] },
            { "type": 8, "value": [] }
        ],
        "draft_materials_copied_info": [],
        "draft_name": request.name,
        "draft_new_version": "",
        "draft_removable_storage_device": "",
        "draft_root_path": fold,
        "draft_segment_extra_info": [],
        "draft_type": "",
        "tm_draft_cloud_completed": "",
        "tm_draft_cloud_modified": 0,
        "tm_draft_create": now_us(),
        "tm_draft_modified": now_us(),
        "tm_draft_removed": 0,
        "tm_duration": duration_us
    })
}
