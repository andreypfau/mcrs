use mcrs_minecraft_brain::ControlFlags;
use mcrs_minecraft_brain::GoalSpec;
use mcrs_minecraft_brain::Goals;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_brain::Point;
use mcrs_minecraft_brain::Selector;
use mcrs_minecraft_brain::Turn;

use crate::GoalWorld;
use crate::LookAroundGoal;
use crate::LookAtPlayerGoal;
use crate::LookParams;
use crate::PathTrack;
use crate::STROLL_IDLE_LIMIT;
use crate::StrollGoal;
use crate::panic;

/// The decision goals of the pig (GOAL-ANI-001), in the order of its list. The float is a reflex
/// of the hot tick that holds `jump`; no goal here needs `jump`. The breed, the tempt and the
/// follow goals come with the love state, the held items and the age.
pub const PIG_GOALS: [GoalSpec; 4] = [
    GoalSpec::new(1, ControlFlags::MOVE),
    GoalSpec::new(6, ControlFlags::MOVE),
    GoalSpec::new(7, ControlFlags::LOOK),
    GoalSpec::new(8, ControlFlags::MOVE.union(ControlFlags::LOOK)),
];

const PANIC: usize = 0;
const STROLL: usize = 1;
const LOOK_AT_PLAYER: usize = 2;
const LOOK_AROUND: usize = 3;

const PANIC_SPEED: f32 = 1.25;
const STROLL_SPEED: f32 = 1.0;
const PIG_LOOK: LookParams = LookParams {
    turn_speed: 10.0,
    pitch_limit: 40.0,
    range: 6.0,
};

/// The brain state of one pig.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PigBrain {
    selector: Selector,
    goals: PigGoals,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
struct PigGoals {
    path: PathTrack,
    stroll: StrollGoal,
    look_at_player: LookAtPlayerGoal,
    look_around: LookAroundGoal,
    published: Panic,
}

impl PigBrain {
    #[must_use]
    pub fn selector(&self) -> &Selector {
        &self.selector
    }

    /// The next full evaluation that is not quiet (GOAL-WAKE-002), when no wake event of
    /// GOAL-WAKE-005 reaches the brain before; the stage wakes the brain for each such event.
    ///
    /// - the next evaluation, when a goal started or stopped in this one, or when a goal that the
    ///   next one examines is eligible with no wait, so that it draws one;
    /// - the next evaluation while the pig is busy (GOAL-WAKE-004): the panic does not run and the
    ///   damage record that causes panic is still fresh then, or the look at a player runs;
    /// - the due evaluation of each wait, and the end of a look around (GOAL-WAKE-006);
    /// - while the stroll waits, the first evaluation in which the idle time can reach 100 with no
    ///   reset: that evaluation ends the wait (GOAL-WAKE-007), and an extra evaluation that changes
    ///   nothing is still exact.
    fn next_wake<W: ?Sized>(&self, turn: &Turn<'_, W>, changed: Changed) -> u64 {
        let goals = &self.goals;
        let tick = turn.tick;
        let next = turn.next_evaluation();
        let ahead = next - tick;
        let selector = self.selector.released();
        let examined = |goal| selector.is_examined(&PIG_GOALS, goal, turn.view.disabled);
        let busy = (!self.selector.is_running(PANIC) && panic::panic_record_fresh(turn.view, next))
            || self.selector.is_running(LOOK_AT_PLAYER);
        let draws = (examined(STROLL)
            && StrollGoal::is_eligible(turn, ahead)
            && goals.stroll.gate().due().is_none())
            || (examined(LOOK_AT_PLAYER) && goals.look_at_player.gate().due().is_none())
            || (examined(LOOK_AROUND) && goals.look_around.gate().due().is_none());
        if changed == Changed::Yes || busy || draws {
            return next;
        }
        let idle_limit = goals.stroll.gate().due().map(|_| {
            let remaining = u64::from(STROLL_IDLE_LIMIT.saturating_sub(turn.view.idle_time));
            turn.cadence.next_evaluation(tick.saturating_add(remaining))
        });
        [
            goals.stroll.gate().due(),
            goals.look_at_player.gate().due(),
            goals.look_around.gate().due(),
            goals.look_around.ends(),
            idle_limit,
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(u64::MAX)
    }
}

impl<W: GoalWorld + ?Sized> MobBrain<W> for PigBrain {
    const PLAYER_RANGE: f64 = PIG_LOOK.range;

