//! The camera must never enter a column the client does not hold. A column
//! under the player is one the loader promised to have ready, and flying past
//! the front means the view is being served slower than it is being moved.
//!
//! It arms itself: nothing is loaded at launch and nothing is loaded for a
//! moment after a dimension change, so the check starts only once the column
//! under the player has actually arrived.

use crate::columns::ColumnStore;
use bevy::prelude::*;
use mcrs_voxel_math::{BlockPos, ColumnPos};
use mcrs_voxel_world::entity::physics::Transform as PhysicsTransform;

use crate::config::Guard;
use crate::player::Player;

pub struct ChunkGuardPlugin;

impl Plugin for ChunkGuardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkGuard>().add_systems(Update, check);
    }
}

#[derive(Default, Resource)]
pub struct ChunkGuard {
    armed: bool,
    reported: usize,
}

fn check(
    mut guard: ResMut<ChunkGuard>,
    store: Option<Res<ColumnStore>>,
    camera: Option<Single<&PhysicsTransform, With<Player>>>,
) {
    let Some(level) = crate::config::chunk_guard() else {
        return;
    };
    let (Some(store), Some(camera)) = (store, camera) else {
        return;
    };
    // A dimension change empties the store, and the player stands in nothing
    // until the first column of the new world lands.
    if store.is_empty() {
        guard.armed = false;
        return;
    }
    let feet = BlockPos::from(camera.translation);
    let column = ColumnPos::new(feet.x >> 4, feet.z >> 4);
    if store.holds(column) {
        guard.armed = true;
        return;
    }
    if !guard.armed {
        return;
    }
    guard.reported += 1;
    error!(
        col_x = column.x,
        col_z = column.z,
        x = feet.x,
        y = feet.y,
        z = feet.z,
        resident = store.len(),
        "the player is standing in a column the client does not hold"
    );
    if level == Guard::Panic {
        panic!(
            "chunk guard: the player is at {feet:?}, in column {},{}, which the client does \
             not hold ({} columns resident) — the camera has outrun the loader",
            column.x,
            column.z,
            store.len(),
        );
    }
}
