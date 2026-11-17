use bevy::prelude::*;
use wgpu::{CompareFunction, TextureFormat};

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FigureTargets {
    pub color: TextureFormat,
    pub depth: TextureFormat,
    pub depth_compare: CompareFunction,
    pub sample_count: u32,
}
