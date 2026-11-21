use core::f64::consts::{PI, TAU};

use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::{Query, Res, ResMut};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_chunk::SkyLight;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_engine::block_simulation::{BlockMemory, BlockShared, BlockSimulation};
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeId};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_environment::WorldClocks;
use mcrs_minecraft_redstone::{
    AnalogSource, HasRedstoneMemory, HasRedstoneSnapshot, MAX_SIGNAL, Sensor,
};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{BlockBehaviour, UpdateRequest};
use mcrs_voxel::{BlockPos, LocalPos, SectionPos};

pub const CHECK_PERIOD: i64 = 20;
const DAY_LENGTH: i64 = 24_000;
const SUN_PULL: f64 = 0.2;

#[must_use]
pub fn sun_angle(ticks: i64) -> f64 {
    let fraction = (ticks.rem_euclid(DAY_LENGTH) as f64 / DAY_LENGTH as f64 - 0.25).rem_euclid(1.0);
    let eased = 1.0 - ((fraction * PI).cos() + 1.0) / 2.0;
    let angle = fraction + (eased - fraction) / 3.0;
    angle * TAU
}

#[must_use]
pub fn sun_power(sky: u8, angle: f64) -> u8 {
    let target = if angle < PI { 0.0 } else { TAU };
    let pulled = angle + (target - angle) * SUN_PULL;
    let power = (f64::from(sky.min(MAX_SIGNAL)) * pulled.cos()).round();
    if power <= 0.0 {
        0
    } else {
        u8::try_from(power as i64)
            .unwrap_or(MAX_SIGNAL)
            .min(MAX_SIGNAL)
    }
}

fn day_ticks(
    registries: Option<&CurrentRegistries>,
    clocks: Option<&WorldClocks>,
    dimension_type: Option<&DimensionTypeId>,
    fallback: i64,
) -> i64 {
    let clock = registries
        .and_then(|registries| registries.registry::<DimensionTypeFile>())
        .and_then(|types| {
            let id = Id::checked(dimension_type?.0.number(), types.len())?;
            types.get(id)?.default_clock
        });
    clock
        .and_then(|clock| clocks?.get(clock))
        .map_or(fallback, |time| time.total_ticks)
}

fn sky_light(index: &SectionIndex, lights: &Query<&SkyLight>, pos: BlockPos) -> u8 {
    let section = SectionPos::from(pos);
    let Some(entity) = index.get(section) else {
        return 0;
    };
    let local = LocalPos::from(pos);
    lights.get(entity).map_or(0, |light| {
        light.0.get(
            usize::from(local.x()),
            usize::from(local.y()),
            usize::from(local.z()),
        )
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query and resource is its own parameter"
)]
pub(crate) fn publish_daylight<B>(
    time: Res<GameTime>,
    registries: Option<Res<CurrentRegistries>>,
    clocks: Option<Res<WorldClocks>>,
    dims: Query<(Entity, &SectionIndex, Option<&DimensionTypeId>), With<Dimension>>,
    lights: Query<&SkyLight>,
    memory: Res<BlockMemory<B>>,
    mut shared: ResMut<BlockShared<B>>,
    mut simulation: ResMut<BlockSimulation>,
) where
    B: BlockBehaviour<WorldKinds> + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    if time.0.rem_euclid(CHECK_PERIOD) != 0 {
        return;
    }
    let Ok((_, index, dimension_type)) = dims.single() else {
        return;
    };
    let ticks = day_ticks(
        registries.as_deref(),
        clocks.as_deref(),
        dimension_type,
        time.0,
    );
    let angle = sun_angle(ticks);
    let sensors = &memory.0.redstone_ref().sensors;
    let analog = &mut shared.0.redstone_mut().analog;
    for (&pos, sensor) in sensors {
        if *sensor != Sensor::Daylight {
            continue;
        }
        let sky = sky_light(index, &lights, pos);
        let value = AnalogSource::Daylight {
            sun: sun_power(sky, angle),
            sky,
        };
        if analog.get(&pos).copied() == Some(value) {
            continue;
        }
        analog.insert(pos, value);
        simulation.request_update(UpdateRequest::Single {
            target: pos,
            block: Block::DaylightDetector.id(),
        });
    }
}
