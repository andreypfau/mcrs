use std::mem;

use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BlockAction;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::PlayerInput;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::dimensions_of;
use mcrs_minecraft_physics::tick_player;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use rustc_hash::FxHashMap;

use crate::ClientTick;
use crate::ClientView;
use crate::Limits;
use crate::Pusher;
use crate::PusherHistory;
use crate::kept_changes::KeptChanges;
use crate::push_bound::PushBound;

/// A silent tick of the client is within this distance of the reference position.
const SILENT_DISTANCE: f64 = 2.0e-4;

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
    /// The position that the client moves itself back to when its edit of a block fails.
    pub(crate) rollback: Option<DVec3>,
    /// Every candidate of the tick ran.
    pub(crate) explored: bool,
}

/// The read-only inputs of a prediction.
pub(crate) struct Context<'a, W, B, H: ?Sized> {
    pub(crate) view: View<'a, W, B>,
    pub(crate) history: &'a H,
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
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Tried {
    /// The first candidate with the smallest offset, when none matches.
    pub(crate) best: Option<Scored>,
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
/// kept candidates go to `work.matched`, with the reported position.
pub(crate) fn run_tick<W: CollisionWorld, B: BodyView, H: PusherHistory + ?Sized>(
    context: &Context<'_, W, B, H>,
    record: &mut Record,
    search: Search,
    steps: &mut u32,
    work: &mut Work,
) -> Result<Tried, Stop> {
    let mut trials = mem::take(&mut work.trials);
    plan(record, &mut trials);
    let result = run_trials(context, record, &trials, search, steps, work);
    work.trials = trials;
    let (tried, explored) = result?;
    record.explored = explored;
    Ok(tried)
}

fn run_trials<W: CollisionWorld, B: BodyView, H: PusherHistory + ?Sized>(
    context: &Context<'_, W, B, H>,
    record: &Record,
    trials: &[Trial],
    search: Search,
    steps: &mut u32,
    work: &mut Work,
) -> Result<(Tried, bool), Stop> {
    let reported = record.tick.reported;
    work.matched.clear();
    let mut tried = Tried::default();
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
        if tried.best.is_none_or(|best| score.offset < best.offset) {
            tried.best = Some(Scored {
                candidate,
                offset: score.offset,
            });
        }
        if !score.matches {
            continue;
        }
        if let Some(position) = reported.position {
            candidate.state.position = position;
        }
        let flags = candidate.state.flags;
        let flags_match = flags.on_ground == reported.on_ground
            && flags.horizontal_collision == reported.horizontal_collision;
        if search == Search::First && flags_match {
            work.matched.clear();
            work.matched.push(candidate);
            return Ok((tried, index + 1 == trials.len()));
        }
        if !work.matched.contains(&candidate) {
            work.matched.push(candidate);
        }
    }
    Ok((tried, true))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Score {
    offset: f64,
    matches: bool,
}

/// Runs one candidate; with a position, again with the push in the bound that comes nearest to
/// the report.
fn run_trial<W: CollisionWorld, B: BodyView, H: PusherHistory + ?Sized>(
    context: &Context<'_, W, B, H>,
    record: &Record,
    candidate: &mut Candidate,
    steps: &mut u32,
    work: &mut Work,
) -> Result<Score, Stop> {
    let tick = &record.tick;
    let limits = context.limits;
    let start = candidate.state;
    let bound = push_bound(context, tick, &start, work)?;
    context.kept.undo_from(candidate.applied, &mut work.undone);
    let world = ClientView {
        base: context.view.world,
        undone: &work.undone,
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
    let Some(position) = tick.reported.position else {
        let distance = candidate.state.position.distance(record.reference);
        let tolerance = SILENT_DISTANCE + bound.reach();
        return Ok(Score {
            offset: (distance - tolerance).max(0.0),
            matches: distance <= tolerance + limits.offset_threshold,
        });
    };
    input.push = bound.clamp(position - candidate.state.position);
    if input.push != DVec3::ZERO {
        step(&input, &mut candidate.state)?;
    }
    let offset = candidate.state.position.distance(position);
    Ok(Score {
        offset,
        matches: offset < limits.offset_threshold,
    })
}

/// The push bound of the pushers near the candidate; zero for a player that no entity pushes.
fn push_bound<W, B, H: PusherHistory + ?Sized>(
    context: &Context<'_, W, B, H>,
    tick: &ClientTick,
    start: &PlayerState,
    work: &mut Work,
) -> Result<PushBound, Stop> {
    let facts = tick.input.facts;
    if facts.spectator || facts.passenger {
        return Ok(PushBound::default());
    }
    let dimensions = dimensions_of(start.pose, &start.attributes);
    let player = Aabb::of_body(start.position, dimensions.width, dimensions.height);
    let near = player
        .expand_towards(start.velocity)
        .inflate(DVec3::splat(1.0));
    work.pushers.clear();
    context
        .history
        .pushers(near, tick.server_ticks.clone(), &mut work.pushers);
    PushBound::of(&work.pushers, &player, start.position, context.limits).ok_or(Stop::Unbounded)
}

/// The candidates of the tick in the try order: per start candidate, the primary one first, then
/// those that differ in more and more inputs; the candidate that moved itself back last.
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
    if let (Some(position), Some(primary)) = (record.rollback, trials.first().copied()) {
        trials.push(Trial {
            position: Some(position),
            ..primary
        });
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
            state.position = position;
        }
    }
}