    fn evaluate(&mut self, turn: &Turn<'_, W>, intents: &mut Intents) -> u64 {
        let mut goals = PigTurn {
            turn,
            goals: &mut self.goals,
            intents,
            destination: None,
            eligible: 0,
            changed: Changed::No,
        };
        self.selector.evaluate(&mut goals, turn.view.disabled);
        let (eligible, changed) = (goals.eligible, goals.changed);
        let ineligible = |goal: usize| eligible & (1 << goal) == 0;
        if ineligible(STROLL) {
            self.goals.stroll.gate_mut().end();
        }
        if ineligible(LOOK_AT_PLAYER) {
            self.goals.look_at_player.gate_mut().end();
        }
        if ineligible(LOOK_AROUND) {
            self.goals.look_around.gate_mut().end();
        }
        let panic = if self.selector.is_running(PANIC) {
            Panic::Panicking
        } else {
            Panic::Calm
        };
        if panic != self.goals.published {
            self.goals.published = panic;
            intents.panic = Some(panic);
        }
        self.next_wake(turn, changed)
    }
}

/// Whether a goal started or stopped in a full evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Changed {
    Yes,
    No,
}

struct PigTurn<'s, 't, 'a, W: ?Sized> {
    turn: &'t Turn<'a, W>,
    goals: &'s mut PigGoals,
    intents: &'s mut Intents,
    destination: Option<Point>,
    /// The goals with a gate that were eligible in this evaluation, one bit for each goal.
    eligible: u8,
    changed: Changed,
}

impl<W: GoalWorld + ?Sized> Goals for PigTurn<'_, '_, '_, W> {
    fn specs(&self) -> &[GoalSpec] {
        &PIG_GOALS
    }

    fn can_start(&mut self, goal: usize) -> bool {
        let turn = self.turn;
        self.destination = match goal {
            PANIC if panic::panic_record_fresh(turn.view, turn.tick) => {
                panic::panic_destination(turn)
            }
            STROLL => {
                if StrollGoal::is_eligible(turn, 0) {
                    self.eligible |= 1 << STROLL;
                }
                self.goals.stroll.start_destination(turn)
            }
            LOOK_AT_PLAYER => {
                self.eligible |= 1 << LOOK_AT_PLAYER;
                return self.goals.look_at_player.can_start(turn, &PIG_LOOK);
            }
            LOOK_AROUND => {
                self.eligible |= 1 << LOOK_AROUND;
                return self.goals.look_around.can_start(turn);
            }
            _ => None,
        };
        self.destination.is_some()
    }

    fn can_continue(&mut self, goal: usize) -> bool {
        let turn = self.turn;
        let path_running = self.goals.path.is_running(turn.view);
        match goal {
            PANIC => path_running,
            STROLL => StrollGoal::can_continue(turn, path_running),
            LOOK_AT_PLAYER => self.goals.look_at_player.can_continue(turn, &PIG_LOOK),
            LOOK_AROUND => self.goals.look_around.can_continue(turn),
            _ => false,
        }
    }

    fn start(&mut self, goal: usize) {
        self.changed = Changed::Yes;
        let turn = self.turn;
        match goal {
            PANIC | STROLL => {
                let speed = if goal == PANIC {
                    PANIC_SPEED
                } else {
                    STROLL_SPEED
                };
                if let Some(destination) = self.destination.take() {
                    self.goals
                        .path
                        .request(turn, destination, speed, self.intents);
                }
            }
            LOOK_AT_PLAYER => self.goals.look_at_player.start(turn),
            LOOK_AROUND => self.goals.look_around.start(turn, self.intents),
            _ => {}
        }
    }

    fn stop(&mut self, goal: usize) {
        self.changed = Changed::Yes;
        match goal {
            STROLL => self.goals.path.clear(self.intents),
            LOOK_AT_PLAYER => self.goals.look_at_player.stop(),
            LOOK_AROUND => self.goals.look_around.stop(self.intents),
            _ => {}
        }
    }

    fn tick(&mut self, goal: usize) {
        if goal == LOOK_AT_PLAYER {
            self.goals
                .look_at_player
                .tick(self.turn, &PIG_LOOK, self.intents);
        }
    }
}
