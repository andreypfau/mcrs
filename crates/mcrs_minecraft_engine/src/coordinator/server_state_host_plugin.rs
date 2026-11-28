use bevy_app::{App, FixedUpdate, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::tick_pace::{TickPace, tick_runs_normally};
use mcrs_minecraft_environment::AdvanceTime;
use mcrs_minecraft_game_rule::GameRule;

use crate::bus::{OutboundPlayerPacket, PacketTarget, to_target};
use crate::server_edit::{self, ServerEdit, Weather};

pub struct ServerStateHostPlugin;

impl Plugin for ServerStateHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameRules>();
        app.init_resource::<Weather>();
        app.add_observer(apply_host_edit);
        app.add_systems(
            FixedUpdate,
            advance_weather
                .run_if(clear_weather_counting)
                .run_if(tick_runs_normally),
        );
        app.add_systems(
            Update,
            announce_tick_state.run_if(resource_exists::<TickPace>),
        );
    }
}

pub fn announce_tick_state(
    mut pace: ResMut<TickPace>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    if pace.bypass_change_detection().take_state_change() {
        packets.write(to_target(
            PacketTarget::AllPlayers,
            server_edit::ticking_state(&pace),
        ));
    }
    if let Some(steps) = pace.bypass_change_detection().take_steps_change() {
        packets.write(to_target(
            PacketTarget::AllPlayers,
            server_edit::ticking_step(steps),
        ));
    }
}

fn apply_host_edit(edit: On<ServerEdit>, mut commands: Commands) {
    let edit = *edit.event();
    commands.queue(move |world: &mut World| {
        server_edit::apply_edit(world, edit);
        if let ServerEdit::GameRule {
            rule: GameRule::AdvanceTime,
            value,
        } = edit
            && let Some(mut advance) = world.get_resource_mut::<AdvanceTime>()
        {
            advance.set_if_neq(AdvanceTime(value != 0));
        }
    });
}

fn clear_weather_counting(weather: Res<Weather>) -> bool {
    weather.clear_ticks > 0
}

fn advance_weather(rules: Res<GameRules>, mut weather: ResMut<Weather>) {
    if rules.is_on(GameRule::AdvanceWeather) && weather.clear_ticks > 0 {
        weather.clear_ticks -= 1;
    }
}
