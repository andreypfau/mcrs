use bevy::prelude::*;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_engine_world::entity::physics::Transform as PhysicsTransform;
use mcrs_voxel::{BlockPos, ColumnPos};

use super::knobs::Guard;
use mcrs_minecraft_client_world::player::Player;

pub struct ChunkGuardPlugin {
    pub level: Guard,
}

impl Plugin for ChunkGuardPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChunkGuard {
            level: self.level,
            armed: false,
            reported: 0,
        })
        .add_systems(Update, check);
    }
}

#[derive(Resource)]
pub struct ChunkGuard {
    level: Guard,
    armed: bool,
    reported: usize,
}

fn check(
    mut guard: ResMut<ChunkGuard>,
    store: Option<Res<ColumnStore>>,
    camera: Single<&PhysicsTransform, With<Player>>,
) {
    let Some(store) = store else {
        return;
    };
    if store.is_empty() {
        guard
            .reborrow()
            .map_unchanged(|guard| &mut guard.armed)
            .set_if_neq(false);
        return;
    }
    let feet = BlockPos::from(camera.translation);
    let column = ColumnPos::new(feet.x >> 4, feet.z >> 4);
    if store.holds(column) {
        guard
            .reborrow()
            .map_unchanged(|guard| &mut guard.armed)
            .set_if_neq(true);
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
    if guard.level == Guard::Panic {
        panic!(
            "chunk guard: the player is at {feet:?}, in column {},{}, which the client does \
             not hold ({} columns resident) — the camera has outrun the loader",
            column.x,
            column.z,
            store.len(),
        );
    }
}
