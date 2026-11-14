use bevy_ecs::batching::BatchingStrategy;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::query::Without;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use bevy_ecs::world::Mut;
use glam::DVec3;
use mcrs_minecraft_engine_physics::BlockWorld;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::BlockAction;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_physics::same_bits;
use mcrs_minecraft_physics::tick_entity;
use mcrs_voxel::SectionPos;

use crate::MotionHold;
use crate::MotionKind;
use crate::MotionState;
use crate::Movers;
use crate::StepOutcome;
use crate::section_cache::SectionCache;
use crate::stirred_tiles::StirredTiles;

const SERVER: Side = Side::Server { authority: true };

type Moving<'a> = (
    Entity,
    Mut<'a, MotionState>,
    &'a MotionKind,
    Mut<'a, StepOutcome>,
);

/// Steps each entity with a motion and no hold once, in parallel. Each batch reads the blocks
/// through its own section cache and keeps its own scratch buffers.
pub(crate) fn step_motion(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    stirred: Res<StirredTiles>,
    movers: ResMut<Movers>,
    mut entities: Query<
        (Entity, &mut MotionState, &MotionKind, &mut StepOutcome),
        Without<MotionHold>,
    >,
) {
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let world = BlockWorld {
        blocks: &blocks,
        table: tables.collision(),
    };
    let world = &world;
    let tables = tables.tables();
    let stirred = &*stirred;
    let movers = &movers.queues;
    entities
        .par_iter_mut()
        .batching_strategy(BatchingStrategy::new().batches_per_thread(4))
        .for_each_init(
            || {
                (
                    Scratch::new(),
                    Vec::<BlockAction>::new(),
                    SectionCache::new(world),
                )
            },
            |(scratch, actions, cache), moving| {
                let Some(moved) = step(moving, cache, tables, stirred, scratch, actions) else {
                    return;
                };
                movers.scope(|queue| queue.push(moved));
            },
        );
}

/// One step of one entity, as the rows pass steps a row that is not living: no push, the block
/// actions of the tick dropped, and the age counted. Gives the entity when its body changed.
fn step<W: CollisionWorld>(
    (entity, mut state, kind, mut outcome): Moving<'_>,
    cache: &mut SectionCache<'_, W>,
    tables: &Tables,
    stirred: &StirredTiles,
    scratch: &mut Scratch,
    actions: &mut Vec<BlockAction>,
) -> Option<Entity> {
    let written = state.is_changed();
    let motion = &mut state.bypass_change_detection().0;
    let start = motion.body;
    cache.cover(start.position);
    if !loaded(&*cache, start.position) {
        return None;
    }
    let wake = written || stirred.wakes(TileKey::at(start.position));
    let view = View::new(&*cache, &NoBodies, tables);
    let kernel = tick_entity(view, SERVER, &kind.0, motion, wake, actions, scratch);
    actions.clear();
    motion.age = motion.age.wrapping_add(1);
    outcome.set_if_neq(StepOutcome {
        start: start.position,
        flags: motion.body.flags,
        kernel,
    });
    if same_bits(&start, &motion.body) {
        return None;
    }
    let body = motion.body;
    state.set_changed();
    let moved = body.position != start.position
        || body.flags != start.flags
        || kernel.bounced
        || wake && body.velocity != start.velocity;
    // The velocity of an item cycles while it rests; a viewer does not see it (TRK-UPD-017).
    moved.then_some(entity)
}

/// True when the section of `position` is loaded or outside the world: an entity in a section
/// that is not loaded does not move.
fn loaded<W: CollisionWorld>(world: &W, position: DVec3) -> bool {
    !matches!(
        world.section(SectionPos::from(block_at(position))),
        SectionLookup::Unloaded
    )
}
