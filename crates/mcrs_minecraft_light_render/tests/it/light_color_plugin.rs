use bevy::app::{App, TaskPoolPlugin};
use bevy::asset::AssetPlugin;
use mcrs_minecraft_light_color::colors::LightColors;
use mcrs_minecraft_light_render::{HeldItemLights, LightColorPlugin};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registry_load::test_registries;

#[test]
fn the_plugin_holds_the_colour_table_and_item_lights_after_the_first_update() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        },
    ));
    let (blocks, items) = test_corpus();
    app.insert_resource(blocks.clone());
    app.insert_resource(items.clone());
    app.insert_resource(CurrentRegistries::new(test_registries().clone()));
    app.add_plugins(LightColorPlugin);
    app.update();
    let colors = app
        .world()
        .get_resource::<LightColors>()
        .expect("the colour table after the first update");
    assert_eq!(colors.type_count(), 14, "13 colours and the default");
    assert!(
        app.world().get_resource::<HeldItemLights>().is_some(),
        "the item light table after the first update"
    );
}
