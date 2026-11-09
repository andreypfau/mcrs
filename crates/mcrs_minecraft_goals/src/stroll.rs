use glam::DVec3;
use mcrs_minecraft_brain::MobFlags;
use mcrs_minecraft_brain::Turn;

use crate::Gate;
use crate::GoalWorld;
use crate::STROLL_SEARCH;
use crate::STROLL_WAIT;
use crate::SearchRange;
use crate::random_position;

/// The chance of the random gate of the stroll in one examined full evaluation: a uniform integer
/// in `[0, 60)` is 0.
pub const STROLL_CHANCE: f64 = 1.0 / 60.0;

/// A mob with this idle time or more starts no stroll.
pub const STROLL_IDLE_LIMIT: u32 = 100;

/// The random stroll that avoids water (GOAL-STR-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StrollGoal {
    gate: Gate,
}

impl StrollGoal {
    /// The parts of the start condition before the gate, `ticks` after the snapshot of the turn:
    /// no passenger controls the mob, and the idle time is less than 100 (GOAL-WAKE-012).
    #[must_use]
    pub fn is_eligible<W: ?Sized>(turn: &Turn<'_, W>, ticks: u64) -> bool {
        let view = turn.view;
        let idle = u64::from(view.idle_time).saturating_add(ticks);
        !view.flags.contains(MobFlags::CONTROLLED) && idle < u64::from(STROLL_IDLE_LIMIT)
    }

    /// The start condition of an examined stroll: the destination when the goal starts. An
    /// examined stroll that is not eligible ends its wait.
    pub fn start_destination<W: GoalWorld + ?Sized>(
        &mut self,
        turn: &Turn<'_, W>,
    ) -> Option<DVec3> {
        if !Self::is_eligible(turn, 0) {
            self.gate.end();
            return None;
        }
        if !self.gate.passes(turn, STROLL_WAIT, STROLL_CHANCE) {
            return None;
        }
        stroll_destination(turn)
    }

    #[must_use]
    pub fn can_continue<W: ?Sized>(turn: &Turn<'_, W>, path_running: bool) -> bool {
        path_running && !turn.view.flags.contains(MobFlags::CONTROLLED)
    }

    #[must_use]
    pub const fn gate(&self) -> Gate {
        self.gate
    }

    pub fn gate_mut(&mut self) -> &mut Gate {
        &mut self.gate
    }
}

/// GOAL-STR-002: `L(15, 7)` and then `D(10, 7)` in water; else `L(10, 7)` with the chance 0.999 and
/// `D(10, 7)` with the chance 0.001.
fn stroll_destination<W: GoalWorld + ?Sized>(turn: &Turn<'_, W>) -> Option<DVec3> {
    let mut random = turn.random(STROLL_SEARCH);
    let world = turn.world;
    let feet = turn.view.position;
    if turn.view.flags.contains(MobFlags::IN_WATER) {
        return random_position::land_search(world, feet, SearchRange::new(15, 7), &mut random)
            .or_else(|| {
                random_position::default_search(world, feet, SearchRange::new(10, 7), &mut random)
            });
    }
    if random.chance(0.999) {
        random_position::land_search(world, feet, SearchRange::new(10, 7), &mut random)
    } else {
        random_position::default_search(world, feet, SearchRange::new(10, 7), &mut random)
    }
}
