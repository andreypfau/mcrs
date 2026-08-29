use bevy::prelude::*;
use mcrs_minecraft_core::registry::snapshot::rl_from_asset_path;
use mcrs_minecraft_world::timeline::Timeline;
use mcrs_minecraft_world::world_clock::WorldClocks;

use super::DebugScreenDisplayer;

const OVERWORLD_DAY: &str = "minecraft:day";

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    timelines: Res<Assets<Timeline>>,
    asset_server: Res<AssetServer>,
    clocks: Res<WorldClocks>,
) {
    let Some(day) = timelines.iter().find_map(|(id, timeline)| {
        let location = rl_from_asset_path(asset_server.get_path(id)?.path())?;
        (location.as_str() == OVERWORLD_DAY).then_some(timeline)
    }) else {
        return;
    };
    let total_ticks = clocks
        .get(day.clock.as_str())
        .map_or(0, |clock| clock.total_ticks);
    let periods = day
        .period_ticks
        .map_or(0, |period| total_ticks / i64::from(period));
    displayer.add_line(format!("Day #{periods}"));
}
