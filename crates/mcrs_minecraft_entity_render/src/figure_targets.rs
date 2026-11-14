use bevy::prelude::*;
use wgpu::{CompareFunction, TextureFormat};

/// The attachments of the render pass that takes the figure draw. A resource of the render world
/// that the host inserts; with none, the figure view draws nothing.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FigureTargets {
    /// The format of the colour attachment.
    pub color: TextureFormat,
    /// The format of the depth attachment.
    pub depth: TextureFormat,
    /// The depth compare of the pass. With `Greater` or `GreaterEqual`, the viewer looks from
    /// clip depth 1 toward clip depth 0; with any other compare, from 0 toward 1.
    pub depth_compare: CompareFunction,
    /// The sample count of the attachments.
    pub sample_count: u32,
}
