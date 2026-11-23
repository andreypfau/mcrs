use bevy_app::{App, FixedUpdate, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::tick_pace::{TickPace, tick_runs_normally};
use mcrs_minecraft_environment::AdvanceTime;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundTickingState, ClientboundTickingStep};

use mcrs_minecraft_engine_world::game_rules::GameRules;

use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget, to_target};
use crate::muxer::online_count::send_online_count;

#[derive(Event, Clone, Copy, Debug, PartialEq)]
pub enum ServerEdit {
    GameRule { rule: GameRule, value: i32 },
    ClearWeather { ticks: u32 },
    StartSprint { ticks: u64 },
    StopSprint,
    TickRate { rate: f32 },
    Freeze,
    Unfreeze,
    Step { ticks: u64 },
    StopStep,
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Weather {
    pub clear_ticks: u32,
    pub rain_ticks: u32,
    pub thunder_ticks: u32,
    pub raining: bool,
    pub thundering: bool,
}

pub fn apply_edit(world: &mut World, edit: ServerEdit) {
    match edit {
        ServerEdit::GameRule { rule, value } => {
            if let Some(mut rules) = world.get_resource_mut::<GameRules>() {
                rules.set(rule, value);
            }
        }
        ServerEdit::ClearWeather { ticks } => {
            if let Some(mut weather) = world.get_resource_mut::<Weather>() {
                weather.set_if_neq(Weather {
                    clear_ticks: ticks,
                    ..Weather::default()
                });
            }
        }
        ServerEdit::StartSprint { ticks } => edit_pace(world, |pace| {
            pace.start_sprint(ticks);
        }),
        ServerEdit::StopSprint => edit_pace(world, |pace| {
            pace.stop_sprint();
        }),
        ServerEdit::TickRate { rate } => edit_pace(world, |pace| pace.set_rate(rate)),
        ServerEdit::Freeze => edit_pace(world, TickPace::freeze),
        ServerEdit::Unfreeze => edit_pace(world, TickPace::unfreeze),
        ServerEdit::Step { ticks } => edit_pace(world, |pace| pace.step(ticks)),
        ServerEdit::StopStep => edit_pace(world, |pace| {
            pace.stop_step();
        }),
    }
}

fn edit_pace(world: &mut World, edit: impl FnOnce(&mut TickPace)) {
    if let Some(mut pace) = world.get_resource_mut::<TickPace>() {
        edit(&mut pace);
    }
}

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
        app.add_systems(Update, announce_tick_state.after(send_online_count));
    }
}

pub fn announce_tick_state(
    pace: Option<ResMut<TickPace>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(mut pace) = pace else {
        return;
    };
    if pace.bypass_change_detection().take_state_change() {
        packets.write(to_target(
            PacketTarget::AllPlayers,
            PacketPayload::TickingState(ClientboundTickingState {
                tick_rate: pace.rate(),
                frozen: pace.is_frozen(),
            }),
        ));
    }
    if let Some(steps) = pace.bypass_change_detection().take_steps_change() {
        packets.write(to_target(
            PacketTarget::AllPlayers,
            PacketPayload::TickingStep(ClientboundTickingStep {
                steps: VarInt(i32::try_from(steps).unwrap_or(i32::MAX)),
            }),
        ));
    }
}

fn apply_host_edit(edit: On<ServerEdit>, mut commands: Commands) {
    let edit = *edit.event();
    commands.queue(move |world: &mut World| {
        apply_edit(world, edit);
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
