use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_engine::bus::{
    EditClock, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use mcrs_minecraft_environment::world_clock::{AdvanceTime, ClockState, WorldClock, WorldClocks};
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_protocol::packets::game::clientbound::{ClientboundSetTime, ClockUpdate};
use mcrs_minecraft_protocol::{VarInt, VarLong};

/// The tick counter that every dimension reads as its game time (SIM-TICK-003).
// chisle: the game time and the clock states start fresh each run and a player gets
// `set_time` only after a change; reading and writing them with the save, and a full
// `set_time` on join and every 20 ticks, lift it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameTime(pub i64);

/// The host side of the world clocks: the game time, and the clock changes that dimensions
/// ask for. The dimensions read both as a copy that the host extracts every tick.
pub struct WorldClockHostPlugin;

impl Plugin for WorldClockHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameTime>();
        app.add_systems(FixedUpdate, advance_game_time);
        app.add_observer(apply_clock_edit);
    }
}

fn advance_game_time(mut time: ResMut<GameTime>) {
    time.0 = time.0.saturating_add(1);
}

/// The `set_time` entry of one clock (CMD-TIME-009).
pub fn clock_update(clock: Id<WorldClock>, state: &ClockState, advance_time: bool) -> ClockUpdate {
    ClockUpdate {
        clock: VarInt(i32::from(clock.number())),
        total_ticks: VarLong(state.total_ticks),
        partial_tick: state.partial_tick,
        rate: if state.paused || !advance_time {
            0.0
        } else {
            state.rate
        },
    }
}

/// Applies the change to the clock and sends its new state to every player of the server.
fn apply_clock_edit(
    edit: On<EditClock>,
    mut clocks: ResMut<WorldClocks>,
    advance_time: Res<AdvanceTime>,
    game_time: Res<GameTime>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(state) = clocks.get_mut(edit.clock) else {
        tracing::warn!(clock = ?edit.clock, "a dimension edits a clock the server does not hold");
        return;
    };
    state.apply(edit.edit);
    writer.write(OutboundPlayerPacket {
        target: PacketTarget::AllPlayers,
        priority: PacketPriority::Normal,
        data: PacketPayload::SetTime(ClientboundSetTime {
            game_time: game_time.0,
            clocks: vec![clock_update(edit.clock, state, advance_time.0)],
        }),
        session: PlayerSession(0),
        epoch: 0,
    });
}

/// The clock changes a dimension sent to the host in this tick. The host applies them
/// between ticks, so the next copy of the clocks holds them and the list is emptied when that
/// copy arrives.
#[derive(Resource, Clone, Debug, Default)]
pub struct PendingClockEdits(Vec<EditClock>);

impl PendingClockEdits {
    pub fn push(&mut self, edit: EditClock) {
        self.0.push(edit);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// The state of the clock in the copy, with the changes of this tick applied.
    pub fn state(&self, clocks: &WorldClocks, clock: Id<WorldClock>) -> Option<ClockState> {
        let mut state = *clocks.get(clock)?;
        for edit in self.0.iter().filter(|edit| edit.clock == clock) {
            state.apply(edit.edit);
        }
        Some(state)
    }
}

/// Copies the host's game time into a dimension and drops the clock changes that the copy of
/// the clocks now holds.
pub(crate) fn extract_clock_state(main_world: &World, sub_world: &mut World) {
    if let Some(time) = main_world.get_resource::<GameTime>() {
        sub_world.insert_resource(*time);
    }
    if let Some(mut pending) = sub_world.get_resource_mut::<PendingClockEdits>()
        && !pending.0.is_empty()
    {
        pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clock_that_does_not_advance_goes_out_with_rate_zero() {
        let clock = Id::<WorldClock>::checked(1, 3).unwrap();
        let state = |paused| ClockState {
            total_ticks: 5,
            partial_tick: 0.25,
            rate: 2.0,
            paused,
        };
        for (paused, advance_time, rate) in
            [(false, true, 2.0), (true, true, 0.0), (false, false, 0.0)]
        {
            assert_eq!(
                clock_update(clock, &state(paused), advance_time),
                ClockUpdate {
                    clock: VarInt(1),
                    total_ticks: VarLong(5),
                    partial_tick: 0.25,
                    rate,
                },
                "paused {paused}, advance_time {advance_time}"
            );
        }
    }
}
