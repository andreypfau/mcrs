use std::mem;

use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BlockAction;
use mcrs_minecraft_physics::BlockOverlay;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::ColliderSource;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::PlayerInput;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::QueryEntity;
use mcrs_minecraft_physics::QueryVariant;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::SourcedCollider;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::dimensions_of;
use mcrs_minecraft_physics::query_colliders;
use mcrs_minecraft_physics::tick_player;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use rustc_hash::FxHashMap;

use crate::ClientTick;
use crate::Limits;
use crate::Pusher;
use crate::kept_changes::KeptChanges;
use crate::push_bound::PushBound;
use crate::rollback::Rollback;

/// A silent tick of the client is within this distance of the reference position.
const SILENT_DISTANCE: f64 = 2.0e-4;

/// A block that stops the movement only touches the moved box, so the box grows by this much.
const CONTACT: f64 = 1.0e-7;

/// One possible state of the client after a client tick, and the number of changes it applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Candidate {
    pub(crate) state: PlayerState,
    pub(crate) applied: u64,
}

/// One client tick with what its prediction starts from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Record {
    pub(crate) start: Vec<Candidate>,
    pub(crate) tick: ClientTick,
    pub(crate) reference: DVec3,
    pub(crate) rollback: Option<Rollback>,
    /// Every candidate of the tick ran.
    pub(crate) explored: bool,
}

/// The read-only inputs of a prediction.
pub(crate) struct Context<'a, W, B> {
    pub(crate) view: View<'a, W, B>,
    pub(crate) pushers: &'a [Pusher],
    pub(crate) kept: &'a KeptChanges,
    pub(crate) limits: &'a Limits,
}

/// The buffers of a prediction, reused between ticks.
#[derive(Clone, Debug, Default)]
pub(crate) struct Work {
    trials: Vec<Trial>,
    pub(crate) undone: FxHashMap<BlockPos, BlockStateId>,
    pub(crate) pushers: Vec<Pusher>,
    pub(crate) actions: Vec<BlockAction>,
    colliders: Vec<SourcedCollider>,
    pub(crate) scratch: Scratch,
    /// The kept candidates of the last tick that ran.
    pub(crate) matched: Vec<Candidate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Search {
    /// Stop at the first candidate that matches the position and the flags.
    First,
    All,
}

/// Why a tick stopped before its candidates ran: the tick is coarse, or it sets the player back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    Steps,
    Unbounded,
    /// The movement touches an edited block whose state the server cannot compute.
    Unsettled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Scored {
    pub(crate) candidate: Candidate,
    pub(crate) offset: f64,
}

/// One candidate to run: a start candidate and one value of each input with two values.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Trial {
    start: usize,
    applied: u64,
    sprint: Option<bool>,
    flight: Option<bool>,
    item_use: Option<bool>,
    position: Option<DVec3>,
    /// The number of inputs that differ from their primary value.
    differs: u64,
}

/// Runs the candidates of the tick of `record` in the try order and marks whether each ran. The
/// kept candidates go to `work.matched`, with the reported position. Gives the first candidate
/// with the smallest offset.
pub(crate) fn run_tick<W: CollisionWorld, B: BodyView>(
    context: &Context<'_, W, B>,
    record: &mut Record,
    search: Search,
    steps: &mut u32,
    work: &mut Work,
) -> Result<Option<Scored>, Stop> {
    let mut trials = mem::take(&mut work.trials);
    plan(record, &mut trials);
    add_moves_back(context, record, &mut trials, work);
    let result = run_trials(context, record, &trials, search, steps, work);
    work.trials = trials;
    let (best, explored) = result?;
    record.explored = explored;
    Ok(best)
}

