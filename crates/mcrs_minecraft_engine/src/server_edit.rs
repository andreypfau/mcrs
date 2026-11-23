use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::tick_pace::TickPace;
use mcrs_minecraft_environment::AdvanceTime;
use mcrs_minecraft_game_rule::GameRule;

use mcrs_minecraft_engine_world::game_rules::GameRules;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerEdit {
    GameRule { rule: GameRule, value: i32 },
    ClearWeather { ticks: u32 },
    StartSprint { ticks: u64 },
    StopSprint,
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

pub struct ServerStateHostPlugin;

impl Plugin for ServerStateHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameRules>();
        app.init_resource::<Weather>();
        app.add_observer(apply_host_edit);
        app.add_systems(FixedUpdate, advance_weather.run_if(clear_weather_counting));
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
