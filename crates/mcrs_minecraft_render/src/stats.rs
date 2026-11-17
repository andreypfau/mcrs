use std::sync::atomic::AtomicU32;

use bevy::prelude::*;
use mcrs_minecraft_mesh::DrawKind;

use crate::shared_types::{DrawArgRow, DrawArgs};

pub const VERTICES_PER_QUAD: u32 = 6;

pub(super) fn quad_list() -> DrawArgs {
    DrawArgs {
        instance_count: 1,
        ..default()
    }
}

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

pub(super) const DISPATCHES: u64 = DrawArgRow::FirstPassQuadDispatch.first_offset_bytes();
pub(super) const DISPATCH_BYTES: u64 = DrawArgRow::byte_len(3);

pub(super) const fn dispatch_offset(row: DrawArgRow, draw: DrawKind) -> Option<u64> {
    match row {
        DrawArgRow::FirstPassQuadDispatch
        | DrawArgRow::SecondPassQuadDispatch
        | DrawArgRow::CandidateDispatch => row.offset_bytes(draw).checked_sub(DISPATCHES),
        _ => None,
    }
}

#[derive(Resource, Default)]
pub struct FrameCounts {
    pub terrain_draws: AtomicU32,
    pub sky_draws: AtomicU32,
    pub upload_bytes: AtomicU32,
}
