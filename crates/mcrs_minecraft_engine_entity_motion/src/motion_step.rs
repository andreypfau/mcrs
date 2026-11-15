use bevy_ecs::batching::BatchingStrategy;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::Has;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::SystemParam;
use bevy_ecs::world::Mut;
use bevy_utils::Parallel;
use glam::DVec3;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::BlockAction;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Rest;
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
use crate::StirredTiles;
use crate::section_cache::SectionCache;

type Moving<'a> = (
    Entity,
    Mut<'a, MotionState>,
    &'a MotionKind,
    Mut<'a, StepOutcome>,
    bool,
);

/// The buffers of one thread of the pass, kept from tick to tick.
#[derive(Default)]
pub(crate) struct StepBuffers {
    scratch: Scratch,
    actions: Vec<BlockAction>,
}

/// The step of the motion pass, without the block source: a host runs it in
/// [`MotionSteps`](crate::MotionSteps).
#[derive(SystemParam)]
pub struct MotionStep<'w, 's> {
    stirred: Res<'w, StirredTiles>,
    movers: ResMut<'w, Movers>,
    buffers: Local<'s, Parallel<StepBuffers>>,
    entities: Query<
        'w,
        's,
        (
            Entity,
            &'static mut MotionState,
            &'static MotionKind,
            &'static mut StepOutcome,
            Has<MotionHold>,
        ),
    >,
}

impl MotionStep<'_, '_> {
    /// Steps each entity with a motion and no hold once, in parallel, over the blocks of `world`
    /// as `side` sees them, and marks each stepped [`StepOutcome`] with `tick`. Each step reads the
    /// blocks through the section cache of its entity, and each thread keeps its own scratch
    /// buffers.
    pub fn run<W: CollisionWorld + Sync>(
        &mut self,
        world: &W,
        tables: &Tables,
        side: Side,
        tick: u64,
    ) {
        let pass = &PassInput {
            world,
            tables,
            side,
            tick,
            stirred: &self.stirred,
        };
        let movers = &self.movers.queues;
        let buffers = &*self.buffers;
        self.entities
            .par_iter_mut()
            .batching_strategy(BatchingStrategy::new().batches_per_thread(4))
            .for_each_init(
                || buffers.borrow_local_mut(),
                |buffers, moving| {
                    let StepBuffers { scratch, actions } = &mut **buffers;
                    let Some(moved) = step(moving, pass, scratch, actions) else {
                        return;
                    };
                    movers.scope(|queue| queue.push(moved));
                },
            );
    }
}

/// What every step of one pass shares.
struct PassInput<'a, W> {
    world: &'a W,
    tables: &'a Tables,
    side: Side,
    tick: u64,
    stirred: &'a StirredTiles,
}

/// One step of one entity, as the rows pass steps a row that is not living: no push, the block
/// actions of the tick dropped, and the age counted. Gives the entity when its body changed, or
/// when another system wrote its motion since the last pass. An entity that the pass skips keeps
/// its wake for the step after.
fn step<W: CollisionWorld>(
    (entity, mut state, kind, mut outcome, held): Moving<'_>,
    pass: &PassInput<'_, W>,
    scratch: &mut Scratch,
    actions: &mut Vec<BlockAction>,
) -> Option<Entity> {
    let written = state.is_changed();
    // The first step of an entity follows its spawn, which already shows the body.
    let external = written && !state.is_added();
    let motion = &mut state.bypass_change_detection().0;
    let start = motion.body;
    let wake = written || pass.stirred.wakes(TileKey::at(start.position));
    let skipped = |motion: &mut EntityState| {
        if wake {
            motion.rest = Rest::Awake;
        }
        external.then_some(entity)
    };
    if held {
        return skipped(motion);
    }
    // The kernel decides inside `tick_entity` whether a resting entity skips its tick, so the
    // window is filled before that call.
    let cache = SectionCache::cover(pass.world, start.position);
    if !loaded(&cache, start.position) {
        return skipped(motion);
    }
    let view = View::new(&cache, &NoBodies, pass.tables);
    let kernel = tick_entity(view, pass.side, &kind.0, motion, wake, actions, scratch);
    actions.clear();
    motion.age = motion.age.wrapping_add(1);
    let stepped_at = Some(pass.tick);
    outcome.bypass_change_detection().stepped_at = stepped_at;
    outcome.set_if_neq(StepOutcome {
        start: start.position,
        kernel,
        stepped_at,
    });
    if same_bits(&start, &motion.body) {
        return external.then_some(entity);
    }
    let body = motion.body;
    state.set_changed();
    let moved = external
        || body.position != start.position
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
