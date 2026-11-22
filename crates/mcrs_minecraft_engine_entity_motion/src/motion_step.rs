use bevy_ecs::batching::BatchingStrategy;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::Has;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::SystemChangeTick;
use bevy_ecs::system::SystemParam;
use bevy_ecs::world::Mut;
use bevy_utils::Parallel;
use glam::DVec3;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::BlockAction;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::CORPSE_REMOVAL_TICKS;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::MovePath;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_physics::grid_row;
use mcrs_minecraft_physics::push_entity;
use mcrs_minecraft_physics::same_bits;
use mcrs_minecraft_physics::tick_entity;
use mcrs_voxel::SectionPos;

use crate::CrowdCandidate;
use crate::CrowdCandidates;
use crate::Crowding;
use crate::Expired;
use crate::Expiry;
use crate::ExpiryCause;
use crate::MobIntent;
use crate::MotionHold;
use crate::MotionId;
use crate::MotionKind;
use crate::MotionState;
use crate::Movers;
use crate::StepOutcome;
use crate::StirredTiles;
use crate::grid_slot::GridSlot;
use crate::regrids::Regrids;
use crate::section_cache::SectionCache;
use crate::tile_edits::SlotChange;
use crate::tile_edits::TileEdits;

const STILL_INPUT: f64 = 1.0e-7;

type Moving<'a> = (
    Entity,
    Mut<'a, MotionState>,
    &'a MotionKind,
    Mut<'a, StepOutcome>,
    Option<Mut<'a, MobIntent>>,
    Option<&'a MotionId>,
    Option<Mut<'a, GridSlot>>,
    bool,
);

#[derive(Default)]
pub(crate) struct StepBuffers {
    scratch: Scratch,
    actions: Vec<BlockAction>,
}

#[derive(SystemParam)]
pub struct MotionStep<'w, 's> {
    stirred: Res<'w, StirredTiles>,
    grid: Res<'w, BodyGrid>,
    movers: ResMut<'w, Movers>,
    crowds: ResMut<'w, CrowdCandidates>,
    expired: ResMut<'w, Expired>,
    regrids: ResMut<'w, Regrids>,
    ticks: SystemChangeTick,
    buffers: Local<'s, Parallel<StepBuffers>>,
    entities: Query<
        'w,
        's,
        (
            Entity,
            &'static mut MotionState,
            &'static MotionKind,
            &'static mut StepOutcome,
            Option<&'static mut MobIntent>,
            Option<&'static MotionId>,
            Option<&'static mut GridSlot>,
            Has<MotionHold>,
        ),
    >,
}

impl MotionStep<'_, '_> {
    pub fn run<W: CollisionWorld + Sync>(
        &mut self,
        world: &W,
        tables: &Tables,
        side: Side,
        tick: u64,
        crowding: &Crowding,
    ) {
        self.regrids.stepped = self.ticks.this_run();
        let pass = &PassInput {
            world,
            tables,
            side,
            tick,
            crowding,
            stirred: &self.stirred,
            grid: &self.grid,
            movers: &self.movers.queues,
            crowds: &self.crowds.queues,
            expired: &self.expired.queues,
            regrids: &self.regrids.queues,
        };
        let buffers = &*self.buffers;
        self.entities
            .par_iter_mut()
            .batching_strategy(BatchingStrategy::new().batches_per_thread(4))
            .for_each_init(
                || buffers.borrow_local_mut(),
                |buffers, moving| {
                    let StepBuffers { scratch, actions } = &mut **buffers;
                    step(moving, pass, scratch, actions);
                },
            );
    }
}

struct PassInput<'a, W> {
    world: &'a W,
    tables: &'a Tables,
    side: Side,
    tick: u64,
    crowding: &'a Crowding,
    stirred: &'a StirredTiles,
    grid: &'a BodyGrid,
    movers: &'a Parallel<Vec<Entity>>,
    crowds: &'a Parallel<Vec<CrowdCandidate>>,
    expired: &'a Parallel<Vec<Expiry>>,
    regrids: &'a Parallel<TileEdits>,
}

impl<W> PassInput<'_, W> {
    fn moved(&self, entity: Entity) {
        self.movers.scope(|queue| queue.push(entity));
    }
}

struct Stepping<'a> {
    entity: Entity,
    kind: &'a MotionRow,
    own: Option<GridRow>,
    start: Body,
    looked: Option<[u32; 3]>,
    wake: bool,
    external: bool,
}

