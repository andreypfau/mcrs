use glam::DVec3;
use mcrs_minecraft_entity_key::KeyedStream;

use crate::BodyView;
use crate::CollisionRule;
use crate::GridFlags;
use crate::GridRow;
use crate::PushTargets;
use crate::Scratch;

/// The smallest horizontal distance of a push: `0.01F`.
pub const MIN_PUSH_DISTANCE: f32 = 0.01;

/// The largest component of one push: `0.05F`.
const STRENGTH: f32 = 0.05;

/// The damage of a crowd.
pub const CROWD_DAMAGE: f32 = 6.0;

/// The game rules of the push.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PushRules {
    /// The game rule `max_entity_cramming`.
    pub max_entity_cramming: i32,
    /// The number of nearest neighbours that push an entity when `max_entity_cramming` is 0.
    pub cap: usize,
}

impl Default for PushRules {
    fn default() -> Self {
        Self {
            max_entity_cramming: 24,
            cap: 24,
        }
    }
}

/// What the push of one entity did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PushOutcome {
    /// A neighbour pushed the entity with a distance of at least `0.01F`.
    pub pushed: bool,
    /// The entity takes the crowd damage in this tick.
    pub crowd_damage: bool,
    /// The number of candidates that the entity can push: the count of the crowd rule.
    pub candidates: u32,
    /// The candidates that are not passengers, when the entity drew the crowd damage; else 0.
    pub free_candidates: u32,
}

/// The push of one direction: the vector that the pusher adds; the candidate adds its negative.
/// `None` when the two ride one root vehicle, one has no physics, or they are nearer than
/// `0.01F` on both horizontal axes.
#[must_use]
pub fn push_vector(pusher: &GridRow, candidate: &GridRow) -> Option<DVec3> {
    if pusher.root_vehicle == candidate.root_vehicle
        || pusher.flags.contains(GridFlags::NO_PHYSICS)
        || candidate.flags.contains(GridFlags::NO_PHYSICS)
    {
        return None;
    }
    let dx = pusher.position.x - candidate.position.x;
    let dz = pusher.position.z - candidate.position.z;
    let xa = push_axis(dx, dz)?;
    let za = push_axis(dz, dx)?;
    (xa.is_finite() && za.is_finite()).then_some(DVec3::new(xa, 0.0, za))
}

/// The push on one horizontal axis from the offset `along` on that axis and `across` on the
/// other one. `None` when both offsets are nearer than `0.01F`.
#[must_use]
pub fn push_axis(along: f64, across: f64) -> Option<f64> {
    let largest = along.abs().max(across.abs());
    if largest < f64::from(MIN_PUSH_DISTANCE) {
        return None;
    }
    let root = largest.sqrt();
    Some(along / root * (1.0 / root).min(1.0) * f64::from(STRENGTH))
}

/// True when `pusher` pushes `candidate` in its own tick: the candidate is not a spectator, is
/// pushable, is a target of the pusher, and the team rules let the push happen.
#[must_use]
pub fn pushes(pusher: &GridRow, candidate: &GridRow) -> bool {
    let target = match pusher.push_targets {
        PushTargets::None => false,
        PushTargets::Each => true,
        PushTargets::NotPlayers => !candidate.flags.contains(GridFlags::PLAYER),
    };
    target
        && !candidate.flags.contains(GridFlags::SPECTATOR)
        && candidate.flags.contains(GridFlags::PUSHABLE)
        && team_allows(pusher, candidate)
}

fn team_allows(pusher: &GridRow, candidate: &GridRow) -> bool {
    let (own, other) = (pusher.team.rule, candidate.team.rule);
    if own == CollisionRule::Never || other == CollisionRule::Never {
        return false;
    }
    let same = pusher.team.team.is_some() && pusher.team.team == candidate.team.team;
    if (own == CollisionRule::PushOwnTeam || other == CollisionRule::PushOwnTeam) && same {
        return false;
    }
    if own != CollisionRule::PushOtherTeams && other != CollisionRule::PushOtherTeams {
        return true;
    }
    same
}

/// True when the entity adds the pushes that it gets: it carries no passenger and is pushable.
fn takes_push(row: &GridRow) -> bool {
    !row.flags.contains(GridFlags::CARRIES_PASSENGER) && row.flags.contains(GridFlags::PUSHABLE)
}

