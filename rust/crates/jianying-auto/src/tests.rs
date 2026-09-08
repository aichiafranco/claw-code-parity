use std::path::PathBuf;

use crate::media::{seconds_to_us, MediaFile, MediaKind};
use crate::project::{build_project, BuildInput};
use crate::script::parse_script;
use crate::timeline::{plan_timeline, CanvasPreset, EditOptions};

fn clip(name: &str, seconds: f64, width: u32, height: u32) -> MediaFile {
    MediaFile {
        path: PathBuf::from(name),
        name: name.to_string(),
        kind: MediaKind::Video,
        duration_us: seconds_to_us(seconds),
        width,
        height,
    }
}

#[test]
fn split_chinese_script_into_captions() {
    let captions = parse_script("今天天气很好。我们去公园。\n\n记得带水。");
    assert_eq!(captions.len(), 3);
    assert_eq!(captions[0].text, "今天天气很好。");
    assert_eq!(captions[2].text, "记得带水。");
}

#[test]
fn parse_srt_timestamps() {
    let srt =
        "1\n00:00:01,000 --> 00:00:03,500\n你好世界\n\n2\n00:00:04,000 --> 00:00:06,000\n第二句\n";
    let captions = parse_script(srt);
    assert_eq!(captions.len(), 2);
    assert_eq!(captions[0].start_us, Some(1_000_000));
    assert_eq!(captions[0].end_us, Some(3_500_000));
    assert_eq!(captions[1].text, "第二句");
}

#[test]
fn script_driven_timeline_matches_caption_count() {
    let visuals = vec![
        clip("a.mp4", 10.0, 1080, 1920),
        clip("b.mp4", 8.0, 1080, 1920),
    ];
    let captions = parse_script("第一句文案。第二句更长一点的口播。第三句收尾。");
    let plan = plan_timeline(&visuals, &captions, &EditOptions::default()).unwrap();
    assert_eq!(plan.clips.len(), 3);
    assert_eq!(plan.captions.len(), 3);
    assert!(plan.duration_us >= 3_600_000);
    assert_eq!(
        plan.clips
            .last()
            .map(|clip| clip.timeline_start_us + clip.duration_us),
        Some(plan.duration_us)
    );
}

#[test]
fn head_trim_shifts_source_start() {
    let visuals = vec![clip("a.mp4", 10.0, 1920, 1080)];
    let options = EditOptions {
        head_trim_us: 1_000_000,
        tail_trim_us: 1_000_000,
        canvas: CanvasPreset::Landscape,
        ..EditOptions::default()
    };
    let plan = plan_timeline(&visuals, &[], &options).unwrap();
    assert_eq!(plan.clips.len(), 1);
    assert_eq!(plan.clips[0].source_start_us, 1_000_000);
    assert_eq!(plan.clips[0].duration_us, 8_000_000);
}

#[test]
fn writes_jianying_draft_files() {
    let dir = tempfile::tempdir().unwrap();
    let media = dir.path().join("clips");
    std::fs::create_dir_all(&media).unwrap();
    std::fs::write(media.join("shot01.mp4"), b"not-a-real-video").unwrap();
    let script = dir.path().join("script.txt");
    std::fs::write(&script, "开场介绍产品。然后展示细节。最后号召下单。").unwrap();
    let output = dir.path().join("draft");

    let result = build_project(&BuildInput {
        media_dir: media,
        script: Some(std::fs::read_to_string(script).unwrap()),
        name: "测试成片".into(),
        output_dir: output.clone(),
        assume_seconds: Some(6.0),
        rewrite_from: Some(dir.path().to_string_lossy().into_owned()),
        rewrite_to: Some("D:/clips".into()),
        options: EditOptions::default(),
    })
    .unwrap();

    assert!(output.join("draft_content.json").exists());
    assert!(output.join("draft_meta_info.json").exists());
    assert!(output.join("timeline.md").exists());
    assert_eq!(result.plan.captions.len(), 3);

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.join("draft_content.json")).unwrap())
            .unwrap();
    assert_eq!(content["canvas_config"]["width"], 1080);
    assert_eq!(content["tracks"].as_array().unwrap().len(), 2);
    assert_eq!(content["materials"]["videos"].as_array().unwrap().len(), 1);
    let path = content["materials"]["videos"][0]["path"].as_str().unwrap();
    assert!(path.starts_with("D:/clips"), "{path}");
}
