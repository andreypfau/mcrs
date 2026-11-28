use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::tick_pace::tick_runs_normally;
use mcrs_minecraft_environment::{
    AdvanceTime, ClockTime, WorldClock, WorldClockSystems, WorldClocks,
};
use mcrs_minecraft_protocol::{VarInt, VarLong};
use mcrs_minecraft_protocol_game::clientbound::{ClientboundSetTime, ClockUpdate};
use mcrs_minecraft_resource::Id;

use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget, to_target};
use crate::coordinator::clock::{EditClock, GameTime};

pub struct WorldClockHostPlugin;

impl Plugin for WorldClockHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameTime>();
        app.add_systems(FixedUpdate, advance_game_time.run_if(tick_runs_normally));
        app.configure_sets(FixedUpdate, WorldClockSystems.run_if(tick_runs_normally));
        app.add_systems(
            FixedUpdate,
            send_every_clock
                .after(advance_game_time)
                .after(WorldClockSystems)
                .run_if(resource_changed::<AdvanceTime>),
        );
        app.add_observer(apply_clock_edit);
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::missed_set_if_neq,
        reason = "the counter changes on every write, so a comparison never skips one"
    )
)]
fn advance_game_time(mut time: ResMut<GameTime>) {
    time.0 = time.0.saturating_add(1);
}

fn clock_update(clock: Id<WorldClock>, state: &ClockTime, advance_time: bool) -> ClockUpdate {
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

fn send_every_clock(
    clocks: Res<WorldClocks>,
    advance_time: Res<AdvanceTime>,
    game_time: Res<GameTime>,
    mut sent: Local<Option<bool>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let before = sent.replace(advance_time.0);
    if before.is_none_or(|before| before == advance_time.0) {
        return;
    }
    let updates: Vec<ClockUpdate> = clocks
        .iter()
        .map(|(clock, state)| clock_update(clock, state, advance_time.0))
        .collect();
    if updates.is_empty() {
        return;
    }
    writer.write(to_target(
        PacketTarget::AllPlayers,
        PacketPayload::SetTime(ClientboundSetTime {
            game_time: game_time.0,
            clocks: updates,
        }),
    ));
}

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
    writer.write(to_target(
        PacketTarget::AllPlayers,
        PacketPayload::SetTime(ClientboundSetTime {
            game_time: game_time.0,
            clocks: vec![clock_update(edit.clock, state, advance_time.0)],
        }),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clock_that_does_not_advance_goes_out_with_rate_zero() {
        let clock = Id::<WorldClock>::checked(1, 3).unwrap();
        let state = |paused| ClockTime {
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
