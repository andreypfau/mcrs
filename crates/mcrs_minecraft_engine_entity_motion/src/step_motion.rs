use bevy_ecs::batching::BatchingStrategy;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::query::Without;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use bevy_ecs::world::Mut;
use bevy_utils::Parallel;
use glam::DVec3;
use mcrs_minecraft_engine_physics::BlockWorld;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
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

/// The buffers of one thread of the pass, kept from tick to tick.
#[derive(Default)]
pub(crate) struct StepBuffers {
    scratch: Scratch,
    actions: Vec<BlockAction>,
}

/// Steps each entity with a motion and no hold once, in parallel. Each step reads the blocks
/// through the section cache of its entity, and each thread keeps its own scratch buffers.
#[expect(
    clippy::too_many_arguments,
    reason = "a system takes each resource that it reads"
)]
pub(crate) fn step_motion(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    stirred: Res<StirredTiles>,
    clock: Res<BirthClock>,
    movers: ResMut<Movers>,
    buffers: Local<Parallel<StepBuffers>>,
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
    let clock = *clock;
    let movers = &movers.queues;
    let buffers = &*buffers;
    entities
        .par_iter_mut()
        .batching_strategy(BatchingStrategy::new().batches_per_thread(4))
        .for_each_init(
            || buffers.borrow_local_mut(),
            |buffers, moving| {
                let StepBuffers { scratch, actions } = &mut **buffers;
                let Some(moved) = step(moving, world, tables, stirred, clock, scratch, actions)
                else {
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
    world: &W,
    tables: &Tables,
    stirred: &StirredTiles,
    clock: BirthClock,
    scratch: &mut Scratch,
    actions: &mut Vec<BlockAction>,
) -> Option<Entity> {
    let written = state.is_changed();
    let motion = &mut state.bypass_change_detection().0;
    let start = motion.body;
    // The kernel decides inside `tick_entity` whether a resting entity skips its tick, so the
    // window is filled before that call.
    let cache = SectionCache::cover(world, start.position);
    if !loaded(&cache, start.position) {
        return None;
    }
    let wake = written || stirred.wakes(TileKey::at(start.position));
    let view = View::new(&cache, &NoBodies, tables);
    let kernel = tick_entity(view, SERVER, &kind.0, motion, wake, actions, scratch);
    actions.clear();
    motion.age = motion.age.wrapping_add(1);
    let stepped_at = Some(clock.0);
    outcome.bypass_change_detection().stepped_at = stepped_at;
    outcome.set_if_neq(StepOutcome {
        start: start.position,
        kernel,
        stepped_at,
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
