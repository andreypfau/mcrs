use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::tick_pace::TickPace;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundTickingState, ClientboundTickingStep};

use mcrs_minecraft_engine_world::game_rules::GameRules;

use crate::bus::PacketPayload;

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
        ServerEdit::StartSprint { .. }
        | ServerEdit::StopSprint
        | ServerEdit::TickRate { .. }
        | ServerEdit::Freeze
        | ServerEdit::Unfreeze
        | ServerEdit::Step { .. }
        | ServerEdit::StopStep => {
            if let Some(mut pace) = world.get_resource_mut::<TickPace>() {
                edit_pace(&mut pace, edit);
            }
        }
    }
}

fn edit_pace(pace: &mut TickPace, edit: ServerEdit) {
    match edit {
        ServerEdit::StartSprint { ticks } => pace.start_sprint(ticks),
        ServerEdit::StopSprint => pace.stop_sprint(),
        ServerEdit::TickRate { rate } => pace.set_rate(rate),
        ServerEdit::Freeze => pace.freeze(),
        ServerEdit::Unfreeze => pace.unfreeze(),
        ServerEdit::Step { ticks } => pace.step(ticks),
        ServerEdit::StopStep => pace.stop_step(),
        ServerEdit::GameRule { .. } | ServerEdit::ClearWeather { .. } => {}
    }
}

pub(crate) fn ticking_state(pace: &TickPace) -> PacketPayload {
    PacketPayload::TickingState(ClientboundTickingState {
        tick_rate: pace.rate(),
        frozen: pace.is_frozen(),
    })
}

pub(crate) fn ticking_step(steps: u64) -> PacketPayload {
    PacketPayload::TickingStep(ClientboundTickingStep {
        steps: VarInt(i32::try_from(steps).unwrap_or(i32::MAX)),
    })
}