fn run_trials<W: CollisionWorld, B: BodyView>(
    context: &Context<'_, W, B>,
    record: &Record,
    trials: &[Trial],
    search: Search,
    steps: &mut u32,
    work: &mut Work,
) -> Result<(Option<Scored>, bool), Stop> {
    let reported = record.tick.reported;
    work.matched.clear();
    let mut best: Option<Scored> = None;
    for (index, trial) in trials.iter().enumerate() {
        let Some(from) = record.start.get(trial.start) else {
            continue;
        };
        let mut state = from.state;
        context.kept.apply(&mut state, from.applied, trial.applied);
        trial.set(&mut state);
        let mut candidate = Candidate {
            state,
            applied: trial.applied,
        };
        let score = run_trial(context, record, &mut candidate, steps, work)?;
        if best.is_none_or(|best| score.offset < best.offset) {
            best = Some(Scored {
                candidate,
                offset: score.offset,
            });
        }
        if !score.matches {
            continue;
        }
        if let Some(position) = reported.position {
            candidate.state.body.position = position;
        }
        let flags = candidate.state.body.flags;
        let flags_match = flags.on_ground == reported.on_ground
            && flags.horizontal_collision == reported.horizontal_collision;
        if search == Search::First && flags_match {
            work.matched.clear();
            work.matched.push(candidate);
            return Ok((best, index + 1 == trials.len()));
        }
        if !work.matched.contains(&candidate) {
            work.matched.push(candidate);
        }
    }
    Ok((best, true))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Score {
    offset: f64,
    matches: bool,
}

/// Runs one candidate; with a position, again with the push in the bound that comes nearest to
/// the report.
fn run_trial<W: CollisionWorld, B: BodyView>(
    context: &Context<'_, W, B>,
    record: &Record,
    candidate: &mut Candidate,
    steps: &mut u32,
    work: &mut Work,
) -> Result<Score, Stop> {
    let tick = &record.tick;
    let limits = context.limits;
    let start = candidate.state;
    let bound = push_bound(context, tick, &start, work)?;
    let rollback = record.rollback.as_ref();
    world_view(context.kept, rollback, candidate.applied, &mut work.undone);
    let world = BlockOverlay {
        base: context.view.world,
        blocks: &work.undone,
    };
    let view = View::new(&world, context.view.bodies, context.view.tables);
    let mut input = PlayerInput {
        push: DVec3::ZERO,
        ..tick.input
    };
    let mut step = |input: &PlayerInput, out: &mut PlayerState| {
        if *steps >= limits.max_steps_per_tick {
            return Err(Stop::Steps);
        }
        *steps = steps.saturating_add(1);
        tick_player(
            view,
            Side::OwnClient,
            &start,
            input,
            out,
            &mut work.actions,
            &mut work.scratch,
        );
        work.actions.clear();
        Ok(())
    };
    step(&input, &mut candidate.state)?;
    if let Some(rollback) = rollback {
        let dimensions = dimensions_of(start.pose, &start.attributes);
        let grown = Aabb::of_body(start.body.position, dimensions.width, dimensions.height)
            .expand_towards(candidate.state.body.position - start.body.position)
            .inflate(DVec3::splat(CONTACT));
        if rollback.unsettled(candidate.applied, &grown) {
            return Err(Stop::Unsettled);
        }
    }
    let Some(position) = tick.reported.position else {
        let distance = candidate.state.body.position.distance(record.reference);
        let tolerance = SILENT_DISTANCE + bound.reach();
        return Ok(Score {
            offset: (distance - tolerance).max(0.0),
            matches: distance <= tolerance + limits.offset_threshold,
        });
    };
    input.push = bound.clamp(position - candidate.state.body.position);
    if input.push != DVec3::ZERO {
        step(&input, &mut candidate.state)?;
    }
    let offset = candidate.state.body.position.distance(position);
    Ok(Score {
        offset,
        matches: offset < limits.offset_threshold,
    })
}

/// The blocks of the world view of the client after `applied` changes, as an overlay on the
/// world of the server.
pub(crate) fn world_view(
    kept: &KeptChanges,
    rollback: Option<&Rollback>,
    applied: u64,
    blocks: &mut FxHashMap<BlockPos, BlockStateId>,
) {
    kept.undo_from(applied, blocks);
    if let Some(rollback) = rollback {
        rollback.cover(applied, blocks);
    }
}

/// The push bound of the pushers near the candidate; zero for a player that no entity pushes.
fn push_bound<W, B>(
    context: &Context<'_, W, B>,
    tick: &ClientTick,
    start: &PlayerState,
    work: &mut Work,
) -> Result<PushBound, Stop> {
    let facts = tick.input.facts;
    if facts.spectator || facts.passenger {
        return Ok(PushBound::default());
    }
    let dimensions = dimensions_of(start.pose, &start.attributes);
    let player = Aabb::of_body(start.body.position, dimensions.width, dimensions.height);
    let near = player
        .expand_towards(start.body.velocity)
        .inflate(DVec3::splat(1.0));
    work.pushers.clear();
    work.pushers.extend(
        context
            .pushers
            .iter()
            .filter(|pusher| pusher.hull().intersects(&near)),
    );
    PushBound::of(&work.pushers, &player, start.body.position, context.limits)
        .ok_or(Stop::Unbounded)
}

/// The candidates of the tick in the try order: per start candidate, the primary one first, then
/// those that differ in more and more inputs.
fn plan(record: &Record, trials: &mut Vec<Trial>) {
    trials.clear();
    let tick = &record.tick;
    for (start, candidate) in record.start.iter().enumerate() {
        let low = candidate.applied.max(tick.changes.applied);
        let high = tick.changes.seen.max(low);
        let state = &candidate.state;
        let sprints = Options::new(tick.sprint, state.sprint0, Primary::New);
        let flights = Options::new(tick.flight, state.flight0, Primary::NewWhenOn);
        let item_uses = Options::new(tick.item_use, state.item_use.using, Primary::New);
        let first = trials.len();
        for applied in (low..=high).rev() {
            for combination in 0..sprints.count() * flights.count() * item_uses.count() {
                let (sprint, sprint_differs) = sprints.get(combination % sprints.count());
                let rest = combination / sprints.count();
                let (flight, flight_differs) = flights.get(rest % flights.count());
                let (item_use, item_differs) = item_uses.get(rest / flights.count());
                trials.push(Trial {
                    start,
                    applied,
                    sprint,
                    flight,
                    item_use,
                    position: None,
                    differs: high - applied + sprint_differs + flight_differs + item_differs,
                });
            }
        }
        if let Some(group) = trials.get_mut(first..) {
            group.sort_by_key(|trial| trial.differs);
        }
    }
}

/// Appends a run from the recorded position of an edit for each run that applies the
/// `block_changed_ack` first, when the kept state of the edited block meets its start box. The
/// run keeps its velocity and its inputs.
fn add_moves_back<W: CollisionWorld, B: BodyView>(
    context: &Context<'_, W, B>,
    record: &Record,
    trials: &mut Vec<Trial>,
    work: &mut Work,
) {
    let Some(Rollback {
        edits,
        ack: Some(ack),
    }) = &record.rollback
    else {
        return;
    };
    for index in 0..trials.len() {
        let Some(trial) = trials.get(index).copied() else {
            continue;
        };
        let Some(from) = record.start.get(trial.start) else {
            continue;
        };
        if from.applied > *ack || trial.applied <= *ack {
            continue;
        }
        context.kept.undo_from(trial.applied, &mut work.undone);
        let world = BlockOverlay {
            base: context.view.world,
            blocks: &work.undone,
        };
        let view = View::new(&world, context.view.bodies, context.view.tables);
        let state = &from.state;
        let dimensions = dimensions_of(state.pose, &state.attributes);
        let bounds = Aabb::of_body(state.body.position, dimensions.width, dimensions.height);
        let entity = QueryEntity {
            key: state.key,
            position: state.body.position,
            fall_distance: state.body.fall_distance,
            shift: state.shift0,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
        };
        work.colliders.clear();
        query_colliders(
            view,
            &entity,
            bounds,
            None,
            QueryVariant::Collision,
            &mut work.colliders,
        );
        for edit in edits {
            let meets = work.colliders.iter().any(|collider| {
                matches!(collider.source, ColliderSource::Block { pos, .. } if pos == edit.block)
            });
            if meets {
                trials.push(Trial {
                    position: Some(edit.position),
                    ..trial
                });
            }
        }
    }
}

/// Which value of an input that changes in the tick is the primary one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Primary {
    New,
    /// The new value for a change to on, the old one for a change to off.
    NewWhenOn,
}

/// The values of one input, the primary one first. `None` keeps the value of the start state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Options {
    change: Option<bool>,
    change_first: bool,
}

impl Options {
    fn new(new: Option<bool>, old: bool, primary: Primary) -> Self {
        let change = new.filter(|&new| new != old);
        Self {
            change,
            change_first: change.is_some_and(|new| new || primary == Primary::New),
        }
    }

    fn count(&self) -> usize {
        if self.change.is_some() { 2 } else { 1 }
    }

    /// The value at `index` and 1 when it is not the primary value.
    fn get(&self, index: usize) -> (Option<bool>, u64) {
        let second = index > 0;
        let value = if second == self.change_first {
            None
        } else {
            self.change
        };
        (value, u64::from(second))
    }
}

impl Trial {
    fn set(&self, state: &mut PlayerState) {
        if let Some(sprint) = self.sprint {
            state.sprint0 = sprint;
        }
        if let Some(flight) = self.flight {
            state.flight0 = flight;
        }
        if let Some(using) = self.item_use {
            state.item_use.using = using;
        }
        if let Some(position) = self.position {
            state.body.position = position;
        }
    }
}
