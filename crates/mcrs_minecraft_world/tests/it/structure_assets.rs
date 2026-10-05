use bevy_app::App;
use bevy_asset::{AssetServer, Assets};
use mcrs_minecraft_worldgen::bevy::{
    ProcessorListAsset, StructureAsset, StructureSetAsset, TemplateAsset, TemplatePoolAsset,
};

pub fn the_structure_registries_land_before_playing(app: &App) {
    let world = app.world();

    assert_eq!(world.resource::<Assets<StructureSetAsset>>().len(), 21);
    assert_eq!(world.resource::<Assets<StructureAsset>>().len(), 52);
    assert_eq!(world.resource::<Assets<TemplatePoolAsset>>().len(), 245);
    assert_eq!(world.resource::<Assets<ProcessorListAsset>>().len(), 39);
    assert_eq!(world.resource::<Assets<TemplateAsset>>().len(), 1511);

    let only_named_beside_a_missing_sibling = world
        .resource::<AssetServer>()
        .get_handle::<TemplateAsset>(
            "minecraft/structure/ancient_city/walls/intact_horizontal_wall_bridge.nbt",
        )
        .expect("every shipped template was requested");
    assert!(
        world
            .resource::<Assets<TemplateAsset>>()
            .get(&only_named_beside_a_missing_sibling)
            .is_some()
    );

    let no_corners = world
        .resource::<AssetServer>()
        .get_handle::<TemplatePoolAsset>(
            "minecraft/worldgen/template_pool/ancient_city/walls/no_corners.json",
        )
        .expect("the pool was requested");
    let pool = world
        .resource::<Assets<TemplatePoolAsset>>()
        .get(&no_corners)
        .expect("a pool naming a template that does not ship still lands");
    let missing = mcrs_minecraft_core::ResourceLocation::minecraft(
        "ancient_city/walls/intact_horizontal_wall_stairs_5",
    )
    .unwrap();
    assert!(!pool.deps.templates.contains_key(&missing));
    assert!(
        world
            .resource::<AssetServer>()
            .get_handle::<TemplateAsset>(
                "minecraft/structure/ancient_city/walls/intact_horizontal_wall_stairs_5.nbt",
            )
            .is_none(),
        "a template that does not ship is never requested"
    );
}
