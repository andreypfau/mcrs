use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_entity_visual::{ClientEntity, EntityLight, Motion, SyncedData, eye_height};
use mcrs_minecraft_volume::LightVolume;
use mcrs_voxel::BlockPos;

/// The block that the [`EntityLight`] of the entity was last sampled at.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LightSampledAt(BlockPos);

fn eye_block(position: DVec3, eye_height: f64) -> BlockPos {
    let eye = (position + DVec3::Y * eye_height).floor().as_ivec3();
    BlockPos::new(eye.x, eye.y, eye.z)
}

fn sample(columns: &ColumnStore, at: BlockPos) -> EntityLight {
    let Ok(light) = columns.light(at) else {
        return EntityLight::OPEN_SKY;
    };
    EntityLight {
        block: light >> 4,
        sky: light & 0x0f,
    }
}

/// Samples the light at the eye of each entity that has no light yet, or whose eye is in another
/// block than the one that its light was sampled at.
// chisle: a light change at an eye that does not move is not seen; a message of the column store
// for each changed light lifts it.
pub(crate) fn write_entity_light(
    columns: Res<ColumnStore>,
    mut entities: Query<
        (
            Entity,
            &ClientEntity,
            &Motion,
            &SyncedData,
            Option<&mut LightSampledAt>,
            Option<&mut EntityLight>,
        ),
        Or<(Changed<Motion>, Changed<SyncedData>, Without<EntityLight>)>,
    >,
    mut commands: Commands,
) {
    for (entity, client, motion, data, sampled_at, light) in &mut entities {
        let at = eye_block(motion.current.position, eye_height(client, data));
        match (sampled_at, light) {
            (Some(mut sampled_at), Some(mut light)) => {
                if sampled_at.0 != at {
                    sampled_at.0 = at;
                    light.set_if_neq(sample(&columns, at));
                }
            }
            _ => {
                commands
                    .entity(entity)
                    .insert((sample(&columns, at), LightSampledAt(at)));
            }
        }
    }
}