/// The push of a pair that is nearer than `0.01F` at the start of the tick (PHY-CMP-027): the
/// push that the entity with the lower key made in its last tick after its own move, on the
/// other one where it stood before its move of that tick. Gives the vector that `of` adds; the
/// other entity adds its negative.
fn staggered_push(of: &GridRow, other: &GridRow) -> Option<DVec3> {
    let offset = of.position - other.position;
    if offset.x.abs().max(offset.z.abs()) >= f64::from(MIN_PUSH_DISTANCE) {
        return None;
    }
    let first = of.key < other.key;
    let (earlier, later) = if first { (of, other) } else { (other, of) };
    let before = GridRow {
        position: later.previous,
        ..*later
    };
    if !pushes(earlier, &before) || !before.bounds().intersects(&earlier.bounds()) {
        return None;
    }
    let push = push_vector(earlier, &before)?;
    Some(if first { push } else { -push })
}

/// The push of the neighbours on `of`, from the start of the tick, added to `velocity` in the
/// order of the UUIDs of the neighbours: for each neighbour, the push of the neighbour
/// first, then the push of the entity on it, or the one push of a pair that is too near for
/// both. The crowd draw comes from `stream`, so the result does not depend on the order of the
/// rows or on the task split.
pub fn push_entity<B: BodyView>(
    bodies: &B,
    of: &GridRow,
    rules: &PushRules,
    stream: &KeyedStream,
    velocity: &mut DVec3,
    scratch: &mut Scratch,
) -> PushOutcome {
    let neighbours = &mut scratch.neighbours;
    neighbours.clear();
    bodies.neighbours(of, neighbours);
    let bounds = of.bounds();
    neighbours.retain(|other| other.key != of.key && other.bounds().intersects(&bounds));
    neighbours.sort_by_key(|other| other.key);
    let mut outcome = PushOutcome {
        candidates: neighbours.iter().filter(|other| pushes(of, other)).count() as u32,
        ..PushOutcome::default()
    };
    let limit = rules.max_entity_cramming;
    if limit > 0 && i64::from(outcome.candidates) > i64::from(limit) - 1 && stream.below(4) == 0 {
        let free = neighbours
            .iter()
            .filter(|other| pushes(of, other) && !other.flags.contains(GridFlags::IS_PASSENGER))
            .count();
        outcome.free_candidates = u32::try_from(free).unwrap_or(u32::MAX);
        outcome.crowd_damage = i64::from(outcome.free_candidates) > i64::from(limit) - 1;
    }
    let capped = limit == 0;
    for index in 0..neighbours.len() {
        let Some(other) = neighbours.get(index).copied() else {
            continue;
        };
        if capped
            && !mutual_nearest(
                bodies,
                of,
                &other,
                neighbours,
                rules.cap,
                &mut scratch.ranked,
            )
        {
            continue;
        }
        let mut add = |push: DVec3, outcome: &mut PushOutcome| {
            *velocity += push;
            outcome.pushed = true;
        };
        if pushes(&other, of)
            && takes_push(of)
            && let Some(push) = push_vector(&other, of)
        {
            add(-push, &mut outcome);
        }
        if pushes(of, &other)
            && takes_push(of)
            && let Some(push) = push_vector(of, &other)
        {
            add(push, &mut outcome);
        }
        if takes_push(of)
            && let Some(push) = staggered_push(of, &other)
        {
            add(push, &mut outcome);
        }
    }
    outcome
}

/// True when each of the two entities is among the `cap` nearest neighbours of the other, by
/// the larger horizontal axis distance and then by the UUID.
fn mutual_nearest<B: BodyView>(
    bodies: &B,
    of: &GridRow,
    other: &GridRow,
    neighbours: &[GridRow],
    cap: usize,
    ranked: &mut Vec<GridRow>,
) -> bool {
    if !within_nearest(of, other, neighbours, cap) {
        return false;
    }
    ranked.clear();
    bodies.neighbours(other, ranked);
    let bounds = other.bounds();
    ranked.retain(|row| row.key != other.key && row.bounds().intersects(&bounds));
    within_nearest(other, of, ranked, cap)
}

/// True when `target` is among the `cap` nearest of the neighbours of `centre`.
fn within_nearest(centre: &GridRow, target: &GridRow, neighbours: &[GridRow], cap: usize) -> bool {
    let rank = |row: &GridRow| (distance(centre, row), row.key);
    let own = rank(target);
    let nearer = neighbours
        .iter()
        .filter(|row| {
            let (distance, key) = rank(row);
            distance < own.0 || (distance == own.0 && key < own.1)
        })
        .count();
    nearer < cap
}

fn distance(a: &GridRow, b: &GridRow) -> f64 {
    (a.position.x - b.position.x)
        .abs()
        .max((a.position.z - b.position.z).abs())
}
