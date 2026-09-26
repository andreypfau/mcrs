mod corpus;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::state::NextState;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_light_color::colors::LightColors;
use mcrs_minecraft_light_color::item::ItemLights;
use mcrs_minecraft_light_color::plugin::LightColorPlugin;

#[test]
fn the_colour_table_and_item_lights_are_resources_after_worldgen_freeze() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        },
        StatesPlugin,
    ));
    app.init_state::<AppState>();
    app.insert_resource(corpus::blocks().clone());
    app.insert_resource(corpus::block_tags().clone());
    app.insert_resource(corpus::items().clone());
    app.add_plugins(LightColorPlugin);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::WorldgenFreeze);
    app.update();
    app.update();
    let colors = app
        .world()
        .get_resource::<LightColors>()
        .expect("the colour table after worldgen freeze");
    assert_eq!(colors.type_count(), 14, "13 colours and the default");
    assert!(
        app.world().get_resource::<ItemLights>().is_some(),
        "the item light table after worldgen freeze"
    );
}