fn step<W: CollisionWorld>(
    (entity, mut state, kind, mut outcome, intent, id, slot, held): Moving<'_>,
    pass: &PassInput<'_, W>,
    scratch: &mut Scratch,
    actions: &mut Vec<BlockAction>,
) {
    let outside_grid = slot.as_ref().is_none_or(|slot| slot.0.is_none());
    if intent.is_none() && outside_grid && state.0.living.is_none() {
        let drifting = Drifting {
            entity,
            state,
            kind,
            outcome,
            held,
        };
        return drift(drifting, pass, scratch, actions);
    }
    let written = state.is_changed();
    let external = written && !state.is_added();
    let motion = &mut state.bypass_change_detection().0;
    let looked = motion.living.as_ref().map(rotations);
    if steer(motion, intent.as_deref()) {
        motion.rest = Rest::Awake;
    }
    if let Some(mut intent) = intent {
        *intent.bypass_change_detection() = MobIntent::default();
    }
    let start = motion.body;
    let at = Stepping {
        entity,
        kind: &kind.0,
        own: id.and_then(|id| grid_row(motion, &kind.0, id.0)),
        start,
        looked,
        wake: written || pass.stirred.wakes(TileKey::at(start.position)),
        external,
    };
    let cache = (!held)
        .then(|| SectionCache::cover(pass.world, start.position))
        .filter(|cache| loaded(cache, start.position));
    let changed = match cache {
        Some(cache) => simulate(&at, &cache, motion, &mut outcome, pass, scratch, actions),
        None => {
            if at.wake {
                motion.rest = Rest::Awake;
            }
            let turned = turned(&at, motion);
            if external || turned {
                pass.moved(entity);
            }
            turned
        }
    };
    let row = at.own.and_then(|own| {
        motion.last_path = MovePath {
            start: start.position,
            end: motion.body.position,
        };
        grid_row(motion, &kind.0, own.id)
    });
    if let Some(mut slot) = slot
        && row != slot.0
    {
        let change = SlotChange {
            old: slot.0,
            new: row,
        };
        pass.regrids.scope(|edits| edits.push(change));
        slot.bypass_change_detection().0 = row;
    }
    if changed || at.own.is_some() && row != at.own {
        state.set_changed();
    }
}

struct Drifting<'a> {
    entity: Entity,
    state: Mut<'a, MotionState>,
    kind: &'a MotionKind,
    outcome: Mut<'a, StepOutcome>,
    held: bool,
}

fn drift<W: CollisionWorld>(
    drifting: Drifting<'_>,
    pass: &PassInput<'_, W>,
    scratch: &mut Scratch,
    actions: &mut Vec<BlockAction>,
) {
    let Drifting {
        entity,
        mut state,
        kind,
        mut outcome,
        held,
    } = drifting;
    let written = state.is_changed();
    let external = written && !state.is_added();
    let motion = &mut state.bypass_change_detection().0;
    let start = motion.body;
    let wake = written || pass.stirred.wakes(TileKey::at(start.position));
    let skipped = |motion: &mut EntityState| {
        if wake {
            motion.rest = Rest::Awake;
        }
        if external {
            pass.moved(entity);
        }
    };
    if held {
        return skipped(motion);
    }
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
    if (kind.0.lifetime).is_some_and(|lifetime| motion.age >= lifetime) {
        let expiry = Expiry {
            entity,
            key: motion.key,
            cause: ExpiryCause::Lifetime,
        };
        pass.expired.scope(|queue| queue.push(expiry));
    }
    if same_bits(&start, &motion.body) {
        if external {
            pass.moved(entity);
        }
        return;
    }
    let body = motion.body;
    state.set_changed();
    let moved = external
        || body.position != start.position
        || body.flags != start.flags
        || kernel.bounced
        || wake && body.velocity != start.velocity;
    if moved {
        pass.moved(entity);
    }
}

