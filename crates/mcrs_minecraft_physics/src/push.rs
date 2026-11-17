use glam::DVec3;
use mcrs_minecraft_entity_key::KeyedStream;

use crate::BodyView;
use crate::CollisionRule;
use crate::GridFlags;
use crate::GridRow;
use crate::PushTargets;
use crate::Scratch;

pub const MIN_PUSH_DISTANCE: f32 = 0.01;

const STRENGTH: f32 = 0.05;

pub const CROWD_DAMAGE: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PushRules {
    pub max_entity_cramming: i32,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PushOutcome {
    pub pushed: bool,
    pub crowd_damage: bool,
    pub candidates: u32,
    pub free_candidates: u32,
}

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

#[must_use]
pub fn push_axis(along: f64, across: f64) -> Option<f64> {
    let largest = along.abs().max(across.abs());
    if largest < f64::from(MIN_PUSH_DISTANCE) {
        return None;
    }
    let root = largest.sqrt();
    Some(along / root * (1.0 / root).min(1.0) * f64::from(STRENGTH))
}

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

fn takes_push(row: &GridRow) -> bool {
    !row.flags.contains(GridFlags::CARRIES_PASSENGER) && row.flags.contains(GridFlags::PUSHABLE)
}

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
