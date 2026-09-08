//! Build a Jianying draft from media + copy.

use std::path::{Path, PathBuf};

use crate::draft::{write_draft, DraftRequest, WrittenDraft};
use crate::media::{collect_visuals, probe_file_or_assume, MediaError, MediaFile};
use crate::script::parse_script;
use crate::timeline::{plan_timeline, EditOptions, PlanError, TimelinePlan};

#[derive(Debug)]
pub struct BuildInput {
    pub media_dir: PathBuf,
    pub script: Option<String>,
    pub name: String,
    pub output_dir: PathBuf,
    pub assume_seconds: Option<f64>,
    pub rewrite_from: Option<String>,
    pub rewrite_to: Option<String>,
    pub options: EditOptions,
}

#[derive(Debug)]
pub struct BuildResult {
    pub plan: TimelinePlan,
    pub draft: WrittenDraft,
    pub visuals: Vec<MediaFile>,
}

#[derive(Debug)]
pub enum BuildError {
    Media(MediaError),
    Plan(PlanError),
    Draft(crate::draft::DraftError),
    Io(std::io::Error),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Media(err) => write!(f, "{err}"),
            Self::Plan(err) => write!(f, "{err}"),
            Self::Draft(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for BuildError {}

impl From<MediaError> for BuildError {
    fn from(value: MediaError) -> Self {
        Self::Media(value)
    }
}

impl From<PlanError> for BuildError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

impl From<crate::draft::DraftError> for BuildError {
    fn from(value: crate::draft::DraftError) -> Self {
        Self::Draft(value)
    }
}

impl From<std::io::Error> for BuildError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn load_visuals(
    media_dir: &Path,
    assume_seconds: Option<f64>,
    image_hold_us: u64,
) -> Result<Vec<MediaFile>, BuildError> {
    let paths = collect_visuals(media_dir)?;
    let mut visuals = Vec::new();
    for path in paths {
        visuals.push(probe_file_or_assume(&path, image_hold_us, assume_seconds)?);
    }
    Ok(visuals)
}

pub fn build_project(input: &BuildInput) -> Result<BuildResult, BuildError> {
    let visuals = load_visuals(&input.media_dir, input.assume_seconds, 3_000_000)?;
    let captions = input
        .script
        .as_deref()
        .map(parse_script)
        .unwrap_or_default();
    let plan = plan_timeline(&visuals, &captions, &input.options)?;
    let draft = write_draft(&DraftRequest {
        name: &input.name,
        plan: &plan,
        visuals: &visuals,
        output_dir: &input.output_dir,
        rewrite_from: input.rewrite_from.as_deref(),
        rewrite_to: input.rewrite_to.as_deref(),
    })?;
    Ok(BuildResult {
        plan,
        draft,
        visuals,
    })
}

pub fn install_draft(written: &WrittenDraft, install_root: &Path) -> Result<PathBuf, BuildError> {
    std::fs::create_dir_all(install_root)?;
    let dest = install_root.join(
        written
            .folder
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("jianying-auto-draft")),
    );
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    copy_dir(&written.folder, &dest)?;
    Ok(dest)
}

fn copy_dir(src: &Path, dest: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dest.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
