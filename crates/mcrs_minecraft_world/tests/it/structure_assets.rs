use bevy_app::App;
use bevy_asset::{AssetServer, Assets};
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_worldgen::bevy::{
    ProcessorListAsset, StructureAsset, StructureSetAsset, TemplateAsset, pool_templates,
};
use mcrs_minecraft_worldgen::tables::template_handle;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;

pub fn the_structure_registries_land_before_playing(app: &App) {
    let world = app.world();

    assert_eq!(world.resource::<Assets<StructureSetAsset>>().len(), 21);
    assert_eq!(world.resource::<Assets<StructureAsset>>().len(), 52);
    assert_eq!(world.resource::<Assets<ProcessorListAsset>>().len(), 40);
    assert_eq!(world.resource::<Assets<TemplateAsset>>().len(), 1511);

    let set = world.resource::<RegistrySet>();
    let pools = set
        .entries::<TemplatePool, TemplatePool>()
        .expect("the template pools are parsed by the loader");
    assert_eq!(pools.as_slice().len(), 245);

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

    let no_corners = set
        .registry::<TemplatePool>()
        .and_then(|pools| pools.by_name("minecraft:ancient_city/walls/no_corners"))
        .expect("the pool is a loaded entry");
    let missing = mcrs_minecraft_core::ResourceLocation::minecraft(
        "ancient_city/walls/intact_horizontal_wall_stairs_5",
    )
    .unwrap();
    assert!(
        pool_templates(&pools[no_corners]).contains(&missing),
        "a pool naming a template that does not ship still loads"
    );
    assert!(
        template_handle(world.resource::<AssetServer>(), &missing).is_none(),
        "a template that does not ship is never requested"
    );
}
