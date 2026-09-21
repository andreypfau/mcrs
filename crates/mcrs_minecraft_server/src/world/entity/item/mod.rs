pub mod pickup;
pub mod tick;

use crate::world::aoi::TrackedBy;
use crate::world::item::StackSet;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, Messages};
use bevy_ecs::prelude::{With, Without};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Command;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation, SectionPos};
use mcrs_minecraft_inventory::{Op, Transaction};
use mcrs_minecraft_item::dropped::BLOCK_DROP_PICKUP_DELAY;
use mcrs_minecraft_item::{DroppedItem, Thrower};
use mcrs_minecraft_level::entity::mob::{EntityInSection, EntityKind, EntityUuid};
use mcrs_minecraft_level::entity::physics::{Rotation, Transform, Velocity};
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::section::SectionIndex;
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemStackValue};
use mcrs_minecraft_world::entity::minecraft as entity_types;
use rand::{Rng, RngExt, rng};

pub const EYE_HEIGHT: f64 = 1.62;
pub const ITEM_WIDTH: f64 = 0.25;
pub const ITEM_HEIGHT: f64 = 0.25;

pub struct DroppedItemPlugin;

impl Plugin for DroppedItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlockDrop>().add_systems(
            FixedUpdate,
            (
                spawn_block_drops,
                launch_thrown_items,
                tick::tick_dropped_items,
                pickup::pickup_items,
            )
                .chain()
                .in_set(StackSet::Mutate),
        );
    }
}

#[derive(Message, Debug)]
pub struct BlockDrop {
    pub dim: Entity,
    pub pos: BlockPos,
    pub item: ResourceLocation,
    pub count: u8,
}

/// Gives a dropped stack its place in the world; its section link is the one
/// it spawns in and is not moved with it.
pub fn place_dropped(world: &mut World, stack: Entity, dim: Entity, pos: DVec3, velocity: DVec3) {
    let section = world
        .get::<SectionIndex>(dim)
        .and_then(|index| index.get(SectionPos::from(pos)));
    let mut entity = world.entity_mut(stack);
    entity.insert((
        InDimension(dim),
        Transform {
            translation: pos,
            rotation: Rotation::new(rng().random::<f32>() * 360.0, 0.0),
        },
        Velocity(velocity),
        EntityUuid::default(),
        EntityKind(&entity_types::ITEM),
        TrackedBy::default(),
    ));
    if let Some(section) = section {
        entity.insert(EntityInSection(section));
    }
}

/// Spawns a stack as a dropped item at a place in the world.
pub fn spawn_dropped(
    world: &mut World,
    value: ItemStackValue,
    dim: Entity,
    pos: DVec3,
    velocity: DVec3,
    pickup_delay: i16,
    thrower: Option<Entity>,
) -> Entity {
    let entity = world.spawn_empty().id();
    place_dropped(world, entity, dim, pos, velocity);
    Transaction(vec![Op::SpawnDropped {
        entity,
        value,
        pickup_delay,
        thrower,
    }])
    .apply(world);
    entity
}

pub fn throw_velocity(yaw: f32, pitch: f32, rng: &mut impl Rng) -> DVec3 {
    let (sin_x, cos_x) = pitch.to_radians().sin_cos();
    let (sin_y, cos_y) = yaw.to_radians().sin_cos();
    let dir = rng.random::<f32>() * std::f32::consts::TAU;
    let pow2 = 0.02 * rng.random::<f32>();
    DVec3::new(
        f64::from(-sin_y * cos_x * 0.3) + f64::from(dir.cos()) * f64::from(pow2),
        f64::from(-sin_x * 0.3 + 0.1 + (rng.random::<f32>() - rng.random::<f32>()) * 0.1),
        f64::from(cos_y * cos_x * 0.3) + f64::from(dir.sin()) * f64::from(pow2),
    )
}

pub fn block_drop_velocity(rng: &mut impl Rng) -> DVec3 {
    DVec3::new(
        rng.random::<f64>() * 0.2 - 0.1,
        0.2,
        rng.random::<f64>() * 0.2 - 0.1,
    )
}

/// A stack a transaction threw has no place yet: it starts in front of its
/// thrower, or is lost with a thrower that has no position.
pub fn launch_thrown_items(world: &mut World) {
    let thrown: Vec<(Entity, Entity)> = world
        .query_filtered::<(Entity, &Thrower), (With<DroppedItem>, Without<Transform>)>()
        .iter(world)
        .map(|(item, thrower)| (item, thrower.0))
        .collect();
    for (item, thrower) in thrown {
        let Some((dim, transform)) = world
            .get::<InDimension>(thrower)
            .zip(world.get::<Transform>(thrower))
            .map(|(dim, transform)| (dim.0, *transform))
        else {
            tracing::warn!(?thrower, ?item, "a stack thrown by a player with no position is lost");
            world.despawn(item);
            continue;
        };
        let pos = transform.translation + DVec3::new(0.0, EYE_HEIGHT - 0.3, 0.0);
        let velocity = throw_velocity(transform.rotation.yaw(), transform.rotation.pitch(), &mut rng());
        place_dropped(world, item, dim, pos, velocity);
    }
}

pub fn spawn_block_drops(world: &mut World) {
    let drops: Vec<BlockDrop> = world
        .resource_mut::<Messages<BlockDrop>>()
        .drain()
        .collect();
    let mut rng = rng();
    for drop in drops {
        let value = ItemStackValue {
            item: ResourceKey::from_location(drop.item),
            count: Bounded(i32::from(drop.count)),
            components: ComponentPatch::EMPTY,
        };
        let mut scatter = || rng.random_range(-0.25..0.25);
        let pos = DVec3::new(
            f64::from(drop.pos.x) + 0.5 + scatter(),
            f64::from(drop.pos.y) + 0.5 + scatter() - ITEM_HEIGHT / 2.0,
            f64::from(drop.pos.z) + 0.5 + scatter(),
        );
        let velocity = block_drop_velocity(&mut rng);
        let item = spawn_dropped(world, value, drop.dim, pos, velocity, BLOCK_DROP_PICKUP_DELAY, None);
        if world.get::<DroppedItem>(item).is_none() {
            world.despawn(item);
        }
    }
}
