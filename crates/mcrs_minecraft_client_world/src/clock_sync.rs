use bevy::prelude::*;
use mcrs_minecraft_environment::{WorldClockSystems, WorldClocks};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetTime;
use mcrs_minecraft_resource::Id;

/// The game time to which the clocks of the client last advanced, from the latest `set_time`
/// and the client ticks since; `None` before the first `set_time`.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientGameTime(pub Option<i64>);

pub(crate) fn add_systems(app: &mut App) {
    app.init_resource::<ClientGameTime>()
        .add_systems(FixedUpdate, count_game_tick.in_set(WorldClockSystems))
        .add_observer(on_set_time);
}

fn count_game_tick(mut time: ResMut<ClientGameTime>) {
    if let Some(game_time) = time.0.as_mut() {
        *game_time = game_time.saturating_add(1);
    }
}

/// Moves the clocks to the game time of the packet, then takes the state of each clock that it
/// names, with the rate that the server gives: 0 for a clock that does not advance.
fn on_set_time(
    event: On<Received<ClientboundSetTime>>,
    mut time: ResMut<ClientGameTime>,
    mut clocks: ResMut<WorldClocks>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let elapsed = time
        .0
        .map_or(0, |last| packet.game_time.saturating_sub(last));
    time.0 = Some(packet.game_time);
    for (_, state) in clocks.iter_mut() {
        state.advance_by_game_ticks(elapsed);
    }
    let count = clocks.len();
    for update in &packet.clocks {
        let Some(id) = u16::try_from(update.clock.0)
            .ok()
            .and_then(|number| Id::checked(number, count))
        else {
            continue;
        };
        if let Some(state) = clocks.get_mut(id) {
            state.total_ticks = update.total_ticks.0;
            state.partial_tick = update.partial_tick;
            state.rate = update.rate;
        }
    }
}
