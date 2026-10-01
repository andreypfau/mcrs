use std::sync::atomic::AtomicU32;

use bevy::prelude::*;
use bevy::render::render_resource::*;

use mcrs_minecraft_mesh::STREAMS;

pub(super) const DRAW_ARGS_SIZE: u64 = size_of::<DrawArgs>() as u64;

/// Quads are drawn as two triangles each, one draw and no instancing: the cull adds six
/// vertices per surviving quad and the vertex shader finds its quad and corner in the
/// vertex index.
pub const VERTICES_PER_QUAD: u32 = 6;

pub type DrawArgs = DrawIndirectArgs;

pub(super) fn quad_list() -> DrawArgs {
    DrawArgs {
        instance_count: 1,
        ..default()
    }
}

/// The draw args as a frame starts: a list per stream for the first pass, one per stream for
/// the second, then per stream the quads the first pass hid, the groups each pass's group cull
/// kept and the groups the first pass left quads of, and last the workgroups the kernels
/// reading those three lists dispatch, laid out as `DISPATCHES` expects.
pub fn args_reset() -> Vec<DrawArgs> {
    let dispatch = DrawArgs {
        vertex_count: 0,
        instance_count: 1,
        first_vertex: 1,
        first_instance: 0,
    };
    (0..2 * STREAMS)
        .map(|_| quad_list())
        .chain((0..4 * STREAMS).map(|_| DrawArgs::default()))
        .chain((0..3 * STREAMS).map(|_| dispatch))
        .collect()
}

/// Where the dispatch sizes start in the args, and how many bytes they take: the quad cull's of
/// each pass, then the second pass's group cull's.
pub(super) const DISPATCHES: u64 = (6 * STREAMS) as u64 * DRAW_ARGS_SIZE;
pub(super) const DISPATCH_BYTES: u64 = (3 * STREAMS) as u64 * DRAW_ARGS_SIZE;

/// What the CPU issued this frame.
#[derive(Resource, Default)]
pub struct FrameCounts {
    pub terrain_draws: AtomicU32,
    pub sky_draws: AtomicU32,
    pub upload_bytes: AtomicU32,
}
