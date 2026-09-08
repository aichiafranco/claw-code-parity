//! Import video clips and copy, plan a timeline, and write a Jianying draft.

#![recursion_limit = "256"]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

pub mod draft;
pub mod id;
pub mod media;
pub mod project;
pub mod script;
pub mod timeline;

pub use project::{build_project, BuildInput, BuildResult};
pub use timeline::{plan_timeline, CanvasPreset, EditOptions, TimelinePlan};

#[cfg(test)]
mod tests;
