//! The changes to the state that the host owns for the whole server: the game rules, the
//! weather and the sprint of the tick. A command of a dimension world asks for them, and the
//! host applies them after the tick.

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::tick_pace::TickPace;
use mcrs_minecraft_environment::world_clock::AdvanceTime;
use mcrs_minecraft_game_rule::GameRule;

use crate::game_rules::GameRules;

/// A change to the state that the host owns for the whole server.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerEdit {
    /// Sets a game rule.
    GameRule { rule: GameRule, value: i32 },
    /// Clears the weather for `ticks` ticks.
    ClearWeather { ticks: u32 },
    /// Starts a sprint of `ticks` ticks, in place of a sprint that runs.
    StartSprint { ticks: u64 },
    /// Ends the sprint that runs, with its report.
    StopSprint,
}

/// The weather of the server, which every weather dimension shares.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Weather {
    pub clear_ticks: u32,
    pub rain_ticks: u32,
    pub thunder_ticks: u32,
    pub raining: bool,
    pub thundering: bool,
}

/// Applies a [`ServerEdit`] to the state that a world holds, the host's or the copy of a
/// dimension world.
pub fn apply_edit(world: &mut World, edit: ServerEdit) {
    match edit {
        ServerEdit::GameRule { rule, value } => {
            if let Some(mut rules) = world.get_resource_mut::<GameRules>() {
                rules.set(rule, value);
            }
        }
        ServerEdit::ClearWeather { ticks } => {
            if let Some(mut weather) = world.get_resource_mut::<Weather>() {
                *weather = Weather {
                    clear_ticks: ticks,
                    ..Weather::default()
                };
            }
        }
        ServerEdit::StartSprint { ticks } => {
            if let Some(mut pace) = world.get_resource_mut::<TickPace>() {
                pace.start_sprint(ticks);
            }
        }
        ServerEdit::StopSprint => {
            if let Some(mut pace) = world.get_resource_mut::<TickPace>() {
                pace.stop_sprint();
            }
        }
    }
}

/// The host side of the game rules, the weather and the sprint.
pub struct ServerStateHostPlugin;

impl Plugin for ServerStateHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameRules>();
        app.init_resource::<Weather>();
        app.add_observer(apply_host_edit);
        app.add_systems(FixedUpdate, advance_weather);
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

fn advance_weather(rules: Res<GameRules>, mut weather: ResMut<Weather>) {
    if rules.is_on(GameRule::AdvanceWeather) && weather.clear_ticks > 0 {
        weather.clear_ticks -= 1;
    }
}
