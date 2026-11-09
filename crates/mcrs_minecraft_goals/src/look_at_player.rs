use glam::DVec3;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::LookIntent;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerId;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::Turn;

use crate::Gate;
use crate::GoalWorld;
use crate::LOOK_AT_PLAYER_COUNT;
use crate::LOOK_AT_PLAYER_WAIT;

/// The chance of the random gate of a look goal in one examined full evaluation.
pub const LOOK_CHANCE: f64 = 0.02;

/// The head turn speed, the pitch limit of a look target, and the look range of a type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookParams {
    pub turn_speed: f32,
    pub pitch_limit: f32,
    pub range: f64,
}

/// The look at a player (GOAL-LKP-001): the player and the count of evaluation units that remain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LookAtPlayerGoal {
    gate: Gate,
    player: Option<PlayerId>,
    count: u8,
}

impl LookAtPlayerGoal {
    pub fn can_start<W: GoalWorld + ?Sized>(
        &mut self,
        turn: &Turn<'_, W>,
        params: &LookParams,
    ) -> bool {
        if !self.gate.passes(turn, LOOK_AT_PLAYER_WAIT, LOOK_CHANCE) {
            return false;
        }
        self.player = nearest_player(turn, params.range).map(|row| row.player);
        self.player.is_some()
    }

    pub fn start<W: ?Sized>(&mut self, turn: &Turn<'_, W>) {
        let draw = turn.random(LOOK_AT_PLAYER_COUNT).below(40);
        self.count = u8::try_from((40 + draw).div_ceil(2)).unwrap_or(u8::MAX);
    }

    /// Sets the look target at the eye of the player while it is alive, and counts one unit.
    pub fn tick<W: ?Sized>(
        &mut self,
        turn: &Turn<'_, W>,
        params: &LookParams,
        intents: &mut Intents,
    ) {
        if let Some(row) = self.row(turn)
            && row.flags.contains(PlayerFlags::ALIVE)
        {
            intents.look = Some(LookIntent::At {
                point: row.eye(),
                turn_speed: params.turn_speed,
                pitch_limit: params.pitch_limit,
                hold: look_hold(turn.max_lag),
            });
        }
        self.count = self.count.saturating_sub(1);
    }

    #[must_use]
    pub fn can_continue<W: ?Sized>(&self, turn: &Turn<'_, W>, params: &LookParams) -> bool {
        self.count > 0
            && self.row(turn).is_some_and(|row| {
                row.flags.contains(PlayerFlags::ALIVE)
                    && row.position.distance_squared(turn.view.position)
                        <= params.range * params.range
            })
    }

    pub fn stop(&mut self) {
        self.player = None;
    }

    #[must_use]
    pub const fn gate(&self) -> Gate {
        self.gate
    }

    pub fn gate_mut(&mut self) -> &mut Gate {
        &mut self.gate
    }

    fn row<'a, W: ?Sized>(&self, turn: &Turn<'a, W>) -> Option<&'a PlayerRow> {
        let player = self.player?;
        turn.players.iter().find(|row| row.player == player)
    }
}

/// The hold of a look target that an evaluation goal sets again in each full evaluation: until the
/// next application, at most `2 + max_lag - 1` ticks (AIT-LAG-009).
#[must_use]
pub fn look_hold(max_lag: u8) -> u8 {
    max_lag.saturating_add(1)
}

/// `Q-LOOK` (GOAL-LKP-002): the qualified player within `max(range × v, 2)` that the mob sees and
/// that does not ride it, nearest to the point at the eye height above the feet of the mob; on a
/// tie the smaller order key.
pub fn nearest_player<'a, W: GoalWorld + ?Sized>(
    turn: &Turn<'a, W>,
    range: f64,
) -> Option<&'a PlayerRow> {
    let view = turn.view;
    let eye = DVec3::new(
        view.position.x,
        view.position.y + f64::from(view.eye_height),
        view.position.z,
    );
    let mut best: Option<(f64, &PlayerRow)> = None;
    for row in turn.players {
        if !row.is_qualified() || row.vehicle == Some(view.mob) {
            continue;
        }
        let reach = (range * f64::from(row.visibility)).max(2.0);
        if row.position.distance_squared(view.position) > reach * reach {
            continue;
        }
        let distance = row.position.distance_squared(eye);
        if best.is_some_and(|(best_distance, _)| distance >= best_distance) {
            continue;
        }
        if turn.world.line_of_sight(eye, row.eye()) {
            best = Some((distance, row));
        }
    }
    best.map(|(_, row)| row)
}