fn simulate<W: CollisionWorld>(
    at: &Stepping<'_>,
    cache: &SectionCache<'_, W>,
    motion: &mut EntityState,
    outcome: &mut Mut<'_, StepOutcome>,
    pass: &PassInput<'_, W>,
    scratch: &mut Scratch,
    actions: &mut Vec<BlockAction>,
) -> bool {
    if let Some(own) = at.own
        && let Some(crowd) = push(pass, at.entity, &own, motion, scratch)
    {
        pass.crowds.scope(|queue| queue.push(crowd));
    }
    let view = View::new(cache, pass.grid, pass.tables);
    let kernel = tick_entity(view, pass.side, at.kind, motion, at.wake, actions, scratch);
    actions.clear();
    let body = motion.body;
    let changed = !same_bits(&at.start, &body) || turned(at, motion);
    motion.age = motion.age.wrapping_add(1);
    let stepped_at = Some(pass.tick);
    outcome.bypass_change_detection().stepped_at = stepped_at;
    outcome.set_if_neq(StepOutcome {
        start: at.start.position,
        kernel,
        stepped_at,
    });
    if let Some(expiry) = expire(at, motion) {
        pass.expired.scope(|queue| queue.push(expiry));
    }
    let start = at.start;
    let moved = at.external
        || changed
            && (at.looked.is_some()
                || body.position != start.position
                || body.flags != start.flags
                || kernel.bounced
                || at.wake && body.velocity != start.velocity);
    if moved {
        pass.moved(at.entity);
    }
    changed
}

fn push<W>(
    pass: &PassInput<'_, W>,
    entity: Entity,
    own: &GridRow,
    motion: &mut EntityState,
    scratch: &mut Scratch,
) -> Option<CrowdCandidate> {
    let rules = &pass.crowding.rules;
    let stream = KeyedStream::new(&motion.key, pass.tick);
    let velocity = &mut motion.body.velocity;
    let pushed = push_entity(pass.grid, own, rules, &stream, velocity, scratch);
    let crowded = rules.max_entity_cramming > 0
        && i64::from(pushed.candidates) > i64::from(rules.max_entity_cramming) - 1;
    if pushed.pushed || crowded {
        motion.rest = Rest::Awake;
    }
    let alive = motion.living.as_ref().is_some_and(Living::is_alive);
    (pushed.crowd_damage && pass.crowding.damage_type.is_some() && alive).then_some(
        CrowdCandidate {
            entity,
            own: *own,
            candidates: pushed.candidates,
            free: pushed.free_candidates,
        },
    )
}

fn expire(at: &Stepping<'_>, motion: &mut EntityState) -> Option<Expiry> {
    let expires = (at.kind.lifetime).is_some_and(|lifetime| motion.age >= lifetime);
    let died = motion.living.as_mut().is_some_and(count_down);
    if motion
        .living
        .as_ref()
        .is_some_and(|living| !living.is_alive())
    {
        motion.rest = Rest::Awake;
    }
    (died || expires).then_some(Expiry {
        entity: at.entity,
        key: motion.key,
        cause: if died {
            ExpiryCause::Death
        } else {
            ExpiryCause::Lifetime
        },
    })
}

fn steer(motion: &mut EntityState, intent: Option<&MobIntent>) -> bool {
    let Some(living) = motion.living.as_mut() else {
        return false;
    };
    if !living.is_alive() {
        living.steering = None;
        living.input = MoveInput::default();
        living.jump = false;
        return false;
    }
    let Some(intent) = intent else {
        return false;
    };
    apply_intent(intent, living);
    intent
        .steering
        .is_some_and(|steering| steering.input.length_squared() >= STILL_INPUT || steering.jump)
}

fn apply_intent(intent: &MobIntent, living: &mut Living) {
    if intent.steering.is_some() {
        living.steering = intent.steering;
    }
    let turns = [
        (intent.yaw, &mut living.orientation.yaw),
        (intent.pitch, &mut living.orientation.pitch),
        (intent.head_yaw, &mut living.orientation.head_yaw),
        (intent.body_yaw, &mut living.orientation.body_yaw),
    ];
    for (turn, angle) in turns {
        if let Some(turn) = turn {
            *angle = turn;
        }
    }
}

fn count_down(living: &mut Living) -> bool {
    living.hurt_time = living.hurt_time.saturating_sub(1);
    living.damage_cooldown = living.damage_cooldown.saturating_sub(1);
    let Some(death_time) = living.death_time.as_mut() else {
        return false;
    };
    *death_time = death_time.saturating_add(1);
    *death_time >= CORPSE_REMOVAL_TICKS
}

fn turned(at: &Stepping<'_>, motion: &EntityState) -> bool {
    at.looked
        .is_some_and(|looked| motion.living.as_ref().map(rotations) != Some(looked))
}

fn rotations(living: &Living) -> [u32; 3] {
    [
        living.orientation.yaw,
        living.orientation.pitch,
        living.orientation.head_yaw,
    ]
    .map(f32::to_bits)
}

fn loaded<W: CollisionWorld>(world: &W, position: DVec3) -> bool {
    !matches!(
        world.section(SectionPos::from(block_at(position))),
        SectionLookup::Unloaded
    )
}
