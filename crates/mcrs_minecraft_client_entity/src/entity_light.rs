use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_entity_visual::{ClientEntity, EntityLight, Motion, SyncedData, eye_height};
use mcrs_minecraft_volume::LightVolume;
use mcrs_voxel::BlockPos;

const OPEN_SKY: u8 = 0x0f;

fn eye_block(position: DVec3, eye_height: f64) -> BlockPos {
    let eye = (position + DVec3::Y * eye_height).floor().as_ivec3();
    BlockPos::new(eye.x, eye.y, eye.z)
}

fn sample(columns: &ColumnStore, at: BlockPos) -> EntityLight {
    let light = columns.light(at).unwrap_or(OPEN_SKY);
    EntityLight {
        block: light >> 4,
        sky: light & 0x0f,
    }
}

/// Gives each entity with no light the light at its eye, before the renderer reads it in the
/// frame that the entity is added.
pub(crate) fn insert_entity_light(
    columns: Res<ColumnStore>,
    entities: Query<(Entity, &ClientEntity, &Motion, &SyncedData), Without<EntityLight>>,
    mut commands: Commands,
) {
    for (entity, client, motion, data) in &entities {
        let at = eye_block(motion.current.position, eye_height(client, data));
        commands.entity(entity).insert(sample(&columns, at));
    }
}

/// Samples the light again for each entity whose eye entered another block in the last tick.
// chisle: a light change at an eye that does not move is not seen; a message of the column store
// for each changed light lifts it.
pub(crate) fn update_entity_light(
    columns: Res<ColumnStore>,
    mut entities: Query<(&ClientEntity, &Motion, &SyncedData, &mut EntityLight), Changed<Motion>>,
) {
    for (client, motion, data, mut light) in &mut entities {
        let height = eye_height(client, data);
        let at = eye_block(motion.current.position, height);
        if at != eye_block(motion.previous.position, height) {
            light.set_if_neq(sample(&columns, at));
        }
    }
}
