use std::sync::atomic::AtomicU32;

use bevy::prelude::*;
use mcrs_minecraft_mesh::DrawKind;

use crate::shared_types::{DrawArgRow, DrawArgs};

/// Quads are drawn as two triangles each, one draw and no instancing: the cull adds six
/// vertices per surviving quad and the vertex shader finds its quad and corner in the
/// vertex index.
pub const VERTICES_PER_QUAD: u32 = 6;

pub(super) fn quad_list() -> DrawArgs {
    DrawArgs {
        instance_count: 1,
        ..default()
    }
}

/// The draw args as a frame starts: a list per draw kind for the first pass, one for the second,
/// then per draw kind the quads the first pass hid, the groups each pass's group cull kept and
/// the groups the first pass left quads of, and last the workgroups the kernels reading those three
/// lists dispatch, laid out as [`DISPATCHES`] expects.
pub fn args_reset() -> Vec<DrawArgs> {
    let dispatch = DrawArgs {
        vertex_count: 0,
        instance_count: 1,
        first_vertex: 1,
        first_instance: 0,
    };
    let mut rows = vec![DrawArgs::default(); DrawArgRow::TOTAL_SLOTS];
    for row in [DrawArgRow::FirstPassDraws, DrawArgRow::SecondPassDraws] {
        let first = row.first_slot();
        if let Some(draws) = rows.get_mut(first..first + DrawKind::COUNT) {
            draws.fill(quad_list());
        }
    }
    for row in [
        DrawArgRow::FirstPassQuadDispatch,
        DrawArgRow::SecondPassQuadDispatch,
        DrawArgRow::CandidateDispatch,
    ] {
        let first = row.first_slot();
        if let Some(dispatches) = rows.get_mut(first..first + DrawKind::COUNT) {
            dispatches.fill(dispatch);
        }
    }
    rows
}

const _: () = {
    assert!(
        DrawArgRow::FirstPassQuadDispatch.index() + 1 == DrawArgRow::SecondPassQuadDispatch.index()
    );
    assert!(
        DrawArgRow::SecondPassQuadDispatch.index() + 1 == DrawArgRow::CandidateDispatch.index()
    );
};

/// Where the dispatch sizes start in the args, and how many bytes they take: the quad cull's of
/// each pass, then the second pass's group cull's.
pub(super) const DISPATCHES: u64 = DrawArgRow::FirstPassQuadDispatch.first_offset_bytes();
pub(super) const DISPATCH_BYTES: u64 = DrawArgRow::byte_len(3);

/// Byte offset inside the copied dispatch buffer for indirect compute dispatch of `row` and `draw`.
pub(super) const fn dispatch_offset(row: DrawArgRow, draw: DrawKind) -> Option<u64> {
    match row {
        DrawArgRow::FirstPassQuadDispatch
        | DrawArgRow::SecondPassQuadDispatch
        | DrawArgRow::CandidateDispatch => row.offset_bytes(draw).checked_sub(DISPATCHES),
        _ => None,
    }
}

/// What the CPU issued this frame.
#[derive(Resource, Default)]
pub struct FrameCounts {
    pub terrain_draws: AtomicU32,
    pub sky_draws: AtomicU32,
    pub upload_bytes: AtomicU32,
}
