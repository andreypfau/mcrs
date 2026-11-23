use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_client_world::columns::{ColumnChange, ColumnStore};
use mcrs_minecraft_entity_visual::{
    ClientEntity, EntityLight, HostLight, Motion, SyncedData, eye_height,
};
use mcrs_minecraft_volume::LightVolume;
use mcrs_voxel::{BlockPos, ColumnPos};

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
        (
            Without<HostLight>,
            Or<(Changed<Motion>, Changed<SyncedData>, Without<EntityLight>)>,
        ),
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

pub(crate) fn resample_relit_entities(
    columns: Res<ColumnStore>,
    mut changes: MessageReader<ColumnChange>,
    mut touched: Local<Vec<ColumnPos>>,
    mut entities: Query<(&LightSampledAt, &mut EntityLight), Without<HostLight>>,
) {
    touched.clear();
    touched.extend(changes.read().filter_map(|change| match change {
        ColumnChange::Arrived(pos, _)
        | ColumnChange::Departed(pos, _)
        | ColumnChange::Relit(pos, _) => Some(*pos),
        _ => None,
    }));
    if touched.is_empty() {
        return;
    }
    touched.sort_unstable();
    touched.dedup();
    for (sampled_at, mut light) in &mut entities {
        if touched
            .binary_search(&ColumnPos::from(sampled_at.0))
            .is_ok()
        {
            light.set_if_neq(sample(&columns, sampled_at.0));
        }
    }
}
